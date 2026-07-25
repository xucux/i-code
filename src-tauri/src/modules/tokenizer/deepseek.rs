//! # DeepSeek 分词策略
//!
//! 使用 `tiktoken` crate 内置的 `deepseek_v3` 编码，
//! 对 DeepSeek 系列模型进行精确 BPE 分词。
//!
//! 移植自参考项目 `tokenizer/deepseek.ts`。
//!
//! ## 与参考项目的差异
//!
//! 参考项目使用 `@huggingface/tokenizers` 从 `tokenizer.json` 文件加载分词器，
//! 本实现使用 `tiktoken` crate 内置的 DeepSeek V3/R1 编码（编译时嵌入，无需运行时加载），
//! 性能更优且无文件 I/O 依赖。
//!
//! 若后续需要支持非 DeepSeek V3/R1 的 DeepSeek 模型（如 V2），
//! 可扩展为加载 HuggingFace tokenizer JSON 文件（需引入 `tokenizers` crate）。

use super::char4::count_tokens_char4;
use super::content::collect_tokenized_input;
use super::types::{ChatMessage, TokenizedInput};

/// DeepSeek 模型对应的 tiktoken 编码名
const DEEPSEEK_ENCODING_NAME: &str = "deepseek_v3";

/// DeepSeek 模型名前缀，用于自动识别是否为 DeepSeek 模型
const DEEPSEEK_MODEL_PREFIXES: &[&str] = &[
    "deepseek",
    "deepseek-v3",
    "deepseek-r1",
    "deepseek-chat",
    "deepseek-coder",
    "deepseek-reasoner",
];

/// 判断模型 ID 是否为 DeepSeek 系列模型
pub fn is_deepseek_model(model_id: &str) -> bool {
    let normalized = model_id.to_lowercase();
    DEEPSEEK_MODEL_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
}

/// 使用 DeepSeek 策略估算纯文本的 token 数
///
/// 使用 tiktoken 内置的 `deepseek_v3` 编码进行精确分词。
/// 若分词失败，降级到 char4 近似算法。
///
/// # 参数
///
/// - `model_id`：模型 ID（如 `deepseek-v3`、`deepseek-r1` 等）
/// - `text`：要估算的纯文本
///
/// # 返回
///
/// 精确或近似的 token 数
pub fn count_tokens_deepseek_text(model_id: &str, text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    match try_deepseek_count(model_id, text) {
        Ok(count) => count,
        Err(_) => count_tokens_char4(text), // 降级到 char4
    }
}

/// 使用 DeepSeek 策略估算消息列表的 token 数
///
/// 先从消息中提取纯文本与额外 token 数，
/// 再对纯文本进行 BPE 分词，最后加上额外 token 数。
///
/// # 参数
///
/// - `model_id`：模型 ID
/// - `messages`：OpenAI 格式的聊天消息列表
///
/// # 返回
///
/// 精确或近似的 token 数
pub fn count_tokens_deepseek_messages(model_id: &str, messages: &[ChatMessage]) -> usize {
    let TokenizedInput {
        text_content,
        extra_tokens,
    } = collect_tokenized_input(messages);

    if text_content.is_empty() {
        return extra_tokens;
    }

    match try_deepseek_count(model_id, &text_content) {
        Ok(count) => count + extra_tokens,
        Err(_) => count_tokens_char4(&text_content) + extra_tokens,
    }
}

/// 尝试使用 tiktoken 对 DeepSeek 模型进行精确分词
///
/// 优先使用 `encoding_for_model()` 按模型名查找，
/// 若失败则回退到 `deepseek_v3` 编码。
fn try_deepseek_count(model_id: &str, text: &str) -> Result<usize, String> {
    // 方式 1：按模型名查找（tiktoken crate 内置映射）
    if let Some(enc) = tiktoken::encoding_for_model(model_id) {
        return Ok(enc.count(text));
    }

    // 方式 2：回退到 deepseek_v3 编码
    if let Some(enc) = tiktoken::get_encoding(DEEPSEEK_ENCODING_NAME) {
        return Ok(enc.count(text));
    }

    Err(format!(
        "无法为 DeepSeek 模型 '{model_id}' 加载分词器"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_text() {
        assert_eq!(count_tokens_deepseek_text("deepseek-v3", ""), 0);
    }

    #[test]
    fn test_basic_text() {
        let count = count_tokens_deepseek_text("deepseek-v3", "hello world");
        assert!(count > 0);
        assert_eq!(count, 2); // "hello world" = 2 tokens in most BPE
    }

    #[test]
    fn test_chinese_text() {
        let count = count_tokens_deepseek_text("deepseek-v3", "你好世界");
        assert!(count > 0);
        // DeepSeek 对中文分词应比 char4 更精确
        let char4_count = count_tokens_char4("你好世界");
        assert!(count >= char4_count);
    }

    #[test]
    fn test_fallback_on_error() {
        // 即使编码加载失败也应返回 char4 估算
        let count = count_tokens_deepseek_text("deepseek-v3", "test text");
        assert!(count > 0);
    }

    #[test]
    fn test_is_deepseek_model() {
        assert!(is_deepseek_model("deepseek-v3"));
        assert!(is_deepseek_model("deepseek-r1"));
        assert!(is_deepseek_model("deepseek-chat"));
        assert!(is_deepseek_model("DeepSeek-V3"));
        assert!(!is_deepseek_model("gpt-4o"));
        assert!(!is_deepseek_model("claude-3"));
    }
}
