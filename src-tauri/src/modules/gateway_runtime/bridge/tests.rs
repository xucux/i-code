//! # 桥接模块单元测试
//!
//! 覆盖 [`docs/proposals/protocol-bridge.md`] §3 全部差异点与 §7 关键决策。
//!
//! 测试组织：
//! - `a_to_o_*`：Anthropic → OpenAI Chat 转换
//! - `o_to_a_*`：OpenAI Chat → Anthropic 转换
//! - `error_*`：错误路径
//!
//! [`docs/proposals/protocol-bridge.md`]: ../../../../../docs/proposals/protocol-bridge.md

use serde_json::{json, Value};

use super::request::{anthropic_to_openai_chat, openai_chat_to_anthropic};
use super::response::{
    anthropic_response_to_openai, convert_error_body, openai_response_to_anthropic,
};
use super::{detect_bridge, BridgeKind, MAX_TOKENS_FALLBACK};
use crate::modules::gateway_runtime::forwarding::context::GatewayProtocol;

// ===== 辅助函数 =====

fn into_obj(value: Value) -> serde_json::Map<String, Value> {
    match value {
        Value::Object(obj) => obj,
        _ => panic!("期望 JSON 对象，实际: {:?}", value),
    }
}

// ===== A→O: anthropic_to_openai_chat =====

#[test]
fn test_a_to_o_system_string_to_messages() {
    let mut body = json!({
        "model": "claude-3-5-sonnet",
        "system": "You are helpful",
        "messages": [
            {"role": "user", "content": "hi"}
        ],
        "max_tokens": 1024
    });
    anthropic_to_openai_chat(&mut body).unwrap();

    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[0]["content"], "You are helpful");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[1]["content"], "hi");
    assert!(body.get("system").is_none(), "system 字段应被移除");
}

#[test]
fn test_a_to_o_system_array_concatenated() {
    let mut body = json!({
        "system": [
            {"type": "text", "text": "Line 1"},
            {"type": "text", "text": "Line 2"}
        ],
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100
    });
    anthropic_to_openai_chat(&mut body).unwrap();

    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[0]["content"], "Line 1\nLine 2");
}

#[test]
fn test_a_to_o_assistant_tool_use_to_tool_calls() {
    let mut body = json!({
        "messages": [
            {"role": "user", "content": "weather?"},
            {
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "Let me check"},
                    {"type": "tool_use", "id": "call_abc", "name": "get_weather", "input": {"city": "SH"}}
                ]
            }
        ],
        "max_tokens": 100
    });
    anthropic_to_openai_chat(&mut body).unwrap();

    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    let assistant = &messages[1];
    assert_eq!(assistant["role"], "assistant");
    // content 应保留 text block（单个 text 扁平化为字符串）
    assert_eq!(assistant["content"], "Let me check");
    // tool_calls 提到顶层
    let tool_calls = assistant["tool_calls"].as_array().unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0]["id"], "call_abc");
    assert_eq!(tool_calls[0]["type"], "function");
    assert_eq!(tool_calls[0]["function"]["name"], "get_weather");
    // arguments 是 JSON 字符串
    assert_eq!(
        tool_calls[0]["function"]["arguments"].as_str().unwrap(),
        r#"{"city":"SH"}"#
    );
}

#[test]
fn test_a_to_o_user_tool_result_to_separate_tool_messages() {
    let mut body = json!({
        "messages": [
            {"role": "user", "content": "weather?"},
            {
                "role": "assistant",
                "content": [
                    {"type": "tool_use", "id": "call_1", "name": "f", "input": {}}
                ]
            },
            {
                "role": "user",
                "content": [
                    {"type": "tool_result", "tool_use_id": "call_1", "content": "result text"}
                ]
            }
        ],
        "max_tokens": 100
    });
    anthropic_to_openai_chat(&mut body).unwrap();

    let messages = body["messages"].as_array().unwrap();
    // user + assistant + tool = 3
    assert_eq!(messages.len(), 3);
    let tool_msg = &messages[2];
    assert_eq!(tool_msg["role"], "tool");
    assert_eq!(tool_msg["tool_call_id"], "call_1");
    assert_eq!(tool_msg["content"], "result text");
}

#[test]
fn test_a_to_o_image_base64_to_data_url() {
    let mut body = json!({
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "what is this?"},
                {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "iVBORw0K"}}
            ]
        }],
        "max_tokens": 100
    });
    anthropic_to_openai_chat(&mut body).unwrap();

    let content = body["messages"][0]["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content[1]["type"], "image_url");
    assert_eq!(
        content[1]["image_url"]["url"],
        "data:image/png;base64,iVBORw0K"
    );
}

#[test]
fn test_a_to_o_image_url_pass_through() {
    let mut body = json!({
        "messages": [{
            "role": "user",
            "content": [
                {"type": "image", "source": {"type": "url", "url": "https://example.com/x.png"}}
            ]
        }],
        "max_tokens": 100
    });
    anthropic_to_openai_chat(&mut body).unwrap();

    let content = body["messages"][0]["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "image_url");
    assert_eq!(
        content[0]["image_url"]["url"],
        "https://example.com/x.png"
    );
}

#[test]
fn test_a_to_o_tools_rename() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "tools": [{
            "name": "get_weather",
            "description": "Get weather",
            "input_schema": {"type": "object", "properties": {}}
        }]
    });
    anthropic_to_openai_chat(&mut body).unwrap();

    let tools = body["tools"].as_array().unwrap();
    assert_eq!(tools[0]["type"], "function");
    assert_eq!(tools[0]["function"]["name"], "get_weather");
    assert_eq!(tools[0]["function"]["description"], "Get weather");
    assert_eq!(
        tools[0]["function"]["parameters"],
        json!({"type": "object", "properties": {}})
    );
}

#[test]
fn test_a_to_o_tool_choice_variants() {
    // auto → "auto"
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "tool_choice": {"type": "auto"}
    });
    anthropic_to_openai_chat(&mut body).unwrap();
    assert_eq!(body["tool_choice"], "auto");

    // any → "required"
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "tool_choice": {"type": "any"}
    });
    anthropic_to_openai_chat(&mut body).unwrap();
    assert_eq!(body["tool_choice"], "required");

    // tool with name → {type:"function", function:{name}}
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "tool_choice": {"type": "tool", "name": "get_weather"}
    });
    anthropic_to_openai_chat(&mut body).unwrap();
    assert_eq!(body["tool_choice"]["type"], "function");
    assert_eq!(body["tool_choice"]["function"]["name"], "get_weather");
}

