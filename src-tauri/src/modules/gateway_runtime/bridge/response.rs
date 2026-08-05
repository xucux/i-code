//! # 非流式响应体与错误体双向转换
//!
//! 在 P2 阶段（设计文档 §5.3 / §7.1）实现：
//!
//! - [`anthropic_response_to_openai`]：上游 Anthropic 响应体 → 入口 OpenAI Chat 响应体
//! - [`openai_response_to_anthropic`]：上游 OpenAI Chat 响应体 → 入口 Anthropic 响应体
//! - [`convert_error_body`]：上游 4xx 错误体按入口协议格式转换
//!
//! ## 关键约束
//!
//! - §3.6 `finish_reason` ↔ `stop_reason` 映射表
//! - §3.6 usage 字段重命名（`prompt_tokens`↔`input_tokens` 等），`total_tokens` 计算
//! - §7.1 HTTP 4xx 错误体按入口协议格式转换；5xx 由调用方走 `upstream_error_response`
//! - §7.8 转换函数输出 `tracing::debug!(target: "i_code::bridge", ...)` 前后 body
//! - §7.10 工具调用 ID 原样透传
//!
//! [`upstream_error_response`]: crate::modules::gateway_runtime::forwarding::util::upstream_error_response

use serde_json::{json, Value};

use super::error::BridgeError;
use super::BridgeKind;

// ===== 公开转换 API =====

/// Anthropic 响应体 → OpenAI Chat Completions 响应体（O→A 桥接的响应阶段）
///
/// 按设计文档 §5.3 就地转换：
///
/// - `content` 数组 → `choices[0].message.content`（字符串）+ `tool_calls`
/// - `stop_reason` → `finish_reason`（§3.6 映射表）
/// - `usage.input_tokens` → `usage.prompt_tokens`
/// - `usage.output_tokens` → `usage.completion_tokens`
/// - `usage.total_tokens` = `input + output`（Anthropic 不返回，需计算）
/// - `usage.cache_read_input_tokens` → `usage.prompt_tokens_details.cached_tokens`
/// - 设置 `object = "chat.completion"`，`choices[0].index = 0`，`message.role = "assistant"`
///
/// 保留原 `id` / `model`；`created` 缺失时置 0。
pub fn anthropic_response_to_openai(body: &mut Value) -> Result<(), BridgeError> {
    let before = body.to_string();

    let obj = body
        .as_object_mut()
        .ok_or_else(|| BridgeError::invalid_field("body", "响应体必须是 JSON 对象"))?;

    // 提取必要字段
    let id = obj.get("id").cloned().unwrap_or(Value::Null);
    let model = obj.get("model").cloned().unwrap_or(Value::Null);

    // content 数组 → message.content (string) + tool_calls
    let content_arr = obj.remove("content").and_then(|v| v.as_array().cloned());
    let (text_content, tool_calls) = anthropic_content_to_openai_message(content_arr)?;

    // stop_reason → finish_reason
    let stop_reason = obj
        .get("stop_reason")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let finish_reason = stop_reason_to_finish_reason(stop_reason.as_deref());

    // usage 转换
    let usage = obj.remove("usage");
    let new_usage = convert_anthropic_usage_to_openai(usage);

    // 清空原对象，重新构造 OpenAI 响应体结构
    obj.clear();

    obj.insert("id".to_string(), id);
    obj.insert("object".to_string(), Value::String("chat.completion".to_string()));
    // created 缺失时设为 0（OpenAI 必填字段，但网关无上游时间戳时无法重建）
    if let Some(created) = before_created_from_value(&before) {
        obj.insert("created".to_string(), Value::Number(created.into()));
    } else {
        obj.insert("created".to_string(), Value::Number(0i64.into()));
    }
    obj.insert("model".to_string(), model);

    // choices[0]
    let mut message = serde_json::Map::new();
    message.insert("role".to_string(), Value::String("assistant".to_string()));
    message.insert("content".to_string(), text_content);
    if !tool_calls.is_empty() {
        message.insert("tool_calls".to_string(), Value::Array(tool_calls));
    }

    let choice = json!({
        "index": 0,
        "message": Value::Object(message),
        "finish_reason": finish_reason,
    });
    obj.insert("choices".to_string(), Value::Array(vec![choice]));

    obj.insert("usage".to_string(), new_usage);

    tracing::debug!(
        target: "i_code::bridge",
        bridge = "A→O",
        kind = "response",
        before = %before,
        after = %body,
        "bridge response transformed"
    );

    Ok(())
}

