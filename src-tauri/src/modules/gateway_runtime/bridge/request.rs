//! # 请求体双向转换
//!
//! 实现 Anthropic Messages ↔ OpenAI Chat Completions 请求体的结构转换。
//!
//! ## 函数
//!
//! - [`anthropic_to_openai_chat`]（A→O）：入口 Anthropic Messages，上游 OpenAI Chat
//! - [`openai_chat_to_anthropic`]（O→A）：入口 OpenAI Chat，上游 Anthropic Messages
//!
//! ## 关键约束
//!
//! - §3 / §5.1 / §5.2 字段映射规则
//! - §7.2 `max_tokens` 缺失时由调用方传入 `model_configs.max_output_tokens`，兜底 [`MAX_TOKENS_FALLBACK`]
//! - §7.4 `response_format` 移除并在 `system` 末尾追加 prompt 提示
//! - §7.10 工具调用 ID 原样透传，不重命名
//! - §7.8 转换函数输出 `tracing::debug!(target: "i_code::bridge", ...)` 前后 body
//!
//! [`MAX_TOKENS_FALLBACK`]: super::MAX_TOKENS_FALLBACK

use serde_json::{json, Value};

use super::error::BridgeError;
use super::MAX_TOKENS_FALLBACK;

// ===== 公开转换 API =====

/// Anthropic Messages → OpenAI Chat Completions（A→O）
///
/// 按设计文档 §5.1 规则就地转换请求体：
///
/// 1. 顶层 `system` 提取为 `messages[0]` 的 `role:"system"`
/// 2. `assistant.content` 中的 `tool_use` block 提到 message 顶层 `tool_calls`
/// 3. `user.content` 中的 `tool_result` block 拆分为独立 `role:"tool"` 消息
/// 4. `tools[].input_schema` → `tools[].function.parameters`（包裹 `type:"function"`）
/// 5. `tool_choice` 结构转换（`auto`/`any`/`tool` → `auto`/`required`/`{type:"function",...}`）
/// 6. `stop_sequences` → `stop`，`metadata.user_id` → `user`
/// 7. 移除 `thinking`
/// 8. 流式请求注入 `stream_options.include_usage = true`
pub fn anthropic_to_openai_chat(body: &mut Value) -> Result<(), BridgeError> {
    let before = body.to_string();

    let obj = body
        .as_object_mut()
        .ok_or_else(|| BridgeError::invalid_field("body", "请求体必须是 JSON 对象"))?;

    // 1. system 提取
    let system_text = extract_anthropic_system_to_text(obj);
    let has_system = system_text.is_some();

    // 2. messages 重组
    let new_messages = transform_anthropic_messages_to_openai(obj)?;
    obj.insert(
        "messages".to_string(),
        Value::Array(new_messages),
    );

    // 把 system 插入 messages 开头
    if let Some(text) = system_text {
        if let Some(arr) = obj.get_mut("messages").and_then(|v| v.as_array_mut()) {
            arr.insert(0, json!({"role": "system", "content": text}));
        }
    }

    // 3. tools 重命名
    transform_anthropic_tools_to_openai(obj);

    // 4. tool_choice 转换
    transform_anthropic_tool_choice_to_openai(obj);

    // 5. 字段重命名
    rename_fields_anthropic_to_openai(obj);

    // 6. 移除 thinking（OpenAI 不识别）
    obj.remove("thinking");

    // 7. 流式注入 stream_options.include_usage（与 prepare_body 既有逻辑一致）
    let is_stream = obj
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if is_stream {
        obj.insert(
            "stream_options".to_string(),
            json!({"include_usage": true}),
        );
    }

    let _ = has_system; // 仅用于消除未使用警告

    tracing::debug!(
        target: "i_code::bridge",
        bridge = "A→O",
        before = %before,
        after = %body,
        "bridge request transformed"
    );

    Ok(())
}

