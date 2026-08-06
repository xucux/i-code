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
use crate::modules::logger::Log;
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
    /// `ctx` 需要 `&mut`：Client 会把去敏后的上游请求头快照写回 `ctx.upstream.request_headers_json`。
    async fn execute(
        &self,
        shared: &GatewaySharedState,
        ctx: &mut ForwardContext,
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
        ctx: &mut ForwardContext,
        body: Value,
    ) -> Result<UpstreamResponse, ClientError> {
        let client = ClientFactory::create(&ctx.upstream.provider.provider_type)?;

        // 解析供应商级重试配置；retry_json 为空时使用默认值（3 次，2s 间隔）
        let retry_cfg = RetryConfig::from_json(ctx.upstream.provider.retry_json.as_deref());
        // 转 owned 避免与重试循环内 `&mut ctx.upstream` 的借用冲突
        let provider_slug = ctx.upstream.provider.slug.clone();
        let model_id = ctx.upstream.upstream_model_id.clone();
        let is_stream = ctx.upstream.is_stream;

        // max_retries = 0 表示不重试，直接执行一次
        if retry_cfg.max_retries == 0 {
            tracing::debug!(
                "转发请求（禁用重试）| provider={} | model={} | stream={}",
                provider_slug, model_id, is_stream
            );
            let request = UpstreamRequest {
                protocol: ctx
                    .gateway_protocol
                    .to_upstream_with_bridge(&ctx.upstream.provider.provider_type),
                body,
                is_stream,
            };
            return client.execute(&mut ctx.upstream, request).await;
        }

        tracing::debug!(
            "转发请求（启用重试）| provider={} | model={} | stream={} | max_retries={} | initial_delay_ms={} | retryable_codes={:?}",
            provider_slug, model_id, is_stream,
            retry_cfg.max_retries, retry_cfg.initial_delay_ms, retry_cfg.status_codes
        );

        let mut last_err: Option<ClientError> = None;
        let mut last_retryable_response: Option<UpstreamResponse> = None;

        for attempt in 0..=retry_cfg.max_retries {
            // 非首次尝试时等待退避延迟
            if attempt > 0 {
                let delay = retry_cfg.retry_delay_ms(attempt);
                let reason = last_retryable_response.as_ref().map(|_| "可重试状态码").or(last_err.as_ref().map(|_| "网络错误")).unwrap_or("未知");
                let msg = format!(
                    "转发重试开始 | provider={} | model={} | attempt={}/{} | backoff_delay={}ms | reason={}",
                    provider_slug, model_id, attempt, retry_cfg.max_retries, delay, reason
                );
                tracing::info!("{}", msg);
                Log::info(&msg);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            } else {
                tracing::debug!(
                    "转发首次请求 | provider={} | model={} | stream={}",
                    provider_slug, model_id, is_stream
                );
            }

            let request = UpstreamRequest {
                protocol: ctx
                    .gateway_protocol
                    .to_upstream_with_bridge(&ctx.upstream.provider.provider_type),
                body: body.clone(),
                is_stream,
            };

            match client.execute(&mut ctx.upstream, request).await {
                Ok(response) => {
                    let status_code = response_status_code(&response);
                    let is_retryable = status_code
                        .map(|code| retry_cfg.status_codes.contains(&code))
                        .unwrap_or(false);

                    if is_retryable && attempt < retry_cfg.max_retries {
                        let msg = format!(
                            "上游返回可重试状态码，准备退避重试 | provider={} | model={} | status={} | attempt={}/{} | remaining={}",
                            provider_slug, model_id,
                            status_code.unwrap_or(0),
                            attempt + 1, retry_cfg.max_retries,
                            retry_cfg.max_retries - attempt - 1
                        );
                        tracing::warn!("{}", msg);
                        Log::warn(&msg);
                        // 保存可重试响应（作为重试耗尽后的返回值），drop 后释放连接
                        last_retryable_response = Some(response);
                        continue;
                    }

                    if is_retryable && attempt == retry_cfg.max_retries {
                        let msg = format!(
                            "上游返回可重试状态码，重试已耗尽 | provider={} | model={} | status={} | total_attempts={}",
                            provider_slug, model_id,
                            status_code.unwrap_or(0),
                            attempt + 1
                        );
                        tracing::warn!("{}", msg);
                        Log::warn(&msg);
                        return Ok(response);
                    }

                    // 成功或不可重试状态码
                    if attempt > 0 {
                        let msg = format!(
                            "转发重试成功 | provider={} | model={} | status={} | attempts_used={}",
                            provider_slug, model_id,
                            status_code.unwrap_or(0),
                            attempt + 1
                        );
                        tracing::info!("{}", msg);
                        Log::info(&msg);
                    } else {
                        tracing::debug!(
                            "转发首次请求成功 | provider={} | model={} | status={}",
                            provider_slug, model_id,
                            status_code.unwrap_or(0)
                        );
                    }
                    return Ok(response);
                }
                Err(err) => {
                    let retryable = is_network_error(&err);
                    if retryable && attempt < retry_cfg.max_retries {
                        let msg = format!(
                            "网络错误，准备退避重试 | provider={} | model={} | attempt={}/{} | remaining={} | error={}",
                            provider_slug, model_id,
                            attempt + 1, retry_cfg.max_retries,
                            retry_cfg.max_retries - attempt - 1,
                            err
                        );
                        tracing::warn!("{}", msg);
                        Log::warn(&msg);
                        last_err = Some(err);
                        continue;
                    }

                    if retryable && attempt == retry_cfg.max_retries {
                        let msg = format!(
                            "网络错误，重试已耗尽 | provider={} | model={} | total_attempts={} | error={}",
                            provider_slug, model_id,
                            attempt + 1, err
                        );
                        tracing::error!("{}", msg);
                        Log::error(&msg);
                        return Err(err);
                    }

                    // 不可重试的错误（如认证失败、请求格式错误）
                    tracing::debug!(
                        "转发请求失败（不可重试）| provider={} | model={} | retryable={} | error={}",
                        provider_slug, model_id, retryable, err
                    );
                    return Err(err);
                }
            }
        }

        // 重试耗尽：优先返回最后一次可重试响应（让上层看到真实 HTTP 错误），否则返回网络错误
        if let Some(response) = last_retryable_response {
            let status = response_status_code(&response).unwrap_or(0);
            let msg = format!(
                "转发重试全部耗尽，返回最后一次可重试响应 | provider={} | model={} | final_status={} | max_retries={}",
                provider_slug, model_id, status, retry_cfg.max_retries
            );
            tracing::error!("{}", msg);
            Log::error(&msg);
            Ok(response)
        } else {
            let msg = format!(
                "转发重试全部耗尽，返回最后一次网络错误 | provider={} | model={} | max_retries={}",
                provider_slug, model_id, retry_cfg.max_retries
            );
            tracing::error!("{}", msg);
            Log::error(&msg);
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
            request_headers_json,
        } = req;

        // 解析 model
        let gateway_model_id = body
            .get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| IcodeError::validation("请求体缺少 model 字段"))?
            .to_string();

        let resolved = resolve_route(shared, &gateway_model_id, &request_id, protocol)?;

        match resolved {
            ResolvedRoute::Direct(ctx) => {
                Self::run_direct(shared, ctx, body, api_key_secret_id, request_headers_json).await
            }
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
                        request_headers_json,
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
        request_headers_json: Option<String>,
    ) -> IcodeResult<Response> {
        let forwarder = DirectForwarder::new();
        Self::execute_and_finalize(
            shared,
            &forwarder,
            &mut ctx,
            &mut body,
            api_key_secret_id,
            request_headers_json,
        )
        .await
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
        request_headers_json: Option<String>,
    ) -> IcodeResult<Response> {
        let request_id = ctx.upstream.request_id.clone();
        let gateway_model_id = ctx.gateway_model_id.clone();

        // 前置：替换 model、设置 stream、注入 stream_options
        Self::prepare_body(ctx, body);

        // 协议桥接：检测桥接方向，必要时转换请求体（§4.2 / §5.1 / §5.2）
        let bridge_kind = crate::modules::gateway_runtime::bridge::detect_bridge(
            ctx.gateway_protocol,
            &ctx.upstream.provider.provider_type,
        );
        if bridge_kind.is_bridged() {
            Self::apply_request_bridge(shared, ctx, body, bridge_kind);
        }

        let is_stream = ctx.upstream.is_stream;
        let tags = protocol_tags(
            &ctx.upstream.provider.provider_type,
            ctx.upstream.provider.transport.as_deref(),
            is_stream,
            bridge_kind,
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
            Ok(mut response) => {
                let status = extract_status(&response);
                // WebSocketStream 与 Streaming 同样按流式处理（SSE 字节流透传 + usage 拦截）
                let is_streaming = matches!(
                    response,
                    UpstreamResponse::Streaming { .. } | UpstreamResponse::WebSocketStream { .. }
                );

                // 协议桥接响应转换（仅非流式；§5.3 / §7.1）
                // - 2xx：响应体反向转换为入口协议格式
                // - 4xx：错误体按入口协议格式转换
                // - 5xx：视为上游不可用，构造 OpenAI 标准错误体并改 502 状态码
                if bridge_kind.is_bridged() && !is_streaming {
                    Self::apply_response_bridge(&mut response, bridge_kind);
                }

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

                // 流式桥接：在 build_response 之前包装字节流（§5.4 / §7.6）
                // - Streaming：用状态机逐事件转换为目标协议 SSE，同时解析原始 chunk 的 usage
                // - WebSocketStream / Complete：不桥接（§4.1 矩阵：WS / Responses 不参与桥接）
                let response = if bridge_kind.is_bridged() && is_streaming {
                    Self::apply_stream_bridge(response, bridge_kind, usage_accumulator.as_ref())
                } else {
                    response
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
                // 上游请求头优先（真实发给供应商的头，已去敏），无则兜底用网关入口头
                let upstream_headers = ctx.upstream.request_headers_json.clone();
                if let Some(h) = upstream_headers.as_deref().or(request_headers_json.as_deref()) {
                    builder = builder.request_headers(h);
                }
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
                let mut builder_err = LogRecord::builder()
                    .kind(LogKind::Forward)
                    .method("POST")
                    .url(&upstream_url)
                    .duration_ms(duration_ms)
                    .request_id(&request_id)
                    .model_id(&gateway_model_id)
                    .request_body(request_body_full.as_deref().unwrap_or(""))
                    .error_message(&icode_err.message)
                    .tags(error_tags.clone())
                    .status(LogStatus::Failed);
                // 上游请求头优先（真实发给供应商的头，已去敏），无则兜底用网关入口头
                let upstream_headers = ctx.upstream.request_headers_json.clone();
                if let Some(h) = upstream_headers.as_deref().or(request_headers_json.as_deref()) {
                    builder_err = builder_err.request_headers(h);
                }
                let log_record = builder_err.build();
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

    /// 应用请求体桥接转换（§4.2 / §5.1 / §5.2）
    ///
    /// 在 `prepare_body` 之后、`forwarder.execute` 之前调用，按 `bridge_kind` 转换请求体。
    /// 转换失败时记录 warn 但不中断转发——让上游用未完全转换的 body 处理后由 4xx 路径反馈。
    fn apply_request_bridge(
        shared: &GatewaySharedState,
        ctx: &mut ForwardContext,
        body: &mut Value,
        bridge_kind: crate::modules::gateway_runtime::bridge::BridgeKind,
    ) {
        use crate::modules::gateway_runtime::bridge::{
            anthropic_to_openai_chat, openai_chat_to_anthropic,
        };

        let result = match bridge_kind {
            crate::modules::gateway_runtime::bridge::BridgeKind::OpenaiToAnthropic => {
                // O→A：入口 OpenAI + 上游 Anthropic，需要 max_output_tokens 兜底
                let max_output_tokens = lookup_max_output_tokens(shared, ctx);
                openai_chat_to_anthropic(body, max_output_tokens)
            }
            crate::modules::gateway_runtime::bridge::BridgeKind::AnthropicToOpenai => {
                anthropic_to_openai_chat(body)
            }
            crate::modules::gateway_runtime::bridge::BridgeKind::None => Ok(()),
        };

        if let Err(e) = result {
            tracing::warn!(
                target: "i_code::bridge",
                kind = bridge_kind.label(),
                error = %e,
                "请求体桥接转换失败，原样转发由上游处理"
            );
        }
    }

    /// 应用响应体桥接转换（§5.3 / §7.1）
    ///
    /// 仅处理 `UpstreamResponse::Complete`；流式响应由 P3 处理。
    ///
    /// - 2xx：响应体反向转换为入口协议格式
    /// - 4xx：错误体按入口协议格式转换（§7.1 方案 B）
    /// - 5xx：视为上游不可用，构造 OpenAI 标准错误体并改 502 状态码（§7.1 / AGENTS.md §10.1）
    fn apply_response_bridge(
        response: &mut UpstreamResponse,
        bridge_kind: crate::modules::gateway_runtime::bridge::BridgeKind,
    ) {
        use crate::modules::gateway_runtime::bridge::{
            anthropic_response_to_openai, convert_error_body, openai_response_to_anthropic,
        };

        let UpstreamResponse::Complete { status, body, .. } = response else {
            // 流式 / WebSocket：P2 不接入桥接，原样透传
            return;
        };

        let code = status.as_u16();

        if code >= 500 {
            // 5xx：构造 OpenAI 标准错误体（与 upstream_error_response 格式一致），状态码统一为 502
            let body_str = String::from_utf8_lossy(body).to_string();
            let body_val: Value = serde_json::from_str(&body_str).unwrap_or(Value::Null);
            let message = body_val
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "上游服务不可用".to_string());
            let err_body = serde_json::json!({
                "error": {
                    "message": message,
                    "type": "api_error",
                    "param": Value::Null,
                    "code": "bad_gateway",
                }
            });
            *body = serde_json::to_vec(&err_body).unwrap_or_default();
            *status = reqwest::StatusCode::BAD_GATEWAY;
            tracing::debug!(
                target: "i_code::bridge",
                kind = bridge_kind.label(),
                upstream_status = code,
                "5xx 桥接：构造 OpenAI 标准错误体并改 502"
            );
            return;
        }

        if code >= 400 {
            // 4xx：错误体按入口协议格式转换（§7.1）
            if let Ok(mut val) = serde_json::from_slice::<Value>(body) {
                let _ = convert_error_body(&mut val, bridge_kind);
                if let Ok(new_bytes) = serde_json::to_vec(&val) {
                    *body = new_bytes;
                }
            }
            return;
        }

        // 2xx：响应体反向转换（§5.3）
        if let Ok(mut val) = serde_json::from_slice::<Value>(body) {
            let convert_result = match bridge_kind {
                crate::modules::gateway_runtime::bridge::BridgeKind::OpenaiToAnthropic => {
                    anthropic_response_to_openai(&mut val)
                }
                crate::modules::gateway_runtime::bridge::BridgeKind::AnthropicToOpenai => {
                    openai_response_to_anthropic(&mut val)
                }
                crate::modules::gateway_runtime::bridge::BridgeKind::None => Ok(()),
            };
            if let Err(e) = convert_result {
                tracing::warn!(
                    target: "i_code::bridge",
                    kind = bridge_kind.label(),
                    error = %e,
                    "响应体转换失败，原样透传上游 body"
                );
                return;
            }
            if let Ok(new_bytes) = serde_json::to_vec(&val) {
                *body = new_bytes;
            }
        }
    }

    /// 应用流式桥接转换（§5.4 / §7.6）
    ///
    /// 在 `build_response` 之前包装 SSE 字节流，逐事件转换为**入口协议**格式：
    ///
    /// - `OpenaiToAnthropic`（入口 O → 上游 A）：响应需 上游 Anthropic SSE → 入口 OpenAI SSE
    /// - `AnthropicToOpenai`（入口 A → 上游 O）：响应需 上游 OpenAI SSE → 入口 Anthropic SSE
    ///
    /// 即响应转换方向与请求转换方向**相反**——与非流式响应转换（`apply_response_bridge`）
    /// 和错误体转换（`convert_error_body`）保持一致。
    ///
    /// 同时在闭包内按**上游协议**解析原始 chunk 的 usage（§5.4.3），更新 `usage_accumulator`。
    /// `build_response` 内部的 `parse_sse_event_for_usage` 会按上游协议解析转换后的字节流,
    /// 因协议不匹配不会重复更新 accumulator。
    ///
    /// WebSocket 流与 Complete 响应不进入本函数（`is_streaming` 已过滤 WebSocketStream；
    /// Complete 由 `apply_response_bridge` 处理）。
    ///
    /// 实现要点：
    /// - 用 [`BridgeStreamState`] 维护跨 chunk 行缓冲与转换状态
    /// - 用独立 `usage_buf` 维护 usage 解析的行缓冲（与状态机内部 `line_buf` 分离）
    /// - 转换后的字节流通过 `UpstreamResponse::WebSocketStream` 变体传递给 `build_response`
    fn apply_stream_bridge(
        response: UpstreamResponse,
        bridge_kind: crate::modules::gateway_runtime::bridge::BridgeKind,
        usage_accumulator: Option<&crate::modules::gateway_runtime::client::SseUsageAccumulator>,
    ) -> UpstreamResponse {
        use crate::modules::gateway_runtime::bridge::stream::{
            anthropic_sse_to_openai, openai_sse_to_anthropic, BridgeStreamState,
        };
        use crate::modules::gateway_runtime::forwarding::usage_extractor::parse_sse_event_for_usage;
        use axum::body::Bytes;
        use futures::StreamExt;

        let UpstreamResponse::Streaming {
            response: reqwest_resp,
            protocol,
        } = response
        else {
            // WebSocketStream / Complete 不桥接
            return response;
        };

        let state = std::sync::Mutex::new(BridgeStreamState::new());
        let usage_buf = std::sync::Mutex::new(String::new());
        let acc = usage_accumulator.cloned();
        let bridge_kind_label = bridge_kind.label();

        let mapped =
            reqwest_resp
                .bytes_stream()
                .map(move |result| -> Result<Bytes, std::convert::Infallible> {
                    let bytes = match result {
                        Ok(b) => b,
                        Err(e) => {
                            tracing::warn!(
                                target: "i_code::bridge",
                                error = %e,
                                "上游 SSE 流读取失败"
                            );
                            return Ok(Bytes::new());
                        }
                    };

                    let text = String::from_utf8_lossy(&bytes).to_string();

                    // 解析 usage（按上游协议，用 line_buf 处理跨 chunk 事件边界）
                    if let Some(acc) = &acc {
                        let mut buf = usage_buf
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        buf.push_str(&text);
                        while let Some(pos) = buf.find("\n\n") {
                            let event_text = buf[..pos].to_string();
                            *buf = buf[pos + 2..].to_string();
                            let mut usage = acc
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            parse_sse_event_for_usage(&event_text, protocol, &mut usage);
                        }
                    }

                    // 桥接转换（state 内部维护 line_buf）
                    // 注意：响应转换方向与请求转换方向相反
                    // - OpenaiToAnthropic（入口 O → 上游 A）：上游返回 Anthropic SSE，
                    //   需转换为入口 OpenAI SSE（`anthropic_sse_to_openai`）
                    // - AnthropicToOpenai（入口 A → 上游 O）：上游返回 OpenAI SSE，
                    //   需转换为入口 Anthropic SSE（`openai_sse_to_anthropic`）
                    let mut st = state
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    let events = match bridge_kind {
                        crate::modules::gateway_runtime::bridge::BridgeKind::OpenaiToAnthropic => {
                            anthropic_sse_to_openai(&text, &mut st)
                        }
                        crate::modules::gateway_runtime::bridge::BridgeKind::AnthropicToOpenai => {
                            openai_sse_to_anthropic(&text, &mut st)
                        }
                        crate::modules::gateway_runtime::bridge::BridgeKind::None => Vec::new(),
                    };
                    drop(st);

                    let mut output = String::new();
                    for event in events {
                        output.push_str(&event);
                    }
                    tracing::debug!(
                        target: "i_code::bridge",
                        kind = bridge_kind_label,
                        category = "stream_chunk",
                        input_size = bytes.len(),
                        output_size = output.len(),
                        "流式桥接 chunk 处理完成"
                    );
                    Ok(Bytes::from(output))
                });

        UpstreamResponse::WebSocketStream {
            stream: Box::pin(mapped),
            protocol,
        }
    }
}

/// 查询 `max_output_tokens`（§7.2）
///
/// 通过 `ctx.upstream.gateway_model_id` → `GatewayModel.model_config_id` → `ModelConfig.max_output_tokens`
/// 链路查询；任一步失败或为 None 时返回 None（由调用方使用 [`MAX_TOKENS_FALLBACK`] 兜底）。
///
/// [`MAX_TOKENS_FALLBACK`]: crate::modules::gateway_runtime::bridge::MAX_TOKENS_FALLBACK
fn lookup_max_output_tokens(
    shared: &GatewaySharedState,
    ctx: &ForwardContext,
) -> Option<i64> {
    let gateway_model_id = ctx.upstream.gateway_model_id.as_deref()?;
    let gateway_model = shared
        .ai_gateway_handle
        .service()
        .get_gateway_model(gateway_model_id)
        .map_err(|e| {
            tracing::warn!(
                target: "i_code::bridge",
                error = %e,
                gateway_model_id = %gateway_model_id,
                "查询 GatewayModel 失败，max_tokens 将使用兜底值"
            );
        })
        .ok()?;
    let model_config = shared
        .ai_gateway_handle
        .service()
        .get_model_config(&gateway_model.model_config_id)
        .map_err(|e| {
            tracing::warn!(
                target: "i_code::bridge",
                error = %e,
                model_config_id = %gateway_model.model_config_id,
                "查询 ModelConfig 失败，max_tokens 将使用兜底值"
            );
        })
        .ok()?;
    model_config.max_output_tokens
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
