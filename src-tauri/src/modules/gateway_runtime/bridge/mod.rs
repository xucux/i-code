//! # 协议桥接模块
//!
//! 当网关入口协议与上游供应商协议不一致时，在转发前对请求体进行双向转换，
//! 在响应阶段（P2/P3）对响应体反向转换。
//!
//! ## 当前阶段（P1，按 §8.2）
//!
//! - `BridgeKind` 枚举与 `detect_bridge` 触发条件判定
//! - 请求体双向转换：[`anthropic_to_openai_chat`] / [`openai_chat_to_anthropic`]
//! - 单元测试覆盖 [`docs/proposals/protocol-bridge.md`] §3 全部差异点
//!
//! ## 后续阶段
//!
//! - **P2**：非流式响应体双向转换、错误体转换（§7.1 方案 B）
//! - **P3**：流式事件状态机双向转换（§7.6 容错策略）
//! - **P4**：日志 `bridge` 标签注入、`websocket` → `ws` 更名、`tracing::debug!` body 输出
//!
//! ## 关键约束
//!
//! - §7.2 `max_tokens` 缺失：从 `model_configs.max_output_tokens` 读取，兜底 [`MAX_TOKENS_FALLBACK`]
//! - §7.4 `response_format`：O→A 时移除字段并在 `system` 末尾追加 prompt 提示
//! - §7.10 工具调用 ID 不重命名，原样透传
//! - §7.8 转换函数中 `tracing::debug!(target: "i_code::bridge", ...)` 输出前后 body
//!
//! [`docs/proposals/protocol-bridge.md`]: ../../../../../docs/proposals/protocol-bridge.md

pub mod error;
pub mod request;
pub mod response;
pub mod stream;

#[cfg(test)]
mod tests;

use crate::modules::gateway_runtime::client::UpstreamProtocol;
use crate::modules::gateway_runtime::forwarding::context::GatewayProtocol;

#[allow(unused_imports)]
pub use error::BridgeError;
pub use request::{anthropic_to_openai_chat, openai_chat_to_anthropic};
pub use response::{anthropic_response_to_openai, convert_error_body, openai_response_to_anthropic};

/// `max_tokens` 缺失时的兜底默认值（§7.2）
///
/// 当请求体未指定 `max_tokens`，且 `model_configs.max_output_tokens` 也读取失败时使用。
/// Anthropic Messages API 要求 `max_tokens` 必填，O→A 桥接时必须注入一个值。
pub const MAX_TOKENS_FALLBACK: i64 = 200_000;

/// OpenAI Chat Completions 兼容协议的 `provider_type` 集合
///
/// 必须与 [`ClientFactory::create`] 的 OpenAI Chat 分支保持一致。
/// 列表变更时同步更新此处。
///
/// [`ClientFactory::create`]: crate::modules::gateway_runtime::client::ClientFactory::create
const OPENAI_CHAT_FAMILY: &[&str] = &[
    "openai",
    "openai-compatible",
    "openai-chat-completion",
    "deepseek",
    "moonshot-ai",
    "kimi-code",
    "newapi",
    "siliconflow",
    "aihubmix",
    "openrouter",
    "minimax",
    "xai-grok-build",
    "ollama",
    "custom",
    "codex",
    "gemini-cli",
    "antigravity",
];

/// Anthropic Messages 协议的 `provider_type` 集合
///
/// 必须与 [`ClientFactory::create`] 的 Anthropic 分支保持一致。
///
/// [`ClientFactory::create`]: crate::modules::gateway_runtime::client::ClientFactory::create
const ANTHROPIC_FAMILY: &[&str] = &["anthropic", "claude-relay-service"];

/// 判断 `provider_type` 是否属于 OpenAI Chat Completions 兼容协议族
pub fn is_openai_chat_family(provider_type: &str) -> bool {
    OPENAI_CHAT_FAMILY.contains(&provider_type)
}

/// 判断 `provider_type` 是否属于 Anthropic Messages 协议族
pub fn is_anthropic_family(provider_type: &str) -> bool {
    ANTHROPIC_FAMILY.contains(&provider_type)
}

/// 桥接类型
///
/// 表示网关入口协议与上游供应商协议不一致时所需的转换方向。
/// 写入 `ForwardContext`（P2 接入时）供响应转换与日志标签使用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeKind {
    /// 无桥接（入口协议与上游协议一致，或属于不参与桥接的协议组合）
    None,
    /// 入口 OpenAI Chat → 上游 Anthropic Messages（O→A）
    OpenaiToAnthropic,
    /// 入口 Anthropic Messages → 上游 OpenAI Chat（A→O）
    AnthropicToOpenai,
}

impl BridgeKind {
    /// 是否发生桥接
    pub fn is_bridged(self) -> bool {
        !matches!(self, Self::None)
    }

    /// 桥接方向的简短标签（用于日志）
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::OpenaiToAnthropic => "O→A",
            Self::AnthropicToOpenai => "A→O",
        }
    }
}