/// OpenAI Chat Completions → Anthropic Messages（O→A）
///
/// 按设计文档 §5.2 规则就地转换请求体：
///
/// 1. `messages` 中所有 `role:"system"` 提取到顶层 `system`（字符串）
/// 2. `role:"tool"` 消息合并到前一条 assistant 的 `content` 数组作为 `tool_result`
/// 3. `assistant.tool_calls` 移入 `content` 数组作为 `tool_use` block
/// 4. `tools[].function.parameters` → `tools[].input_schema`（去除 `type:"function"` 包裹）
/// 5. `tool_choice` 反向结构转换
/// 6. `stop` → `stop_sequences`（单值包装为数组），`user` → `metadata.user_id`
/// 7. `max_tokens` 缺失时按 §7.2 注入默认值（`max_output_tokens` 或 [`MAX_TOKENS_FALLBACK`]）
/// 8. 丢弃 `frequency_penalty`/`presence_penalty`/`seed`/`n`/`logprobs`/`service_tier`/`stream_options`
/// 9. `response_format` 按 §7.4 移除字段并注入 system prompt 提示
/// 10. `reasoning_effort` → `thinking.budget_tokens`（按 §3.5 固定映射表）
///
/// `max_output_tokens` 由调用方从 `model_configs.max_output_tokens` 查询后传入，
/// `None` 表示查询失败或未配置，此时使用 [`MAX_TOKENS_FALLBACK`]。
pub fn openai_chat_to_anthropic(
    body: &mut Value,
    max_output_tokens: Option<i64>,
) -> Result<(), BridgeError> {
    let before = body.to_string();

    let obj = body
        .as_object_mut()
        .ok_or_else(|| BridgeError::invalid_field("body", "请求体必须是 JSON 对象"))?;

    // 1. system 提取
    let system_text = extract_openai_system_to_text(obj);

    // 2. messages 重组
    let new_messages = transform_openai_messages_to_anthropic(obj)?;

    // 3. response_format 处理（§7.4）：先处理 prompt 注入，再移除字段
    let response_format_prompt = build_response_format_prompt(obj);

    // 合并 system_text + response_format_prompt
    let final_system = match (system_text, response_format_prompt) {
        (Some(s), Some(p)) => Some(format!("{}{}", s, p)),
        (Some(s), None) => Some(s),
        (None, Some(p)) => Some(p),
        (None, None) => None,
    };
    if let Some(text) = final_system {
        obj.insert("system".to_string(), Value::String(text));
    }

    obj.insert(
        "messages".to_string(),
        Value::Array(new_messages),
    );

    // 4. tools 重命名
    transform_openai_tools_to_anthropic(obj);

    // 5. tool_choice 转换
    transform_openai_tool_choice_to_anthropic(obj);

    // 6. 字段重命名
    rename_fields_openai_to_anthropic(obj);

    // 7. max_tokens 兜底
    apply_max_tokens_fallback(obj, max_output_tokens);

    // 8. 移除被丢弃的字段
    for field in [
        "frequency_penalty",
        "presence_penalty",
        "seed",
        "n",
        "logprobs",
        "top_logprobs",
        "service_tier",
        "stream_options",
        "response_format",
    ] {
        obj.remove(field);
    }

    // 9. reasoning_effort → thinking
    convert_reasoning_effort_to_thinking(obj);

    tracing::debug!(
        target: "i_code::bridge",
        bridge = "O→A",
        before = %before,
        after = %body,
        "bridge request transformed"
    );

    Ok(())
}

// ===== A→O 内部辅助函数 =====

/// 把 Anthropic `body.system`（string 或 text block 数组）拼接为纯字符串
///
/// 提取后从 `obj` 中移除 `system` 字段。返回 `None` 表示无 system 或为空。
fn extract_anthropic_system_to_text(obj: &mut serde_json::Map<String, Value>) -> Option<String> {
    let system = obj.remove("system")?;
    match system {
        Value::String(s) => {
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        Value::Array(blocks) => {
            let mut buf = String::new();
            for block in blocks {
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    if !buf.is_empty() {
                        buf.push('\n');
                    }
                    buf.push_str(text);
                }
            }
            if buf.is_empty() {
                None
            } else {
                Some(buf)
            }
        }
        _ => None,
    }
}

/// 转换 Anthropic messages 数组为 OpenAI messages 数组
///
/// 角色映射：
/// - `user` → `user`（content 中的 `tool_result` 拆分为独立 `tool` 消息）
/// - `assistant` → `assistant`（content 中的 `tool_use` 提到顶层 `tool_calls`）
/// - `system` → 一般不存在于 Anthropic messages（已被顶层 `system` 提取），若出现按 user 处理
fn transform_anthropic_messages_to_openai(
    obj: &mut serde_json::Map<String, Value>,
) -> Result<Vec<Value>, BridgeError> {
    let messages = obj
        .remove("messages")
        .and_then(|v| match v {
            Value::Array(arr) => Some(arr),
            _ => None,
        })
        .ok_or_else(|| BridgeError::invalid_field("messages", "messages 必须是数组"))?;

    let mut out: Vec<Value> = Vec::with_capacity(messages.len());

    for (i, msg) in messages.into_iter().enumerate() {
        let role = msg
            .get("role")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BridgeError::invalid_field(
                    format!("messages[{}].role", i),
                    "缺少 role 字段或非字符串",
                )
            })?
            .to_string();

        match role.as_str() {
            "user" => {
                let new_msgs = transform_anthropic_user_to_openai(&msg, i)?;
                out.extend(new_msgs);
            }
            "assistant" => {
                let new_msg = transform_anthropic_assistant_to_openai(&msg, i)?;
                out.push(new_msg);
            }
            // system 理论上不在 messages 中，但容错处理：作为 user 消息透传
            "system" => {
                let mut sys_msg = msg.clone();
                if let Some(obj) = sys_msg.as_object_mut() {
                    // Anthropic 的 system role 走顶层提取，这里把残留的当作 user 处理
                    obj.insert("role".to_string(), Value::String("user".to_string()));
                }
                out.push(sys_msg);
            }
            _ => {
                // 未知角色：原样保留，让上游判定
                out.push(msg);
            }
        }
    }

    Ok(out)
}

