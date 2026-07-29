//! # axum 路由定义与请求转发
//!
//! 定义 Gateway HTTP Server 的路由树与各路径的 handler。
//!
//! ## 路由清单
//!
//! - `GET /health`：存活检查（always 200）
//! - `GET /readyz`：就绪检查（含数据库连通性）
//! - `GET /v1/models`：列出所有对外暴露的模型
//! - `POST /v1/chat/completions`：聊天接口（转发到上游供应商）
//! - `POST /v1/messages`：Anthropic 兼容接口（转发到上游供应商）
//!
//! ## 架构
//!
//! 本文件仅负责路由注册与 handler 编排，实际转发逻辑委托给
//! [`forwarding::ForwardPipeline`](super::forwarding::ForwardPipeline)，
//! 日志记录委托给 [`logging::LogPipeline`](super::logging::LogPipeline)。

use axum::body::Body;
use axum::extract::{Extension, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use serde_json::{json, Value};
use tower_http::trace::{MakeSpan, TraceLayer};
use tracing::Span;

use crate::core::trace_id::next_trace_id;
use crate::modules::gateway_runtime::forwarding::context::GatewayProtocol;
use crate::modules::gateway_runtime::forwarding::{
    parse_usage_from_response_body, upstream_error_response, ForwardPipeline, ForwardRequest,
};
use crate::modules::gateway_runtime::logging::{LogKind, LogPipeline, LogRecord, LogStatus};

use crate::error::IcodeResult;

use super::auth::{AuthState, RequestApiKey};
use super::service::GatewaySharedState;

/// 构建网关路由
///
/// 将共享状态注入 axum State，注册认证中间件与各路径 handler。
///
/// # Layer 顺序（后加的在外层）
///
/// 1. `auth_middleware`（内层）：认证逻辑在请求 span 内执行
/// 2. `TraceLayer`（外层）：即使认证失败也会创建 span，不丢失观测
///
/// `TraceLayer` 使用自定义 [`TraceIdSpan`] 为每个 HTTP 请求生成 `trace_id`，
/// 配合 `TraceIdLayer`（在 `init_tracing` 中注册）让请求路径内所有 `log::info!`
/// 自动带 `[tid=...]` 前缀，实现全链路日志关联。
pub fn build_router(shared: GatewaySharedState) -> Router {
    let auth_state = AuthState::new(
        shared.ai_gateway_handle.clone(),
        shared.secret_handle.clone(),
        shared.inner_cli_api_key.clone(),
    );

    Router::new()
        .route("/health", get(health))
        .route("/readyz", get(readyz))
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/messages", post(anthropic_messages))
        .with_state(shared)
        // 认证中间件：仅对非 /health、/readyz 路径生效（中间件内部判断豁免）
        .layer(axum::middleware::from_fn_with_state(
            auth_state,
            super::auth::auth_middleware,
        ))
        // TraceLayer（最外层）：为每个 HTTP 请求创建 tracing span 并注入 trace_id
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(TraceIdSpan),
        )
}

/// 自定义 `MakeSpan`：为每个 HTTP 请求生成 `trace_id` 并注入 span 字段
///
/// 生成的 span 名为 `http.request`，包含字段：
/// - `trace_id`：雪花 ID 转 32 进制（13 字符），关键字段名必须为 `trace_id`
///   以便 `TraceIdLayer::on_new_span` 提取并存入 span extensions
/// - `method`：HTTP 方法
/// - `uri`：请求 URI
///
/// span 进入时 `TraceIdLayer::on_enter` 将 `trace_id` 写入 thread-local，
/// 请求路径内所有 `log::info!` 经 `log` feature 桥接后自动带 `[tid=...]` 前缀。
#[derive(Clone)]
struct TraceIdSpan;

impl<B> MakeSpan<B> for TraceIdSpan {
    fn make_span(&mut self, request: &axum::http::Request<B>) -> Span {
        let trace_id = next_trace_id();
        tracing::info_span!(
            "http.request",
            trace_id = %trace_id,
            method = %request.method(),
            uri = %request.uri(),
        )
    }
}

// ===== Handlers =====

/// 健康检查：存活检查
///
/// 始终返回 200，用于 HTTP Server 是否响应的探针
async fn health(State(_state): State<GatewaySharedState>) -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "service": "i-code-gateway"
    }))
}

/// 就绪检查：含数据库连通性
///
/// v0.1 仅检查数据库连接，后续可扩展上游供应商可达性检查
async fn readyz(State(_state): State<GatewaySharedState>) -> impl IntoResponse {
    let database_ok = check_database_connection();

    Json(json!({
        "alive": true,
        "ready": database_ok,
        "databaseOk": database_ok,
        "checkedAt": chrono::Utc::now().timestamp_millis()
    }))
}

