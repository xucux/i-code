//! # 响应处理器
//!
//! 统一处理上游 `UpstreamResponse` → axum `Response` 的转换，包含：
//!
//! - 流式 SSE 响应：透传字节流，并通过流中间件拦截 usage 数据
//! - 非流式响应：构造标准 HTTP Response
//!
//! 流中间件在 chunk 透传过程中解析 SSE 事件 usage，流结束时通过回调
//! 同步写入调用记录，替代旧版 `tokio::spawn + sleep(5s)` 轮询方案。

use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use futures::stream::StreamExt;
use reqwest::header::HeaderMap;

use crate::modules::gateway_runtime::client::{
    SseUsageAccumulator, UpstreamProtocol, UpstreamResponse,
};
use crate::modules::gateway_runtime::service::GatewaySharedState;

use super::usage_extractor::parse_sse_event_for_usage;

/// 流式响应的完成回调
///
/// 流结束时调用，参数为流中累积的 usage 数据。
pub type StreamDoneCallback = Arc<dyn Fn(&GatewaySharedState, &SseUsageData) + Send + Sync>;

use crate::modules::gateway_runtime::client::SseUsageData;

/// 将 `UpstreamResponse` 转换为 axum Response
///
/// - `Streaming`：构造 SSE Response，透传字节流，同时拦截 usage 数据；
///   流结束通过 `on_done` 回调同步写入调用记录（无延迟轮询）。
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
        UpstreamResponse::Streaming { response, protocol } => build_sse_response(
            response, protocol, usage_accumulator, shared, log_id, duration_ms, on_done,
        ),
        UpstreamResponse::Complete { status, headers, body } => {
            build_json_response(status, headers, body)
        }
    }
}

/// 将 reqwest 流式响应转换为 axum SSE Response
///
/// 直接透传上游 SSE 字节流（`data: ...\n\n`），同时拦截每个事件解析 usage。
/// 流结束后调用 `on_done` 回调同步写入调用记录。
#[allow(clippy::too_many_arguments)]
fn build_sse_response(
    upstream_resp: reqwest::Response,
    protocol: UpstreamProtocol,
    usage_accumulator: Option<SseUsageAccumulator>,
    shared: &GatewaySharedState,
    log_id: Option<&str>,
    duration_ms: i64,
    on_done: Option<StreamDoneCallback>,
) -> Response {
    let status =
        StatusCode::from_u16(upstream_resp.status().as_u16()).unwrap_or(StatusCode::OK);

    let line_buf = std::sync::Mutex::new(String::new());
    let accumulator = usage_accumulator;
    // 在移入 map 闭包前克隆一份，供流结束回调使用
    let acc_for_done = accumulator.clone();
    let logger = shared.logger_handle.clone();
    let shared_for_done = shared.clone();
    let log_id_for_map = log_id.map(|s| s.to_string());
    let log_id_for_done = log_id_for_map.clone();
    let callback_for_done = on_done.clone();

    let byte_stream = upstream_resp.bytes_stream().map(move |result| -> Result<Bytes, std::convert::Infallible> {
        let bytes = match result {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("上游 SSE 流读取失败: {}", e);
                logger.service().log_system(
                    crate::modules::logger::types::LogLevel::Warn,
                    &format!("上游 SSE 流读取失败: {}", e),
                    Some(file!()),
                );
                return Ok(Bytes::new());
            }
        };

        let text = String::from_utf8_lossy(&bytes);
        tracing::debug!(
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
    let mapped = byte_stream.chain(futures::stream::once(async move {
        if let (Some(acc), Some(cb)) = (acc_for_done.as_ref(), callback_for_done.as_ref()) {
            let usage = acc.lock().unwrap_or_else(|e| e.into_inner()).clone();
            cb(&shared_for_done, &usage);
        } else if let Some(acc) = acc_for_done.as_ref() {
            let usage = acc.lock().unwrap_or_else(|e| e.into_inner()).clone();
            if let Some(lid) = &log_id_for_done {
                super::call_log::finish_streaming_usage(&shared_for_done, lid, duration_ms, &usage);
            }
        }
        Ok::<Bytes, std::convert::Infallible>(Bytes::new())
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