/// 转换 Anthropic `user` 消息（可能含 `tool_result` blocks）为 OpenAI 消息序列
///
/// 一个 Anthropic user 消息可能展开为多条 OpenAI 消息：
/// - `tool_result` block → 独立 `role:"tool"` 消息
/// - 其他 content（text/image）保留在原 user 消息中
fn transform_anthropic_user_to_openai(
    msg: &Value,
    index: usize,
) -> Result<Vec<Value>, BridgeError> {
    let content = msg.get("content");
    let mut result: Vec<Value> = Vec::new();

    let mut user_blocks: Vec<Value> = Vec::new();
    if let Some(content) = content {
        match content {
            Value::String(s) => {
                user_blocks.push(json!({"type": "text", "text": s}));
            }
            Value::Array(blocks) => {
                for block in blocks {
                    let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match block_type {
                        "tool_result" => {
                            let tool_use_id = block
                                .get("tool_use_id")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    BridgeError::invalid_field(
                                        format!("messages[{}].content[].tool_use_id", index),
                                        "tool_result 缺少 tool_use_id",
                                    )
                                })?
                                .to_string();
                            let content_str = anthropic_tool_result_content_to_string(block.get("content"));
                            result.push(json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": content_str,
                            }));
                        }
                        "text" => {
                            user_blocks.push(block.clone());
                        }
                        "image" => {
                            if let Some(oai_block) = anthropic_image_to_openai_image_url(block) {
                                user_blocks.push(oai_block);
                            }
                        }
                        _ => {
                            // 未知 block 类型：原样保留
                            user_blocks.push(block.clone());
                        }
                    }
                }
            }
            _ => {
                return Err(BridgeError::invalid_field(
                    format!("messages[{}].content", index),
                    "content 必须是字符串或数组",
                ));
            }
        }
    }

    if !user_blocks.is_empty() {
        let mut user_msg = serde_json::Map::new();
        user_msg.insert("role".to_string(), Value::String("user".to_string()));
        // 单个文本 block 且无其他类型：扁平化为字符串（更接近 OpenAI 习惯）
        if user_blocks.len() == 1
            && user_blocks[0].get("type").and_then(|v| v.as_str()) == Some("text")
        {
            if let Some(text) = user_blocks[0].get("text").and_then(|v| v.as_str()) {
                user_msg.insert("content".to_string(), Value::String(text.to_string()));
            }
        } else {
            user_msg.insert("content".to_string(), Value::Array(user_blocks));
        }
        result.push(Value::Object(user_msg));
    }

    Ok(result)
}

