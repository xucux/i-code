//! # 消息内容提取器
//!
//! 从 OpenAI 格式的 `ChatMessage` 列表中提取纯文本与额外 token 数，
//! 供 BPE 分词器（openai/deepseek）使用。
//!
//! 移植自参考项目 `tokenizer/content.ts`。
//! 与 `conservative.rs` 的逻辑同构，区别在于：
//! - `conservative` 直接按 UTF-8 字节计数
//! - `content` 收集纯文本字符串，交给下游 BPE 编码

use super::types::{
    ChatMessage, ContentPart, MessageContent, TokenizedInput, MESSAGE_OVERHEAD_TOKENS,
    IMAGE_PART_TOKENS,
};

/// 从单条消息中提取文本内容与额外 token 数
///
/// 处理逻辑：
/// - `MessageContent::Text`：直接收集文本
/// - `MessageContent::Parts`：遍历各 ContentPart
///   - `ContentPart::Text`：收集文本
///   - `ContentPart::ImageUrl`：按固定 token 数计入
/// - `tool_calls`：工具调用的函数名 + 参数 JSON 计入文本
/// - 每条消息额外 +4 token 开销
fn collect_from_message(msg: &ChatMessage, text_parts: &mut Vec<String>, extra_tokens: &mut usize) {
    // 消息格式开销
    *extra_tokens += MESSAGE_OVERHEAD_TOKENS;

    // 处理 content
    match &msg.content {
        MessageContent::Text(s) => {
            if !s.is_empty() {
                text_parts.push(s.clone());
            }
        }
        MessageContent::Parts(parts) => {
            for part in parts {
                match part {
                    ContentPart::Text { text } => {
                        if !text.is_empty() {
                            text_parts.push(text.clone());
                        }
                    }
                    ContentPart::ImageUrl { .. } => {
                        *extra_tokens += IMAGE_PART_TOKENS;
                    }
                }
            }
        }
    }

    // 处理 tool_calls（assistant 消息中的工具调用）
    if let Some(tool_calls) = &msg.tool_calls {
        for tc in tool_calls {
            text_parts.push(tc.function.name.clone());
            if !tc.function.arguments.is_empty() {
                text_parts.push(tc.function.arguments.clone());
            }
        }
    }
}

/// 从消息列表中提取文本内容与额外 token 数
///
/// # 参数
///
/// - `messages`：OpenAI 格式的聊天消息列表
///
/// # 返回
///
/// `TokenizedInput` 包含：
/// - `text_content`：所有文本部分拼接的纯文本
/// - `extra_tokens`：非文本部分折算的 token 数
pub fn collect_tokenized_input(messages: &[ChatMessage]) -> TokenizedInput {
    let mut text_parts: Vec<String> = Vec::new();
    let mut extra_tokens: usize = 0;

    for msg in messages {
        collect_from_message(msg, &mut text_parts, &mut extra_tokens);
    }

    TokenizedInput {
        text_content: text_parts.join(""),
        extra_tokens,
    }
}

/// 从纯文本字符串构造 TokenizedInput
///
/// 简便方法：将整个文本作为 text_content，extra_tokens = 0。
#[allow(dead_code)]
pub fn collect_from_plain_text(text: &str) -> TokenizedInput {
    TokenizedInput {
        text_content: text.to_string(),
        extra_tokens: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{ChatRole, ToolCall, FunctionCall};

    fn make_text_message(role: ChatRole, text: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: MessageContent::Text(text.to_string()),
            tool_call_id: None,
            tool_calls: None,
        }
    }

    #[test]
    fn test_single_text_message() {
        let msgs = vec![make_text_message(ChatRole::User, "Hello world")];
        let result = collect_tokenized_input(&msgs);
        assert_eq!(result.text_content, "Hello world");
        assert_eq!(result.extra_tokens, MESSAGE_OVERHEAD_TOKENS);
    }

    #[test]
    fn test_multiple_messages() {
        let msgs = vec![
            make_text_message(ChatRole::System, "You are helpful"),
            make_text_message(ChatRole::User, "Hi"),
            make_text_message(ChatRole::Assistant, "Hello!"),
        ];
        let result = collect_tokenized_input(&msgs);
        assert_eq!(result.text_content, "You are helpfulHiHello!");
        assert_eq!(result.extra_tokens, MESSAGE_OVERHEAD_TOKENS * 3);
    }

    #[test]
    fn test_message_with_image() {
        let msgs = vec![ChatMessage {
            role: ChatRole::User,
            content: MessageContent::Parts(vec![
                ContentPart::Text { text: "What's in this image?".to_string() },
                ContentPart::ImageUrl {
                    image_url: super::super::types::ImageUrl {
                        url: "https://example.com/img.png".to_string(),
                        detail: "auto".to_string(),
                    },
                },
            ]),
            tool_call_id: None,
            tool_calls: None,
        }];
        let result = collect_tokenized_input(&msgs);
        assert_eq!(result.text_content, "What's in this image?");
        assert_eq!(result.extra_tokens, MESSAGE_OVERHEAD_TOKENS + IMAGE_PART_TOKENS);
    }

    #[test]
    fn test_message_with_tool_calls() {
        let msgs = vec![ChatMessage {
            role: ChatRole::Assistant,
            content: MessageContent::Text(String::new()),
            tool_call_id: None,
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "get_weather".to_string(),
                    arguments: r#"{"city":"Beijing"}"#.to_string(),
                },
            }]),
        }];
        let result = collect_tokenized_input(&msgs);
        assert_eq!(result.text_content, "get_weather{\"city\":\"Beijing\"}");
        assert_eq!(result.extra_tokens, MESSAGE_OVERHEAD_TOKENS);
    }

    #[test]
    fn test_plain_text() {
        let result = collect_from_plain_text("Hello world");
        assert_eq!(result.text_content, "Hello world");
        assert_eq!(result.extra_tokens, 0);
    }
}
