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

    let prompt_tokens = usage.get("prompt_tokens").and_then(|v| v.as_i64());
    let completion_tokens = usage.get("completion_tokens").and_then(|v| v.as_i64());
    let mut total_tokens = usage.get("total_tokens").and_then(|v| v.as_i64());

    let completion_tokens = completion_tokens
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