/// 转换 Anthropic `assistant` 消息为 OpenAI `assistant` 消息
///
/// - `content` 数组中的 `tool_use` block 提取到 message 顶层 `tool_calls`
/// - `content` 数组中的 `text`/`image` block 保留在 content 中
/// - `content` 字符串扁平化处理
fn transform_anthropic_assistant_to_openai(
    msg: &Value,
    index: usize,
) -> Result<Value, BridgeError> {
    let mut new_msg = serde_json::Map::new();
    new_msg.insert(
        "role".to_string(),
        Value::String("assistant".to_string()),
    );

    let mut new_content: Vec<Value> = Vec::new();
    let mut tool_calls: Vec<Value> = Vec::new();

    if let Some(content) = msg.get("content") {
        match content {
            Value::String(s) => {
                if !s.is_empty() {
                    new_content.push(json!({"type": "text", "text": s}));
                }
            }
            Value::Array(blocks) => {
                for block in blocks {
                    let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match block_type {
                        "text" => new_content.push(block.clone()),
                        "image" => {
                            if let Some(oai_block) = anthropic_image_to_openai_image_url(block) {
                                new_content.push(oai_block);
                            }
                        }
                        "tool_use" => {
                            let id = block
                                .get("id")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    BridgeError::invalid_field(
                                        format!("messages[{}].content[].id", index),
                                        "tool_use 缺少 id",
                                    )
                                })?
                                .to_string();
                            let name = block
                                .get("name")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    BridgeError::invalid_field(
                                        format!("messages[{}].content[].name", index),
                                        "tool_use 缺少 name",
                                    )
                                })?
                                .to_string();
                            // input 是 JSON 对象，OpenAI 的 arguments 是 JSON 字符串
                            let input = block.get("input").cloned().unwrap_or(Value::Null);
                            let arguments = serde_json::to_string(&input)
                                .unwrap_or_else(|_| "null".to_string());
                            tool_calls.push(json!({
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": arguments,
                                }
                            }));
                        }
                        _ => new_content.push(block.clone()),
                    }
                }
            }
            Value::Null => {}
            _ => {
                return Err(BridgeError::invalid_field(
                    format!("messages[{}].content", index),
                    "content 必须是字符串或数组",
                ));
            }
        }
    }

    // 设置 content：空数组时设为 null（OpenAI 习惯），单个 text 时扁平化
    if new_content.is_empty() {
        new_msg.insert("content".to_string(), Value::Null);
    } else if new_content.len() == 1
        && new_content[0].get("type").and_then(|v| v.as_str()) == Some("text")
    {
        if let Some(text) = new_content[0].get("text").and_then(|v| v.as_str()) {
            new_msg.insert("content".to_string(), Value::String(text.to_string()));
        }
    } else {
        new_msg.insert("content".to_string(), Value::Array(new_content));
    }

    if !tool_calls.is_empty() {
        new_msg.insert("tool_calls".to_string(), Value::Array(tool_calls));
    }

    Ok(Value::Object(new_msg))
}

/// 把 Anthropic `image` block 转换为 OpenAI `image_url` block
///
/// - `source.type = "base64"` → `data:{media_type};base64,{data}`
/// - `source.type = "url"` → 直接使用 `source.url`
fn anthropic_image_to_openai_image_url(block: &Value) -> Option<Value> {
    let source = block.get("source")?;
    let src_type = source.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let url = match src_type {
        "base64" => {
            let media_type = source.get("media_type").and_then(|v| v.as_str()).unwrap_or("");
            let data = source.get("data").and_then(|v| v.as_str()).unwrap_or("");
            if media_type.is_empty() || data.is_empty() {
                return None;
            }
            format!("data:{};base64,{}", media_type, data)
        }
        "url" => {
            let u = source.get("url").and_then(|v| v.as_str()).unwrap_or("");
            if u.is_empty() {
                return None;
            }
            u.to_string()
        }
        _ => return None,
    };
    Some(json!({
        "type": "image_url",
        "image_url": {"url": url},
    }))
}

/// 把 Anthropic `tool_result.content`（string / array / null）转为 OpenAI tool 消息的字符串 content
fn anthropic_tool_result_content_to_string(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(arr)) => {
            // 拼接所有 text block，无分隔符；非 text block 用 JSON 表示
            let mut buf = String::new();
            for item in arr {
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    buf.push_str(text);
                } else {
                    buf.push_str(&item.to_string());
                }
            }
            buf
        }
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// 转换 Anthropic `tools` 为 OpenAI `tools` 结构
///
/// `{name, description, input_schema}` → `{type:"function", function:{name, description, parameters}}`
fn transform_anthropic_tools_to_openai(obj: &mut serde_json::Map<String, Value>) {
    let Some(tools) = obj.remove("tools") else {
        return;
    };
    let Some(arr) = tools.as_array() else {
        // 非数组：原样放回
        obj.insert("tools".to_string(), tools);
        return;
    };
    let new_tools: Vec<Value> = arr
        .iter()
        .map(|tool| {
            let name = tool.get("name").cloned().unwrap_or(Value::Null);
            let description = tool.get("description").cloned().unwrap_or(Value::Null);
            let parameters = tool
                .get("input_schema")
                .cloned()
                .unwrap_or_else(|| json!({}));
            json!({
                "type": "function",
                "function": {
                    "name": name,
                    "description": description,
                    "parameters": parameters,
                }
            })
        })
        .collect();
    obj.insert("tools".to_string(), Value::Array(new_tools));
}

