//! # OpenAI Responses API 客户端
//!
//! 处理 `openai-responses` 供应商的 `POST /v1/responses` 端点，支持两种传输：
//!
//! - **HTTP / SSE**（默认，`transport` 为 `auto` / `sse` / 未配置）：与
//!   `OpenAiChatClient` 一致，按 `stream` 字段决定流式 / 非流式。
//! - **WebSocket**（`transport = websocket`）：连接 `wss://{base}/responses`，
//!   发送 `response.create` 事件，将 WS 文本帧转换为 SSE 格式
//!   （`data: {json}\n\n`）字节流返回，上层按 SSE 透传并复用 usage 拦截。
//!
//! ## 设计要点
//!
//! - 认证复用 `openai_chat_client::resolve_auth` / `build_headers`
//!   （Bearer token + extra headers）。
//! - WebSocket 传输暂不应用代理配置（`tokio-tungstenite` 无内置代理支持），
//!   直接连接；`transport = websocket` 且上游不支持时返回明确错误，不静默回退。
//! - `transport = auto` 时默认走 HTTP/SSE（网关场景 HTTP 更稳，无需 WS 探测）。

use std::convert::Infallible;

use axum::body::Bytes;
use futures::stream::{SplitSink, SplitStream, Stream, StreamExt};
use futures::SinkExt;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::modules::ai_gateway::types::Provider;

use super::openai_chat_client::OpenAiChatClient;
use super::{
    build_upstream_url, format_client_error, http_client_for, read_response_body,
    ClientError, UpstreamClient, UpstreamContext, UpstreamProtocol, UpstreamRequest,
    UpstreamResponse, MAX_RESPONSE_BODY_BYTES,
};

/// OpenAI Responses 客户端
///
/// 按 `provider.transport` 选择传输方式：
/// - `websocket` → WebSocket 传输（`UpstreamResponse::WebSocketStream`）
/// - `auto` / `sse` / 未配置 → HTTP/SSE 传输
#[derive(Debug, Clone, Default)]
pub struct OpenAiResponsesClient;

impl OpenAiResponsesClient {
    /// 创建新的 OpenAI Responses Client
    pub fn new() -> Self {
        Self
    }

