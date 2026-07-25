//! # Forwarder trait 与统一管道入口
//!
//! - `Forwarder` trait：抽象"执行一次上游请求"的行为
//! - `DirectForwarder`：真实供应商转发（构造 ClientFactory 执行）
//! - `VirtualForwarder`：虚拟供应商转发（健康度排序逐条降级重试）
//! - `ForwardPipeline`：统一管道入口，编排前置准备 / 执行 / 响应处理 / 日志记录

use std::time::Instant;

use async_trait::async_trait;
use axum::response::Response;
use serde_json::Value;
use std::sync::Arc;
use tauri::Emitter;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::gateway_runtime::client::{
    new_sse_usage_accumulator, ClientError, ClientFactory, UpstreamRequest,
    UpstreamResponse,
};
use crate::modules::gateway_runtime::logging::{
    LogKind, LogPipeline, LogRecord, LogStatus,
};
use crate::modules::gateway_runtime::service::GatewaySharedState;
use crate::modules::logger::types::LogLevel;

use super::call_log::{
    finish_call_log, finish_call_log_full, lookup_model_price, start_call_log,
};
use super::context::{ForwardContext, ForwardRequest, GatewayProtocol};
use super::response_handler::build_response;
use super::route_resolver::{resolve_route, ResolvedRoute};
use super::util::{build_error_tags, is_network_error, protocol_tags};
use super::virtual_forwarder::VirtualForwarder;

/// 转发执行结果
///
/// 在管道中携带上游响应、用于日志的请求/响应快照、usage 累加器等。
struct ExecutionOutcome {
    /// 上游响应
    response: UpstreamResponse,
    /// 上游 URL（用于日志）
    upstream_url: String,
    /// 原始请求体字符串（未截断，供 tauri-plugin-log 与 logger 复用）
    request_body_full: Option<String>,
    /// 协议标签
    tags: Vec<String>,
    /// 流式响应的 usage 累加器（非流式为 None）
    usage_accumulator: Option<
        crate::modules::gateway_runtime::client::SseUsageAccumulator,
    >,
    /// 已通过 ForwardContext（含失败时降级路由所需信息）
    virtual_route_id: Option<String>,
}

/// Forwarder trait：执行一次上游请求
#[async_trait]
pub trait Forwarder: Send + Sync {
    /// 执行上游请求
    ///
    /// 参数 `ctx` 已解析好目标供应商与认证；`body` 已替换为真实 model_id 并注入 stream_options。
    async fn execute(
        &self,
        shared: &GatewaySharedState,
        ctx: &ForwardContext,
        body: Value,
    ) -> Result<UpstreamResponse, ClientError>;
}

/// 真实供应商转发器
pub struct DirectForwarder;

impl DirectForwarder {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DirectForwarder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Forwarder for DirectForwarder {
    async fn execute(
        &self,
        _shared: &GatewaySharedState,
        ctx: &ForwardContext,
        body: Value,
    ) -> Result<UpstreamResponse, ClientError> {
        let client = ClientFactory::create(&ctx.upstream.provider.provider_type)?;
        let request = UpstreamRequest {
            protocol: ctx.gateway_protocol.to_upstream(),
            body,
            is_stream: ctx.upstream.is_stream,
        };
        client.execute(&ctx.upstream, request).await
    }
}

/// 统一管道入口
///
/// 编排：
/// 1. 解析 model → 真实或虚拟路由
/// 2. 前置：替换 model_id、设置 stream、注入 stream_options、start_call_log
/// 3. 执行：DirectForwarder 或 VirtualForwarder
/// 4. 后置：build_response、记录转发日志、完成调用记录
pub struct ForwardPipeline;

impl ForwardPipeline {
    /// 执行一次转发
    pub async fn run(
        shared: &GatewaySharedState,
        req: ForwardRequest,
    ) -> IcodeResult<Response> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let ForwardRequest {
            protocol,
            body,
            api_key_secret_id,
        } = req;

        // 解析 model
        let gateway_model_id = body
            .get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| IcodeError::validation("请求体缺少 model 字段"))?
            .to_string();

        let resolved = resolve_route(shared, &gateway_model_id, &request_id, protocol)?;

