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

use axum::http::{header::RETRY_AFTER, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{Duration, Utc};
use serde_json::Value;
use tauri::Emitter;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::gateway_runtime::service::GatewaySharedState;
use crate::modules::logger::types::LogLevel;
use crate::modules::virtual_provider::settings;
use crate::modules::virtual_provider::types::{
    RouteFailureKind, RoutePenalty, VirtualModelRoute,
};

use super::context::GatewayProtocol;
use super::error_classifier::{AttemptFailure, AttemptOutcome, RouteVerdict};
use super::forwarder::{Forwarder, ForwardPipeline};
use super::route_resolver::build_virtual_context;

/// 单次虚拟供应商请求的总时间预算（毫秒）
///
/// 达到预算后停止故障转移，返回最后一次失败响应，避免把客户端请求拖到数分钟。
/// 单次路由尝试已由 `ForwardPipeline` 限制为「等待响应头 ≤ 60s」，
/// 这里的预算用于限制路由数量较多时的累计耗时（至少可尝试 3 条路由）。
const VIRTUAL_TOTAL_BUDGET_MS: u64 = 180_000;

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
        body: Value,
        api_key_secret_id: Option<String>,
        request_headers_json: Option<String>,
    ) -> IcodeResult<Response> {
        let _ = upstream_model_id; // 已在 route_resolver 中通过 build_virtual_context 传入 target_model_id
        let start = Instant::now();
        // 每次尝试都从原始请求体出发：避免上一条路由的 extra_body / 协议桥接改写污染后续请求
        let original_body = body.clone();
        // 全部路由失败时用于聚合错误（按尝试顺序保留每条路由的失败原因）
        let mut failures: Vec<AttemptFailure> = Vec::new();

        // 全部路由都处于永久性故障：按「兜底间隔」只尝试最久未尝试的一条，
        // 避免全部失活后每个请求都持续调用上游（失败且烧钱）。
        let attempt_routes: Vec<VirtualModelRoute> = if all_routes_permanently_failed(&routes) {
            let interval = settings::all_failed_fallback_interval_secs();
            let threshold = (Utc::now() - Duration::seconds(interval as i64)).to_rfc3339();
            let eligible = routes
                .iter()
                .filter(|r| attempt_eligible(r, &threshold))
                .min_by(|a, b| compare_last_attempted(a, b));
            match eligible {
                Some(route) => {
                    tracing::warn!(
                        "虚拟供应商 '{}' 全部路由均处于永久故障，本次仅按兜底间隔尝试 1 条 | route_id={} | request_id={}",
                        alias,
                        route.id,
                        request_id
                    );
                    let _ = shared.logger_handle.service().log_system(
                        LogLevel::Warn,
                        &format!(
                            "虚拟供应商 '{}' 全部路由永久故障，本次按兜底间隔尝试 1 条：route_id={}",
                            alias, route.id
                        ),
                        Some(file!()),
                    );
                    vec![route.clone()]
                }
                None => {
                    let response = all_cooling_response(&alias, &routes, interval);
                    return Ok(response);
                }
            }
        } else {
            routes.clone()
        };

        for (index, route) in attempt_routes.iter().enumerate() {
            // 预算检查：超过总耗时即停止故障转移（单条路由的内部重试由 DirectForwarder 限制为 1 次）
            if start.elapsed().as_millis() as u64 >= VIRTUAL_TOTAL_BUDGET_MS {
                tracing::warn!(
                    "虚拟供应商 '{}' 超过总耗时预算 {}ms，停止故障转移 | request_id={}",
                    alias,
                    VIRTUAL_TOTAL_BUDGET_MS,
                    request_id
                );
                break;
            }

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
                    // 路由目标供应商不可用（禁用 / 媒体协议 / 读取失败）→ 配置类长冷却
                    degrade_route(
                        shared,
                        &route.id,
                        RouteVerdict::new(
                            RouteFailureKind::ProviderMisconfigured,
                            RoutePenalty::Long,
                        ),
                        &e.message,
                    );
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
                    failures.push(AttemptFailure {
                        verdict: RouteVerdict::new(
                            RouteFailureKind::ProviderMisconfigured,
                            RoutePenalty::Long,
                        ),
                        status: 0,
                        message: e.message.clone(),
                        response: super::util::upstream_error_response(e),
                    });
                    continue;
                }
            };

            // 调用公共管道执行单次转发（使用原始请求体的副本）
            let mut ctx = ctx;
            let mut attempt_body = original_body.clone();
            let route_start = Instant::now();
            let result = ForwardPipeline::execute_and_finalize(
                shared,
                self.direct.as_ref(),
                &mut ctx,
                &mut attempt_body,
                api_key_secret_id.clone(),
                request_headers_json.clone(),
            )
            .await;
            let duration_ms = route_start.elapsed().as_millis() as u64;

            match result {
                AttemptOutcome::Success(resp) => {
                    // 2xx：记录成功 attempt 并直接返回
                    let status_code = resp.status().as_u16();
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
                AttemptOutcome::Failure(failure) => {
                    // 失败：记录真实状态码（网络层错误为 0，不记录）
                    record_attempt(
                        shared,
                        &route.id,
                        &alias,
                        &request_id,
                        index,
                        false,
                        if failure.status > 0 {
                            Some(failure.status)
                        } else {
                            None
                        },
                        Some(failure.message.as_str()),
                        duration_ms,
                    );

                    // 请求级错误（参数非法 / 上下文超限 / 内容安全拒绝）：直接透传上游错误体
                    if !failure.verdict.failover {
                        tracing::info!(
                            "虚拟路由 {} 返回请求级错误，不再尝试后续路由 | status={} | kind={}",
                            route.id,
                            failure.status,
                            failure.verdict.kind.as_str()
                        );
                        return Ok(failure.response);
                    }

                    // 路由级故障：按判定惩罚降级该路由，继续下一条
                    degrade_route(shared, &route.id, failure.verdict, &failure.message);
                    failures.push(failure);
                    continue;
                }
            }
        }

        // 全部失败（或预算耗尽）：返回聚合错误（标准 OpenAI 错误体 + 每条路由失败摘要）
        if !failures.is_empty() {
            let (response, error) =
                aggregate_failure_response(&alias, &request_id, routes.len(), &failures);
            if let Some(app) = &shared.app_handle {
                let _ = app.emit("virtual:all-routes-failed", &error);
            }
            return Ok(response);
        }

        Err(IcodeError::internal(format!(
            "虚拟供应商 '{}' 全部 {} 条路由均失败",
            alias,
            attempt_routes.len()
        )))
    }
}