#[test]
fn test_a_to_o_stop_sequences_to_stop() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "stop_sequences": ["stop1", "stop2"]
    });
    anthropic_to_openai_chat(&mut body).unwrap();
    assert_eq!(body["stop"], json!(["stop1", "stop2"]));
    assert!(body.get("stop_sequences").is_none());
}

#[test]
fn test_a_to_o_metadata_user_id_to_user() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "metadata": {"user_id": "u-123"}
    });
    anthropic_to_openai_chat(&mut body).unwrap();
    assert_eq!(body["user"], "u-123");
    assert!(body.get("metadata").is_none());
}

#[test]
fn test_a_to_o_thinking_removed() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "thinking": {"type": "enabled", "budget_tokens": 4096}
    });
    anthropic_to_openai_chat(&mut body).unwrap();
    assert!(body.get("thinking").is_none(), "thinking 应被移除");
}

#[test]
fn test_a_to_o_stream_injects_stream_options() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "stream": true
    });
    anthropic_to_openai_chat(&mut body).unwrap();
    assert_eq!(body["stream_options"]["include_usage"], true);
}

#[test]
fn test_a_to_o_no_stream_does_not_inject_stream_options() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "stream": false
    });
    anthropic_to_openai_chat(&mut body).unwrap();
    assert!(body.get("stream_options").is_none());
}

#[test]
fn test_a_to_o_content_string_flattened() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "hello"}],
        "max_tokens": 100
    });
    anthropic_to_openai_chat(&mut body).unwrap();
    // 单字符串 content 应保持为字符串（OpenAI 习惯）
    assert_eq!(body["messages"][0]["content"], "hello");
}

#[test]
fn test_a_to_o_tool_use_id_preserved() {
    // §7.10：工具调用 ID 不重命名
    let mut body = json!({
        "messages": [
            {"role": "user", "content": "q"},
            {
                "role": "assistant",
                "content": [
                    {"type": "tool_use", "id": "toolu_01abcXYZ", "name": "f", "input": {}}
                ]
            }
        ],
        "max_tokens": 100
    });
    anthropic_to_openai_chat(&mut body).unwrap();
    let tool_calls = body["messages"][1]["tool_calls"].as_array().unwrap();
    assert_eq!(tool_calls[0]["id"], "toolu_01abcXYZ");
}

#[test]
fn test_a_to_o_preserves_temperature_top_p() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "temperature": 0.7,
        "top_p": 0.9
    });
    anthropic_to_openai_chat(&mut body).unwrap();
    assert_eq!(body["temperature"], 0.7);
    assert_eq!(body["top_p"], 0.9);
}

#[test]
fn test_a_to_o_no_system_does_not_insert_system_message() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100
    });
    anthropic_to_openai_chat(&mut body).unwrap();
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
}

// ===== O→A: openai_chat_to_anthropic =====

#[test]
fn test_o_to_a_system_extracted_to_top_level() {
    let mut body = json!({
        "model": "gpt-4",
        "messages": [
            {"role": "system", "content": "You are helpful"},
            {"role": "user", "content": "hi"}
        ],
        "max_tokens": 100
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();

    assert_eq!(body["system"], "You are helpful");
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1, "system 消息应被移除");
    assert_eq!(messages[0]["role"], "user");
}

#[test]
fn test_o_to_a_multiple_system_messages_concatenated() {
    let mut body = json!({
        "messages": [
            {"role": "system", "content": "Rule 1"},
            {"role": "system", "content": "Rule 2"},
            {"role": "user", "content": "q"}
        ],
        "max_tokens": 100
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    assert_eq!(body["system"], "Rule 1\nRule 2");
}

#[test]
fn test_o_to_a_user_content_string_to_text_block() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "hello"}],
        "max_tokens": 100
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    let content = body["messages"][0]["content"].as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "hello");
}