        match resolved {
            ResolvedRoute::Direct(ctx) => Self::run_direct(shared, ctx, body, api_key_secret_id).await,
            ResolvedRoute::Virtual {
                routes,
                alias,
                gateway_model_id,
                upstream_model_id,
                request_id,
                gateway_protocol,
            } => {
                let virtual_forwarder = VirtualForwarder::new(Arc::new(DirectForwarder::new()));
                virtual_forwarder
                    .run(
                        shared,
                        routes,
                        alias,
                        gateway_model_id,
                        upstream_model_id,
                        request_id,
                        gateway_protocol,
                        body,
                        api_key_secret_id,
                    )
                    .await
            }
        }
    }

    /// 真实供应商直连转发
    async fn run_direct(
        shared: &GatewaySharedState,
        mut ctx: ForwardContext,
        mut body: Value,
        api_key_secret_id: Option<String>,
    ) -> IcodeResult<Response> {
        let forwarder = DirectForwarder::new();
        Self::execute_and_finalize(shared, &forwarder, &mut ctx, &mut body, api_key_secret_id).await
    }

    /// 公共：执行单次 Forwarder 并完成响应/日志/调用记录收尾
    ///
    /// 被 `DirectForwarder` 与 `VirtualForwarder`（每条路由尝试）复用。
    pub(super) async fn execute_and_finalize(
        shared: &GatewaySharedState,
        forwarder: &dyn Forwarder,
        ctx: &mut ForwardContext,
        body: &mut Value,
        api_key_secret_id: Option<String>,
    ) -> IcodeResult<Response> {
        let request_id = ctx.upstream.request_id.clone();
        let gateway_model_id = ctx.gateway_model_id.clone();

        // 前置：替换 model、设置 stream、注入 stream_options
        Self::prepare_body(ctx, body);

        let is_stream = ctx.upstream.is_stream;
        let tags = protocol_tags(&ctx.upstream.provider.provider_type, is_stream);

        // 调用记录起始
        let log_id = start_call_log(shared, ctx, api_key_secret_id)?;
        let start_time = Instant::now();

        // 估算 prompt token
        let estimated_prompt_tokens =
            super::util::estimate_prompt_tokens(&gateway_model_id, body);

        // 请求体快照（未截断）
        let request_body_full = Some(body.to_string());

        // 执行上游请求
        let upstream_url = super::util::build_log_url(ctx);
        let result = forwarder.execute(shared, ctx, body.clone()).await;

        match result {
            Ok(response) => {
                let status = extract_status(&response);
                let is_streaming = matches!(response, UpstreamResponse::Streaming { .. });

                // 先克隆快照（不消费 response），供日志与 usage 解析复用
                let snapshot = clone_upstream_response(&response);

                // usage 与调用记录收尾
                let usage_accumulator = if is_streaming {
                    let acc = new_sse_usage_accumulator();
                    // 流式：先用估算 prompt_tokens 完成基础调用记录
                    let _ = finish_call_log(
                        shared,
                        &log_id,
                        start_time,
                        status.map(|s| s as i64),
                        None,
                        estimated_prompt_tokens,
                    );
                    Some(acc)
                } else {
                    // 非流式：从快照 body 解析 usage 并完成调用记录
                    let (prompt, completion, total, cached, cache_hit) =
                        parse_usage_from_snapshot(&snapshot);
                    let price = lookup_model_price(
                        shared,
                        &ctx.upstream.provider.id,
                        &ctx.upstream.upstream_model_id,
                    );
                    let _ = finish_call_log_full(
                        shared,
                        &log_id,
                        start_time,
                        status.map(|s| s as i64),
                        None,
                        prompt.or(estimated_prompt_tokens),
                        completion,
                        total,
                        cached,
                        cache_hit,
                        price,
                    );
                    None
                };

                // 构造 axum Response（此处消费 response）
                let duration_ms = start_time.elapsed().as_millis() as i64;
                let axum_resp = build_response(
                    response,
                    shared,
                    usage_accumulator.clone(),
                    Some(&log_id),
                    duration_ms,
                    None,
                );

                // 写入转发日志
                let (resp_body_for_log, usage_for_log) =
                    extract_response_for_log(&snapshot, &usage_accumulator).await;
                let mut builder = LogRecord::builder()
                    .kind(LogKind::Forward)
                    .method("POST")
                    .url(&upstream_url)
                    .duration_ms(duration_ms as u64)
                    .request_id(&request_id)
                    .model_id(&gateway_model_id)
                    .request_body(request_body_full.as_deref().unwrap_or(""))
                    .tags(tags.clone());
                if let Some(s) = status {
                    builder = builder.status_code(s).status(LogStatus::from_status(Some(s)));
                }
                if let Some(body) = resp_body_for_log {
                    builder = builder.response_body(&body);
                }
                if let Some(u) = usage_for_log {
                    if let Some(v) = u.prompt_tokens {
                        builder = builder.prompt_tokens(v);
                    }
                    if let Some(v) = u.completion_tokens {
                        builder = builder.completion_tokens(v);
                    }
                    if let Some(v) = u.total_tokens {
                        builder = builder.total_tokens(v);
                    }
                    if let Some(v) = u.cached_tokens {
                        builder = builder.cached_tokens(v);
                    }
                }
                let log_record = builder.build();
                LogPipeline::default_forward().record(shared, &log_record);

                Ok(axum_resp)
            }
            Err(err) => {
                let is_network = is_network_error(&err);
                let icode_err: IcodeError = err.into();
                let duration_ms = start_time.elapsed().as_millis() as u64;
                let _ = finish_call_log(
                    shared,
                    &log_id,
                    start_time,
                    None,
                    Some(icode_err.message.clone()),
                    estimated_prompt_tokens,
                );

                let error_tags = build_error_tags(&tags, is_network);
                let log_record = LogRecord::builder()
                    .kind(LogKind::Forward)
                    .method("POST")
                    .url(&upstream_url)
                    .duration_ms(duration_ms)
                    .request_id(&request_id)
                    .model_id(&gateway_model_id)
                    .request_body(request_body_full.as_deref().unwrap_or(""))
                    .error_message(&icode_err.message)
                    .tags(error_tags.clone())
                    .status(LogStatus::Failed)
                    .build();
                LogPipeline::default_forward().record(shared, &log_record);

                // 系统错误日志
                let prefix = ctx.gateway_protocol.label();
                let error_log_msg = if is_network {
                    format!("{} 转发失败 [network]: {}", prefix, icode_err.message)
                } else {
                    format!("{} 转发失败: {}", prefix, icode_err.message)
                };
                shared.logger_handle.service().log_system(
                    LogLevel::Error,
                    &error_log_msg,
                    Some(file!()),
                );

                // 虚拟路由失败降级
                if let Some(route_id) = &ctx.virtual_route_id {
                    super::virtual_forwarder::degrade_route(shared, route_id);
                }

                let _ = shared.app_handle.emit("forward:failed", &icode_err);
                Err(icode_err)
            }
        }
    }

    /// 前置准备：替换 model_id、设置 stream、注入 stream_options
    fn prepare_body(ctx: &mut ForwardContext, body: &mut Value) {
        // 替换 model 为真实上游 model_id
        if let Some(obj) = body.as_object_mut() {
            obj.insert(
                "model".to_string(),
                Value::String(ctx.upstream.upstream_model_id.clone()),
            );
        }

        // 判断 stream
        let is_stream = body
            .get("stream")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        ctx.set_stream(is_stream);

        // OpenAI 流式请求注入 stream_options.include_usage
        if is_stream
            && ctx.gateway_protocol == GatewayProtocol::ChatCompletions
        {
            if let Some(obj) = body.as_object_mut() {
                obj.insert(
                    "stream_options".to_string(),
                    serde_json::json!({"include_usage": true}),
                );
            }
        }
    }
}