/// OpenAI Chat Completions 响应体 → Anthropic 响应体（A→O 桥接的响应阶段）
///
/// 按设计文档 §5.3 就地转换：
///
/// - `choices[0].message.content`（字符串）→ `content: [{type:"text", text}]`
/// - `choices[0].message.tool_calls` → `content` 数组的 `tool_use` blocks
/// - `finish_reason` → `stop_reason`（§3.6 映射表）
/// - `usage.prompt_tokens` → `usage.input_tokens`
/// - `usage.completion_tokens` → `usage.output_tokens`
/// - `usage.prompt_tokens_details.cached_tokens` → `usage.cache_read_input_tokens`
/// - 设置 `type = "message"`，`role = "assistant"`
///
/// 保留原 `id` / `model`；`stop_reason` 缺失时回退到 `end_turn`。
pub fn openai_response_to_anthropic(body: &mut Value) -> Result<(), BridgeError> {
    let before = body.to_string();

    let obj = body
        .as_object_mut()
        .ok_or_else(|| BridgeError::invalid_field("body", "响应体必须是 JSON 对象"))?;

    // 提取必要字段
    let id = obj.get("id").cloned().unwrap_or(Value::Null);
    let model = obj.get("model").cloned().unwrap_or(Value::Null);

    // choices[0].message → content 数组
    let choices = obj.remove("choices").and_then(|v| v.as_array().cloned());
    let (content, stop_reason) = openai_choices_to_anthropic_content(choices)?;

    // usage 转换
    let usage = obj.remove("usage");
    let new_usage = convert_openai_usage_to_anthropic(usage);

    // 清空原对象，重新构造 Anthropic 响应体结构
    obj.clear();

    obj.insert("id".to_string(), id);
    obj.insert("type".to_string(), Value::String("message".to_string()));
    obj.insert("role".to_string(), Value::String("assistant".to_string()));
    obj.insert("model".to_string(), model);
    obj.insert("content".to_string(), Value::Array(content));

    let stop_reason_str = stop_reason
        .map(|s| Value::String(s))
        .unwrap_or_else(|| Value::String("end_turn".to_string()));
    obj.insert("stop_reason".to_string(), stop_reason_str);

    obj.insert("usage".to_string(), new_usage);

    tracing::debug!(
        target: "i_code::bridge",
        bridge = "O→A",
        kind = "response",
        before = %before,
        after = %body,
        "bridge response transformed"
    );

    Ok(())
}

/// 上游错误体按入口协议格式转换（§7.1 方案 B）
///
/// 根据 `kind` 决定转换方向：
///
/// - `OpenaiToAnthropic`：上游返回 Anthropic 错误体 → 转为入口 OpenAI 错误体
/// - `AnthropicToOpenai`：上游返回 OpenAI 错误体 → 转为入口 Anthropic 错误体
/// - `None`：直接返回（不应被调用）
///
/// 转换规则见 §7.1：
///
/// ```text
/// Anthropic: { "type":"error", "error":{ "type":"...", "message":"..." } }
/// OpenAI:    { "error":{ "message":"...", "type":"...", "param":null, "code":null } }
/// ```
///
/// 若 body 不符合预期结构，记录 warn 后保持原样（避免错误体被破坏导致客户端无法解析）。
pub fn convert_error_body(body: &mut Value, kind: BridgeKind) -> Result<(), BridgeError> {
    if !kind.is_bridged() {
        return Ok(());
    }

    let before = body.to_string();

    let result = match kind {
        BridgeKind::OpenaiToAnthropic => convert_anthropic_error_to_openai(body),
        BridgeKind::AnthropicToOpenai => convert_openai_error_to_anthropic(body),
        BridgeKind::None => Ok(()),
    };

    // 转换失败时记录 warn，但保持原 body 不变（避免破坏错误体）
    if let Err(e) = &result {
        tracing::warn!(
            target: "i_code::bridge",
            kind = kind.label(),
            error = %e,
            before = %before,
            "错误体转换失败，原样透传"
        );
        return Ok(());
    }

    tracing::debug!(
        target: "i_code::bridge",
        kind = kind.label(),
        category = "error_body",
        before = %before,
        after = %body,
        "bridge error body transformed"
    );

    Ok(())
}