/// 转换 Anthropic `tool_choice` 为 OpenAI `tool_choice`
///
/// - `{type:"auto"}` → `"auto"`
/// - `{type:"any"}` → `"required"`
/// - `{type:"tool", name:"f"}` → `{type:"function", function:{name:"f"}}`
fn transform_anthropic_tool_choice_to_openai(obj: &mut serde_json::Map<String, Value>) {
    let Some(tc) = obj.remove("tool_choice") else {
        return;
    };
    let tc_type = tc.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let new_tc = match tc_type {
        "auto" => Value::String("auto".to_string()),
        "any" => Value::String("required".to_string()),
        "tool" => {
            let name = tc.get("name").cloned().unwrap_or(Value::Null);
            json!({"type": "function", "function": {"name": name}})
        }
        // 未知类型：原样保留
        _ => tc,
    };
    obj.insert("tool_choice".to_string(), new_tc);
}

/// 执行 A→O 字段重命名：`stop_sequences` → `stop`，`metadata.user_id` → `user`
fn rename_fields_anthropic_to_openai(obj: &mut serde_json::Map<String, Value>) {
    if let Some(stop) = obj.remove("stop_sequences") {
        // OpenAI 接受 string 或 array，直接透传
        obj.insert("stop".to_string(), stop);
    }
    if let Some(metadata) = obj.remove("metadata") {
        if let Some(user_id) = metadata.get("user_id").and_then(|v| v.as_str()) {
            obj.insert("user".to_string(), Value::String(user_id.to_string()));
        }
    }
}

// ===== O→A 内部辅助函数 =====