/// 检测是否需要桥接，并返回桥接方向
///
/// 触发条件矩阵详见设计文档 §4.1：
///
/// | 网关入口协议 | 上游 provider_type | 桥接方向 |
/// |-------------|-------------------|---------|
/// | `ChatCompletions` | OpenAI Chat 族 | `None` |
/// | `ChatCompletions` | Anthropic 族 | `OpenaiToAnthropic` |
/// | `AnthropicMessages` | Anthropic 族 | `None` |
/// | `AnthropicMessages` | OpenAI Chat 族 | `AnthropicToOpenai` |
/// | `Responses` | 任意 | `None`（本次不桥接 Responses） |
/// | 任意 | `websocket` / `openai-codex` / `openai-responses` | `None`（WS / Responses 不参与桥接） |
pub fn detect_bridge(gateway_protocol: GatewayProtocol, provider_type: &str) -> BridgeKind {
    let is_openai = is_openai_chat_family(provider_type);
    let is_anthropic = is_anthropic_family(provider_type);

    match (gateway_protocol, is_openai, is_anthropic) {
        (GatewayProtocol::ChatCompletions, false, true) => BridgeKind::OpenaiToAnthropic,
        (GatewayProtocol::AnthropicMessages, true, false) => BridgeKind::AnthropicToOpenai,
        _ => BridgeKind::None,
    }
}

/// `GatewayProtocol::to_upstream_with_bridge` 的纯函数实现
///
/// 返回桥接场景下 `UpstreamRequest.protocol` 应当使用的上游协议。
/// 当不桥接时，回退到 [`GatewayProtocol::to_upstream`] 的默认行为。
///
/// 设计文档 §4.3：桥接时 `UpstreamRequest.protocol` 必须改为上游协议，
/// 否则 Client 入口的 `request.protocol !=` 校验会失败。
pub fn bridge_upstream_protocol(
    gateway_protocol: GatewayProtocol,
    provider_type: &str,
) -> UpstreamProtocol {
    match detect_bridge(gateway_protocol, provider_type) {
        BridgeKind::OpenaiToAnthropic => UpstreamProtocol::AnthropicMessages,
        BridgeKind::AnthropicToOpenai => UpstreamProtocol::ChatCompletions,
        BridgeKind::None => gateway_protocol.to_upstream(),
    }
}

#[cfg(test)]
mod detect_tests {
    use super::*;

    #[test]
    fn test_detect_no_bridge_when_protocol_matches_openai() {
        assert_eq!(
            detect_bridge(GatewayProtocol::ChatCompletions, "openai"),
            BridgeKind::None
        );
        assert_eq!(
            detect_bridge(GatewayProtocol::ChatCompletions, "deepseek"),
            BridgeKind::None
        );
    }

    #[test]
    fn test_detect_no_bridge_when_protocol_matches_anthropic() {
        assert_eq!(
            detect_bridge(GatewayProtocol::AnthropicMessages, "anthropic"),
            BridgeKind::None
        );
        assert_eq!(
            detect_bridge(GatewayProtocol::AnthropicMessages, "claude-relay-service"),
            BridgeKind::None
        );
    }

    #[test]
    fn test_detect_openai_to_anthropic() {
        assert_eq!(
            detect_bridge(GatewayProtocol::ChatCompletions, "anthropic"),
            BridgeKind::OpenaiToAnthropic
        );
        assert_eq!(
            detect_bridge(GatewayProtocol::ChatCompletions, "claude-relay-service"),
            BridgeKind::OpenaiToAnthropic
        );
    }

    #[test]
    fn test_detect_anthropic_to_openai() {
        assert_eq!(
            detect_bridge(GatewayProtocol::AnthropicMessages, "openai"),
            BridgeKind::AnthropicToOpenai
        );
        assert_eq!(
            detect_bridge(GatewayProtocol::AnthropicMessages, "deepseek"),
            BridgeKind::AnthropicToOpenai
        );
    }

    #[test]
    fn test_detect_responses_never_bridges() {
        assert_eq!(
            detect_bridge(GatewayProtocol::Responses, "anthropic"),
            BridgeKind::None
        );
        assert_eq!(
            detect_bridge(GatewayProtocol::Responses, "openai"),
            BridgeKind::None
        );
        assert_eq!(
            detect_bridge(GatewayProtocol::Responses, "openai-responses"),
            BridgeKind::None
        );
    }

    #[test]
    fn test_detect_websocket_never_bridges() {
        assert_eq!(
            detect_bridge(GatewayProtocol::ChatCompletions, "websocket"),
            BridgeKind::None
        );
        assert_eq!(
            detect_bridge(GatewayProtocol::AnthropicMessages, "openai-codex"),
            BridgeKind::None
        );
    }

    #[test]
    fn test_bridge_upstream_protocol_matches_detect() {
        // O→A 桥接：上游协议切到 AnthropicMessages
        assert_eq!(
            bridge_upstream_protocol(GatewayProtocol::ChatCompletions, "anthropic"),
            UpstreamProtocol::AnthropicMessages
        );
        // A→O 桥接：上游协议切到 ChatCompletions
        assert_eq!(
            bridge_upstream_protocol(GatewayProtocol::AnthropicMessages, "openai"),
            UpstreamProtocol::ChatCompletions
        );
        // 不桥接：回退到默认
        assert_eq!(
            bridge_upstream_protocol(GatewayProtocol::ChatCompletions, "openai"),
            UpstreamProtocol::ChatCompletions
        );
        assert_eq!(
            bridge_upstream_protocol(GatewayProtocol::Responses, "openai-responses"),
            UpstreamProtocol::Responses
        );
    }
}