#[test]
fn test_o_to_a_assistant_tool_calls_to_tool_use_blocks() {
    let mut body = json!({
        "messages": [
            {"role": "user", "content": "q"},
            {
                "role": "assistant",
                "content": "Let me check",
                "tool_calls": [{
                    "id": "call_abc",
                    "type": "function",
                    "function": {"name": "get_weather", "arguments": "{\"city\":\"SH\"}"}
                }]
            }
        ],
        "max_tokens": 100
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();

    let assistant = &body["messages"][1];
    assert_eq!(assistant["role"], "assistant");
    let content = assistant["content"].as_array().unwrap();
    // text block + tool_use block
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "Let me check");
    assert_eq!(content[1]["type"], "tool_use");
    assert_eq!(content[1]["id"], "call_abc");
    assert_eq!(content[1]["name"], "get_weather");
    // input 是 parsed JSON 对象
    assert_eq!(content[1]["input"], json!({"city": "SH"}));
    // tool_calls 字段应不存在
    assert!(assistant.get("tool_calls").is_none());
}

#[test]
fn test_o_to_a_tool_message_merged_into_previous_assistant() {
    let mut body = json!({
        "messages": [
            {"role": "user", "content": "q"},
            {
                "role": "assistant",
                "content": null,
                "tool_calls": [{"id": "c1", "type": "function", "function": {"name": "f", "arguments": "{}"}}]
            },
            {"role": "tool", "tool_call_id": "c1", "content": "result"}
        ],
        "max_tokens": 100
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();

    let messages = body["messages"].as_array().unwrap();
    // user + assistant（tool_result 已合并到 assistant.content）
    assert_eq!(messages.len(), 2);
    let assistant = &messages[1];
    let content = assistant["content"].as_array().unwrap();
    // 1 tool_use + 1 tool_result
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "tool_use");
    assert_eq!(content[1]["type"], "tool_result");
    assert_eq!(content[1]["tool_use_id"], "c1");
    assert_eq!(content[1]["content"], "result");
}

#[test]
fn test_o_to_a_tool_message_without_previous_assistant_wrapped_in_user() {
    let mut body = json!({
        "messages": [
            {"role": "tool", "tool_call_id": "c1", "content": "result"}
        ],
        "max_tokens": 100
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();

    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
    let content = messages[0]["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "tool_result");
}

#[test]
fn test_o_to_a_image_url_data_to_base64() {
    let mut body = json!({
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "what?"},
                {"type": "image_url", "image_url": {"url": "data:image/png;base64,iVBORw0K"}}
            ]
        }],
        "max_tokens": 100
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();

    let content = body["messages"][0]["content"].as_array().unwrap();
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["source"]["type"], "base64");
    assert_eq!(content[1]["source"]["media_type"], "image/png");
    assert_eq!(content[1]["source"]["data"], "iVBORw0K");
}

#[test]
fn test_o_to_a_image_url_https_to_url_source() {
    let mut body = json!({
        "messages": [{
            "role": "user",
            "content": [
                {"type": "image_url", "image_url": {"url": "https://example.com/x.png"}}
            ]
        }],
        "max_tokens": 100
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();

    let content = body["messages"][0]["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "image");
    assert_eq!(content[0]["source"]["type"], "url");
    assert_eq!(content[0]["source"]["url"], "https://example.com/x.png");
}

#[test]
fn test_o_to_a_tools_rename() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "tools": [{
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get weather",
                "parameters": {"type": "object", "properties": {}}
            }
        }]
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();

    let tools = body["tools"].as_array().unwrap();
    assert_eq!(tools[0]["name"], "get_weather");
    assert_eq!(tools[0]["description"], "Get weather");
    assert_eq!(
        tools[0]["input_schema"],
        json!({"type": "object", "properties": {}})
    );
    assert!(tools[0].get("type").is_none(), "type:function 应被去除");
    assert!(tools[0].get("function").is_none(), "function 包裹应被去除");
}

#[test]
fn test_o_to_a_tool_choice_string_variants() {
    // "auto" → {type:"auto"}
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "tool_choice": "auto"
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    assert_eq!(body["tool_choice"], json!({"type": "auto"}));

    // "required" → {type:"any"}
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "tool_choice": "required"
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    assert_eq!(body["tool_choice"], json!({"type": "any"}));
}

#[test]
fn test_o_to_a_tool_choice_function_to_tool() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "tool_choice": {"type": "function", "function": {"name": "get_weather"}}
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    assert_eq!(body["tool_choice"]["type"], "tool");
    assert_eq!(body["tool_choice"]["name"], "get_weather");
}

#[test]
fn test_o_to_a_stop_string_wrapped_as_array() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "stop": "stop1"
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    assert_eq!(body["stop_sequences"], json!(["stop1"]));
    assert!(body.get("stop").is_none());
}

#[test]
fn test_o_to_a_stop_array_preserved() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "stop": ["stop1", "stop2"]
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    assert_eq!(body["stop_sequences"], json!(["stop1", "stop2"]));
}

#[test]
fn test_o_to_a_user_to_metadata_user_id() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "user": "u-123"
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    assert_eq!(body["metadata"]["user_id"], "u-123");
    assert!(body.get("user").is_none());
}

#[test]
fn test_o_to_a_max_tokens_missing_uses_model_config() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}]
    });
    openai_chat_to_anthropic(&mut body, Some(8192)).unwrap();
    assert_eq!(body["max_tokens"], 8192);
}

#[test]
fn test_o_to_a_max_tokens_missing_uses_fallback() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}]
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    assert_eq!(body["max_tokens"], MAX_TOKENS_FALLBACK);
}

#[test]
fn test_o_to_a_max_tokens_present_preserved() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 500
    });
    openai_chat_to_anthropic(&mut body, Some(8192)).unwrap();
    assert_eq!(body["max_tokens"], 500);
}

#[test]
fn test_o_to_a_discarded_fields_removed() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "frequency_penalty": 0.5,
        "presence_penalty": 0.3,
        "seed": 42,
        "n": 2,
        "logprobs": true,
        "top_logprobs": 5,
        "service_tier": "default",
        "stream_options": {"include_usage": true}
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    for field in [
        "frequency_penalty",
        "presence_penalty",
        "seed",
        "n",
        "logprobs",
        "top_logprobs",
        "service_tier",
        "stream_options",
    ] {
        assert!(body.get(field).is_none(), "字段 {} 应被移除", field);
    }
}

#[test]
fn test_o_to_a_response_format_json_object_appends_system_prompt() {
    let mut body = json!({
        "messages": [{"role": "system", "content": "Be helpful"}, {"role": "user", "content": "q"}],
        "max_tokens": 100,
        "response_format": {"type": "json_object"}
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();

    let system = body["system"].as_str().unwrap();
    assert!(
        system.starts_with("Be helpful"),
        "原 system 应保留在开头: {}",
        system
    );
    assert!(
        system.contains("Please respond with valid JSON only"),
        "应追加 JSON prompt: {}",
        system
    );
    assert!(
        body.get("response_format").is_none(),
        "response_format 字段应被移除"
    );
}

#[test]
fn test_o_to_a_response_format_json_schema_includes_schema() {
    let schema = json!({"type": "object", "properties": {"name": {"type": "string"}}});
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "response_format": {
            "type": "json_schema",
            "json_schema": {"name": "test", "schema": schema}
        }
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();

    let system = body["system"].as_str().unwrap();
    assert!(
        system.contains("Please respond with valid JSON matching the following schema"),
        "应包含 schema prompt: {}",
        system
    );
    assert!(
        system.contains("\"type\":\"string\""),
        "应包含 schema 内容: {}",
        system
    );
}

#[test]
fn test_o_to_a_response_format_without_system_creates_system() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "response_format": {"type": "json_object"}
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    assert!(body.get("system").is_some(), "无 system 时应创建 system 字段");
}

#[test]
fn test_o_to_a_reasoning_effort_to_thinking_all_tiers() {
    let cases = [
        ("minimal", 1024),
        ("low", 2048),
        ("medium", 4096),
        ("high", 8192),
    ];
    for (effort, budget) in cases {
        let mut body = json!({
            "messages": [{"role": "user", "content": "q"}],
            "max_tokens": 100,
            "reasoning_effort": effort
        });
        openai_chat_to_anthropic(&mut body, None).unwrap();
        assert_eq!(
            body["thinking"]["type"], "enabled",
            "reasoning_effort={} 应产生 thinking.type=enabled",
            effort
        );
        assert_eq!(
            body["thinking"]["budget_tokens"], budget,
            "reasoning_effort={} 应映射到 budget_tokens={}",
            effort, budget
        );
        assert!(
            body.get("reasoning_effort").is_none(),
            "reasoning_effort={} 应被移除",
            effort
        );
    }
}

#[test]
fn test_o_to_a_reasoning_effort_unknown_no_thinking() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "reasoning_effort": "unknown_value"
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    assert!(
        body.get("thinking").is_none(),
        "未知 reasoning_effort 不应注入 thinking"
    );
    assert!(
        body.get("reasoning_effort").is_none(),
        "未知 reasoning_effort 也应被移除"
    );
}