    /// 上游请求路径（`base_url` 通常已含 `/v1`，此处不再重复版本前缀）
    fn build_path() -> &'static str {
        "/responses"
    }

    /// 是否使用 WebSocket 传输
    fn use_websocket(provider: &Provider) -> bool {
        matches!(provider.transport.as_deref(), Some("websocket"))
    }

    /// HTTP/SSE 传输执行
    async fn execute_http(
        ctx: &UpstreamContext,
        request: UpstreamRequest,
    ) -> Result<UpstreamResponse, ClientError> {
        let upstream_url = build_upstream_url(&ctx.provider, Self::build_path())?;
        let auth_resolution = OpenAiChatClient::resolve_auth(ctx.auth_config.clone())?;

        let headers =
            OpenAiChatClient::build_headers(&ctx.provider, &auth_resolution, &ctx.extra_headers)?;

        let client = http_client_for(&ctx.provider, request.is_stream)?;
        let upstream_resp = client
            .post(&upstream_url)
            .headers(headers)
            .json(&request.body)
            .send()
            .await
            .map_err(|e| ClientError::RequestFailed(format_client_error(&e)))?;

        if request.is_stream {
            // 流式响应：直接返回 reqwest Response，由上层透传 SSE
            Ok(UpstreamResponse::Streaming {
                response: upstream_resp,
                protocol: UpstreamProtocol::Responses,
            })
        } else {
            // 非流式响应：读取完整 body，应用上限保护防止异常上游 OOM
            let (status, headers, body, _) =
                read_response_body(upstream_resp, MAX_RESPONSE_BODY_BYTES).await?;
            Ok(UpstreamResponse::Complete {
                status,
                headers,
                body,
            })
        }
    }

    /// WebSocket 传输执行
    ///
    /// 建立 `wss://{base}/responses` 连接，发送 `response.create` 事件，
    /// 将上游 WS 文本帧转换为 SSE 格式字节流（`data: {json}\n\n`）返回。
    ///
    /// 收到 `response.completed` / `response.failed` / `response.incomplete` /
    /// `error` 任一终止事件后，向对端发送 Close 帧并结束流。
    async fn execute_websocket(
        ctx: &UpstreamContext,
        request: UpstreamRequest,
    ) -> Result<UpstreamResponse, ClientError> {
        // 1. 构造 wss:// URL（复用 /v1 自动补全逻辑，再把 scheme 换成 ws/wss）
        let http_url = build_upstream_url(&ctx.provider, Self::build_path())?;
        let ws_url = to_ws_url(&http_url)?;

        // 2. 认证头（Bearer token + extra headers）+ OpenAI-Beta
        let auth_resolution = OpenAiChatClient::resolve_auth(ctx.auth_config.clone())?;
        if !auth_resolution.query_params.is_empty() {
            return Err(ClientError::AuthError(
                "WebSocket 传输不支持 query 参数认证（如 ?key=），请改用 API Key / OAuth Bearer".to_string(),
            ));
        }
        let mut headers =
            OpenAiChatClient::build_headers(&ctx.provider, &auth_resolution, &ctx.extra_headers)?;
        headers.insert(
            "OpenAI-Beta",
            "responses=v1"
                .parse()
                .map_err(|e| ClientError::BuildRequestError(format!("构造 OpenAI-Beta 失败: {}", e)))?,
        );

        // 3. 建立连接（连接超时由 socket 层处理，此处直接异步连接）
        let mut ws_request = ws_url
            .into_client_request()
            .map_err(|e| ClientError::BuildRequestError(format!("构造 WebSocket 请求失败: {}", e)))?;
        for (k, v) in headers.iter() {
            ws_request.headers_mut().insert(k.clone(), v.clone());
        }
        let (ws_stream, _resp) = tokio_tungstenite::connect_async(ws_request)
            .await
            .map_err(|e| ClientError::RequestFailed(format!("WebSocket 连接失败: {}", e)))?;
        let (mut write, read) = ws_stream.split();

        // 4. 发送 response.create 事件（WS 天然流式，剥离 stream 字段）
        let mut body = request.body.clone();
        if let Some(obj) = body.as_object_mut() {
            obj.remove("stream");
        }
        let mut payload = json!({ "type": "response.create" });
        if let Value::Object(map) = &mut payload {
            if let Some(obj) = body.as_object() {
                for (k, v) in obj {
                    map.insert(k.clone(), v.clone());
                }
            }
        }
        write
            .send(Message::Text(payload.to_string().into()))
            .await
            .map_err(|e| ClientError::RequestFailed(format!("WebSocket 发送失败: {}", e)))?;

        // 5. 读取事件流 → SSE 字节流
        let stream = ws_frame_to_sse_stream(read, write);
        Ok(UpstreamResponse::WebSocketStream {
            stream: Box::pin(stream),
            protocol: UpstreamProtocol::Responses,
        })
    }
}

#[async_trait::async_trait]
impl UpstreamClient for OpenAiResponsesClient {
    async fn execute(
        &self,
        ctx: &UpstreamContext,
        request: UpstreamRequest,
    ) -> Result<UpstreamResponse, ClientError> {
        if request.protocol != UpstreamProtocol::Responses {
            return Err(ClientError::UnsupportedProtocol {
                provider_type: ctx.provider.provider_type.clone(),
                protocol: request.protocol,
            });
        }

        if Self::use_websocket(&ctx.provider) {
            Self::execute_websocket(ctx, request).await
        } else {
            Self::execute_http(ctx, request).await
        }
    }

    fn provider_type(&self) -> &'static str {
        "openai-responses"
    }
}

