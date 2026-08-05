//! # 流式响应双向转换状态机
//!
//! 在 P3 阶段（设计文档 §5.4 / §7.6 / §8.2）实现：
//!
//! - [`openai_sse_to_anthropic`]：A→O 桥接的流式响应（上游 OpenAI SSE → 入口 Anthropic SSE）
//! - [`anthropic_sse_to_openai`]：O→A 桥接的流式响应（上游 Anthropic SSE → 入口 OpenAI SSE）
//!
//! ## 关键约束
//!
//! - §5.4.1 / §5.4.2 转换规则
//! - §5.4.3 usage 提取：状态机内部累积 usage 用于构造目标协议的 usage 字段；
//!   `parse_sse_event_for_usage` 仍按上游协议解析 usage（与流事件格式一致）
//! - §7.6 容错策略：解析失败的事件原样透传（按字节流输出）+ tracing::warn!
//! - §7.8 tracing::debug!(target: "i_code::bridge", ...) 输出转换前后 chunk
//! - §7.10 工具调用 ID 原样透传
//! - AGENTS.md §10.1：桥接场景下需重新构造 SSE 字节流，但仍保持
//!   Content-Type: text/event-stream 与 Cache-Control: no-cache（由 build_response 设置）
//!
//! ## 状态机字段
//!
//! [`BridgeStreamState`] 内部维护跨 chunk 的行缓冲与双向转换所需的状态字段：
//!
//! - O→A 状态（[`openai_sse_to_anthropic`]）：是否已发 `message_start`、当前
//!   content_block 索引、是否有打开的 block、累积 output_tokens、累积 input_tokens
//! - A→O 状态（[`anthropic_sse_to_openai`]）：input_tokens 累积、output_tokens 累积、
//!   是否已发首个 `delta.role:assistant`、当前 tool_calls index、message id/model

use serde_json::{json, Value};

/// 桥接流式状态机
///
/// 维护 SSE chunk 跨边界的行缓冲，以及双向转换所需的状态字段。
/// `o2a_*` 字段用于 [`openai_sse_to_anthropic`]，`a2o_*` 字段用于 [`anthropic_sse_to_openai`]。
///
/// 同一状态机实例在同一流中只用于一个方向（由调用方根据 [`BridgeKind`] 选择），
/// 因此两个方向的字段互不干扰。
#[derive(Debug, Default)]
pub struct BridgeStreamState {
    /// 行缓冲：跨 chunk 的事件分割（按 `\n\n` 分隔完整事件）
    line_buf: String,
    // ===== O→A 状态（OpenAI SSE → Anthropic SSE）=====
    /// 是否已发 `message_start`
    o2a_message_started: bool,
    /// 当前 content_block 索引（text=0，tool_use 从 1 开始递增）
    o2a_block_index: i64,
    /// 是否有打开的 content_block（用于判断是否需要发 content_block_stop）
    o2a_block_open: bool,
    /// 累积的 output_tokens（从 OpenAI 末尾 usage.completion_tokens 提取）
    o2a_output_tokens: i64,
    /// 累积的 input_tokens（从 OpenAI 末尾 usage.prompt_tokens 提取）
    o2a_input_tokens: i64,
    // ===== A→O 状态（Anthropic SSE → OpenAI SSE）=====
    /// 累积的 input_tokens（从 message_start.message.usage.input_tokens 提取）
    a2o_input_tokens: i64,
    /// 累积的 output_tokens（从 message_delta.usage.output_tokens 提取）
    a2o_output_tokens: i64,
    /// 是否已发首个 `delta.role:"assistant"`
    a2o_role_sent: bool,
    /// 当前 tool_calls index（用于 input_json_delta 累积，对应 Anthropic content_block.index）
    a2o_current_tool_index: Option<i64>,
    /// message id（从 Anthropic message_start.message.id 提取，用于 OpenAI chunk id）
    a2o_message_id: Option<String>,
    /// model（从 Anthropic message_start.message.model 提取）
    a2o_model: Option<String>,
}

impl BridgeStreamState {
    /// 创建新的桥接流式状态机
    pub fn new() -> Self {
        Self::default()
    }
}

// ===== 公开转换 API =====

