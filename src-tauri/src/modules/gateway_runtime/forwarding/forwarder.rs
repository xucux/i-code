//! # Forwarder trait 与统一管道入口
//!
//! - `Forwarder` trait：抽象"执行一次上游请求"的行为
//! - `DirectForwarder`：真实供应商转发（构造 ClientFactory 执行，含供应商级重试）
//! - `VirtualForwarder`：虚拟供应商转发（健康度排序逐条降级重试）
//! - `ForwardPipeline`：统一管道入口，编排前置准备 / 执行 / 响应处理 / 日志记录

use std::time::{Duration, Instant};

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
use crate::modules::shared::RetryConfig;

use super::call_log::{
    finish_call_log, finish_call_log_full, lookup_model_price, start_call_log,
};
use super::context::{ForwardContext, ForwardRequest, GatewayProtocol};
use super::response_handler::build_response;
use super::route_resolver::{resolve_route, ResolvedRoute};
use super::util::{build_error_tags, is_network_error, protocol_tags};
use super::virtual_forwarder::VirtualForwarder;

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
///
/// 在 `execute` 中实现供应商级重试：读取 `provider.retry_json` 解析 `RetryConfig`，
/// 对网络错误与可重试 HTTP 状态码（429/500/502/503/504）按指数退避+抖动策略重试。
/// 重试在 HTTP 请求层面进行，不涉及流式响应的中断重传——一旦上游返回 2xx 并开始
/// 流式传输，不再重试。
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

/// 从 `UpstreamResponse` 提取 HTTP 状态码（不消费响应）
fn response_status_code(response: &UpstreamResponse) -> Option<u16> {
    extract_status(response)
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

        // 解析供应商级重试配置；retry_json 为空时使用默认值（3 次，2s 间隔）
        let retry_cfg = RetryConfig::from_json(ctx.upstream.provider.retry_json.as_deref());

        // max_retries = 0 表示不重试，直接执行一次
        if retry_cfg.max_retries == 0 {
            let request = UpstreamRequest {
                protocol: ctx.gateway_protocol.to_upstream(),
                body,
                is_stream: ctx.upstream.is_stream,
            };
            return client.execute(&ctx.upstream, request).await;
        }

        let provider_slug = &ctx.upstream.provider.slug;
        let mut last_err: Option<ClientError> = None;
        let mut last_retryable_response: Option<UpstreamResponse> = None;

        for attempt in 0..=retry_cfg.max_retries {
            // 非首次尝试时等待退避延迟
            if attempt > 0 {
                let delay = retry_cfg.retry_delay_ms(attempt);
                tracing::info!(
                    "转发重试 | provider={} | attempt={}/{} | delay={}ms",
                    provider_slug, attempt, retry_cfg.max_retries, delay
                );
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }

            let request = UpstreamRequest {
                protocol: ctx.gateway_protocol.to_upstream(),
                body: body.clone(),
                is_stream: ctx.upstream.is_stream,
            };

            match client.execute(&ctx.upstream, request).await {
                Ok(response) => {
                    // 检查是否为可重试的 HTTP 状态码
                    let status_code = response_status_code(&response);
                    let is_retryable = status_code
                        .map(|code| retry_cfg.status_codes.contains(&code))
                        .unwrap_or(false);

                    if is_retryable && attempt < retry_cfg.max_retries {
                        tracing::warn!(
                            "上游返回可重试状态码 | provider={} | status={} | attempt={}/{}",
                            provider_slug,
                            status_code.unwrap_or(0),
                            attempt + 1,
                            retry_cfg.max_retries
                        );
                        // 保存可重试响应（作为重试耗尽后的返回值），drop 后释放连接
                        last_retryable_response = Some(response);
                        continue;
                    }
                    // 不可重试或重试已耗尽 → 返回响应（成功或最后一次可重试响应）
                    return Ok(response);
                }
                Err(err) => {
                    // 网络错误（DNS/TCP/TLS/超时）可重试
                    let retryable = is_network_error(&err);
                    if retryable && attempt < retry_cfg.max_retries {
                        tracing::warn!(
                            "网络错误，准备重试 | provider={} | attempt={}/{} | error={}",
                            provider_slug, attempt + 1, retry_cfg.max_retries, err
                        );
                        last_err = Some(err);
                        continue;
                    }
                    return Err(err);
                }
            }
        }

        // 重试耗尽：优先返回最后一次可重试响应（让上层看到真实 HTTP 错误），否则返回网络错误
        if let Some(response) = last_retryable_response {
            tracing::warn!(
                "转发重试耗尽，返回最后一次可重试响应 | provider={}",
                provider_slug
            );
            Ok(response)
        } else {
            Err(last_err.unwrap_or_else(|| {
                ClientError::Other("转发重试耗尽但无错误记录".to_string())
            }))
        }
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
        // 复用 TraceIdSpan 注入的 trace_id 作为 request_id，
        // 使转发日志、调用记录的 request_id 与 tracing 日志的 [tid=...] 前缀一致，
        // 便于全链路关联（终端/文件日志 ↔ 日志页面 ↔ 调用记录页面）。
        // TraceLayer 已在最外层为每个请求创建 span 并设置 thread-local，
        // 此处读取即可；fallback 仅防御性兜底（理论上不会触发）。
        let request_id = crate::core::trace_id_layer::current_trace_id()
            .unwrap_or_else(crate::core::trace_id::next_trace_id);
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
        let tags = protocol_tags(
            &ctx.upstream.provider.provider_type,
            ctx.upstream.provider.transport.as_deref(),
            is_stream,
        );

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
                // WebSocketStream 与 Streaming 同样按流式处理（SSE 字节流透传 + usage 拦截）
                let is_streaming = matches!(
                    response,
                    UpstreamResponse::Streaming { .. } | UpstreamResponse::WebSocketStream { .. }
                );

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
        UpstreamResponse::WebSocketStream { .. } => Some(200),
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
        UpstreamResponse::WebSocketStream { .. } => UpstreamResponseSnapshot::Streaming {
            status: 200,
        },
        UpstreamResponse::Complete { status, body, .. } => {
            UpstreamResponseSnapshot::Complete {
                status: status.as_u16(),
                body: body.clone(),
            }
        }
    }
}

enum UpstreamResponseSnapshot {
    Streaming {
        #[expect(dead_code)]
        status: u16,
    },
    Complete {
        #[expect(dead_code)]
        status: u16,
        body: Vec<u8>,
    },
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