/// 全部路由失败时构造聚合错误响应
///
/// 返回标准 OpenAI 错误体（`error.{message,type,param,code}`）：
/// - `message` 逐条列出各路由的失败原因（避免客户端只看到最后一条）
/// - `error.details` 附带结构化尝试链（routeIndex / status / kind / message）
fn aggregate_failure_response(
    alias: &str,
    request_id: &str,
    total_routes: usize,
    failures: &[AttemptFailure],
) -> (Response, IcodeError) {
    let mut lines = Vec::with_capacity(failures.len());
    let mut attempts = Vec::with_capacity(failures.len());
    for (i, failure) in failures.iter().enumerate() {
        let status_text = if failure.status == 0 {
            "network".to_string()
        } else {
            format!("HTTP {}", failure.status)
        };
        let message = truncate_chars(&failure.message, 200);
        lines.push(format!(
            "第 {} 条路由 {}（{}）：{}",
            i + 1,
            status_text,
            failure.verdict.kind.as_str(),
            message
        ));
        attempts.push(serde_json::json!({
            "routeIndex": i + 1,
            "status": if failure.status == 0 {
                Value::Null
            } else {
                serde_json::json!(failure.status)
            },
            "kind": failure.verdict.kind.as_str(),
            "message": message,
        }));
    }

    let message = format!(
        "虚拟供应商 '{}' 共 {} 条路由，本次尝试 {} 条均失败（request_id={}）\n{}",
        alias,
        total_routes,
        failures.len(),
        request_id,
        lines.join("\n")
    );
    let body = serde_json::json!({
        "error": {
            "message": message,
            "type": "api_error",
            "param": Value::Null,
            "code": "all_routes_failed",
            "details": {
                "requestId": request_id,
                "attempts": attempts,
            },
        }
    });

    let codes: Vec<u16> = failures.iter().map(|f| f.status).collect();
    let status = select_status(&codes);
    let code = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
    let error = IcodeError::gateway(message);
    ((code, Json(body)).into_response(), error)
}

/// 聚合状态码：5xx 优先（502）→ 429 → 402 → 鉴权（403）→ 404 → 最后一条
fn select_status(codes: &[u16]) -> u16 {
    let mut saw_5xx = false;
    let mut saw_429 = false;
    let mut saw_402 = false;
    let mut saw_auth = false;
    let mut saw_404 = false;
    let mut last = 0u16;

    for &status in codes {
        if status == 0 {
            continue;
        }
        last = status;
        match status {
            500..=599 => saw_5xx = true,
            429 => saw_429 = true,
            402 => saw_402 = true,
            401 | 403 => saw_auth = true,
            404 => saw_404 = true,
            _ => {}
        }
    }

    if saw_5xx {
        502
    } else if saw_429 {
        429
    } else if saw_402 {
        402
    } else if saw_auth {
        403
    } else if saw_404 {
        404
    } else if last > 0 {
        last
    } else {
        502
    }
}

/// 路由是否处于永久性故障（额度 / 凭证 / 模型 / 配置），不看冷却是否到期
fn is_permanent_failure(route: &VirtualModelRoute) -> bool {
    route
        .failure_kind
        .as_deref()
        .and_then(RouteFailureKind::from_str)
        .map(|kind| kind.is_permanent())
        .unwrap_or(false)
}

/// 全部路由都处于永久性故障（此时只允许按兜底间隔尝试一条）
fn all_routes_permanently_failed(routes: &[VirtualModelRoute]) -> bool {
    !routes.is_empty() && routes.iter().all(is_permanent_failure)
}