/// 提取 OpenAI messages 中所有 `role:"system"` 文本，拼接为顶层 system 字符串
///
/// 从 messages 数组中删除 system 消息。返回 `None` 表示无 system 文本。
fn extract_openai_system_to_text(obj: &mut serde_json::Map<String, Value>) -> Option<String> {
    let messages = obj.get_mut("messages")?;
    let arr = messages.as_array_mut()?;

    let mut sys_texts: Vec<String> = Vec::new();
    let mut i = 0;
    while i < arr.len() {
        let is_system = arr[i]
            .get("role")
            .and_then(|v| v.as_str())
            .map(|r| r == "system")
            .unwrap_or(false);
        if is_system {
            let msg = arr.remove(i);
            if let Some(content) = msg.get("content") {
                match content {
                    Value::String(s) => sys_texts.push(s.clone()),
                    Value::Array(parts) => {
                        for p in parts {
                            if let Some(t) = p.get("text").and_then(|v| v.as_str()) {
                                sys_texts.push(t.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        } else {
            i += 1;
        }
    }

    if sys_texts.is_empty() {
        None
    } else {
        Some(sys_texts.join("\n"))
    }
}

/// 转换 OpenAI messages 数组为 Anthropic messages 数组
///
/// 角色映射：
/// - `system` → 已被顶层提取，跳过
/// - `user` → `user`（content 字符串转为 text block 数组，image_url → image）
/// - `assistant` → `assistant`（content 字符串转 text block，tool_calls 转 tool_use blocks）
/// - `tool` → 合并到前一条 assistant 的 content 作为 `tool_result`；无前一条则包装为 user
fn transform_openai_messages_to_anthropic(
    obj: &mut serde_json::Map<String, Value>,
) -> Result<Vec<Value>, BridgeError> {
    let messages = obj
        .remove("messages")
        .and_then(|v| match v {
            Value::Array(arr) => Some(arr),
            _ => None,
        })
        .ok_or_else(|| BridgeError::invalid_field("messages", "messages 必须是数组"))?;

    let mut out: Vec<Value> = Vec::with_capacity(messages.len());

    for (i, msg) in messages.into_iter().enumerate() {
        let role = msg
            .get("role")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BridgeError::invalid_field(
                    format!("messages[{}].role", i),
                    "缺少 role 字段或非字符串",
                )
            })?
            .to_string();

        match role.as_str() {
            "system" => {
                // 已被 extract_openai_system_to_text 提取，跳过
                // （extract 函数已就地删除，此处理论上不会再遇到，但容错处理）
            }
            "user" => {
                let new_msg = transform_openai_user_to_anthropic(&msg, i)?;
                out.push(new_msg);
            }
            "assistant" => {
                let new_msg = transform_openai_assistant_to_anthropic(&msg, i)?;
                out.push(new_msg);
            }
            "tool" => {
                let mut tool_result_block = Some(build_tool_result_from_openai_tool_msg(&msg, i)?);
                // 尝试合并到前一条 assistant
                let mut merged = false;
                if let Some(last) = out.last_mut() {
                    let is_assistant = last
                        .get("role")
                        .and_then(|v| v.as_str())
                        .map(|r| r == "assistant")
                        .unwrap_or(false);
                    if is_assistant {
                        // 确保 content 是数组
                        ensure_content_is_array(last);
                        if let Some(arr) = last.get_mut("content").and_then(|v| v.as_array_mut()) {
                            if let Some(block) = tool_result_block.take() {
                                arr.push(block);
                                merged = true;
                            }
                        }
                    }
                }
                if !merged {
                    // 无前一条 assistant：包装为 user 消息
                    let block = tool_result_block
                        .take()
                        .expect("tool_result_block 在未合并时必然存在");
                    out.push(json!({
                        "role": "user",
                        "content": [block],
                    }));
                }
            }
            _ => {
                // 未知角色：原样保留
                out.push(msg);
            }
        }
    }

    Ok(out)
}

/// 转换 OpenAI `user` 消息为 Anthropic `user` 消息
///
/// - content 字符串 → `[{type:"text", text}]`
/// - content 数组 → 逐项转换（text 保留，image_url → image）
fn transform_openai_user_to_anthropic(
    msg: &Value,
    index: usize,
) -> Result<Value, BridgeError> {
    let mut new_msg = serde_json::Map::new();
    new_msg.insert("role".to_string(), Value::String("user".to_string()));

    let mut new_content: Vec<Value> = Vec::new();
    if let Some(content) = msg.get("content") {
        match content {
            Value::String(s) => {
                if !s.is_empty() {
                    new_content.push(json!({"type": "text", "text": s}));
                }
            }
            Value::Array(parts) => {
                for part in parts {
                    let part_type = part.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match part_type {
                        "text" => new_content.push(part.clone()),
                        "image_url" => {
                            if let Some(img) = openai_image_url_to_anthropic_image(part) {
                                new_content.push(img);
                            }
                        }
                        _ => new_content.push(part.clone()),
                    }
                }
            }
            Value::Null => {}
            _ => {
                return Err(BridgeError::invalid_field(
                    format!("messages[{}].content", index),
                    "content 必须是字符串或数组",
                ));
            }
        }
    }

    if new_content.is_empty() {
        new_msg.insert("content".to_string(), Value::String(String::new()));
    } else if new_content.len() == 1
        && new_content[0].get("type").and_then(|v| v.as_str()) == Some("text")
    {
        // 单个 text block 时也保留数组形式，Anthropic 习惯
        new_msg.insert("content".to_string(), Value::Array(new_content));
    } else {
        new_msg.insert("content".to_string(), Value::Array(new_content));
    }

    // 移除 OpenAI 的 name 字段（Anthropic 不支持）
    new_msg.remove("name");
    Ok(Value::Object(new_msg))
}

/// 转换 OpenAI `assistant` 消息为 Anthropic `assistant` 消息
///
/// - content 字符串 → `[{type:"text", text}]`
/// - tool_calls 数组 → content 中的 `tool_use` blocks
fn transform_openai_assistant_to_anthropic(
    msg: &Value,
    index: usize,
) -> Result<Value, BridgeError> {
    let mut new_msg = serde_json::Map::new();
    new_msg.insert(
        "role".to_string(),
        Value::String("assistant".to_string()),
    );

    let mut new_content: Vec<Value> = Vec::new();

    if let Some(content) = msg.get("content") {
        match content {
            Value::String(s) => {
                if !s.is_empty() {
                    new_content.push(json!({"type": "text", "text": s}));
                }
            }
            Value::Array(parts) => {
                for part in parts {
                    let part_type = part.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match part_type {
                        "text" => new_content.push(part.clone()),
                        "image_url" => {
                            if let Some(img) = openai_image_url_to_anthropic_image(part) {
                                new_content.push(img);
                            }
                        }
                        _ => new_content.push(part.clone()),
                    }
                }
            }
            Value::Null => {}
            _ => {
                return Err(BridgeError::invalid_field(
                    format!("messages[{}].content", index),
                    "content 必须是字符串或数组",
                ));
            }
        }
    }

    // 转换 tool_calls
    if let Some(tool_calls) = msg.get("tool_calls").and_then(|v| v.as_array()) {
        for (j, tc) in tool_calls.iter().enumerate() {
            let id = tc
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    BridgeError::invalid_field(
                        format!("messages[{}].tool_calls[{}].id", index, j),
                        "tool_call 缺少 id",
                    )
                })?
                .to_string();
            let func = tc.get("function").ok_or_else(|| {
                BridgeError::invalid_field(
                    format!("messages[{}].tool_calls[{}].function", index, j),
                    "tool_call 缺少 function 字段",
                )
            })?;
            let name = func
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    BridgeError::invalid_field(
                        format!("messages[{}].tool_calls[{}].function.name", index, j),
                        "function 缺少 name",
                    )
                })?
                .to_string();
            // arguments 是 JSON 字符串，需解析回对象
            let arguments_str = func.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
            let input: Value = serde_json::from_str(arguments_str).unwrap_or_else(|_| {
                tracing::warn!(
                    target: "i_code::bridge",
                    field = format!("messages[{}].tool_calls[{}].function.arguments", index, j),
                    "解析 tool_call.arguments 为 JSON 失败，回退为空对象",
                );
                json!({})
            });
            new_content.push(json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input,
            }));
        }
    }

    if new_content.is_empty() {
        new_msg.insert("content".to_string(), Value::String(String::new()));
    } else {
        new_msg.insert("content".to_string(), Value::Array(new_content));
    }

    new_msg.remove("name");
    Ok(Value::Object(new_msg))
}