/// 列出所有对外暴露的模型
///
/// OpenAI 兼容格式：`{ object: "list", data: [...] }`
/// 数据源同时包含真实供应商模型与虚拟供应商模型。
async fn list_models(
    State(state): State<GatewaySharedState>,
) -> Response {
    let start = std::time::Instant::now();
    // 复用 TraceIdSpan 注入的 trace_id 作为 request_id，
    // 使自研 logger 的 request_id 与 tracing 日志的 [tid=...] 前缀一致，便于全链路关联。
    // TraceLayer 已在最外层为每个请求创建 span 并设置 thread-local，
    // 此处读取即可；fallback 仅防御性兜底（理论上不会触发）。
    let request_id = crate::core::trace_id_layer::current_trace_id()
        .unwrap_or_else(crate::core::trace_id::next_trace_id);

    let result = build_exposed_models_response(&state);

    let response: Response = match result {
        Ok(data) => Json(json!({
            "object": "list",
            "data": data
        })).into_response(),
        Err(err) => upstream_error_response(err),
    };

    log_gateway_response(
        &state,
        "GET",
        "/v1/models",
        &request_id,
        start,
        None,
        None,
        response,
    ).await
}

/// 构造 `/v1/models` 的模型列表数据
///
/// 合并真实供应商暴露模型与虚拟供应商暴露模型，统一返回 OpenAI 兼容格式。
fn build_exposed_models_response(
    state: &GatewaySharedState,
) -> IcodeResult<Vec<Value>> {
    let real_models = state.ai_gateway_handle.service().list_exposed_models()?;
    let virtual_models = state
        .virtual_provider_handle
        .service()
        .list_exposed_virtual_models()?;

    let mut data: Vec<Value> = real_models
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "object": "model",
                "created": 0,
                "owned_by": m.provider_slug,
                "display_name": m.display_name,
            })
        })
        .collect();

    data.extend(virtual_models.iter().map(|m| {
        json!({
            "id": m.id,
            "object": "model",
            "created": 0,
            "owned_by": m.alias,
            "display_name": m.display_name,
        })
    }));

    Ok(data)
}

/// 聊天接口（OpenAI 兼容）
///
/// 将请求转发到真实供应商；支持流式 SSE 与非流式 JSON 响应。
async fn chat_completions(
    State(state): State<GatewaySharedState>,
    Extension(api_key): Extension<RequestApiKey>,
    Json(body): Json<Value>,
) -> Response {
    let start = std::time::Instant::now();
    // 复用 TraceIdSpan 注入的 trace_id 作为 request_id，
    // 使自研 logger 的 request_id 与 tracing 日志的 [tid=...] 前缀一致，便于全链路关联。
    // TraceLayer 已在最外层为每个请求创建 span 并设置 thread-local，
    // 此处读取即可；fallback 仅防御性兜底（理论上不会触发）。
    let request_id = crate::core::trace_id_layer::current_trace_id()
        .unwrap_or_else(crate::core::trace_id::next_trace_id);
    let gateway_model_id = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // 请求体快照（未截断，截断交给 LogPipeline）
    let request_body_full = serde_json::to_string(&body).ok();

    let response = match ForwardPipeline::run(
        &state,
        ForwardRequest {
            protocol: GatewayProtocol::ChatCompletions,
            body,
            api_key_secret_id: api_key.0,
        },
    )
    .await
    {
        Ok(resp) => resp,
        Err(err) => upstream_error_response(err),
    };

    log_gateway_response(
        &state,
        "POST",
        "/v1/chat/completions",
        &request_id,
        start,
        request_body_full.as_deref(),
        gateway_model_id.as_deref(),
        response,
    ).await
}

/// Anthropic 兼容消息接口
///
/// 将请求转发到真实供应商；支持流式 SSE 与非流式 JSON 响应。
async fn anthropic_messages(
    State(state): State<GatewaySharedState>,
    Extension(api_key): Extension<RequestApiKey>,
    Json(body): Json<Value>,
) -> Response {
    let start = std::time::Instant::now();
    // 复用 TraceIdSpan 注入的 trace_id 作为 request_id，
    // 使自研 logger 的 request_id 与 tracing 日志的 [tid=...] 前缀一致，便于全链路关联。
    // TraceLayer 已在最外层为每个请求创建 span 并设置 thread-local，
    // 此处读取即可；fallback 仅防御性兜底（理论上不会触发）。
    let request_id = crate::core::trace_id_layer::current_trace_id()
        .unwrap_or_else(crate::core::trace_id::next_trace_id);
    let gateway_model_id = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // 请求体快照（未截断，截断交给 LogPipeline）
    let request_body_full = serde_json::to_string(&body).ok();

    let response = match ForwardPipeline::run(
        &state,
        ForwardRequest {
            protocol: GatewayProtocol::AnthropicMessages,
            body,
            api_key_secret_id: api_key.0,
        },
    )
    .await
    {
        Ok(resp) => resp,
        Err(err) => upstream_error_response(err),
    };

    log_gateway_response(
        &state,
        "POST",
        "/v1/messages",
        &request_id,
        start,
        request_body_full.as_deref(),
        gateway_model_id.as_deref(),
        response,
    ).await
}

// ===== 辅助函数 =====

/// 检查数据库连接是否正常
fn check_database_connection() -> bool {
    use crate::db::get_db_pool;
    match get_db_pool() {
        Ok(pool) => match pool.get() {
            Ok(conn) => {
                // 执行简单查询验证连接
                match conn.query_row("SELECT 1", [], |row| row.get::<_, i64>(0)) {
                    Ok(1) => true,
                    _ => false,
                }
            }
            Err(_) => false,
        },
        Err(_) => false,
    }
}