/// 从 UpstreamResponse 中提取状态码
fn extract_status(response: &UpstreamResponse) -> Option<u16> {
    match response {
        UpstreamResponse::Streaming { response, .. } => Some(response.status().as_u16()),
        UpstreamResponse::Complete { status, .. } => Some(status.as_u16()),
    }
}

/// 克隆 UpstreamResponse 用于日志（流式仅取状态，不消费 body）
fn clone_upstream_response(response: &UpstreamResponse) -> UpstreamResponseSnapshot {
    match response {
        UpstreamResponse::Streaming { response, .. } => {
            UpstreamResponseSnapshot::Streaming {
                status: response.status().as_u16(),
            }
        }
        UpstreamResponse::Complete { status, body, .. } => {
            UpstreamResponseSnapshot::Complete {
                status: status.as_u16(),
                body: body.clone(),
            }
        }
    }
}

enum UpstreamResponseSnapshot {
    Streaming { status: u16 },
    Complete { status: u16, body: Vec<u8> },
}

/// 从响应快照解析 usage 数据（不消费原始 response）
///
/// 仅非流式快照有 body；流式快照返回全 None。
fn parse_usage_from_snapshot(
    snapshot: &UpstreamResponseSnapshot,
) -> (Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<bool>) {
    match snapshot {
        UpstreamResponseSnapshot::Complete { body, .. } => {
            let body_str = String::from_utf8_lossy(body);
            super::usage_extractor::parse_usage_from_response_body(&body_str)
        }
        UpstreamResponseSnapshot::Streaming { .. } => (None, None, None, None, None),
    }
}

/// 从响应快照提取日志所需的响应体与 usage
///
/// - 非流式：返回 (响应体字符串, usage)
/// - 流式：返回 (None, None)（流式 body 在透传中无法预先读取）
async fn extract_response_for_log(
    snapshot: &UpstreamResponseSnapshot,
    _accumulator: &Option<
        crate::modules::gateway_runtime::client::SseUsageAccumulator,
    >,
) -> (Option<String>, Option<
    crate::modules::gateway_runtime::client::SseUsageData,
>) {
    match snapshot {
        UpstreamResponseSnapshot::Complete { body, .. } => {
            let body_str = String::from_utf8_lossy(body).to_string();
            // usage 已在 finish_call_log_full 中写入调用记录，日志中置空避免重复
            (Some(body_str), None)
        }
        UpstreamResponseSnapshot::Streaming { .. } => (None, None),
    }
}