// ===== 内部辅助：响应体转换 =====

/// 把 Anthropic 响应的 `content` 数组拆分为 OpenAI message 的 `content`（字符串）与 `tool_calls`
///
/// - 多个 `text` block 拼接为单个字符串（按 `\n` 分隔）
/// - `tool_use` block → `tool_calls` 数组项（`input` 序列化为 JSON 字符串）
/// - 其他类型 block（image 等）：text 部分按 JSON 字符串拼接进 content
fn anthropic_content_to_openai_message(
    content: Option<Vec<Value>>,
) -> Result<(Value, Vec<Value>), BridgeError> {
    let Some(blocks) = content else {
        // content 缺失时返回 null
        return Ok((Value::Null, Vec::new()));
    };

    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<Value> = Vec::new();

    for (i, block) in blocks.iter().enumerate() {
        let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match block_type {
            "text" => {
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    text_parts.push(text.to_string());
                }
            }
            "tool_use" => {
                let id = block
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        BridgeError::invalid_field(
                            format!("content[{}].id", i),
                            "tool_use 缺少 id",
                        )
                    })?
                    .to_string();
                let name = block
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        BridgeError::invalid_field(
                            format!("content[{}].name", i),
                            "tool_use 缺少 name",
                        )
                    })?
                    .to_string();
                let input = block.get("input").cloned().unwrap_or(Value::Null);
                let arguments = serde_json::to_string(&input).unwrap_or_else(|_| "null".to_string());
                tool_calls.push(json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": arguments,
                    }
                }));
            }
            _ => {
                // 未知 block 类型：序列化为 JSON 字符串拼接进文本，避免丢数据
                tracing::warn!(
                    target: "i_code::bridge",
                    block_type = block_type,
                    index = i,
                    "响应 content 中存在未知 block 类型，按 JSON 序列化拼接"
                );
                text_parts.push(block.to_string());
            }
        }
    }

    let text_content = if text_parts.is_empty() {
        // 无文本内容时返回 null（OpenAI 习惯）
        Value::Null
    } else {
        Value::String(text_parts.join("\n"))
    };

    Ok((text_content, tool_calls))
}

/// 把 OpenAI 响应的 `choices[0]` 拆分为 Anthropic `content` 数组与 `stop_reason`
///
/// 返回 `(content_array, stop_reason)`：
/// - `content_array`：text block + tool_use blocks
/// - `stop_reason`：从 `choices[0].finish_reason` 映射；choices 缺失时返回 None
fn openai_choices_to_anthropic_content(
    choices: Option<Vec<Value>>,
) -> Result<(Vec<Value>, Option<String>), BridgeError> {
    let Some(choices) = choices else {
        return Ok((Vec::new(), None));
    };

    let Some(first) = choices.into_iter().next() else {
        return Ok((Vec::new(), None));
    };

    let message = first.get("message").cloned().unwrap_or(Value::Null);
    let finish_reason = first
        .get("finish_reason")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let stop_reason = finish_reason_to_stop_reason(finish_reason.as_deref());

    let mut content: Vec<Value> = Vec::new();

    // message.content（string | null | array）→ text blocks
    if let Some(content_val) = message.get("content") {
        match content_val {
            Value::String(s) => {
                if !s.is_empty() {
                    content.push(json!({"type": "text", "text": s}));
                }
            }
            Value::Array(parts) => {
                for part in parts {
                    // OpenAI 响应中 content 数组较少见，按原样保留
                    content.push(part.clone());
                }
            }
            Value::Null => {}
            _ => {
                // 其他类型序列化为 text block
                content.push(json!({"type": "text", "text": content_val.to_string()}));
            }
        }
    }

    // message.tool_calls → tool_use blocks
    if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
        for (i, tc) in tool_calls.iter().enumerate() {
            let id = tc
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    BridgeError::invalid_field(
                        format!("choices[0].message.tool_calls[{}].id", i),
                        "tool_call 缺少 id",
                    )
                })?
                .to_string();
            let func = tc.get("function").ok_or_else(|| {
                BridgeError::invalid_field(
                    format!("choices[0].message.tool_calls[{}].function", i),
                    "tool_call 缺少 function 字段",
                )
            })?;
            let name = func
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    BridgeError::invalid_field(
                        format!("choices[0].message.tool_calls[{}].function.name", i),
                        "function 缺少 name",
                    )
                })?
                .to_string();
            let arguments_str = func
                .get("arguments")
                .and_then(|v| v.as_str())
                .unwrap_or("{}");
            let input: Value = serde_json::from_str(arguments_str).unwrap_or_else(|_| {
                tracing::warn!(
                    target: "i_code::bridge",
                    field = format!("choices[0].message.tool_calls[{}].function.arguments", i),
                    "解析 tool_call.arguments 为 JSON 失败，回退为空对象",
                );
                json!({})
            });
            content.push(json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input,
            }));
        }
    }

    Ok((content, stop_reason))
}