/// OpenAI SSE → Anthropic SSE（A→O 桥接的流式响应）
///
/// 输入：上游 OpenAI SSE chunk（可能包含多个事件或部分事件）
/// 输出：转换后的 Anthropic SSE 事件字符串列表（每个已格式化为
///   `event: xxx\ndata: {...}\n\n`）
///
/// 转换规则见设计文档 §5.4.1。容错策略见 §7.6：解析失败的事件原样透传。
///
/// 状态机维护行缓冲，调用方对每个上游 chunk 调用本函数即可，无需关心事件边界。
pub fn openai_sse_to_anthropic(chunk: &str, state: &mut BridgeStreamState) -> Vec<String> {
    state.line_buf.push_str(chunk);
    let mut outputs: Vec<String> = Vec::new();
    while let Some(pos) = state.line_buf.find("\n\n") {
        let event_text = state.line_buf[..pos].to_string();
        state.line_buf = state.line_buf[pos + 2..].to_string();
        let trimmed = event_text.trim();
        if trimmed.is_empty() {
            continue;
        }
        match convert_openai_event_to_anthropic(trimmed, state) {
            Ok(events) => outputs.extend(events),
            Err(e) => {
                tracing::warn!(
                    target: "i_code::bridge",
                    kind = "O→A",
                    error = %e,
                    event = %trimmed,
                    "SSE 事件解析失败，原样透传"
                );
                // 原样透传（保留 \n\n 分隔）
                outputs.push(format!("{}\n\n", trimmed));
            }
        }
    }
    if !outputs.is_empty() {
        tracing::debug!(
            target: "i_code::bridge",
            kind = "O→A",
            category = "stream",
            chunk_size = chunk.len(),
            output_count = outputs.len(),
            "bridge stream chunk transformed"
        );
    }
    outputs
}

/// Anthropic SSE → OpenAI SSE（O→A 桥接的流式响应）
///
/// 输入：上游 Anthropic SSE chunk
/// 输出：转换后的 OpenAI SSE 事件字符串列表（每个已格式化为 `data: {...}\n\n`）
///
/// 转换规则见设计文档 §5.4.2。容错策略见 §7.6。
pub fn anthropic_sse_to_openai(chunk: &str, state: &mut BridgeStreamState) -> Vec<String> {
    state.line_buf.push_str(chunk);
    let mut outputs: Vec<String> = Vec::new();
    while let Some(pos) = state.line_buf.find("\n\n") {
        let event_text = state.line_buf[..pos].to_string();
        state.line_buf = state.line_buf[pos + 2..].to_string();
        let trimmed = event_text.trim();
        if trimmed.is_empty() {
            continue;
        }
        match convert_anthropic_event_to_openai(trimmed, state) {
            Ok(events) => outputs.extend(events),
            Err(e) => {
                tracing::warn!(
                    target: "i_code::bridge",
                    kind = "A→O",
                    error = %e,
                    event = %trimmed,
                    "SSE 事件解析失败，原样透传"
                );
                outputs.push(format!("{}\n\n", trimmed));
            }
        }
    }
    if !outputs.is_empty() {
        tracing::debug!(
            target: "i_code::bridge",
            kind = "A→O",
            category = "stream",
            chunk_size = chunk.len(),
            output_count = outputs.len(),
            "bridge stream chunk transformed"
        );
    }
    outputs
}

// ===== OpenAI → Anthropic 内部转换 =====

/// 将单个 OpenAI SSE 事件文本转换为 Anthropic SSE 事件列表
///
/// 返回 `Err(String)` 表示解析失败，调用方按 §7.6 容错策略原样透传。
fn convert_openai_event_to_anthropic(
    event_text: &str,
    state: &mut BridgeStreamState,
) -> Result<Vec<String>, String> {
    let data_str = extract_first_data_line(event_text)
        .ok_or_else(|| "缺少 data: 行".to_string())?;

    // [DONE]：已发 message_stop，忽略
    if data_str == "[DONE]" {
        return Ok(Vec::new());
    }

    let val: Value =
        serde_json::from_str(&data_str).map_err(|e| format!("JSON 解析失败: {}", e))?;

    // 从 chunk 顶层提取 id / model（用于 message_start）
    let chunk_id = val.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
    let chunk_model = val
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // 累积 usage（OpenAI 在末尾 delta 中带 usage）
    if let Some(usage) = val.get("usage") {
        if let Some(pt) = usage.get("prompt_tokens").and_then(|v| v.as_i64()) {
            state.o2a_input_tokens = pt;
        }
        if let Some(ct) = usage.get("completion_tokens").and_then(|v| v.as_i64()) {
            state.o2a_output_tokens = ct;
        }
    }

    let mut outputs: Vec<String> = Vec::new();

    // 处理 choices[0]
    let choices = val
        .get("choices")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if let Some(choice) = choices.into_iter().next() {
        let delta = choice.get("delta");
        let finish_reason = choice
            .get("finish_reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // delta.role:"assistant" 触发 message_start + content_block_start
        if let Some(delta) = delta {
            // 首个 role:assistant 触发 message_start
            if let Some(role) = delta.get("role").and_then(|v| v.as_str()) {
                if role == "assistant" && !state.o2a_message_started {
                    outputs.push(make_message_start(
                        chunk_id.as_deref(),
                        chunk_model.as_deref(),
                    ));
                    state.o2a_message_started = true;
                    // 同时发 content_block_start (text, index=0)
                    outputs.push(make_content_block_start_text(state.o2a_block_index));
                    state.o2a_block_open = true;
                }
            }

            // delta.content → content_block_delta (text_delta)
            if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                // 防御：如果首个 delta 没有 role:assistant，自动补发 message_start
                if !state.o2a_message_started {
                    outputs.push(make_message_start(
                        chunk_id.as_deref(),
                        chunk_model.as_deref(),
                    ));
                    state.o2a_message_started = true;
                    outputs.push(make_content_block_start_text(state.o2a_block_index));
                    state.o2a_block_open = true;
                }
                outputs.push(make_content_block_delta_text(state.o2a_block_index, content));
            }

            // delta.tool_calls
            if let Some(tool_calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                for tc in tool_calls {
                    process_openai_tool_call_delta(tc, state, &mut outputs);
                }
            }
        }

        // finish_reason 触发 content_block_stop + message_delta + message_stop
        if let Some(reason) = finish_reason {
            // 关闭当前 block
            if state.o2a_block_open {
                outputs.push(make_content_block_stop(state.o2a_block_index));
                state.o2a_block_open = false;
            }
            let stop_reason = finish_reason_to_stop_reason_o2a(&reason);
            outputs.push(make_message_delta(stop_reason, state.o2a_output_tokens));
            outputs.push(make_message_stop());
        }
    }

    Ok(outputs)
}

