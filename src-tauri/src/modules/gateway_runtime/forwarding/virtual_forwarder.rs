//! # 虚拟供应商转发器
//!
//! 持有虚拟供应商的全部候选路由，按健康度排序逐条尝试，失败时降级该路由
//! 健康度并继续下一条，直到成功或全部失败。
//!
//! 与旧实现差异：
//! - 不再硬编码仅尝试前 2 条路由
//! - 失败降级是即时的，下条路由立刻重试，无需等待下次请求
//! - 路由解析（route_resolver）与执行（DirectForwarder）职责分离

use std::sync::Arc;

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
                    tracing::warn!("虚拟路由 {} 不可用: {}", route.id, e.message);
                    last_err = Some(e);
                    continue;
                }
            };

            // 调用公共管道执行单次转发
            let mut ctx = ctx;
            let result = ForwardPipeline::execute_and_finalize(
                shared,
                self.direct.as_ref(),
                &mut ctx,
                &mut body,
                api_key_secret_id.clone(),
            )
            .await;

            match result {
                Ok(resp) => return Ok(resp),
                Err(err) => {
                    // 失败降级该路由健康度
                    degrade_route(shared, &route.id);
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