/// 读取当前直连网关日志配置
fn get_gateway_log_config(state: &GatewaySharedState) -> crate::modules::logger::types::GatewayLogConfig {
    state.logger_handle.service().get_settings().to_gateway_config()
}

/// 根据响应特征检测协议标签
///
/// - `Content-Type: text/event-stream` → `sse`
/// - `Upgrade: websocket` → `websocket`
fn detect_response_tags(response: &Response) -> Vec<String> {
    let mut tags = Vec::new();
    let headers = response.headers();
    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        if let Ok(s) = ct.to_str() {
            if s.contains("text/event-stream") {
                tags.push("sse".to_string());
            }
        }
    }
    if let Some(upgrade) = headers.get(header::UPGRADE) {
        if let Ok(s) = upgrade.to_str() {
            if s.to_lowercase().contains("websocket") {
                tags.push("websocket".to_string());
            }
        }
    }
    tags
}

/// 记录网关响应日志并返回响应
///
/// 若开启直连网关响应体记录且响应非流式，会读取响应体用于日志，并重新构造响应返回。
/// 对于流式 / WebSocket 响应，仅在返回非正常状态码时读取响应体，避免破坏正常流式传输。
///
/// 使用 `LogPipeline`（设计模式）统一推送到自研 logger 与 tauri-plugin-log，
/// 不再手写多条 `write_*` / `emit_*` 函数。
async fn log_gateway_response(
    state: &GatewaySharedState,
    method: &str,
    path: &str,
    request_id: &str,
    start: std::time::Instant,
    request_body_full: Option<&str>,
    model_id: Option<&str>,
    response: Response,
) -> Response {
    let cfg = get_gateway_log_config(state);
    let status = response.status();
    let tags = detect_response_tags(&response);

    let is_streaming =
        tags.contains(&"sse".to_string()) || tags.contains(&"websocket".to_string());
    let is_error = !status.is_success();
    // 非流式响应按原逻辑读取 body；流式 / WebSocket 仅在非正常状态码时读取错误响应体
    let should_log_body = cfg.enable_gateway_response_log && (!is_streaming || is_error);

    // 解析 usage 数据（截断前），同时保留完整 body 供 tauri-plugin-log 使用
    let (
        response,
        response_body_full,
        parsed_prompt_tokens,
        parsed_completion_tokens,
        parsed_total_tokens,
        parsed_cached_tokens,
    ) = if should_log_body {
        let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
        match axum::body::to_bytes(response.into_body(), usize::MAX).await {
            Ok(bytes) => {
                let body_str = String::from_utf8_lossy(&bytes).into_owned();
                // 在截断之前解析 usage 数据
                let (prompt_tokens, completion_tokens, total_tokens, cached_tokens, _cache_hit) =
                    parse_usage_from_response_body(&body_str);
                let mut builder = Response::builder().status(status);
                if let Some(ct) = content_type {
                    builder = builder.header(header::CONTENT_TYPE, ct);
                }
                let rebuilt = builder.body(Body::from(bytes)).unwrap_or_else(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("构造响应失败: {}", e),
                    )
                        .into_response()
                });
                (
                    rebuilt,
                    Some(body_str),
                    prompt_tokens,
                    completion_tokens,
                    total_tokens,
                    cached_tokens,
                )
            }
            Err(e) => {
                log::warn!("读取直连网关响应体失败: {}", e);
                state.logger_handle.service().log_system(
                    crate::modules::logger::types::LogLevel::Warn,
                    &format!("读取直连网关响应体失败: {}", e),
                    Some(file!()),
                );
                (
                    (StatusCode::BAD_GATEWAY, "读取响应体失败").into_response(),
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
        }
    } else {
        (response, None, None, None, None, None)
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    // 通过 LogPipeline 统一推送（自研 logger 自动按 gateway 配置截断，tauri-plugin-log 不截断）
    let mut builder = LogRecord::builder()
        .kind(LogKind::Gateway)
        .method(method)
        .url(path)
        .status_code(status.as_u16())
        .duration_ms(duration_ms)
        .request_id(request_id)
        .tags(tags.clone())
        .status(LogStatus::from_status(Some(status.as_u16())));

    if let Some(mid) = model_id {
        builder = builder.model_id(mid);
    }
    if let Some(body) = request_body_full {
        builder = builder.request_body(body);
    }
    if let Some(body) = response_body_full.as_deref() {
        builder = builder.response_body(body);
    }
    if let Some(v) = parsed_prompt_tokens {
        builder = builder.prompt_tokens(v);
    }
    if let Some(v) = parsed_completion_tokens {
        builder = builder.completion_tokens(v);
    }
    if let Some(v) = parsed_total_tokens {
        builder = builder.total_tokens(v);
    }
    if let Some(v) = parsed_cached_tokens {
        builder = builder.cached_tokens(v);
    }

    let record = builder.build();
    LogPipeline::default_gateway().record(state, &record);

    response
}