#[test]
fn test_o_to_a_tool_call_id_preserved_as_tool_use_id() {
    // §7.10：工具调用 ID 不重命名
    let mut body = json!({
        "messages": [
            {"role": "user", "content": "q"},
            {
                "role": "assistant",
                "tool_calls": [{"id": "call_xyz123", "type": "function", "function": {"name": "f", "arguments": "{}"}}]
            },
            {"role": "tool", "tool_call_id": "call_xyz123", "content": "r"}
        ],
        "max_tokens": 100
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();

    let assistant_content = body["messages"][1]["content"].as_array().unwrap();
    let tool_use = assistant_content
        .iter()
        .find(|b| b["type"] == "tool_use")
        .unwrap();
    assert_eq!(tool_use["id"], "call_xyz123");
    let tool_result = assistant_content
        .iter()
        .find(|b| b["type"] == "tool_result")
        .unwrap();
    assert_eq!(tool_result["tool_use_id"], "call_xyz123");
}

#[test]
fn test_o_to_a_stream_preserved() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "stream": true,
        "stream_options": {"include_usage": true}
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    assert_eq!(body["stream"], true);
    // stream_options 应被移除（Anthropic 默认带 usage）
    assert!(body.get("stream_options").is_none());
}

#[test]
fn test_o_to_a_preserves_temperature_top_p() {
    let mut body = json!({
        "messages": [{"role": "user", "content": "q"}],
        "max_tokens": 100,
        "temperature": 0.5,
        "top_p": 0.8
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    assert_eq!(body["temperature"], 0.5);
    assert_eq!(body["top_p"], 0.8);
}

#[test]
fn test_o_to_a_assistant_invalid_arguments_falls_back_to_empty_object() {
    let mut body = json!({
        "messages": [
            {"role": "user", "content": "q"},
            {
                "role": "assistant",
                "tool_calls": [{
                    "id": "c1",
                    "type": "function",
                    "function": {"name": "f", "arguments": "not valid json"}
                }]
            }
        ],
        "max_tokens": 100
    });
    openai_chat_to_anthropic(&mut body, None).unwrap();
    let content = body["messages"][1]["content"].as_array().unwrap();
    let tool_use = content.iter().find(|b| b["type"] == "tool_use").unwrap();
    assert_eq!(tool_use["input"], json!({}));
}

// ===== 错误路径 =====

#[test]
fn test_error_body_not_object() {
    let mut body = json!([1, 2, 3]);
    let result = anthropic_to_openai_chat(&mut body);
    assert!(result.is_err());
    let result = openai_chat_to_anthropic(&mut json!(42), None);
    assert!(result.is_err());
}

#[test]
fn test_error_messages_missing() {
    let mut body = json!({"max_tokens": 100});
    let result = anthropic_to_openai_chat(&mut body);
    assert!(result.is_err());
    let result = openai_chat_to_anthropic(&mut json!({"max_tokens": 100}), None);
    assert!(result.is_err());
}

#[test]
fn test_error_message_missing_role() {
    let mut body = json!({
        "messages": [{"content": "q"}],
        "max_tokens": 100
    });
    let result = anthropic_to_openai_chat(&mut body);
    assert!(result.is_err());
}

// ===== detect_bridge 集成测试（与请求转换联合） =====

#[test]
fn test_detect_bridge_picks_correct_conversion_function() {
    // 入口 Anthropic + 上游 OpenAI → A→O
    let kind = detect_bridge(GatewayProtocol::AnthropicMessages, "openai");
    assert_eq!(kind, BridgeKind::AnthropicToOpenai);
    assert_eq!(kind.label(), "A→O");

    // 入口 OpenAI + 上游 Anthropic → O→A
    let kind = detect_bridge(GatewayProtocol::ChatCompletions, "anthropic");
    assert_eq!(kind, BridgeKind::OpenaiToAnthropic);
    assert_eq!(kind.label(), "O→A");

    // 协议一致 → None
    let kind = detect_bridge(GatewayProtocol::ChatCompletions, "openai");
    assert_eq!(kind, BridgeKind::None);
    assert!(!kind.is_bridged());
}

// ===== 复杂场景：完整请求双向转换 =====

#[test]
fn test_complex_a_to_o_full_request() {
    let mut body = json!({
        "model": "claude-3-5-sonnet",
        "system": "You are helpful",
        "messages": [
            {"role": "user", "content": "what's the weather?"},
            {
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "I'll check"},
                    {"type": "tool_use", "id": "t1", "name": "get_weather", "input": {"city": "SH"}}
                ]
            },
            {
                "role": "user",
                "content": [
                    {"type": "tool_result", "tool_use_id": "t1", "content": "sunny"}
                ]
            }
        ],
        "max_tokens": 1024,
        "stop_sequences": ["END"],
        "tools": [{"name": "get_weather", "description": "Get weather", "input_schema": {"type": "object"}}],
        "tool_choice": {"type": "auto"},
        "metadata": {"user_id": "u1"},
        "thinking": {"type": "enabled", "budget_tokens": 4096},
        "temperature": 0.5,
        "stream": true
    });
    anthropic_to_openai_chat(&mut body).unwrap();

    // 验证关键字段
    assert_eq!(body["model"], "claude-3-5-sonnet");
    let messages = body["messages"].as_array().unwrap();
    // system + user + assistant + tool = 4
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[2]["role"], "assistant");
    assert_eq!(messages[3]["role"], "tool");

    assert_eq!(body["max_tokens"], 1024);
    assert_eq!(body["stop"], json!(["END"]));
    assert!(body.get("stop_sequences").is_none());
    assert_eq!(body["tools"][0]["function"]["name"], "get_weather");
    assert_eq!(body["tool_choice"], "auto");
    assert_eq!(body["user"], "u1");
    assert!(body.get("thinking").is_none());
    assert!(body.get("metadata").is_none());
    assert_eq!(body["temperature"], 0.5);
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
}

