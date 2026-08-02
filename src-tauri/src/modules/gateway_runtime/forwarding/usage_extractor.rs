//! # usage 数据提取
//!
//! 统一处理非流式响应 body 与流式 SSE 事件的 usage 解析。

use serde_json::Value;

use crate::modules::gateway_runtime::client::UpstreamProtocol;

/// 从非流式响应 body JSON 中解析 usage 字段
///
/// 返回 `(prompt_tokens, completion_tokens, total_tokens, cached_tokens, cache_hit)`
///
/// 兼容格式：
/// - OpenAI: `{ "usage": { "prompt_tokens", "completion_tokens", "total_tokens" } }`
/// - Anthropic: `{ "usage": { "input_tokens", "output_tokens" } }`
/// - OpenAI Responses: `{ "usage": { "input_tokens", "output_tokens", "total_tokens",
///   "input_tokens_details.cached_tokens" } }`
/// - DeepSeek 等兼容 OpenAI 并额外含 `prompt_cache_hit_tokens` /
///   `prompt_tokens_details.cached_tokens`
pub fn parse_usage_from_response_body(
    body: &str,
) -> (Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<bool>) {
    let val = match serde_json::from_str::<Value>(body) {
        Ok(v) => v,
        Err(_) => return (None, None, None, None, None),
    };

    let usage = match val.get("usage") {
        Some(u) => u,
        None => return (None, None, None, None, None),
    };

    // prompt_tokens：OpenAI 兼容；Responses 用 input_tokens
    let prompt_tokens = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| usage.get("input_tokens").and_then(|v| v.as_i64()));
    // completion_tokens：OpenAI 兼容；Anthropic / Responses 用 output_tokens
    let mut completion_tokens = usage.get("completion_tokens").and_then(|v| v.as_i64());
    let mut total_tokens = usage.get("total_tokens").and_then(|v| v.as_i64());

    completion_tokens = completion_tokens
        .or_else(|| usage.get("output_tokens").and_then(|v| v.as_i64()));

    if total_tokens.is_none() {
        if let (Some(pt), Some(ct)) = (prompt_tokens, completion_tokens) {
            total_tokens = Some(pt + ct);
        }
    }

    let cached_tokens = usage
        .get("prompt_cache_hit_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| {
            let creation = usage
                .get("cache_creation_input_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let read = usage
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if creation > 0 || read > 0 {
                Some(creation + read)
            } else {
                None
            }
        })
        .or_else(|| {
            usage
                .get("prompt_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
                .and_then(|v| v.as_i64())
        })
        .or_else(|| {
            // Responses API：input_tokens_details.cached_tokens
            usage
                .get("input_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
                .and_then(|v| v.as_i64())
        });

    let cache_hit = cached_tokens.map(|ct| ct > 0);

    (prompt_tokens, completion_tokens, total_tokens, cached_tokens, cache_hit)
}

/// 从单个 SSE 事件文本中解析 usage 数据并写入累加器
///
/// OpenAI 格式：`data: {"id":"...","usage":{"prompt_tokens":...,"completion_tokens":...}}`
/// Anthropic 格式：`event: message_delta\ndata: {"type":"message_delta","usage":{"output_tokens":...}}`
pub fn parse_sse_event_for_usage(
    event_text: &str,
    protocol: UpstreamProtocol,
    usage: &mut crate::modules::gateway_runtime::client::SseUsageData,
) {
    for line in event_text.lines() {
        let line = line.trim();
        if !line.starts_with("data:") {
            continue;
        }
        let data = line.strip_prefix("data:").unwrap_or("").trim();
        if data == "[DONE]" {
            continue;
        }

        let val: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        match protocol {
            UpstreamProtocol::ChatCompletions => {
                if let Some(usage_obj) = val.get("usage") {
                    if usage_obj.get("prompt_tokens").is_some()
                        || usage_obj.get("total_tokens").is_some()
                    {
                        usage.prompt_tokens =
                            usage_obj.get("prompt_tokens").and_then(|v| v.as_i64());
                        usage.completion_tokens =
                            usage_obj.get("completion_tokens").and_then(|v| v.as_i64());
                        usage.total_tokens =
                            usage_obj.get("total_tokens").and_then(|v| v.as_i64());
                        usage.cached_tokens = usage_obj
                            .get("prompt_tokens_details")
                            .and_then(|d| d.get("cached_tokens"))
                            .and_then(|v| v.as_i64())
                            .or_else(|| {
                                usage_obj
                                    .get("prompt_cache_hit_tokens")
                                    .and_then(|v| v.as_i64())
                            });
                        usage.cache_hit = usage.cached_tokens.map(|ct| ct > 0);
                    }
                }
            }
            UpstreamProtocol::Responses => {
                // 流式 Responses：usage 在 response.completed 事件的 response.usage 中
                // data: {"type":"response.completed","response":{"usage":{...}}}
                let event_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if event_type == "response.completed" {
                    if let Some(usage_obj) = val
                        .get("response")
                        .and_then(|r| r.get("usage"))
                    {
                        usage.prompt_tokens =
                            usage_obj.get("input_tokens").and_then(|v| v.as_i64());
                        usage.completion_tokens =
                            usage_obj.get("output_tokens").and_then(|v| v.as_i64());
                        usage.total_tokens =
                            usage_obj.get("total_tokens").and_then(|v| v.as_i64());
                        usage.cached_tokens = usage_obj
                            .get("input_tokens_details")
                            .and_then(|d| d.get("cached_tokens"))
                            .and_then(|v| v.as_i64());
                        usage.cache_hit = usage.cached_tokens.map(|ct| ct > 0);
                    }
                }
            }
            UpstreamProtocol::AnthropicMessages => {
                let event_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if event_type == "message_delta" {
                    if let Some(usage_obj) = val.get("usage") {
                        usage.completion_tokens =
                            usage_obj.get("output_tokens").and_then(|v| v.as_i64());
                        usage.total_tokens = usage.completion_tokens;
                        let creation = usage_obj
                            .get("cache_creation_input_tokens")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        let read = usage_obj
                            .get("cache_read_input_tokens")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        if creation > 0 || read > 0 {
                            usage.cached_tokens = Some(creation + read);
                            usage.cache_hit = Some(read > 0);
                        }
                    }
                }
                if event_type == "message_start" {
                    if let Some(msg) = val.get("message") {
                        if let Some(usage_obj) = msg.get("usage") {
                            usage.prompt_tokens =
                                usage_obj.get("input_tokens").and_then(|v| v.as_i64());
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Responses API 非流式 usage（input_tokens / output_tokens / input_tokens_details）
    #[test]
    fn test_parse_responses_usage_non_streaming() {
        let body = r#"{
            "id": "resp_abc",
            "object": "response",
            "usage": {
                "input_tokens": 328,
                "input_tokens_details": { "cached_tokens": 100 },
                "output_tokens": 52,
                "output_tokens_details": { "reasoning_tokens": 4 },
                "total_tokens": 380
            }
        }"#;

        let (prompt, completion, total, cached, cache_hit) =
            parse_usage_from_response_body(body);

        assert_eq!(prompt, Some(328));
        assert_eq!(completion, Some(52));
        assert_eq!(total, Some(380));
        assert_eq!(cached, Some(100));
        assert_eq!(cache_hit, Some(true));
    }

    /// Chat Completions 既有格式回归：prompt_tokens / completion_tokens 不受影响
    #[test]
    fn test_parse_chat_completions_usage_regression() {
        let body = r#"{
            "object": "chat.completion",
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20,
                "total_tokens": 30,
                "prompt_tokens_details": { "cached_tokens": 5 }
            }
        }"#;

        let (prompt, completion, total, cached, cache_hit) =
            parse_usage_from_response_body(body);

        assert_eq!(prompt, Some(10));
        assert_eq!(completion, Some(20));
        assert_eq!(total, Some(30));
        assert_eq!(cached, Some(5));
        assert_eq!(cache_hit, Some(true));
    }

    /// Responses API 流式 usage：response.completed 事件中的 response.usage
    #[test]
    fn test_parse_responses_streaming_usage() {
        let event = r#"data: {"type":"response.completed","response":{"id":"resp_abc","status":"completed","usage":{"input_tokens":100,"input_tokens_details":{"cached_tokens":40},"output_tokens":25,"output_tokens_details":{"reasoning_tokens":2},"total_tokens":125}}}"#;

        let mut usage = crate::modules::gateway_runtime::client::SseUsageData::default();
        parse_sse_event_for_usage(event, UpstreamProtocol::Responses, &mut usage);

        assert_eq!(usage.prompt_tokens, Some(100));
        assert_eq!(usage.completion_tokens, Some(25));
        assert_eq!(usage.total_tokens, Some(125));
        assert_eq!(usage.cached_tokens, Some(40));
        assert_eq!(usage.cache_hit, Some(true));
    }

    /// Responses 流式中途事件（非 completed）不应污染 usage
    #[test]
    fn test_parse_responses_streaming_non_terminal_event() {
        let event = r#"data: {"type":"response.output_text.delta","item_id":"msg_1","output_index":0,"delta":"hi"}"#;

        let mut usage = crate::modules::gateway_runtime::client::SseUsageData::default();
        parse_sse_event_for_usage(event, UpstreamProtocol::Responses, &mut usage);

        assert_eq!(usage.prompt_tokens, None);
        assert_eq!(usage.completion_tokens, None);
        assert_eq!(usage.total_tokens, None);
    }

    /// 无 usage 的响应体返回全 None
    #[test]
    fn test_parse_usage_missing() {
        let (prompt, completion, total, cached, cache_hit) =
            parse_usage_from_response_body(r#"{"object":"chat.completion","choices":[]}"#);
        assert_eq!(prompt, None);
        assert_eq!(completion, None);
        assert_eq!(total, None);
        assert_eq!(cached, None);
        assert_eq!(cache_hit, None);
    }
}
