//! # 工具函数
//!
//! 模型 ID 解析、token 估算、错误响应构造等。

use axum::response::{IntoResponse, Response};
use serde_json::Value;

use crate::error::IcodeError;
use crate::modules::gateway_runtime::auth::error_to_status_code;

/// 解析客户端请求中的模型 ID
///
/// 输入格式：`{prefix}/{real_id}` 或 `{real_id}`（直连模式）
/// 返回：`(prefix, real_id)`；prefix 为空表示直连模式。
pub fn parse_model_id(model_id: &str) -> crate::error::IcodeResult<(String, String)> {
    if let Some((prefix, real_id)) = model_id.split_once('/') {
        if prefix.is_empty() || real_id.is_empty() {
            return Err(IcodeError::validation(format!(
                "模型 ID 格式错误: {}",
                model_id
            )));
        }
        Ok((prefix.to_string(), real_id.to_string()))
    } else {
        Ok((String::new(), model_id.to_string()))
    }
}

/// 估算 prompt token 数
///
/// 从 `messages`（Chat Completions）或 `input`（Responses API：字符串或 item 数组）
/// 字段提取文本，使用 conservative 策略（3 字节/token + 消息开销）估算。
/// 此函数不会失败——估算仅作为补充，不应阻塞请求转发。
pub fn estimate_prompt_tokens(model_id: &str, body: &Value) -> Option<i64> {
    // Chat Completions：messages 数组
    if let Some(messages) = body.get("messages").and_then(|v| v.as_array()) {
        if messages.is_empty() {
            return None;
        }
        let mut total_bytes: usize = 0;
        let mut message_count: usize = 0;
        let mut image_count: usize = 0;

        for msg in messages {
            message_count += 1;
            if let Some(content) = msg.get("content") {
                match content {
                    Value::String(s) => {
                        total_bytes += s.len();
                    }
                    Value::Array(parts) => {
                        for part in parts {
                            if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                                total_bytes += text.len();
                            }
                            if part.get("type").and_then(|v| v.as_str()) == Some("image_url") {
                                image_count += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Some(tool_calls) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                for tc in tool_calls {
                    if let Some(func) = tc.get("function") {
                        if let Some(name) = func.get("name").and_then(|v| v.as_str()) {
                            total_bytes += name.len();
                        }
                        if let Some(args) = func.get("arguments").and_then(|v| v.as_str()) {
                            total_bytes += args.len();
                        }
                    }
                }
            }
        }

        let tokens_from_bytes = (total_bytes + 2) / 3;
        let overhead = message_count * 4;
        let image_tokens = image_count * 512;
        let estimated = tokens_from_bytes + overhead + image_tokens;

        tracing::debug!(
            "tokenizer 估算: model={}, messages={}, bytes={}, estimated_tokens={}",
            model_id, message_count, total_bytes, estimated
        );
        return Some(estimated as i64);
    }

    // Responses API：input 可以是字符串或 item 数组
    if let Some(input) = body.get("input") {
        let mut total_bytes: usize = 0;
        let mut message_count: usize = 0;
        match input {
            Value::String(s) => {
                if s.is_empty() {
                    return None;
                }
                total_bytes += s.len();
                message_count = 1;
            }
            Value::Array(items) => {
                if items.is_empty() {
                    return None;
                }
                for item in items {
                    message_count += 1;
                    if let Some(content) = item.get("content") {
                        match content {
                            Value::String(s) => total_bytes += s.len(),
                            Value::Array(parts) => {
                                for part in parts {
                                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                                        total_bytes += text.len();
                                    }
                                    if part.get("type").and_then(|v| v.as_str()) == Some("input_image") {
                                        total_bytes += 512 * 3; // 图像按 512 token 估算
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    // 工具调用 item（function_call）的参数
                    if let Some(args) = item.get("arguments").and_then(|v| v.as_str()) {
                        total_bytes += args.len();
                    }
                }
            }
            _ => return None,
        }

        let tokens_from_bytes = (total_bytes + 2) / 3;
        let overhead = message_count * 4;
        let estimated = tokens_from_bytes + overhead;

        tracing::debug!(
            "tokenizer 估算(responses): model={}, input_items={}, bytes={}, estimated_tokens={}",
            model_id, message_count, total_bytes, estimated
        );
        return Some(estimated as i64);
    }

    None
}

/// 构造上游错误响应
///
/// 将内部 `IcodeError` 转换为符合 OpenAI 标准的错误体，避免把数据库/堆栈等
/// 内部信息泄漏给客户端。
pub fn upstream_error_response(err: IcodeError) -> Response {
    let status = error_to_status_code(&err);

    let (error_type, error_code, message) = match err.code.as_str() {
        "VALIDATION" => ("invalid_request_error", "bad_request", err.message),
        "UNAUTHORIZED" => ("authentication_error", "invalid_api_key", err.message),
        "FORBIDDEN" => ("permission_error", "forbidden", err.message),
        "NOT_FOUND" => ("invalid_request_error", "not_found", err.message),
        "CONFLICT" => ("invalid_request_error", "conflict", err.message),
        "GATEWAY" => ("api_error", "bad_gateway", err.message),
        _ => (
            "api_error",
            "internal_error",
            "The gateway encountered an internal error.".to_string(),
        ),
    };

    let body = serde_json::json!({
        "error": {
            "message": message,
            "type": error_type,
            "param": serde_json::Value::Null,
            "code": error_code,
        }
    });
    (status, axum::Json(body)).into_response()
}

/// 根据供应商类型、传输方式与流式标志生成协议标签
///
/// - 流式请求标记为 `sse`（HTTP/SSE 透传）
/// - `transport = websocket` 的 OpenAI Responses / Codex 供应商标记为 `ws`
///   （§6.3 / §7.5：自 P4 起 `websocket` 统一更名为 `ws`，前端筛选器与日志展示同步迁移）
/// - 桥接转发额外标记为 `bridge`（§7.5 / §8.2 P4）
pub fn protocol_tags(
    provider_type: &str,
    transport: Option<&str>,
    is_stream: bool,
    bridge_kind: crate::modules::gateway_runtime::bridge::BridgeKind,
) -> Vec<String> {
    let mut tags = Vec::new();
    if is_stream {
        tags.push("sse".to_string());
    }
    if (provider_type == "openai-responses" || provider_type == "openai-codex")
        && transport == Some("websocket")
    {
        tags.push("ws".to_string());
    }
    if bridge_kind.is_bridged() {
        tags.push("bridge".to_string());
    }
    tags
}

/// 判断是否为网络层错误
///
/// 网络层错误（DNS/TCP/TLS/超时）需在日志中额外打 `network` 标签。
pub fn is_network_error(err: &super::super::client::ClientError) -> bool {
    matches!(
        err,
        super::super::client::ClientError::RequestFailed(_)
    )
}

/// 构造转发失败时的错误标签
///
/// 在 `protocol_tags` 基础上追加 `network`（网络层错误时）。
/// `bridge` 标签已由 `protocol_tags` 注入（§7.5）。
pub fn build_error_tags(tags: &[String], is_network: bool) -> Vec<String> {
    let mut error_tags = tags.to_vec();
    if is_network {
        error_tags.push("network".to_string());
    }
    error_tags
}

use crate::modules::gateway_runtime::client::build_upstream_url;
use crate::modules::gateway_runtime::forwarding::context::ForwardContext;

/// 构造用于日志记录的 URL
///
/// 仅用于日志展示，不参与实际请求。
///
/// **必须与实际请求 URL 一致**：桥接场景下，入口协议（如 `ChatCompletions`）
/// 与上游协议（如 `AnthropicMessages`）不同，日志应展示上游实际路径
/// （`/v1/messages`），否则会误导排查（用户看到 chat 路径却实际请求了 messages 路径）。
///
/// 因此此处通过 [`bridge_upstream_protocol`] 计算桥接后的上游协议再选路径，
/// 与 `AnthropicClient` / `OpenAiChatClient` 内部 `build_upstream_url` 的入参一致。
///
/// [`bridge_upstream_protocol`]: crate::modules::gateway_runtime::bridge::bridge_upstream_protocol
pub fn build_log_url(ctx: &ForwardContext) -> String {
    use crate::modules::gateway_runtime::bridge::bridge_upstream_protocol;
    use crate::modules::gateway_runtime::client::UpstreamProtocol;
    let upstream_protocol = bridge_upstream_protocol(
        ctx.gateway_protocol,
        &ctx.upstream.provider.provider_type,
    );
    let path = match upstream_protocol {
        UpstreamProtocol::ChatCompletions => "/chat/completions",
        UpstreamProtocol::AnthropicMessages => "/v1/messages",
        UpstreamProtocol::Responses => "/responses",
    };
    build_upstream_url(&ctx.upstream.provider, path).unwrap_or_else(|_| {
        format!(
            "{}/{}",
            ctx.upstream.provider.base_url,
            upstream_protocol
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_model_id() {
        let (slug, id) = parse_model_id("openai/gpt-4.1").unwrap();
        assert_eq!(slug, "openai");
        assert_eq!(id, "gpt-4.1");

        let (slug, id) = parse_model_id("openai-main/gpt-4.1").unwrap();
        assert_eq!(slug, "openai-main");
        assert_eq!(id, "gpt-4.1");
    }

    #[test]
    fn test_parse_model_id_invalid() {
        assert!(parse_model_id("/gpt-4.1").is_err());
        assert!(parse_model_id("openai/").is_err());
    }
}
