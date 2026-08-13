//! # 虚拟供应商转发器
//!
//! 持有虚拟供应商的全部候选路由，按健康度排序逐条尝试，失败时降级该路由
//! 健康度并继续下一条，直到成功或全部失败。
//!
//! 与旧实现差异：
//! - 不再硬编码仅尝试前 2 条路由
//! - 失败降级是即时的，下条路由立刻重试，无需等待下次请求
//! - 路由解析（route_resolver）与执行（DirectForwarder）职责分离
//! - 每条路由尝试结束后异步写入 `virtual_route_attempts` 历史记录

use std::sync::Arc;
use std::time::Instant;

use axum::response::Response;
use serde_json::Value;
use tauri::Emitter;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::gateway_runtime::service::GatewaySharedState;
use crate::modules::logger::types::LogLevel;
use crate::modules::virtual_provider::types::VirtualModelRoute;

use super::context::GatewayProtocol;
use super::forwarder::{Forwarder, ForwardPipeline};
use super::route_resolver::build_virtual_context;

/// 虚拟供应商转发器
pub struct VirtualForwarder {
    /// 内部使用的真实供应商转发器
    direct: Arc<dyn Forwarder>,
}

impl VirtualForwarder {
    /// 构造虚拟转发器
    pub fn new(direct: Arc<dyn Forwarder>) -> Self {
        Self { direct }
    }

    /// 执行虚拟路由故障转移
    ///
    /// 按 `routes` 顺序逐条尝试，每条路由失败时降级健康度并继续下一条；
    /// 全部失败时返回最后一条路由的错误。
    ///
    /// 每条路由尝试结束后异步写入 `virtual_route_attempts` 历史记录，不阻塞响应返回。
    pub async fn run(
        &self,
        shared: &GatewaySharedState,
        routes: Vec<VirtualModelRoute>,
        alias: String,
        gateway_model_id: String,
        upstream_model_id: String,
        request_id: String,
        gateway_protocol: GatewayProtocol,
        mut body: Value,
        api_key_secret_id: Option<String>,
        request_headers_json: Option<String>,
    ) -> IcodeResult<Response> {
        let _ = upstream_model_id; // 已在 route_resolver 中通过 build_virtual_context 传入 target_model_id
        let mut last_err: Option<IcodeError> = None;

        for (index, route) in routes.iter().enumerate() {
            // 构造当前路由的 ForwardContext
            let ctx = match build_virtual_context(
                shared,
                route,
                index,
                &request_id,
                gateway_protocol,
                gateway_model_id.clone(),
            ) {
                Ok(ctx) => ctx,
                Err(e) => {
                    // 路由目标供应商不可用，降级并继续下一条
                    degrade_route(shared, &route.id);
                    // 异步写入 attempts：构造失败视为 attempt 失败
                    record_attempt(
                        shared,
                        &route.id,
                        &alias,
                        &request_id,
                        index,
                        false,
                        None,
                        Some(e.message.as_str()),
                        0,
                    );
                    tracing::warn!("虚拟路由 {} 不可用: {}", route.id, e.message);
                    last_err = Some(e);
                    continue;
                }
            };

            // 调用公共管道执行单次转发
            let mut ctx = ctx;
            let start = Instant::now();
            let result = ForwardPipeline::execute_and_finalize(
                shared,
                self.direct.as_ref(),
                &mut ctx,
                &mut body,
                api_key_secret_id.clone(),
                request_headers_json.clone(),
            )
            .await;
            let duration_ms = start.elapsed().as_millis() as u64;

            match result {
                Ok(resp) => {
                    // 提取 HTTP 状态码用于 attempts 记录
                    let status_code = resp.status().as_u16();
                    // 异步写入成功 attempt
                    record_attempt(
                        shared,
                        &route.id,
                        &alias,
                        &request_id,
                        index,
                        true,
                        Some(status_code),
                        None,
                        duration_ms,
                    );
                    return Ok(resp);
                }
                Err(err) => {
                    // IcodeError.code 是 SCREAMING_SNAKE_CASE 字符串（如 VALIDATION / NOT_FOUND），
                    // 不直接对应 HTTP 状态码，因此 attempts 不记录 status_code，UI 通过 error_message 展示
                    // 失败降级该路由健康度
                    degrade_route(shared, &route.id);
                    // 异步写入失败 attempt
                    let err_msg = err.message.clone();
                    record_attempt(
                        shared,
                        &route.id,
                        &alias,
                        &request_id,
                        index,
                        false,
                        None,
                        Some(err_msg.as_str()),
                        duration_ms,
                    );
                    last_err = Some(err);
                    continue;
                }
            }
        }

        // 全部失败
        let err = last_err.unwrap_or_else(|| {
            IcodeError::internal(format!(
                "虚拟供应商 '{}' 全部 {} 条路由均失败",
                alias,
                routes.len()
            ))
        });

        let _ = shared.app_handle.emit("virtual:all-routes-failed", &err);
        Err(err)
    }
}

/// 降级虚拟供应商路由健康度
///
/// pub(crate) 供 forwarder.rs 在虚拟路由失败时调用。
pub(crate) fn degrade_route(shared: &GatewaySharedState, route_id: &str) {
    if let Err(e) = shared
        .virtual_provider_handle
        .service()
        .degrade_route_health(route_id)
    {
        tracing::warn!(
            "降级虚拟路由健康度失败: route_id={}, error={}",
            route_id,
            e.message
        );
        shared.logger_handle.service().log_system(
            LogLevel::Warn,
            &format!("降级虚拟路由健康度失败: route_id={}, error={}", route_id, e.message),
            Some(file!()),
        );
    } else {
        tracing::info!("虚拟路由已降级: route_id={}", route_id);
        shared.logger_handle.service().log_system(
            LogLevel::Info,
            &format!("虚拟路由已降级: route_id={}", route_id),
            Some(file!()),
        );
    }
}

/// 异步写入一条路由尝试历史
///
/// 通过 `tauri::async_runtime::spawn` 在后台执行，不阻塞响应返回；
/// 写入失败仅记录日志，不影响主流程。
///
/// `alias` 用作 virtual_provider_id 的替代（attempts 表需要 provider_id 字段，
/// 但 VirtualForwarder 拿到的是 alias；这里用 alias 占位，前端展示时通过 alias 反查）。
fn record_attempt(
    shared: &GatewaySharedState,
    route_id: &str,
    alias: &str,
    request_id: &str,
    attempt_index: usize,
    success: bool,
    status_code: Option<u16>,
    error_message: Option<&str>,
    duration_ms: u64,
) {
    let route_id = route_id.to_string();
    let alias = alias.to_string();
    let request_id = request_id.to_string();
    let error_message = error_message.map(|s| s.to_string());

    // 通过 alias 反查 virtual_provider_id（同步查询，避免在 spawn 内再次持有引用）
    let virtual_provider_id = shared
        .virtual_provider_handle
        .service()
        .list_providers()
        .ok()
        .and_then(|ps| ps.into_iter().find(|p| p.alias == alias).map(|p| p.id))
        .unwrap_or_else(|| alias.clone());

    // 克隆 handle（内部 Arc），让 async task 拥有 'static 句柄
    let handle = shared.virtual_provider_handle.clone();
    tauri::async_runtime::spawn(async move {
        handle.service().record_route_attempt(
            &route_id,
            &virtual_provider_id,
            &request_id,
            attempt_index,
            success,
            status_code,
            error_message.as_deref(),
            duration_ms,
        );
    });
}