/// 上次真实尝试是否已超过兜底间隔（`None` 视为最久未尝试）
fn attempt_eligible(route: &VirtualModelRoute, threshold: &str) -> bool {
    route
        .last_attempted_at
        .as_deref()
        .map_or(true, |last| last < threshold)
}

/// 按「最近一次尝试时间」升序比较（`None` 视为最久未尝试，排最前）
fn compare_last_attempted(
    a: &VirtualModelRoute,
    b: &VirtualModelRoute,
) -> std::cmp::Ordering {
    match (a.last_attempted_at.as_deref(), b.last_attempted_at.as_deref()) {
        (None, None) => a.priority.cmp(&b.priority),
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(x), Some(y)) => x.cmp(y).then_with(|| a.priority.cmp(&b.priority)),
    }
}

/// 永久故障类型 → 代表性 HTTP 状态码（用于「全部冷却」响应）
fn status_for_failure_kind(kind: Option<&str>) -> u16 {
    match kind.and_then(RouteFailureKind::from_str) {
        Some(RouteFailureKind::QuotaExhausted) => 402,
        Some(RouteFailureKind::RateLimited) => 429,
        Some(RouteFailureKind::AuthFailure) => 403,
        Some(RouteFailureKind::ModelUnavailable) => 404,
        Some(RouteFailureKind::ProviderMisconfigured)
        | Some(RouteFailureKind::UpstreamUnavailable)
        | Some(RouteFailureKind::Timeout) => 502,
        _ => 0,
    }
}

/// 全部路由永久失活且兜底间隔未到：直接返回标准错误体（不调用上游），并带 `Retry-After`
fn all_cooling_response(
    alias: &str,
    routes: &[VirtualModelRoute],
    interval_secs: u64,
) -> Response {
    let now = Utc::now();
    let mut retry_after = interval_secs;
    let mut details = Vec::with_capacity(routes.len());
    let mut codes = Vec::with_capacity(routes.len());

    for (i, route) in routes.iter().enumerate() {
        let remaining = match route
            .last_attempted_at
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        {
            Some(t) => {
                let elapsed = now
                    .signed_duration_since(t.with_timezone(&Utc))
                    .num_seconds()
                    .max(0) as u64;
                interval_secs.saturating_sub(elapsed)
            }
            None => 0,
        };
        retry_after = retry_after.min(remaining);

        let code = status_for_failure_kind(route.failure_kind.as_deref());
        if code > 0 {
            codes.push(code);
        }
        details.push(serde_json::json!({
            "routeIndex": i + 1,
            "kind": route.failure_kind,
            "lastAttemptedAt": route.last_attempted_at,
            "retryAfterSecs": remaining,
        }));
    }

    let message = format!(
        "虚拟供应商 '{}' 全部 {} 条路由均处于永久故障冷却，兜底间隔 {}s 未到，本次跳过上游调用（最早可重试：{}s 后）",
        alias,
        routes.len(),
        interval_secs,
        retry_after
    );
    let body = serde_json::json!({
        "error": {
            "message": message,
            "type": "api_error",
            "param": Value::Null,
            "code": "all_routes_cooling",
            "details": {
                "retryAfterSecs": retry_after,
                "routes": details,
            },
        }
    });

    let status = select_status(&codes);
    let mut response =
        (StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY), Json(body))
            .into_response();
    if let Ok(value) = HeaderValue::from_str(&retry_after.to_string()) {
        response.headers_mut().insert(RETRY_AFTER, value);
    }
    response
}

fn truncate_chars(s: &str, limit: usize) -> String {
    if s.chars().count() <= limit {
        s.to_string()
    } else {
        s.chars().take(limit).collect()
    }
}

/// 降级虚拟供应商路由健康度
///
/// 请求级错误（`failover=false`）与零惩罚（`RoutePenalty::None`）不写降级状态，
/// 避免"用户请求非法"或"上下文超限"把正常路由标记为故障。
pub(crate) fn degrade_route(
    shared: &GatewaySharedState,
    route_id: &str,
    verdict: RouteVerdict,
    error_text: &str,
) {
    if !verdict.failover || verdict.penalty == RoutePenalty::None {
        return;
    }

    if let Err(e) = shared
        .virtual_provider_handle
        .service()
        .degrade_route_health(route_id, verdict.kind, verdict.penalty, error_text)
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
        tracing::info!(
            "虚拟路由已降级: route_id={}, kind={}, penalty={:?}",
            route_id,
            verdict.kind.as_str(),
            verdict.penalty
        );
        shared.logger_handle.service().log_system(
            LogLevel::Info,
            &format!(
                "虚拟路由已降级: route_id={}, kind={}, error={}",
                route_id,
                verdict.kind.as_str(),
                error_text
            ),
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

    // 克隆 handle（内部 Arc），让 async task 拥有 'static 句柄。
    // 使用 tokio::spawn（网关运行在 tokio 运行时内），避免额外拉起 Tauri 全局运行时线程池。
    let handle = shared.virtual_provider_handle.clone();
    tokio::spawn(async move {
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