/// `stop_reason` → `finish_reason`（§3.6 映射表）
///
/// | Anthropic `stop_reason` | OpenAI `finish_reason` |
/// |-------------------------|------------------------|
/// | `end_turn` | `stop` |
/// | `max_tokens` | `length` |
/// | `stop_sequence` | `stop`（无直接对应，回退到 stop） |
/// | `tool_use` | `tool_calls` |
/// | 其他 | `stop`（兜底） |
fn stop_reason_to_finish_reason(stop_reason: Option<&str>) -> String {
    match stop_reason {
        Some("end_turn") => "stop".to_string(),
        Some("max_tokens") => "length".to_string(),
        Some("stop_sequence") => "stop".to_string(),
        Some("tool_use") => "tool_calls".to_string(),
        Some(other) => {
            tracing::warn!(
                target: "i_code::bridge",
                stop_reason = other,
                "未知 stop_reason，回退为 stop"
            );
            "stop".to_string()
        }
        None => "stop".to_string(),
    }
}

/// `finish_reason` → `stop_reason`（§3.6 映射表）
///
/// | OpenAI `finish_reason` | Anthropic `stop_reason` |
/// |------------------------|-------------------------|
/// | `stop` | `end_turn` |
/// | `length` | `max_tokens` |
/// | `tool_calls` | `tool_use` |
/// | `content_filter` | `end_turn`（无直接对应） |
/// | 其他 | `end_turn`（兜底） |
fn finish_reason_to_stop_reason(finish_reason: Option<&str>) -> Option<String> {
    let result = match finish_reason {
        Some("stop") => "end_turn".to_string(),
        Some("length") => "max_tokens".to_string(),
        Some("tool_calls") => "tool_use".to_string(),
        Some("content_filter") => "end_turn".to_string(),
        Some(other) => {
            tracing::warn!(
                target: "i_code::bridge",
                finish_reason = other,
                "未知 finish_reason，回退为 end_turn"
            );
            "end_turn".to_string()
        }
        None => return None,
    };
    Some(result)
}

