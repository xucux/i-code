//! # 响应处理器
//!
//! 统一处理上游 `UpstreamResponse` → axum `Response` 的转换，包含：
//!
//! - 流式 SSE 响应：透传字节流，并通过流中间件拦截 usage 数据
//! - 非流式响应：构造标准 HTTP Response
//!
//! 流中间件在 chunk 透传过程中解析 SSE 事件 usage，流结束时通过回调
//! 同步写入调用记录，替代旧版 `tokio::spawn + sleep(5s)` 轮询方案。

use std::convert::Infallible;
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use futures::stream::{BoxStream, StreamExt};
use reqwest::header::HeaderMap;

use crate::core::trace_id_layer::SSE_LOG_TARGET;
use crate::modules::gateway_runtime::client::{
    SseUsageAccumulator, UpstreamProtocol, UpstreamResponse,
};
use crate::modules::gateway_runtime::service::GatewaySharedState;

use super::usage_extractor::parse_sse_event_for_usage;

use serde_json::Value;

/// 检测单个 SSE 事件是否包含真实正文，并记录是否收到 `[DONE]`
///
/// 用于流结束时判断「只出了 thinking（reasoning_content）却没有正文」的异常场景，
/// 从而定位如 "Sorry, no response was returned." 这类上游空消息问题。
///
/// 语义：
/// - `has_content` 置 true 表示该事件含**正文**：
///   - OpenAI Chat Completions：`choices[0].delta.content` 非空
///   - Anthropic Messages：`content_block_delta` 且 `delta.type == "text_delta"` 且 text 非空
///   - Responses：`response.output_text.delta` 事件且 `delta` 非空
/// - `reasoning_content` / `input_json_delta` 等思考与工具调用增量**不计为正文**
/// - `saw_done` 置 true 表示收到 `data: [DONE]`
fn track_sse_event_content(
    event_text: &str,
    protocol: UpstreamProtocol,
    has_content: &mut bool,
    saw_done: &mut bool,
) {
    for line in event_text.lines() {
        let line = line.trim();
        if !line.starts_with("data:") {
            continue;
        }
        let data = line.strip_prefix("data:").unwrap_or("").trim();
        if data == "[DONE]" {
            *saw_done = true;
            continue;
        }

        let val: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        match protocol {
            UpstreamProtocol::ChatCompletions => {
                if let Some(choices) = val.get("choices").and_then(|c| c.as_array()) {
                    for choice in choices {
                        if let Some(delta) = choice.get("delta") {
                            if delta
                                .get("content")
                                .and_then(|v| v.as_str())
                                .is_some_and(|s| !s.is_empty())
                            {
                                *has_content = true;
                            }
                        }
                    }
                }
            }
            UpstreamProtocol::AnthropicMessages => {
                if val.get("type").and_then(|v| v.as_str()) == Some("content_block_delta") {
                    if let Some(delta) = val.get("delta") {
                        if delta.get("type").and_then(|v| v.as_str()) == Some("text_delta")
                            && delta
                                .get("text")
                                .and_then(|v| v.as_str())
                                .is_some_and(|s| !s.is_empty())
                        {
                            *has_content = true;
                        }
                    }
                }
            }
            UpstreamProtocol::Responses => {
                if val.get("type").and_then(|v| v.as_str()) == Some("response.output_text.delta") {
                    if val
                        .get("delta")
                        .and_then(|v| v.as_str())
                        .is_some_and(|s| !s.is_empty())
                    {
                        *has_content = true;
                    }
                }
            }
        }
    }
}

/// 流式响应的完成回调
///
/// 流结束时调用，参数为流中累积的 usage 数据。
pub type StreamDoneCallback = Arc<dyn Fn(&GatewaySharedState, &SseUsageData) + Send + Sync>;

use crate::modules::gateway_runtime::client::SseUsageData;