/// 把 OpenAI `role:"tool"` 消息构造为 Anthropic `tool_result` block
fn build_tool_result_from_openai_tool_msg(
    msg: &Value,
    index: usize,
) -> Result<Value, BridgeError> {
    let tool_call_id = msg
        .get("tool_call_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            BridgeError::invalid_field(
                format!("messages[{}].tool_call_id", index),
                "tool 消息缺少 tool_call_id",
            )
        })?
        .to_string();
    let content_str = match msg.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    };
    Ok(json!({
        "type": "tool_result",
        "tool_use_id": tool_call_id,
        "content": content_str,
    }))
}

/// 把 OpenAI `image_url` block 转换为 Anthropic `image` block
///
/// - `data:{media_type};base64,{data}` → `{source:{type:"base64", media_type, data}}`
/// - `http(s)://...` → `{source:{type:"url", url}}`
fn openai_image_url_to_anthropic_image(block: &Value) -> Option<Value> {
    let url = block
        .get("image_url")
        .and_then(|v| v.get("url"))
        .and_then(|v| v.as_str())?;
    if url.is_empty() {
        return None;
    }
    if let Some(rest) = url.strip_prefix("data:") {
        // data:{media_type};base64,{data}
        if let Some((meta, data)) = rest.split_once(",") {
            if let Some(media_type) = meta.strip_suffix(";base64") {
                if media_type.is_empty() || data.is_empty() {
                    return None;
                }
                return Some(json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": media_type,
                        "data": data,
                    }
                }));
            }
        }
        return None;
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        return Some(json!({
            "type": "image",
            "source": {
                "type": "url",
                "url": url,
            }
        }));
    }
    None
}

/// 转换 OpenAI `tools` 为 Anthropic `tools` 结构
fn transform_openai_tools_to_anthropic(obj: &mut serde_json::Map<String, Value>) {
    let Some(tools) = obj.remove("tools") else {
        return;
    };
    let Some(arr) = tools.as_array() else {
        obj.insert("tools".to_string(), tools);
        return;
    };
    let new_tools: Vec<Value> = arr
        .iter()
        .map(|tool| {
            let func = tool.get("function").cloned().unwrap_or_else(|| json!({}));
            let name = func.get("name").cloned().unwrap_or(Value::Null);
            let description = func.get("description").cloned().unwrap_or(Value::Null);
            let input_schema = func
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| json!({}));
            json!({
                "name": name,
                "description": description,
                "input_schema": input_schema,
            })
        })
        .collect();
    obj.insert("tools".to_string(), Value::Array(new_tools));
}

/// 转换 OpenAI `tool_choice` 为 Anthropic `tool_choice`
///
/// - `"auto"` → `{type:"auto"}`
/// - `"required"` → `{type:"any"}`
/// - `"none"` → `{type:"auto"}`（Anthropic 无 none，使用 auto 近似）
/// - `{type:"function", function:{name}}` → `{type:"tool", name}`
fn transform_openai_tool_choice_to_anthropic(obj: &mut serde_json::Map<String, Value>) {
    let Some(tc) = obj.remove("tool_choice") else {
        return;
    };
    let new_tc = match tc {
        Value::String(s) => match s.as_str() {
            "auto" => json!({"type": "auto"}),
            "required" => json!({"type": "any"}),
            "none" => json!({"type": "auto"}),
            _ => json!({"type": "auto"}),
        },
        Value::Object(map) => {
            let tc_type = map
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if tc_type == "function" {
                let name = map
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .cloned()
                    .unwrap_or(Value::Null);
                json!({"type": "tool", "name": name})
            } else {
                // 未知结构：原样保留
                Value::Object(map)
            }
        }
        other => other,
    };
    obj.insert("tool_choice".to_string(), new_tc);
}

