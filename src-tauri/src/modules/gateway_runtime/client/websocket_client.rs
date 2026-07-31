//! # WebSocket 协议客户端（未实现，扩展占位）
//!
//! 用于 OpenAI Responses API、OpenAI Codex 等需要 WebSocket 长连接的供应商。
//!
//! ## 当前状态：未实现
//!
//! `execute()` 调用后立即返回 `UnsupportedProtocol` 错误，**不发送任何网络请求**。
//! `ClientFactory` 仍把 `openai-responses` / `openai-codex` / `websocket` 路由到这里，
//! 以便后续实现时无需调整路由层；当前请勿在生产场景使用这些 provider_type。
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
//! 1. **依赖**：`Cargo.toml` 引入 `tokio-tungstenite` 或 `reqwest-websocket`。
//! 2. **`UpstreamResponse` 扩展**：当前 `Streaming` 变体直接持有 `reqwest::Response`，
//!    WebSocket 帧流无法适配。需新增 `WebSocket { stream: BoxStream<Result<...>> }`
//!    或抽象为统一事件流枚举，由 `forwarding/response_handler` 分支处理。
//! 3. **`WebSocketTransport` trait**：定义 `connect` / `send` / `next_event` / `close`。
//! 4. **`OpenAiResponsesTransport`**：处理 session_key 握手、热会话复用、事件流
//!    转 SSE 的协议转换。
//! 5. **会话管理**：`WebSocketSessionManager` 管理连接复用、请求排队、abort。
//! 6. **`ClientFactory::create`**：把 `openai-responses` / `openai-codex` 路由到
//!    真实实现而非本占位。

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
        _ctx: &UpstreamContext,
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