// ===== WebSocket 帧 → SSE 字节流转换 =====

/// WebSocket 传输终止事件类型
fn is_terminal_event(event: &Value) -> bool {
    matches!(
        event.get("type").and_then(|v| v.as_str()),
        Some("response.completed" | "response.failed" | "response.incomplete" | "error")
    )
}

/// 将 WS 读取流转换为 SSE 格式字节流
///
/// - 文本帧：格式化为 `data: {json}\n\n` 输出
/// - 收到终止事件：输出该帧后向对端发送 Close(1000) 并结束
/// - 连接错误：输出 SSE `error` 事件（`{"type":"error",...}`）后结束
///
/// 使用 unfold 状态机持有读写半部，实现终止事件后的 Close 发送。
struct WsToSseState<S> {
    /// WS 读取半部
    read: SplitStream<WebSocketStream<S>>,
    /// WS 写入半部（终止事件后发送 Close）
    write: Option<SplitSink<WebSocketStream<S>, Message>>,
    /// 是否已发送终止帧（下一轮发送 Close 并结束）
    closing: bool,
    /// 是否已结束
    done: bool,
}

/// 构造 WS 帧 → SSE 字节流
///
/// 每次 poll 处理一个 WS 帧；终止事件后发送 Close 帧并结束流。
fn ws_frame_to_sse_stream<S>(
    read: SplitStream<WebSocketStream<S>>,
    write: SplitSink<WebSocketStream<S>, Message>,
) -> impl Stream<Item = Result<Bytes, Infallible>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    WebSocketStream<S>: Unpin,
{
    let state = WsToSseState {
        read,
        write: Some(write),
        closing: false,
        done: false,
    };

    futures::stream::unfold(state, |mut st| async move {
        // 关闭阶段：发送 Close 帧后结束
        if st.closing {
            if let Some(mut write) = st.write.take() {
                let _ = write.send(Message::Close(None)).await;
            }
            st.done = true;
            return None;
        }
        if st.done {
            return None;
        }

        loop {
            match st.read.next().await {
                Some(Ok(Message::Text(text))) => {
                    let sse_line = format!("data: {}\n\n", text);
                    let bytes = Bytes::from(sse_line);
                    // 判断终止事件：输出本帧后进入关闭阶段
                    if let Ok(v) = serde_json::from_str::<Value>(&text) {
                        if is_terminal_event(&v) {
                            st.closing = true;
                            return Some((Ok(bytes), st));
                        }
                    }
                    return Some((Ok(bytes), st));
                }
                Some(Ok(Message::Close(_))) => {
                    st.done = true;
                    return None;
                }
                // 忽略二进制帧 / Ping / Pong
                Some(Ok(_)) => continue,
                Some(Err(e)) => {
                    // 连接异常：输出 SSE error 事件后结束
                    let err_event = json!({
                        "type": "error",
                        "error": {
                            "message": format!("WebSocket 流读取失败: {}", e),
                            "type": "server_error",
                            "param": Value::Null,
                            "code": "upstream_ws_error",
                        }
                    });
                    st.done = true;
                    let bytes = Bytes::from(format!("data: {}\n\n", err_event));
                    return Some((Ok(bytes), st));
                }
                None => {
                    st.done = true;
                    return None;
                }
            }
        }
    })
}

/// 将 HTTP(S) URL 转换为 WebSocket URL
///
/// `https://` → `wss://`，`http://` → `ws://`；其余 scheme 报错。
fn to_ws_url(http_url: &str) -> Result<String, ClientError> {
    let ws_url = if let Some(rest) = http_url.strip_prefix("https://") {
        format!("wss://{}", rest)
    } else if let Some(rest) = http_url.strip_prefix("http://") {
        format!("ws://{}", rest)
    } else {
        return Err(ClientError::BuildRequestError(format!(
            "WebSocket 传输要求 base_url 为 http(s)://，当前: {}",
            http_url
        )));
    };
    Ok(ws_url)
}