#[test]
fn test_complex_o_to_a_full_request() {
    let mut body = json!({
        "model": "gpt-4",
        "messages": [
            {"role": "system", "content": "Be helpful"},
            {"role": "user", "content": "q"},
            {
                "role": "assistant",
                "content": "thinking",
                "tool_calls": [{
                    "id": "c1",
                    "type": "function",
                    "function": {"name": "f", "arguments": "{\"x\":1}"}
                }]
            },
            {"role": "tool", "tool_call_id": "c1", "content": "result"}
        ],
        "max_tokens": 500,
        "stop": "END",
        "user": "u1",
        "tools": [{"type": "function", "function": {"name": "f", "description": "d", "parameters": {"type": "object"}}}],
        "tool_choice": "auto",
        "stream": true,
        "stream_options": {"include_usage": true},
        "frequency_penalty": 0.5,
        "seed": 42,
        "response_format": {"type": "json_object"},
        "reasoning_effort": "high"
    });
    openai_chat_to_anthropic(&mut body, Some(8192)).unwrap();

    // 验证关键字段
    assert_eq!(body["model"], "gpt-4");
    let system = body["system"].as_str().unwrap();
    assert!(system.starts_with("Be helpful"));
    assert!(system.contains("valid JSON"));

    let messages = body["messages"].as_array().unwrap();
    // user + assistant（含 tool_result 合并）= 2
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[1]["role"], "assistant");
    let assistant_content = messages[1]["content"].as_array().unwrap();
    // text + tool_use + tool_result = 3
    assert_eq!(assistant_content.len(), 3);

    assert_eq!(body["max_tokens"], 500);
    assert_eq!(body["stop_sequences"], json!(["END"]));
    assert_eq!(body["metadata"]["user_id"], "u1");
    assert_eq!(body["tools"][0]["name"], "f");
    assert_eq!(body["tool_choice"], json!({"type": "auto"}));
    assert_eq!(body["stream"], true);
    assert!(body.get("stream_options").is_none());
    assert!(body.get("frequency_penalty").is_none());
    assert!(body.get("seed").is_none());
    assert!(body.get("response_format").is_none());
    assert_eq!(body["thinking"]["budget_tokens"], 8192);
    assert_eq!(body["thinking"]["type"], "enabled");
    assert!(body.get("reasoning_effort").is_none());
}

// ===== round-trip 验证（best-effort） =====

#[test]
fn test_roundtrip_simple_text_preserves_content() {
    // 简单文本消息：O→A → A→O 应保留核心语义
    let original = json!({
        "model": "gpt-4",
        "messages": [
            {"role": "system", "content": "Be helpful"},
            {"role": "user", "content": "hello"}
        ],
        "max_tokens": 100
    });
    let mut body = original.clone();
    openai_chat_to_anthropic(&mut body, None).unwrap();
    // 现在 body 是 Anthropic 格式
    assert_eq!(body["system"], "Be helpful");
    anthropic_to_openai_chat(&mut body).unwrap();
    // 现在 body 又是 OpenAI 格式
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[0]["content"], "Be helpful");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[1]["content"], "hello");
}

// ===== Map<String, Value> 辅助测试 =====

#[test]
fn test_into_obj_helper() {
    let obj = into_obj(json!({"a": 1}));
    assert_eq!(obj["a"], 1);
}

// ===== 响应体转换：A→O anthropic_response_to_openai =====

#[test]
fn test_a_to_o_response_basic_text_content() {
    let mut body = json!({
        "id": "msg_01abc",
        "type": "message",
        "role": "assistant",
        "model": "claude-3-5-sonnet",
        "content": [
            {"type": "text", "text": "Hello world"}
        ],
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 10,
            "output_tokens": 5
        }
    });
    anthropic_response_to_openai(&mut body).unwrap();

    assert_eq!(body["id"], "msg_01abc");
    assert_eq!(body["object"], "chat.completion");
    assert_eq!(body["model"], "claude-3-5-sonnet");
    assert_eq!(body["created"], 0); // Anthropic 无 created，置 0

    let choices = body["choices"].as_array().unwrap();
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0]["index"], 0);
    assert_eq!(choices[0]["message"]["role"], "assistant");
    assert_eq!(choices[0]["message"]["content"], "Hello world");
    assert_eq!(choices[0]["finish_reason"], "stop");

    assert_eq!(body["usage"]["prompt_tokens"], 10);
    assert_eq!(body["usage"]["completion_tokens"], 5);
    assert_eq!(body["usage"]["total_tokens"], 15);
}

#[test]
fn test_a_to_o_response_tool_use_to_tool_calls() {
    let mut body = json!({
        "id": "msg_01",
        "model": "claude-3",
        "content": [
            {"type": "text", "text": "Let me check"},
            {"type": "tool_use", "id": "toolu_01", "name": "get_weather", "input": {"city": "SH"}}
        ],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 8, "output_tokens": 12}
    });
    anthropic_response_to_openai(&mut body).unwrap();

    let msg = &body["choices"][0]["message"];
    assert_eq!(msg["content"], "Let me check");
    let tool_calls = msg["tool_calls"].as_array().unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0]["id"], "toolu_01");
    assert_eq!(tool_calls[0]["type"], "function");
    assert_eq!(tool_calls[0]["function"]["name"], "get_weather");
    assert_eq!(
        tool_calls[0]["function"]["arguments"].as_str().unwrap(),
        r#"{"city":"SH"}"#
    );
    assert_eq!(body["choices"][0]["finish_reason"], "tool_calls");
}