/// 处理 OpenAI delta.tool_calls 数组中单个 tool_call 增量
///
/// - 出现 `id` + `function.name`：关闭上一个 block，发新的 content_block_start (tool_use)
/// - 出现 `function.arguments` 增量：发 content_block_delta (input_json_delta)
fn process_openai_tool_call_delta(
    tc: &Value,
    state: &mut BridgeStreamState,
    outputs: &mut Vec<String>,
) {
    let func = tc.get("function");
    let id = tc.get("id").and_then(|v| v.as_str());
    let name = func
        .and_then(|f| f.get("name"))
        .and_then(|v| v.as_str());
    let arguments = func
        .and_then(|f| f.get("arguments"))
        .and_then(|v| v.as_str());

    // id + name 出现：新 tool_use block
    if let (Some(id), Some(name)) = (id, name) {
        // 关闭当前 block（如果有）
        if state.o2a_block_open {
            outputs.push(make_content_block_stop(state.o2a_block_index));
            state.o2a_block_open = false;
        }
        state.o2a_block_index += 1;
        outputs.push(make_content_block_start_tool_use(
            state.o2a_block_index,
            id,
            name,
        ));
        state.o2a_block_open = true;
    }

    // arguments 增量 → input_json_delta
    if let Some(args) = arguments {
        if !args.is_empty() {
            outputs.push(make_content_block_delta_input_json(
                state.o2a_block_index,
                args,
            ));
        }
    }
}

// ===== Anthropic → OpenAI 内部转换 =====