/// 执行 O→A 字段重命名：`stop` → `stop_sequences`（单值包装为数组），`user` → `metadata.user_id`
fn rename_fields_openai_to_anthropic(obj: &mut serde_json::Map<String, Value>) {
    if let Some(stop) = obj.remove("stop") {
        let stop_arr = match stop {
            Value::String(s) => Value::Array(vec![Value::String(s)]),
            Value::Array(arr) => Value::Array(arr),
            other => other,
        };
        obj.insert("stop_sequences".to_string(), stop_arr);
    }
    if let Some(user) = obj.remove("user") {
        if let Some(s) = user.as_str() {
            obj.insert(
                "metadata".to_string(),
                json!({"user_id": s}),
            );
        }
    }
}

/// 应用 `max_tokens` 兜底逻辑（§7.2）
///
/// 当 `max_tokens` 缺失时按以下顺序解析：
/// 1. 调用方传入的 `max_output_tokens`（来自 `model_configs.max_output_tokens`）
/// 2. 兜底常量 [`MAX_TOKENS_FALLBACK`]
fn apply_max_tokens_fallback(obj: &mut serde_json::Map<String, Value>, max_output_tokens: Option<i64>) {
    let has_max_tokens = obj
        .get("max_tokens")
        .map(|v| !v.is_null())
        .unwrap_or(false);
    if has_max_tokens {
        return;
    }
    let value = max_output_tokens.unwrap_or(MAX_TOKENS_FALLBACK);
    obj.insert("max_tokens".to_string(), Value::Number(value.into()));
}

/// 构造 `response_format` 的 system prompt 注入文本（§7.4）
///
/// 返回 `Some(prompt)` 时调用方应将其追加到 system 末尾；
/// 返回 `None` 表示无 `response_format` 或类型不支持注入。
///
/// 注意：此函数仅读取 `response_format`，不移除字段。调用方需在适当时机移除。
fn build_response_format_prompt(obj: &serde_json::Map<String, Value>) -> Option<String> {
    let rf = obj.get("response_format")?;
    let rf_type = rf.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match rf_type {
        "json_object" => Some(
            "\n\nPlease respond with valid JSON only, without any additional text or markdown formatting."
                .to_string(),
        ),
        "json_schema" => {
            let schema = rf
                .get("json_schema")
                .and_then(|v| v.get("schema"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            let schema_str = serde_json::to_string(&schema).unwrap_or_else(|_| "{}".to_string());
            Some(format!(
                "\n\nPlease respond with valid JSON matching the following schema, without any additional text or markdown formatting:\n{}",
                schema_str
            ))
        }
        _ => None,
    }
}

/// 转换 `reasoning_effort` → `thinking.budget_tokens`（§3.5 固定映射表）
///
/// | OpenAI reasoning_effort | Anthropic thinking.budget_tokens |
/// |-------------------------|-----------------------------------|
/// | `minimal` | 1024 |
/// | `low` | 2048 |
/// | `medium` | 4096 |
/// | `high` | 8192 |
fn convert_reasoning_effort_to_thinking(obj: &mut serde_json::Map<String, Value>) {
    let Some(effort) = obj.remove("reasoning_effort") else {
        return;
    };
    let budget = match effort.as_str() {
        Some("minimal") => 1024,
        Some("low") => 2048,
        Some("medium") => 4096,
        Some("high") => 8192,
        _ => {
            // 未知值：不注入 thinking
            return;
        }
    };
    obj.insert(
        "thinking".to_string(),
        json!({"type": "enabled", "budget_tokens": budget}),
    );
}

/// 确保消息的 content 字段是数组形式（用于向 assistant 消息追加 tool_result）
fn ensure_content_is_array(msg: &mut Value) {
    if let Some(obj) = msg.as_object_mut() {
        let current = obj.get("content").cloned().unwrap_or(Value::Null);
        let new_content = match current {
            Value::Array(arr) => Value::Array(arr),
            Value::String(s) => {
                if s.is_empty() {
                    Value::Array(vec![])
                } else {
                    Value::Array(vec![json!({"type": "text", "text": s})])
                }
            }
            Value::Null => Value::Array(vec![]),
            other => Value::Array(vec![other]),
        };
        obj.insert("content".to_string(), new_content);
    }
}