#[test]
fn test_a_to_o_response_multiple_text_blocks_concatenated_with_newline() {
    let mut body = json!({
        "id": "m1",
        "model": "claude",
        "content": [
            {"type": "text", "text": "Line 1"},
            {"type": "text", "text": "Line 2"}
        ],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 1, "output_tokens": 2}
    });
    anthropic_response_to_openai(&mut body).unwrap();

    assert_eq!(
        body["choices"][0]["message"]["content"],
        "Line 1\nLine 2"
    );
}

#[test]
fn test_a_to_o_response_empty_content_array() {
    let mut body = json!({
        "id": "m1",
        "model": "claude",
        "content": [],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 1, "output_tokens": 0}
    });
    anthropic_response_to_openai(&mut body).unwrap();

    // 空文本内容应为 null（OpenAI 习惯）
    assert!(body["choices"][0]["message"]["content"].is_null());
    assert!(body["choices"][0]["message"].get("tool_calls").is_none());
}

#[test]
fn test_a_to_o_response_missing_content_field() {
    let mut body = json!({
        "id": "m1",
        "model": "claude",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 1, "output_tokens": 0}
    });
    anthropic_response_to_openai(&mut body).unwrap();

    assert!(body["choices"][0]["message"]["content"].is_null());
}

#[test]
fn test_a_to_o_response_stop_reason_mapping() {
    let cases = [
        ("end_turn", "stop"),
        ("max_tokens", "length"),
        ("stop_sequence", "stop"),
        ("tool_use", "tool_calls"),
    ];
    for (sr, fr) in cases {
        let mut body = json!({
            "id": "m", "model": "c",
            "content": [{"type": "text", "text": "x"}],
            "stop_reason": sr,
            "usage": {"input_tokens": 1, "output_tokens": 1}
        });
        anthropic_response_to_openai(&mut body).unwrap();
        assert_eq!(
            body["choices"][0]["finish_reason"], fr,
            "stop_reason={} 应映射为 finish_reason={}", sr, fr
        );
    }
}

#[test]
fn test_a_to_o_response_unknown_stop_reason_falls_back_to_stop() {
    let mut body = json!({
        "id": "m", "model": "c",
        "content": [{"type": "text", "text": "x"}],
        "stop_reason": "unknown_reason",
        "usage": {"input_tokens": 1, "output_tokens": 1}
    });
    anthropic_response_to_openai(&mut body).unwrap();
    assert_eq!(body["choices"][0]["finish_reason"], "stop");
}

#[test]
fn test_a_to_o_response_missing_stop_reason_falls_back_to_stop() {
    let mut body = json!({
        "id": "m", "model": "c",
        "content": [{"type": "text", "text": "x"}],
        "usage": {"input_tokens": 1, "output_tokens": 1}
    });
    anthropic_response_to_openai(&mut body).unwrap();
    assert_eq!(body["choices"][0]["finish_reason"], "stop");
}