/// 将 `UpstreamResponse` 转换为 axum Response
///
/// - `Streaming`：构造 SSE Response，透传字节流，同时拦截 usage 数据；
///   流结束通过 `on_done` 回调同步写入调用记录（无延迟轮询）。
/// - `WebSocketStream`：与 `Streaming` 同构——上游 WS 帧已由 Client 转为
///   SSE 字节流，按 SSE 透传并复用 usage 拦截。
/// - `Complete`：构造标准 HTTP Response，返回完整 body。
///
/// 参数：
/// - `shared`：共享状态，供日志/调用记录使用
/// - `log_id`：调用记录 ID，用于流结束后更新 usage
/// - `duration_ms`：请求起始耗时（用于流结束后写调用记录）
/// - `on_done`：流完成回调（可选）
#[allow(clippy::too_many_arguments)]
pub fn build_response(
    response: UpstreamResponse,
    shared: &GatewaySharedState,
    usage_accumulator: Option<SseUsageAccumulator>,
    log_id: Option<&str>,
    duration_ms: i64,
    on_done: Option<StreamDoneCallback>,
) -> Response {
    match response {
        UpstreamResponse::Streaming { response, protocol } => {
            let status = StatusCode::from_u16(response.status().as_u16())
                .unwrap_or(StatusCode::OK);
            // 读取错误转为空字节（不中断流），与上游断开行为一致
            let logger = shared.logger_handle.clone();
            let byte_stream = response.bytes_stream().map(
                move |result| -> Result<Bytes, Infallible> {
                    match result {
                        Ok(b) => Ok(b),
                        Err(e) => {
                            tracing::warn!("上游 SSE 流读取失败: {}", e);
                            logger.service().log_system(
                                crate::modules::logger::types::LogLevel::Warn,
                                &format!("上游 SSE 流读取失败: {}", e),
                                Some(file!()),
                            );
                            Ok(Bytes::new())
                        }
                    }
                },
            );
            build_sse_from_stream(
                Box::pin(byte_stream),
                status,
                protocol,
                usage_accumulator,
                shared,
                log_id,
                duration_ms,
                on_done,
            )
        }
        UpstreamResponse::WebSocketStream { stream, protocol } => build_sse_from_stream(
            stream,
            StatusCode::OK,
            protocol,
            usage_accumulator,
            shared,
            log_id,
            duration_ms,
            on_done,
        ),
        UpstreamResponse::Complete { status, headers, body } => {
            build_json_response(status, headers, body)
        }
    }
}