/// 将单个 Anthropic SSE 事件文本转换为 OpenAI SSE 事件列表
fn convert_anthropic_event_to_openai(
    event_text: &str,
    state: &mut BridgeStreamState,
) -> Result<Vec<String>, String> {
    let data_str = extract_first_data_line(event_text)
        .ok_or_else(|| "缺少 data: 行".to_string())?;

    let val: Value =
        serde_json::from_str(&data_str).map_err(|e| format!("JSON 解析失败: {}", e))?;

    let event_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let mut outputs: Vec<String> = Vec::new();

    // OpenAI chunk 时间戳（Anthropic 不提供，用当前时间）
    let created = chrono::Utc::now().timestamp();

    match event_type {
        "message_start" => {
            // 不发，但记录 input_tokens / id / model
            if let Some(msg) = val.get("message") {
                if let Some(id) = msg.get("id").and_then(|v| v.as_str()) {
                    state.a2o_message_id = Some(id.to_string());
                }
                if let Some(model) = msg.get("model").and_then(|v| v.as_str()) {
                    state.a2o_model = Some(model.to_string());
                }
                if let Some(usage) = msg.get("usage") {
                    if let Some(it) = usage.get("input_tokens").and_then(|v| v.as_i64()) {
                        state.a2o_input_tokens = it;
                    }
                }
            }
        }
        "content_block_start" => {
            let index = val.get("index").and_then(|v| v.as_i64()).unwrap_or(0);
            let block_type = val
                .get("content_block")
                .and_then(|b| b.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match block_type {
                "text" => {
                    // 首个 text block：发 delta.role:assistant
                    if !state.a2o_role_sent {
                        outputs.push(make_openai_chunk(
                            state,
                            created,
                            json!({"role": "assistant"}),
                            Value::Null,
                        ));
                        state.a2o_role_sent = true;
                    }
                }
                "tool_use" => {
                    let id = val
                        .get("content_block")
                        .and_then(|b| b.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = val
                        .get("content_block")
                        .and_then(|b| b.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    // 首次发 tool_call 前如果没发过 role:assistant，先发
                    if !state.a2o_role_sent {
                        outputs.push(make_openai_chunk(
                            state,
                            created,
                            json!({"role": "assistant"}),
                            Value::Null,
                        ));
                        state.a2o_role_sent = true;
                    }
                    outputs.push(make_openai_chunk(
                        state,
                        created,
                        json!({
                            "tool_calls": [{
                                "index": index,
                                "id": id,
                                "type": "function",
                                "function": {"name": name, "arguments": ""}
                            }]
                        }),
                        Value::Null,
                    ));
                    state.a2o_current_tool_index = Some(index);
                }
                _ => {}
            }
        }
        "content_block_delta" => {
            let index = val.get("index").and_then(|v| v.as_i64()).unwrap_or(0);
            let delta = val.get("delta");
            let delta_type = delta
                .and_then(|d| d.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match delta_type {
                "text_delta" => {
                    let text = delta
                        .and_then(|d| d.get("text"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    outputs.push(make_openai_chunk(
                        state,
                        created,
                        json!({"content": text}),
                        Value::Null,
                    ));
                }
                "input_json_delta" => {
                    let partial = delta
                        .and_then(|d| d.get("partial_json"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let tool_index = state.a2o_current_tool_index.unwrap_or(index);
                    outputs.push(make_openai_chunk(
                        state,
                        created,
                        json!({
                            "tool_calls": [{
                                "index": tool_index,
                                "function": {"arguments": partial}
                            }]
                        }),
                        Value::Null,
                    ));
                }
                _ => {}
            }
        }
        "content_block_stop" => {
            // 忽略
        }
        "message_delta" => {
            // 累积 output_tokens
            if let Some(usage) = val.get("usage") {
                if let Some(ot) = usage.get("output_tokens").and_then(|v| v.as_i64()) {
                    state.a2o_output_tokens = ot;
                }
            }
            // stop_reason → finish_reason
            let stop_reason = val
                .get("delta")
                .and_then(|d| d.get("stop_reason"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if let Some(sr) = stop_reason {
                let finish_reason = stop_reason_to_finish_reason_a2o(&sr);
                outputs.push(make_openai_chunk(
                    state,
                    created,
                    json!({}),
                    Value::String(finish_reason.to_string()),
                ));
            }
        }
        "message_stop" => {
            // 末尾发带 usage 的 delta + [DONE]
            let total = state.a2o_input_tokens + state.a2o_output_tokens;
            let usage = json!({
                "prompt_tokens": state.a2o_input_tokens,
                "completion_tokens": state.a2o_output_tokens,
                "total_tokens": total
            });
            // OpenAI 末尾 usage chunk：choices 为空数组
            let mut chunk = json!({
                "id": state.a2o_message_id.clone().unwrap_or_default(),
                "object": "chat.completion.chunk",
                "created": created,
                "model": state.a2o_model.clone().unwrap_or_default(),
                "choices": []
            });
            chunk["usage"] = usage;
            outputs.push(format_openai_chunk(chunk));
            outputs.push("data: [DONE]\n\n".to_string());
        }
        _ => {}
    }

    Ok(outputs)
}

// ===== 辅助：OpenAI → Anthropic 事件构造 =====

fn make_message_start(id: Option<&str>, model: Option<&str>) -> String {
    let message = json!({
        "id": id.unwrap_or(""),
        "type": "message",
        "role": "assistant",
        "content": [],
        "model": model.unwrap_or(""),
        "stop_reason": Value::Null,
        "usage": {"input_tokens": 0, "output_tokens": 0}
    });
    format_anthropic_event("message_start", json!({"type": "message_start", "message": message}))
}

fn make_content_block_start_text(index: i64) -> String {
    format_anthropic_event(
        "content_block_start",
        json!({
            "type": "content_block_start",
            "index": index,
            "content_block": {"type": "text", "text": ""}
        }),
    )
}

fn make_content_block_start_tool_use(index: i64, id: &str, name: &str) -> String {
    format_anthropic_event(
        "content_block_start",
        json!({
            "type": "content_block_start",
            "index": index,
            "content_block": {
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": {}
            }
        }),
    )
}

fn make_content_block_delta_text(index: i64, text: &str) -> String {
    format_anthropic_event(
        "content_block_delta",
        json!({
            "type": "content_block_delta",
            "index": index,
            "delta": {"type": "text_delta", "text": text}
        }),
    )
}

fn make_content_block_delta_input_json(index: i64, partial_json: &str) -> String {
    format_anthropic_event(
        "content_block_delta",
        json!({
            "type": "content_block_delta",
            "index": index,
            "delta": {"type": "input_json_delta", "partial_json": partial_json}
        }),
    )
}

fn make_content_block_stop(index: i64) -> String {
    format_anthropic_event(
        "content_block_stop",
        json!({"type": "content_block_stop", "index": index}),
    )
}

fn make_message_delta(stop_reason: &str, output_tokens: i64) -> String {
    format_anthropic_event(
        "message_delta",
        json!({
            "type": "message_delta",
            "delta": {"stop_reason": stop_reason},
            "usage": {"output_tokens": output_tokens}
        }),
    )
}

fn make_message_stop() -> String {
    format_anthropic_event("message_stop", json!({"type": "message_stop"}))
}

/// OpenAI `finish_reason` → Anthropic `stop_reason`（§3.6 映射表，O→A 方向）
fn finish_reason_to_stop_reason_o2a(finish_reason: &str) -> &'static str {
    match finish_reason {
        "stop" => "end_turn",
        "length" => "max_tokens",
        "tool_calls" => "tool_use",
        "content_filter" => "end_turn",
        _ => "end_turn",
    }
}

// ===== 辅助：Anthropic → OpenAI 事件构造 =====

/// 构造 OpenAI chat.completion.chunk
///
/// `delta` 为 delta 对象（如 `{"role":"assistant"}` / `{"content":"..."}` / `{"tool_calls":[...]}`）。
/// `finish_reason` 为 `Value::Null`（未结束）或 `Value::String(...)`（结束）。
fn make_openai_chunk(
    state: &BridgeStreamState,
    created: i64,
    delta: Value,
    finish_reason: Value,
) -> String {
    let chunk = json!({
        "id": state.a2o_message_id.clone().unwrap_or_default(),
        "object": "chat.completion.chunk",
        "created": created,
        "model": state.a2o_model.clone().unwrap_or_default(),
        "choices": [{
            "index": 0,
            "delta": delta,
            "finish_reason": finish_reason
        }]
    });
    format_openai_chunk(chunk)
}

/// Anthropic `stop_reason` → OpenAI `finish_reason`（§3.6 映射表，A→O 方向）
fn stop_reason_to_finish_reason_a2o(stop_reason: &str) -> &'static str {
    match stop_reason {
        "end_turn" => "stop",
        "max_tokens" => "length",
        "tool_use" => "tool_calls",
        "stop_sequence" => "stop",
        _ => "stop",
    }
}

// ===== 辅助：SSE 事件格式化 =====

/// 从 SSE 事件文本中提取首个 `data:` 行的内容（去前缀、去首尾空白）
fn extract_first_data_line(event_text: &str) -> Option<String> {
    for line in event_text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("data:") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// 格式化 Anthropic SSE 事件：`event: {type}\ndata: {json}\n\n`
fn format_anthropic_event(event_type: &str, data: Value) -> String {
    format!("event: {}\ndata: {}\n\n", event_type, data)
}

/// 格式化 OpenAI SSE 事件：`data: {json}\n\n`
fn format_openai_chunk(data: Value) -> String {
    format!("data: {}\n\n", data)
}

// ===== 单元测试 =====

#[cfg(test)]
mod tests {
    use super::*;

    // ===== OpenAI SSE → Anthropic SSE（§5.4.1）=====

    /// 完整文本流：role + content + finish_reason + [DONE]
    #[test]
    fn test_openai_to_anthropic_text_stream() {
        let mut state = BridgeStreamState::new();
        // 首个 delta：role:assistant
        let chunk1 = r#"data: {"id":"chatcmpl-1","object":"chat.completion.chunk","created":1000,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

"#;
        let out1 = openai_sse_to_anthropic(chunk1, &mut state);
        // 期望：message_start + content_block_start (text, index=0)
        assert_eq!(out1.len(), 2);
        assert!(out1[0].contains("event: message_start"));
        assert!(out1[1].contains("event: content_block_start"));
        assert!(out1[1].contains("\"type\":\"text\""));

        // 第二个 delta：content
        let chunk2 = r#"data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

"#;
        let out2 = openai_sse_to_anthropic(chunk2, &mut state);
        assert_eq!(out2.len(), 1);
        assert!(out2[0].contains("event: content_block_delta"));
        assert!(out2[0].contains("\"text_delta\""));
        assert!(out2[0].contains("\"text\":\"Hello\""));

        // 第三个 delta：content
        let chunk3 = r#"data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}

"#;
        let out3 = openai_sse_to_anthropic(chunk3, &mut state);
        assert_eq!(out3.len(), 1);
        assert!(out3[0].contains("\"text\":\" world\""));

        // 末尾 delta：finish_reason + usage
        let chunk4 = r#"data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12}}

"#;
        let out4 = openai_sse_to_anthropic(chunk4, &mut state);
        // 期望：content_block_stop + message_delta + message_stop
        assert_eq!(out4.len(), 3);
        assert!(out4[0].contains("event: content_block_stop"));
        assert!(out4[1].contains("event: message_delta"));
        assert!(out4[1].contains("\"stop_reason\":\"end_turn\""));
        assert!(out4[1].contains("\"output_tokens\":2"));
        assert!(out4[2].contains("event: message_stop"));

        // [DONE] 忽略
        let chunk5 = "data: [DONE]\n\n";
        let out5 = openai_sse_to_anthropic(chunk5, &mut state);
        assert!(out5.is_empty());
    }

    /// 工具调用流：role + tool_call(id+name) + arguments 增量 + finish_reason:tool_calls
    #[test]
    fn test_openai_to_anthropic_tool_call_stream() {
        let mut state = BridgeStreamState::new();

        // role:assistant
        let chunk1 = r#"data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

"#;
        let out1 = openai_sse_to_anthropic(chunk1, &mut state);
        assert_eq!(out1.len(), 2); // message_start + content_block_start text

        // tool_call 出现 id+name → 关闭 text block + 开 tool_use block
        let chunk2 = r#"data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"get_weather","arguments":""}}]},"finish_reason":null}]}

"#;
        let out2 = openai_sse_to_anthropic(chunk2, &mut state);
        // 期望：content_block_stop (text index=0) + content_block_start (tool_use index=1)
        assert_eq!(out2.len(), 2);
        assert!(out2[0].contains("event: content_block_stop"));
        assert!(out2[0].contains("\"index\":0"));
        assert!(out2[1].contains("event: content_block_start"));
        assert!(out2[1].contains("\"type\":\"tool_use\""));
        assert!(out2[1].contains("\"index\":1"));
        assert!(out2[1].contains("\"id\":\"call_abc\""));
        assert!(out2[1].contains("\"name\":\"get_weather\""));

        // arguments 增量 → input_json_delta
        let chunk3 = r#"data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"city\":"}}]},"finish_reason":null}]}

"#;
        let out3 = openai_sse_to_anthropic(chunk3, &mut state);
        assert_eq!(out3.len(), 1);
        assert!(out3[0].contains("event: content_block_delta"));
        assert!(out3[0].contains("\"input_json_delta\""));
        assert!(out3[0].contains("\"partial_json\":\"{\\\"city\\\":\""));

        // arguments 增量
        let chunk4 = r#"data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"SH\"}"}}]},"finish_reason":null}]}

"#;
        let out4 = openai_sse_to_anthropic(chunk4, &mut state);
        assert_eq!(out4.len(), 1);
        assert!(out4[0].contains("\"input_json_delta\""));

        // finish_reason:tool_calls
        let chunk5 = r#"data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":5,"completion_tokens":10,"total_tokens":15}}

"#;
        let out5 = openai_sse_to_anthropic(chunk5, &mut state);
        // 期望：content_block_stop (tool_use index=1) + message_delta (tool_use) + message_stop
        assert_eq!(out5.len(), 3);
        assert!(out5[0].contains("event: content_block_stop"));
        assert!(out5[0].contains("\"index\":1"));
        assert!(out5[1].contains("\"stop_reason\":\"tool_use\""));
        assert!(out5[1].contains("\"output_tokens\":10"));
    }

    /// finish_reason:length → stop_reason:max_tokens
    #[test]
    fn test_openai_to_anthropic_length_finish_reason() {
        let mut state = BridgeStreamState::new();
        let chunk = r#"data: {"id":"x","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

"#;
        let _ = openai_sse_to_anthropic(chunk, &mut state);

        let chunk = r#"data: {"id":"x","choices":[{"index":0,"delta":{},"finish_reason":"length"}]}

"#;
        let out = openai_sse_to_anthropic(chunk, &mut state);
        assert!(out[1].contains("\"stop_reason\":\"max_tokens\""));
    }

    /// 防御：首个 delta 没有 role:assistant，直接发 content
    #[test]
    fn test_openai_to_anthropic_first_delta_content_without_role() {
        let mut state = BridgeStreamState::new();
        let chunk = r#"data: {"id":"x","choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":null}]}

"#;
        let out = openai_sse_to_anthropic(chunk, &mut state);
        // 期望：自动补发 message_start + content_block_start + content_block_delta
        assert_eq!(out.len(), 3);
        assert!(out[0].contains("event: message_start"));
        assert!(out[1].contains("event: content_block_start"));
        assert!(out[2].contains("event: content_block_delta"));
    }

    // ===== Anthropic SSE → OpenAI SSE（§5.4.2）=====

    /// 完整文本流：message_start + content_block_start text + text_delta + content_block_stop + message_delta + message_stop
    #[test]
    fn test_anthropic_to_openai_text_stream() {
        let mut state = BridgeStreamState::new();

        // message_start：记录 input_tokens，不发
        let chunk1 = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"role\":\"assistant\",\"model\":\"claude-3-5\",\"usage\":{\"input_tokens\":10,\"output_tokens\":1}}}\n\n";
        let out1 = anthropic_sse_to_openai(chunk1, &mut state);
        assert!(out1.is_empty());
        assert_eq!(state.a2o_input_tokens, 10);
        assert_eq!(state.a2o_message_id.as_deref(), Some("msg_1"));
        assert_eq!(state.a2o_model.as_deref(), Some("claude-3-5"));

        // content_block_start text → delta.role:assistant
        let chunk2 = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
        let out2 = anthropic_sse_to_openai(chunk2, &mut state);
        assert_eq!(out2.len(), 1);
        assert!(out2[0].contains("\"role\":\"assistant\""));
        assert!(state.a2o_role_sent);

        // content_block_delta text_delta → delta.content
        let chunk3 = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n";
        let out3 = anthropic_sse_to_openai(chunk3, &mut state);
        assert_eq!(out3.len(), 1);
        assert!(out3[0].contains("\"content\":\"Hello\""));

        // content_block_stop：忽略
        let chunk4 = "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n";
        let out4 = anthropic_sse_to_openai(chunk4, &mut state);
        assert!(out4.is_empty());

        // message_delta stop_reason → delta.finish_reason
        let chunk5 = "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":2}}\n\n";
        let out5 = anthropic_sse_to_openai(chunk5, &mut state);
        assert_eq!(out5.len(), 1);
        assert!(out5[0].contains("\"finish_reason\":\"stop\""));
        assert_eq!(state.a2o_output_tokens, 2);

        // message_stop → 带 usage 的 delta + [DONE]
        let chunk6 = "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        let out6 = anthropic_sse_to_openai(chunk6, &mut state);
        assert_eq!(out6.len(), 2);
        assert!(out6[0].contains("\"prompt_tokens\":10"));
        assert!(out6[0].contains("\"completion_tokens\":2"));
        assert!(out6[0].contains("\"total_tokens\":12"));
        assert!(out6[0].contains("\"choices\":[]"));
        assert_eq!(out6[1], "data: [DONE]\n\n");
    }

    /// 工具调用流：content_block_start tool_use + input_json_delta + content_block_stop
    #[test]
    fn test_anthropic_to_openai_tool_call_stream() {
        let mut state = BridgeStreamState::new();

        // message_start
        let chunk1 = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude\",\"usage\":{\"input_tokens\":5,\"output_tokens\":1}}}\n\n";
        let _ = anthropic_sse_to_openai(chunk1, &mut state);

        // content_block_start tool_use → delta.tool_calls[index]={id, name, arguments:""}
        let chunk2 = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tool_abc\",\"name\":\"get_weather\",\"input\":{}}}\n\n";
        let out2 = anthropic_sse_to_openai(chunk2, &mut state);
        // 期望：先发 role:assistant，再发 tool_calls
        assert_eq!(out2.len(), 2);
        assert!(out2[0].contains("\"role\":\"assistant\""));
        assert!(out2[1].contains("\"tool_calls\""));
        assert!(out2[1].contains("\"index\":1"));
        assert!(out2[1].contains("\"id\":\"tool_abc\""));
        assert!(out2[1].contains("\"name\":\"get_weather\""));
        assert!(out2[1].contains("\"arguments\":\"\""));
        assert_eq!(state.a2o_current_tool_index, Some(1));

        // input_json_delta → delta.tool_calls[index].function.arguments
        let chunk3 = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"city\\\":\"}}\n\n";
        let out3 = anthropic_sse_to_openai(chunk3, &mut state);
        assert_eq!(out3.len(), 1);
        assert!(out3[0].contains("\"arguments\":\"{\\\"city\\\":\""));
        assert!(out3[0].contains("\"index\":1"));

        // content_block_stop：忽略
        let chunk4 = "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\n";
        let out4 = anthropic_sse_to_openai(chunk4, &mut state);
        assert!(out4.is_empty());

        // message_delta stop_reason:tool_use → finish_reason:tool_calls
        let chunk5 = "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":8}}\n\n";
        let out5 = anthropic_sse_to_openai(chunk5, &mut state);
        assert_eq!(out5.len(), 1);
        assert!(out5[0].contains("\"finish_reason\":\"tool_calls\""));

        // message_stop
        let chunk6 = "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        let out6 = anthropic_sse_to_openai(chunk6, &mut state);
        assert_eq!(out6.len(), 2);
        assert!(out6[0].contains("\"completion_tokens\":8"));
        assert_eq!(out6[1], "data: [DONE]\n\n");
    }

    /// stop_reason:max_tokens → finish_reason:length
    #[test]
    fn test_anthropic_to_openai_max_tokens() {
        let mut state = BridgeStreamState::new();
        // message_start
        let _ = anthropic_sse_to_openai(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"model\":\"c\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n\n",
            &mut state,
        );
        // content_block_start text → role:assistant
        let _ = anthropic_sse_to_openai(
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            &mut state,
        );
        // message_delta stop_reason:max_tokens
        let out = anthropic_sse_to_openai(
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"max_tokens\"},\"usage\":{\"output_tokens\":100}}\n\n",
            &mut state,
        );
        assert!(out[0].contains("\"finish_reason\":\"length\""));
    }

    // ===== 容错（§7.6）=====

    /// 解析失败的事件原样透传
    #[test]
    fn test_openai_to_anthropic_malformed_event_passthrough() {
        let mut state = BridgeStreamState::new();
        // 无效 JSON
        let chunk = "data: not-a-json\n\n";
        let out = openai_sse_to_anthropic(chunk, &mut state);
        assert_eq!(out.len(), 1);
        // 原样输出（保留 \n\n）
        assert!(out[0].contains("data: not-a-json"));
    }

    /// Anthropic 流中 malformed 事件原样透传
    #[test]
    fn test_anthropic_to_openai_malformed_event_passthrough() {
        let mut state = BridgeStreamState::new();
        let chunk = "data: {broken\n\n";
        let out = anthropic_sse_to_openai(chunk, &mut state);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("data: {broken"));
    }

    /// 缺少 data: 行的事件原样透传
    #[test]
    fn test_openai_to_anthropic_no_data_line_passthrough() {
        let mut state = BridgeStreamState::new();
        let chunk = "event: unknown\ndata: \n\n";
        let out = openai_sse_to_anthropic(chunk, &mut state);
        // data: 后内容为空 → JSON 解析失败 → 原样透传
        assert_eq!(out.len(), 1);
    }

    // ===== 跨 chunk 处理 =====

    /// 跨 chunk 的不完整事件由 line_buf 缓冲，完整后才处理
    #[test]
    fn test_cross_chunk_event_buffering() {
        let mut state = BridgeStreamState::new();
        // 第一个 chunk 不完整（没有 \n\n）
        let chunk1 = "data: {\"id\":\"x\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}";
        let out1 = openai_sse_to_anthropic(chunk1, &mut state);
        assert!(out1.is_empty()); // 不完整事件不输出

        // 第二个 chunk 补完 \n\n
        let chunk2 = "\n\n";
        let out2 = openai_sse_to_anthropic(chunk2, &mut state);
        assert_eq!(out2.len(), 2); // message_start + content_block_start
    }

    // ===== 工具调用 ID 不重命名（§7.10）=====

    #[test]
    fn test_tool_call_id_preserved_o2a() {
        let mut state = BridgeStreamState::new();
        let _ = openai_sse_to_anthropic(
            "data: {\"id\":\"x\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n",
            &mut state,
        );
        let out = openai_sse_to_anthropic(
            "data: {\"id\":\"x\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_xyz_123\",\"type\":\"function\",\"function\":{\"name\":\"fn\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\n",
            &mut state,
        );
        // 找到 content_block_start tool_use 事件，断言 id 原样保留
        let tool_use_event = out
            .iter()
            .find(|s| s.contains("\"type\":\"tool_use\""))
            .expect("应包含 tool_use content_block_start");
        assert!(tool_use_event.contains("\"id\":\"call_xyz_123\""));
    }

    #[test]
    fn test_tool_call_id_preserved_a2o() {
        let mut state = BridgeStreamState::new();
        let _ = anthropic_sse_to_openai(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"model\":\"c\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n\n",
            &mut state,
        );
        let out = anthropic_sse_to_openai(
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tool_abc_xyz\",\"name\":\"fn\",\"input\":{}}}\n\n",
            &mut state,
        );
        let tool_call_chunk = out
            .iter()
            .find(|s| s.contains("\"tool_calls\""))
            .expect("应包含 tool_calls delta");
        assert!(tool_call_chunk.contains("\"id\":\"tool_abc_xyz\""));
    }
}