#[test]
fn test_a_to_o_response_usage_total_calculated() {
    let mut body = json!({
        "id": "m", "model": "c",
        "content": [{"type": "text", "text": "x"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 100, "output_tokens": 50}
    });
    anthropic_response_to_openai(&mut body).unwrap();
    assert_eq!(body["usage"]["prompt_tokens"], 100);
    assert_eq!(body["usage"]["completion_tokens"], 50);
    assert_eq!(body["usage"]["total_tokens"], 150);
}

#[test]
fn test_a_to_o_response_usage_cache_read_mapped_to_prompt_tokens_details() {
    let mut body = json!({
        "id": "m", "model": "c",
        "content": [{"type": "text", "text": "x"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 100, "output_tokens": 50, "cache_read_input_tokens": 30}
    });
    anthropic_response_to_openai(&mut body).unwrap();
    assert_eq!(body["usage"]["prompt_tokens_details"]["cached_tokens"], 30);
}

#[test]
fn test_a_to_o_response_no_cache_read_omits_prompt_tokens_details() {
    let mut body = json!({
        "id": "m", "model": "c",
        "content": [{"type": "text", "text": "x"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 100, "output_tokens": 50}
    });
    anthropic_response_to_openai(&mut body).unwrap();
    assert!(body["usage"].get("prompt_tokens_details").is_none());
}

#[test]
fn test_a_to_o_response_missing_usage() {
    let mut body = json!({
        "id": "m", "model": "c",
        "content": [{"type": "text", "text": "x"}],
        "stop_reason": "end_turn"
    });
    anthropic_response_to_openai(&mut body).unwrap();
    assert_eq!(body["usage"]["prompt_tokens"], 0);
    assert_eq!(body["usage"]["completion_tokens"], 0);
    assert_eq!(body["usage"]["total_tokens"], 0);
}

#[test]
fn test_a_to_o_response_preserves_created_when_present() {
    let mut body = json!({
        "id": "m", "model": "c", "created": 1700000000_i64,
        "content": [{"type": "text", "text": "x"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 1, "output_tokens": 1}
    });
    anthropic_response_to_openai(&mut body).unwrap();
    assert_eq!(body["created"], 1700000000_i64);
}

#[test]
fn test_a_to_o_response_tool_use_id_preserved() {
    // §7.10：工具调用 ID 不重命名
    let mut body = json!({
        "id": "m", "model": "c",
        "content": [
            {"type": "tool_use", "id": "toolu_original", "name": "f", "input": {}}
        ],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 1, "output_tokens": 1}
    });
    anthropic_response_to_openai(&mut body).unwrap();
    assert_eq!(
        body["choices"][0]["message"]["tool_calls"][0]["id"],
        "toolu_original"
    );
}

// ===== 响应体转换：O→A openai_response_to_anthropic =====

#[test]
fn test_o_to_a_response_basic_text_content() {
    let mut body = json!({
        "id": "chatcmpl-01",
        "object": "chat.completion",
        "created": 1700000000_i64,
        "model": "gpt-4.1",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Hello world"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 8, "completion_tokens": 5, "total_tokens": 13}
    });
    openai_response_to_anthropic(&mut body).unwrap();

    assert_eq!(body["id"], "chatcmpl-01");
    assert_eq!(body["type"], "message");
    assert_eq!(body["role"], "assistant");
    assert_eq!(body["model"], "gpt-4.1");

    let content = body["content"].as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "Hello world");

    assert_eq!(body["stop_reason"], "end_turn");
    assert_eq!(body["usage"]["input_tokens"], 8);
    assert_eq!(body["usage"]["output_tokens"], 5);
}

#[test]
fn test_o_to_a_response_tool_calls_to_tool_use_blocks() {
    let mut body = json!({
        "id": "c1", "model": "gpt-4",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Let me check",
                "tool_calls": [{
                    "id": "call_abc",
                    "type": "function",
                    "function": {"name": "get_weather", "arguments": "{\"city\":\"SH\"}"}
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {"prompt_tokens": 5, "completion_tokens": 10, "total_tokens": 15}
    });
    openai_response_to_anthropic(&mut body).unwrap();

    let content = body["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "Let me check");
    assert_eq!(content[1]["type"], "tool_use");
    assert_eq!(content[1]["id"], "call_abc");
    assert_eq!(content[1]["name"], "get_weather");
    assert_eq!(content[1]["input"], json!({"city": "SH"}));

    assert_eq!(body["stop_reason"], "tool_use");
}

#[test]
fn test_o_to_a_response_null_content_omits_text_block() {
    let mut body = json!({
        "id": "c1", "model": "gpt-4",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": null},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    });
    openai_response_to_anthropic(&mut body).unwrap();
    let content = body["content"].as_array().unwrap();
    assert!(content.is_empty(), "null content 不应产生 text block");
}

#[test]
fn test_o_to_a_response_empty_content_string_omits_text_block() {
    let mut body = json!({
        "id": "c1", "model": "gpt-4",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": ""},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    });
    openai_response_to_anthropic(&mut body).unwrap();
    let content = body["content"].as_array().unwrap();
    assert!(content.is_empty(), "空字符串 content 不应产生 text block");
}

#[test]
fn test_o_to_a_response_only_tool_calls_no_text() {
    let mut body = json!({
        "id": "c1", "model": "gpt-4",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_x", "type": "function",
                    "function": {"name": "f", "arguments": "{}"}
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    });
    openai_response_to_anthropic(&mut body).unwrap();
    let content = body["content"].as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "tool_use");
}

#[test]
fn test_o_to_a_response_finish_reason_mapping() {
    let cases = [
        ("stop", "end_turn"),
        ("length", "max_tokens"),
        ("tool_calls", "tool_use"),
        ("content_filter", "end_turn"),
    ];
    for (fr, sr) in cases {
        let mut body = json!({
            "id": "c", "model": "g",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "x"},
                "finish_reason": fr
            }],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
        });
        openai_response_to_anthropic(&mut body).unwrap();
        assert_eq!(
            body["stop_reason"], sr,
            "finish_reason={} 应映射为 stop_reason={}", fr, sr
        );
    }
}

#[test]
fn test_o_to_a_response_unknown_finish_reason_falls_back_to_end_turn() {
    let mut body = json!({
        "id": "c", "model": "g",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "x"},
            "finish_reason": "weird_reason"
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    });
    openai_response_to_anthropic(&mut body).unwrap();
    assert_eq!(body["stop_reason"], "end_turn");
}

#[test]
fn test_o_to_a_response_missing_finish_reason_falls_back_to_end_turn() {
    let mut body = json!({
        "id": "c", "model": "g",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "x"}
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    });
    openai_response_to_anthropic(&mut body).unwrap();
    assert_eq!(body["stop_reason"], "end_turn");
}

#[test]
fn test_o_to_a_response_usage_cached_tokens_mapped() {
    let mut body = json!({
        "id": "c", "model": "g",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "x"},
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 50,
            "total_tokens": 150,
            "prompt_tokens_details": {"cached_tokens": 30}
        }
    });
    openai_response_to_anthropic(&mut body).unwrap();
    assert_eq!(body["usage"]["input_tokens"], 100);
    assert_eq!(body["usage"]["output_tokens"], 50);
    assert_eq!(body["usage"]["cache_read_input_tokens"], 30);
}

#[test]
fn test_o_to_a_response_no_cached_tokens_omits_field() {
    let mut body = json!({
        "id": "c", "model": "g",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "x"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    });
    openai_response_to_anthropic(&mut body).unwrap();
    assert!(body["usage"].get("cache_read_input_tokens").is_none());
}

#[test]
fn test_o_to_a_response_missing_usage() {
    let mut body = json!({
        "id": "c", "model": "g",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "x"},
            "finish_reason": "stop"
        }]
    });
    openai_response_to_anthropic(&mut body).unwrap();
    assert_eq!(body["usage"]["input_tokens"], 0);
    assert_eq!(body["usage"]["output_tokens"], 0);
}

#[test]
fn test_o_to_a_response_empty_choices_array() {
    let mut body = json!({
        "id": "c", "model": "g",
        "choices": [],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    });
    openai_response_to_anthropic(&mut body).unwrap();
    // 空 choices → 空 content 数组 + 默认 stop_reason=end_turn
    assert!(body["content"].as_array().unwrap().is_empty());
    assert_eq!(body["stop_reason"], "end_turn");
}

#[test]
fn test_o_to_a_response_missing_choices_field() {
    let mut body = json!({
        "id": "c", "model": "g",
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    });
    openai_response_to_anthropic(&mut body).unwrap();
    assert!(body["content"].as_array().unwrap().is_empty());
}

#[test]
fn test_o_to_a_response_tool_call_id_preserved() {
    // §7.10：工具调用 ID 不重命名
    let mut body = json!({
        "id": "c", "model": "g",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_original_xyz",
                    "type": "function",
                    "function": {"name": "f", "arguments": "{}"}
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    });
    openai_response_to_anthropic(&mut body).unwrap();
    assert_eq!(body["content"][0]["id"], "call_original_xyz");
}

#[test]
fn test_o_to_a_response_invalid_arguments_json_falls_back_to_empty_object() {
    let mut body = json!({
        "id": "c", "model": "g",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1", "type": "function",
                    "function": {"name": "f", "arguments": "not-valid-json"}
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    });
    openai_response_to_anthropic(&mut body).unwrap();
    assert_eq!(body["content"][0]["input"], json!({}));
}

// ===== 错误体转换：convert_error_body =====

