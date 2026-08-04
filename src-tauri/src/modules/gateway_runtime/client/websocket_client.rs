//! # WebSocket 协议客户端（部分占位）
//!
//! 用于 OpenAI Codex 等需要 WebSocket 长连接的供应商。
//!
//! ## 当前状态
//!
//! `openai-responses` 供应商已由 [`super::openai_responses_client::OpenAiResponsesClient`]
//! 支持 WebSocket 传输（`transport = websocket`，WS 帧转 SSE 字节流）。
//! 本占位仅服务 `openai-codex` / `websocket` 类型：`execute()` 调用后立即返回
//! `UnsupportedProtocol` 错误，**不发送任何网络请求**。
//!
//! ## 参考设计
//!
//! 参考项目中的 `websocket-session-manager.ts` 与 `openai/responses-websocket-transport.ts`：
//! - `WebSocketSessionTransport`：定义连接、发送、事件监听、关闭契约。
//! - `WebSocketSessionManager`：管理会话复用、请求排队、abort 信号。
//! - `OpenAIResponsesWebSocketTransport`：实现 OpenAI Responses WebSocket 的具体握手与消息格式。
//!
//! ## i-code 落地所需改造（按顺序）
//!
//! 1. **依赖**：`Cargo.toml` 引入 `tokio-tungstenite`（已引入，供 Responses WS 使用）。
//! 2. **`UpstreamResponse` 扩展**：已新增 `WebSocketStream` 变体（持有 SSE 格式字节流），
//!    由 `forwarding/response_handler` 按 SSE 透传并复用 usage 拦截。
//! 3. **`WebSocketTransport` trait**：定义 `connect` / `send` / `next_event` / `close`。
//! 4. **`OpenAiCodexTransport`**：实现 Codex 的会话握手与事件流转 SSE 的协议转换。
//! 5. **会话管理**：`WebSocketSessionManager` 管理连接复用、请求排队、abort。
//! 6. **`ClientFactory::create`**：把 `openai-codex` 路由到真实实现而非本占位。

use super::{ClientError, UpstreamClient, UpstreamContext, UpstreamRequest, UpstreamResponse};

/// WebSocket 协议客户端占位
///
/// 见模块级文档了解落地步骤。当前所有方法均返回 `UnsupportedProtocol`。
#[derive(Debug, Clone)]
pub struct WebSocketClient {
    /// 供应商类型标识，用于错误提示
    provider_type: String,
}

impl WebSocketClient {
    /// 创建新的 WebSocket Client 占位实例
    pub fn new(provider_type: impl Into<String>) -> Self {
        Self {
            provider_type: provider_type.into(),
        }
    }
}

#[async_trait::async_trait]
impl UpstreamClient for WebSocketClient {
    async fn execute(
        &self,
        _ctx: &mut UpstreamContext,
        request: UpstreamRequest,
    ) -> Result<UpstreamResponse, ClientError> {
        // WebSocket 协议尚未实现，直接返回明确的协议不支持错误。
        // 不发送任何网络请求，避免误导调用方。
        tracing::debug!(
            "WebSocket 协议未实现 | provider_type={} | protocol={:?} | is_stream={} | body={}",
            self.provider_type,
            request.protocol,
            request.is_stream,
            request.body
        );
        Err(ClientError::UnsupportedProtocol {
            provider_type: self.provider_type.clone(),
            protocol: request.protocol,
        })
    }

    fn provider_type(&self) -> &'static str {
        "websocket"
    }
}