/// Anthropic `usage` → OpenAI `usage`
///
/// - `input_tokens` → `prompt_tokens`
/// - `output_tokens` → `completion_tokens`
/// - `total_tokens` = `input + output`（计算）
/// - `cache_read_input_tokens` → `prompt_tokens_details.cached_tokens`
fn convert_anthropic_usage_to_openai(usage: Option<Value>) -> Value {
    let Some(usage) = usage else {
        return json!({
            "prompt_tokens": 0,
            "completion_tokens": 0,
            "total_tokens": 0,
        });
    };

    let input = usage
        .get("input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let total = input + output;
    let cached = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let mut obj = serde_json::Map::new();
    obj.insert("prompt_tokens".to_string(), Value::Number(input.into()));
    obj.insert("completion_tokens".to_string(), Value::Number(output.into()));
    obj.insert("total_tokens".to_string(), Value::Number(total.into()));
    if cached > 0 {
        obj.insert(
            "prompt_tokens_details".to_string(),
            json!({"cached_tokens": cached}),
        );
    }
    Value::Object(obj)
}

/// OpenAI `usage` → Anthropic `usage`
///
/// - `prompt_tokens` → `input_tokens`
/// - `completion_tokens` → `output_tokens`
/// - `prompt_tokens_details.cached_tokens` → `cache_read_input_tokens`
/// - 不设置 `cache_creation_input_tokens`（OpenAI 无对应字段）
fn convert_openai_usage_to_anthropic(usage: Option<Value>) -> Value {
    let Some(usage) = usage else {
        return json!({
            "input_tokens": 0,
            "output_tokens": 0,
        });
    };

    let input = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output = usage
        .get("completion_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cached = usage
        .get("prompt_tokens_details")
        .and_then(|v| v.get("cached_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let mut obj = serde_json::Map::new();
    obj.insert("input_tokens".to_string(), Value::Number(input.into()));
    obj.insert("output_tokens".to_string(), Value::Number(output.into()));
    if cached > 0 {
        obj.insert(
            "cache_read_input_tokens".to_string(),
            Value::Number(cached.into()),
        );
    }
    Value::Object(obj)
}

/// 从原始 body 字符串中提取 `created` 字段（仅用于 A→O 转换保留上游时间戳）
///
/// Anthropic 响应体无 `created` 字段，返回 None。
fn before_created_from_value(before: &str) -> Option<i64> {
    // 简单尝试解析为 JSON 并提取 created；失败时返回 None
    let parsed: Value = serde_json::from_str(before).ok()?;
    parsed.get("created").and_then(|v| v.as_i64())
}

// ===== 内部辅助：错误体转换 =====

/// Anthropic 错误体 → OpenAI 错误体
///
/// 输入：`{ "type":"error", "error":{ "type":"...", "message":"..." } }`
/// 输出：`{ "error":{ "message":"...", "type":"...", "param":null, "code":null } }`
fn convert_anthropic_error_to_openai(body: &mut Value) -> Result<(), BridgeError> {
    let obj = body
        .as_object_mut()
        .ok_or_else(|| BridgeError::invalid_field("body", "错误体必须是 JSON 对象"))?;

    let error = obj
        .remove("error")
        .ok_or_else(|| BridgeError::invalid_field("error", "Anthropic 错误体缺少 error 字段"))?;

    let error_obj = error
        .as_object()
        .ok_or_else(|| BridgeError::invalid_field("error", "error 字段必须是对象"))?;

    let message = error_obj
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let error_type = error_obj
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("api_error")
        .to_string();

    obj.clear();
    obj.insert(
        "error".to_string(),
        json!({
            "message": message,
            "type": error_type,
            "param": Value::Null,
            "code": Value::Null,
        }),
    );

    Ok(())
}

/// OpenAI 错误体 → Anthropic 错误体
///
/// 输入：`{ "error":{ "message":"...", "type":"...", "param":null, "code":null } }`
/// 输出：`{ "type":"error", "error":{ "type":"...", "message":"..." } }`
fn convert_openai_error_to_anthropic(body: &mut Value) -> Result<(), BridgeError> {
    let obj = body
        .as_object_mut()
        .ok_or_else(|| BridgeError::invalid_field("body", "错误体必须是 JSON 对象"))?;

    let error = obj
        .remove("error")
        .ok_or_else(|| BridgeError::invalid_field("error", "OpenAI 错误体缺少 error 字段"))?;

    let error_obj = error
        .as_object()
        .ok_or_else(|| BridgeError::invalid_field("error", "error 字段必须是对象"))?;

    let message = error_obj
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let error_type = error_obj
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("api_error")
        .to_string();

    obj.clear();
    obj.insert("type".to_string(), Value::String("error".to_string()));
    obj.insert(
        "error".to_string(),
        json!({
            "type": error_type,
            "message": message,
        }),
    );

    Ok(())
}