#[test]
fn test_convert_error_body_anthropic_to_openai() {
    // 桥接 kind = OpenaiToAnthropic：上游 Anthropic 错误体 → 入口 OpenAI 错误体
    let mut body = json!({
        "type": "error",
        "error": {
            "type": "invalid_request_error",
            "message": "max_tokens is required"
        }
    });
    convert_error_body(&mut body, BridgeKind::OpenaiToAnthropic).unwrap();

    assert_eq!(body["error"]["message"], "max_tokens is required");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert!(body["error"]["param"].is_null());
    assert!(body["error"]["code"].is_null());
    // 顶层 type=error 已被清除
    assert!(body.get("type").is_none());
}

#[test]
fn test_convert_error_body_openai_to_anthropic() {
    // 桥接 kind = AnthropicToOpenai：上游 OpenAI 错误体 → 入口 Anthropic 错误体
    let mut body = json!({
        "error": {
            "message": "Invalid API key",
            "type": "authentication_error",
            "param": null,
            "code": "invalid_api_key"
        }
    });
    convert_error_body(&mut body, BridgeKind::AnthropicToOpenai).unwrap();

    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "authentication_error");
    assert_eq!(body["error"]["message"], "Invalid API key");
    // param/code 已被剥离（Anthropic 错误体不包含）
    assert!(body["error"].get("param").is_none());
    assert!(body["error"].get("code").is_none());
}

#[test]
fn test_convert_error_body_none_kind_no_op() {
    let mut body = json!({
        "error": {"message": "x", "type": "t", "param": null, "code": null}
    });
    let before = body.clone();
    convert_error_body(&mut body, BridgeKind::None).unwrap();
    assert_eq!(body, before, "BridgeKind::None 应为 no-op");
}

#[test]
fn test_convert_error_body_malformed_anthropic_no_error_field_no_op() {
    // 缺少 error 字段：转换失败但不应破坏 body
    let mut body = json!({"type": "error", "unexpected": "data"});
    let before = body.clone();
    convert_error_body(&mut body, BridgeKind::OpenaiToAnthropic).unwrap();
    assert_eq!(body, before, "错误体转换失败时应保持原样");
}

#[test]
fn test_convert_error_body_malformed_openai_no_error_field_no_op() {
    let mut body = json!({"unexpected": "data"});
    let before = body.clone();
    convert_error_body(&mut body, BridgeKind::AnthropicToOpenai).unwrap();
    assert_eq!(body, before, "错误体转换失败时应保持原样");
}

#[test]
fn test_convert_error_body_anthropic_missing_message_uses_empty() {
    let mut body = json!({
        "type": "error",
        "error": {"type": "invalid_request_error"}
    });
    convert_error_body(&mut body, BridgeKind::OpenaiToAnthropic).unwrap();
    assert_eq!(body["error"]["message"], "");
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

#[test]
fn test_convert_error_body_anthropic_missing_type_uses_api_error() {
    let mut body = json!({
        "type": "error",
        "error": {"message": "something went wrong"}
    });
    convert_error_body(&mut body, BridgeKind::OpenaiToAnthropic).unwrap();
    assert_eq!(body["error"]["type"], "api_error");
    assert_eq!(body["error"]["message"], "something went wrong");
}

#[test]
fn test_convert_error_body_roundtrip_anthropic_to_openai_to_anthropic() {
    // Anthropic 错误体 → OpenAI → Anthropic，message 与 type 应保持一致
    let original = json!({
        "type": "error",
        "error": {"type": "invalid_request_error", "message": "bad request"}
    });

    let mut step1 = original.clone();
    convert_error_body(&mut step1, BridgeKind::OpenaiToAnthropic).unwrap();

    let mut step2 = step1.clone();
    convert_error_body(&mut step2, BridgeKind::AnthropicToOpenai).unwrap();

    assert_eq!(step2["type"], "error");
    assert_eq!(step2["error"]["type"], "invalid_request_error");
    assert_eq!(step2["error"]["message"], "bad request");
}

#[test]
fn test_convert_error_body_non_object_body_no_op() {
    // body 非 JSON 对象：转换函数返回 Err，外层包装为 Ok 并保持原样
    let mut body = json!("just a string");
    let before = body.clone();
    convert_error_body(&mut body, BridgeKind::OpenaiToAnthropic).unwrap();
    assert_eq!(body, before);
}

// ===== 响应体转换 round-trip 验证 =====

#[test]
fn test_response_roundtrip_text_preserves_content() {
    // A→O→A：Anthropic 响应 → OpenAI → Anthropic，text content 与 usage 应保持
    let original = json!({
        "id": "msg_01",
        "model": "claude-3",
        "content": [{"type": "text", "text": "Hello"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 5}
    });

    let mut step1 = original.clone();
    anthropic_response_to_openai(&mut step1).unwrap();
    // step1 已是 OpenAI 格式
    assert_eq!(step1["choices"][0]["message"]["content"], "Hello");

    let mut step2 = step1.clone();
    openai_response_to_anthropic(&mut step2).unwrap();
    // step2 已回到 Anthropic 格式
    assert_eq!(step2["content"][0]["type"], "text");
    assert_eq!(step2["content"][0]["text"], "Hello");
    assert_eq!(step2["stop_reason"], "end_turn");
    assert_eq!(step2["usage"]["input_tokens"], 10);
    assert_eq!(step2["usage"]["output_tokens"], 5);
}

#[test]
fn test_response_roundtrip_tool_use_preserves_id_and_input() {
    let original = json!({
        "id": "msg_01",
        "model": "claude-3",
        "content": [
            {"type": "tool_use", "id": "toolu_abc", "name": "get_weather", "input": {"city": "SH"}}
        ],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 8, "output_tokens": 12}
    });

    let mut step1 = original.clone();
    anthropic_response_to_openai(&mut step1).unwrap();
    assert_eq!(step1["choices"][0]["message"]["tool_calls"][0]["id"], "toolu_abc");

    let mut step2 = step1.clone();
    openai_response_to_anthropic(&mut step2).unwrap();
    assert_eq!(step2["content"][0]["type"], "tool_use");
    assert_eq!(step2["content"][0]["id"], "toolu_abc");
    assert_eq!(step2["content"][0]["name"], "get_weather");
    assert_eq!(step2["content"][0]["input"], json!({"city": "SH"}));
    assert_eq!(step2["stop_reason"], "tool_use");
}