/// 从统一 SSE 字节流构造 axum SSE Response
///
/// 直接透传 SSE 字节流（`data: ...\n\n`），同时拦截每个事件解析 usage。
/// 流结束后调用 `on_done` 回调同步写入调用记录。
///
/// 由 `build_response` 的 `Streaming`（reqwest 流）与 `WebSocketStream`
/// （Client 转换的 WS 帧流）两个入口共用。
#[allow(clippy::too_many_arguments)]
fn build_sse_from_stream(
    byte_stream: BoxStream<'static, Result<Bytes, Infallible>>,
    status: StatusCode,
    protocol: UpstreamProtocol,
    usage_accumulator: Option<SseUsageAccumulator>,
    shared: &GatewaySharedState,
    log_id: Option<&str>,
    duration_ms: i64,
    on_done: Option<StreamDoneCallback>,
) -> Response {
    let line_buf = std::sync::Mutex::new(String::new());
    let accumulator = usage_accumulator;
    // 在移入 map 闭包前克隆一份，供流结束回调使用
    let acc_for_done = accumulator.clone();
    let shared_for_done = shared.clone();
    let log_id_for_map = log_id.map(|s| s.to_string());
    let log_id_for_done = log_id_for_map.clone();
    let callback_for_done = on_done.clone();

    // 流内容跟踪：判断「是否产出过正文 content」与「是否收到 [DONE]」，
    // 供流结束时定位「只出了 thinking 却没正文」的上游空消息问题。
    let has_content = Arc::new(std::sync::Mutex::new(false));
    let saw_done = Arc::new(std::sync::Mutex::new(false));
    let has_content_for_done = has_content.clone();
    let saw_done_for_done = saw_done.clone();

    let mapped = byte_stream.map(move |result| -> Result<Bytes, Infallible> {
        let bytes = match result {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("上游 SSE 流读取失败: {}", e);
                return Ok(Bytes::new());
            }
        };

        let text = String::from_utf8_lossy(&bytes);
        // 独立 target：SSE chunk 只进专属文件（i-code-sse.*.log，按小时滚动），
        // 不污染常规 i-code-*.log，并在专属文件中省略 target/file:line 前缀。
        tracing::debug!(
            target: SSE_LOG_TARGET,
            "SSE chunk | log_id={} | size={} bytes | text={}",
            log_id_for_map.as_deref().unwrap_or("-"),
            bytes.len(),
            text
        );

        // 追加到行缓冲，按 \n\n 分割完整事件解析 usage
        {
            let mut buf = line_buf.lock().unwrap_or_else(|e| e.into_inner());
            buf.push_str(&text);
            while let Some(pos) = buf.find("\n\n") {
                let event_text = buf[..pos].to_string();
                *buf = buf[pos + 2..].to_string();
                // 跟踪正文与 [DONE]，供流结束时判断是否「只 thinking 无正文」
                track_sse_event_content(
                    &event_text,
                    protocol,
                    &mut has_content.lock().unwrap_or_else(|e| e.into_inner()),
                    &mut saw_done.lock().unwrap_or_else(|e| e.into_inner()),
                );
                if let Some(acc) = &accumulator {
                    let mut usage = acc.lock().unwrap_or_else(|e| e.into_inner());
                    parse_sse_event_for_usage(&event_text, protocol, &mut usage);
                }
            }
        }

        // 原样透传字节
        Ok(bytes)
    });

    // 在流结束时（chain 末尾）触发回调写入调用记录
    let mapped = mapped.chain(futures::stream::once(async move {
        // 流结束检测：全程未收到任何正文 content 时打告警——多半是上游只出了
        // thinking（reasoning_content）就被截断/终止，下游会得到空消息
        // （如 "Sorry, no response was returned."）。saw_done 用于区分是否收到 [DONE]。
        let had_content =
            has_content_for_done.lock().unwrap_or_else(|e| e.into_inner()).to_owned();
        let received_done =
            saw_done_for_done.lock().unwrap_or_else(|e| e.into_inner()).to_owned();
        if !had_content {
            let msg = format!(
                "SSE 流结束但未收到任何正文 content | log_id={} | duration_ms={}ms | saw_done={} —— 疑似上游仅输出 thinking 后被截断/终止",
                log_id_for_done.as_deref().unwrap_or("-"),
                duration_ms,
                received_done,
            );
            tracing::warn!(target: SSE_LOG_TARGET, "{}", msg);
            shared_for_done.logger_handle.service().log_system(
                crate::modules::logger::types::LogLevel::Warn,
                &msg,
                Some(file!()),
            );
        }

        if let (Some(acc), Some(cb)) = (acc_for_done.as_ref(), callback_for_done.as_ref()) {
            let usage = acc.lock().unwrap_or_else(|e| e.into_inner()).clone();
            cb(&shared_for_done, &usage);
        } else if let Some(acc) = acc_for_done.as_ref() {
            let usage = acc.lock().unwrap_or_else(|e| e.into_inner()).clone();
            if let Some(lid) = &log_id_for_done {
                super::call_log::finish_streaming_usage(&shared_for_done, lid, duration_ms, &usage);
            }
        }
        Ok::<Bytes, Infallible>(Bytes::new())
    }));

    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(mapped))
        .unwrap_or_else(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("构造 SSE 响应失败: {}", e),
            )
                .into_response()
        })
}

/// 将完整响应体转换为 axum Response
fn build_json_response(status: reqwest::StatusCode, headers: HeaderMap, body: Vec<u8>) -> Response {
    let axum_status = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK);
    let content_type = headers.get(header::CONTENT_TYPE).cloned();

    let mut builder = Response::builder().status(axum_status);
    if let Some(ct) = content_type {
        builder = builder.header(header::CONTENT_TYPE, ct);
    }
    builder.body(Body::from(body)).unwrap_or_else(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("构造响应失败: {}", e),
        )
            .into_response()
    })
}
