//! # Conservative 保守估算策略
//!
//! 基于 UTF-8 字节的保守 token 计数估算器。
//!
//! 核心公式：`ceil(utf8_bytes / 3) + extra_tokens`
//!
//! 设计意图：宁可高估也不低估，确保不超出模型上下文窗口。
//! 代价是可能过早触发上下文压缩。
//!
//! 移植自参考项目 `tokenizer/conservative.ts`。

use super::types::{
    ChatMessage, ContentPart, MessageContent, ToolCall,
    CONSERVATIVE_BYTES_PER_TOKEN, MESSAGE_OVERHEAD_TOKENS, IMAGE_PART_TOKENS,
};

/// 计数状态
struct CountState {
    /// 累计 UTF-8 字节数
    utf8_bytes: usize,
    /// 额外 token 数（图片、二进制等非文本部分）
    extra_tokens: usize,
}

impl CountState {
    fn new() -> Self {
        Self {
            utf8_bytes: 0,
            extra_tokens: 0,
        }
    }

    /// 累加文本的 UTF-8 字节数
    fn add_utf8_bytes(&mut self, text: &str) {
        if !text.is_empty() {
            self.utf8_bytes += text.len(); // Rust str::len() 返回 UTF-8 字节数
        }
    }

    /// 计算最终 token 数
    fn total_tokens(&self) -> usize {
        let tokens_from_bytes = (self.utf8_bytes + CONSERVATIVE_BYTES_PER_TOKEN - 1)
            / CONSERVATIVE_BYTES_PER_TOKEN; // 向上取整
        tokens_from_bytes + self.extra_tokens
    }
}

/// 从单条消息中统计 UTF-8 字节数与额外 token 数
fn count_from_message(state: &mut CountState, msg: &ChatMessage) {
    // 每条消息额外开销
    state.extra_tokens += MESSAGE_OVERHEAD_TOKENS;

    // 处理 content
    match &msg.content {
        MessageContent::Text(s) => {
            state.add_utf8_bytes(s);
        }
        MessageContent::Parts(parts) => {
            for part in parts {
                count_from_content_part(state, part);
            }
        }
    }

    // 处理 tool_calls
    if let Some(tool_calls) = &msg.tool_calls {
        for tc in tool_calls {
            count_from_tool_call(state, tc);
        }
    }
}

/// 从 ContentPart 中统计
fn count_from_content_part(state: &mut CountState, part: &ContentPart) {
    match part {
        ContentPart::Text { text } => {
            state.add_utf8_bytes(text);
        }
        ContentPart::ImageUrl { .. } => {
            // 图片固定按 512 token 估算
            state.extra_tokens += IMAGE_PART_TOKENS;
        }
    }
}

/// 从 ToolCall 中统计
fn count_from_tool_call(state: &mut CountState, tc: &ToolCall) {
    // 工具名计入字节
    state.add_utf8_bytes(&tc.function.name);
    // 工具参数 JSON 计入字节
    state.add_utf8_bytes(&tc.function.arguments);
}

/// 使用 conservative 策略估算消息列表的 token 数
///
/// # 参数
///
/// - `messages`：OpenAI 格式的聊天消息列表
///
/// # 返回
///
/// 估算的 token 数
pub fn count_tokens_conservative(messages: &[ChatMessage]) -> usize {
    let mut state = CountState::new();

    for msg in messages {
        count_from_message(&mut state, msg);
    }

    state.total_tokens()
}

/// 使用 conservative 策略估算纯文本的 token 数
///
/// 简便方法：直接按 UTF-8 字节数估算，无消息开销。
pub fn count_tokens_conservative_text(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let bytes = text.len(); // UTF-8 字节数
    (bytes + CONSERVATIVE_BYTES_PER_TOKEN - 1) / CONSERVATIVE_BYTES_PER_TOKEN
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{ChatRole, FunctionCall, ImageUrl};

    fn make_text_message(role: ChatRole, text: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: MessageContent::Text(text.to_string()),
            tool_call_id: None,
            tool_calls: None,
        }
    }

    #[test]
    fn test_empty_text() {
        assert_eq!(count_tokens_conservative_text(""), 0);
    }

    #[test]
    fn test_ascii_text() {
        // "Hello" = 5 字节 → ceil(5/3) = 2 tokens
        assert_eq!(count_tokens_conservative_text("Hello"), 2);
        // "Hi" = 2 字节 → ceil(2/3) = 1 token
        assert_eq!(count_tokens_conservative_text("Hi"), 1);
    }

    #[test]
    fn test_chinese_text() {
        // "你好" = 6 字节（每中文字符 3 UTF-8 字节）→ ceil(6/3) = 2 tokens
        assert_eq!(count_tokens_conservative_text("你好"), 2);
        // "你好世界" = 12 字节 → ceil(12/3) = 4 tokens
        assert_eq!(count_tokens_conservative_text("你好世界"), 4);
    }

    #[test]
    fn test_single_message() {
        let msgs = vec![make_text_message(ChatRole::User, "Hello")];
        let count = count_tokens_conservative(&msgs);
        // "Hello" = 5 字节 → ceil(5/3) = 2, + 4 (overhead) = 6
        assert_eq!(count, 6);
    }

    #[test]
    fn test_multiple_messages() {
        let msgs = vec![
            make_text_message(ChatRole::System, "You are helpful"), // 16 字节 → 6 + 4 = 10
            make_text_message(ChatRole::User, "Hi"),                // 2 字节 → 1 + 4 = 5
        ];
        let count = count_tokens_conservative(&msgs);
        // 总字节 = 18 → ceil(18/3) = 6, 额外 = 4*2 = 8, 总计 = 14
        assert_eq!(count, 14);
    }

    #[test]
    fn test_message_with_image() {
        let msgs = vec![ChatMessage {
            role: ChatRole::User,
            content: MessageContent::Parts(vec![
                ContentPart::Text { text: "What?".to_string() },
                ContentPart::ImageUrl {
                    image_url: ImageUrl {
                        url: "https://example.com/img.png".to_string(),
                        detail: "auto".to_string(),
                    },
                },
            ]),
            tool_call_id: None,
            tool_calls: None,
        }];
        let count = count_tokens_conservative(&msgs);
        // "What?" = 5 字节 → ceil(5/3) = 2, + 4 (overhead) + 512 (image) = 518
        assert_eq!(count, 518);
    }

    #[test]
    fn test_tool_call_message() {
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
        let count = count_tokens_conservative(&msgs);
        // "get_weather" = 11 字节, '{"city":"Beijing"}' = 18 字节, 共 29 字节
        // ceil(29/3) = 10, + 4 (overhead) = 14
        assert_eq!(count, 14);
    }
}
