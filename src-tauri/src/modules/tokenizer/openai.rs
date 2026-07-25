//! # OpenAI tiktoken 精确分词策略
//!
//! 使用 `tiktoken` crate（纯 Rust 高性能 BPE 分词器），
//! 按模型自动选择编码（o200k / cl100k / p50k / gpt2 / llama3 / deepseek_v3 等）。
//!
//! 移植自参考项目 `tokenizer/openai.ts`。
//!
//! ## 与参考项目的差异
//!
//! 参考项目手动维护模型→编码映射表（`O200K_BASE_PREFIXES` 等），
//! 本实现利用 `tiktoken` crate 内置的 `encoding_for_model()` 自动解析，
//! 支持范围更广且无需手动同步映射表。

use super::char4::count_tokens_char4;
use super::content::collect_tokenized_input;
use super::types::{ChatMessage, TokenizedInput};

/// 使用 OpenAI tiktoken 策略估算纯文本的 token 数
///
/// 根据模型 ID 选择对应 BPE 编码，对文本进行精确分词计数。
/// 若模型 ID 无法识别或分词失败，降级到 char4 近似算法。
///
/// # 参数
///
/// - `model_id`：模型 ID（如 `gpt-4o`、`gpt-4-turbo`、`o1-mini` 等）
/// - `text`：要估算的纯文本
///
/// # 返回
///
/// 精确或近似的 token 数
pub fn count_tokens_openai_text(model_id: &str, text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    match try_tiktoken_count(model_id, text) {
        Ok(count) => count,
        Err(_) => count_tokens_char4(text), // 降级到 char4
    }
}

/// 使用 OpenAI tiktoken 策略估算消息列表的 token 数
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
pub fn count_tokens_openai_messages(model_id: &str, messages: &[ChatMessage]) -> usize {
    let TokenizedInput {
        text_content,
        extra_tokens,
    } = collect_tokenized_input(messages);

    if text_content.is_empty() {
        return extra_tokens;
    }

    match try_tiktoken_count(model_id, &text_content) {
        Ok(count) => count + extra_tokens,
        Err(_) => {
            // 降级：char4 估算文本 + extra_tokens
            count_tokens_char4(&text_content) + extra_tokens
        }
    }
}

/// 尝试使用 tiktoken crate 对文本进行精确分词计数
///
/// 优先使用 `encoding_for_model()` 按模型名查找编码，
/// 若失败则尝试按已知编码名查找（`o200k_base`、`cl100k_base` 等）。
fn try_tiktoken_count(model_id: &str, text: &str) -> Result<usize, String> {
    // 方式 1：按模型名查找编码（覆盖最广，含 OpenAI / DeepSeek / Llama 等）
    if let Some(enc) = tiktoken::encoding_for_model(model_id) {
        return Ok(enc.count(text));
    }

    // 方式 2：模型名可能是编码名本身（如用户直接指定 `o200k_base`）
    if let Some(enc) = tiktoken::get_encoding(model_id) {
        return Ok(enc.count(text));
    }

    // 方式 3：按 OpenAI 编码名尝试（兼容参考项目的映射逻辑）
    let encoding_name = resolve_openai_encoding_name(model_id);
    if let Some(enc) = tiktoken::get_encoding(encoding_name) {
        return Ok(enc.count(text));
    }

    Err(format!("无法为模型 '{model_id}' 找到合适的 tiktoken 编码"))
}

/// 按 OpenAI 模型前缀/精确匹配解析编码名
///
/// 移植自参考项目 `openai.ts` 的 `resolveOpenAIEncodingName`。
/// 这是 `encoding_for_model()` 的后备方案，覆盖部分变体模型名。
fn resolve_openai_encoding_name(model_id: &str) -> &'static str {
    let normalized = model_id.to_lowercase();

    // o200k_base：GPT-4o / 4.1 / 4.5 / 5 / o1 / o3 / o4 系列
    const O200K_PREFIXES: &[&str] = &[
        "gpt-4o",
        "gpt-4.1",
        "gpt-4.5",
        "gpt-5",
        "chatgpt-4o",
        "o1",
        "o3",
        "o4",
        "gpt-oss",
        "codex-mini",
        "computer-use",
    ];
    const O200K_EXACT: &[&str] = &[
        "gpt-4.1",
        "gpt-4.1-mini",
        "gpt-4.1-nano",
        "gpt-4.5-preview",
        "gpt-5",
        "gpt-5-mini",
        "gpt-5-nano",
        "gpt-5-chat-latest",
        "o1",
        "o1-mini",
        "o1-preview",
        "o1-pro",
        "o3",
        "o3-mini",
        "o4-mini",
    ];

    if O200K_EXACT.contains(&normalized.as_str())
        || O200K_PREFIXES
            .iter()
            .any(|p| normalized.starts_with(p))
    {
        return "o200k_base";
    }

    // cl100k_base：GPT-4 / 4-turbo / 3.5-turbo 系列
    const CL100K_PREFIXES: &[&str] = &[
        "gpt-4-turbo",
        "gpt-4-32k",
        "gpt-4-vision-preview",
        "gpt-4-",
        "gpt-3.5-turbo",
        "babbage-002",
        "davinci-002",
    ];
    const CL100K_EXACT: &[&str] = &[
        "gpt-4",
        "gpt-4-turbo",
        "gpt-4-turbo-preview",
        "gpt-3.5-turbo",
        "gpt-3.5-turbo-instruct",
        "babbage-002",
        "davinci-002",
    ];

    if CL100K_EXACT.contains(&normalized.as_str())
        || CL100K_PREFIXES
            .iter()
            .any(|p| normalized.starts_with(p))
    {
        return "cl100k_base";
    }

    // p50k_base：旧版 davinci / cushman 系列
    const P50K_PREFIXES: &[&str] = &[
        "text-davinci-003",
        "text-davinci-002",
        "code-davinci-002",
        "code-cushman-002",
    ];
    if P50K_PREFIXES
        .iter()
        .any(|p| normalized.starts_with(p))
    {
        return "p50k_base";
    }

    // gpt2
    if normalized.starts_with("gpt2") {
        return "r50k_base";
    }

    // 默认回退到最新的 o200k_base
    "o200k_base"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_text() {
        assert_eq!(count_tokens_openai_text("gpt-4o", ""), 0);
    }

    #[test]
    fn test_basic_encoding() {
        // "hello world" 在 cl100k_base 下应为 2 tokens
        let count = count_tokens_openai_text("gpt-4", "hello world");
        assert_eq!(count, 2);
    }

    #[test]
    fn test_o200k_encoding() {
        let count = count_tokens_openai_text("gpt-4o", "hello world");
        assert_eq!(count, 2);
    }

    #[test]
    fn test_chinese_text() {
        // 中文文本的 token 计数应大于 char4 估算
        let count = count_tokens_openai_text("gpt-4o", "你好世界");
        assert!(count > 0);
        // char4 会给出 1 token（4字符/4），tiktoken 应给出更多
        let char4_count = count_tokens_char4("你好世界");
        assert!(count >= char4_count);
    }

    #[test]
    fn test_fallback_on_unknown_model() {
        // 未知模型应降级到 char4
        let count = count_tokens_openai_text("unknown-model-xyz", "hello world");
        assert!(count > 0);
    }

    #[test]
    fn test_encoding_name_resolution() {
        assert_eq!(resolve_openai_encoding_name("gpt-4o"), "o200k_base");
        assert_eq!(resolve_openai_encoding_name("gpt-4"), "cl100k_base");
        assert_eq!(resolve_openai_encoding_name("gpt-3.5-turbo"), "cl100k_base");
        assert_eq!(resolve_openai_encoding_name("o1"), "o200k_base");
        assert_eq!(resolve_openai_encoding_name("o3-mini"), "o200k_base");
        assert_eq!(
            resolve_openai_encoding_name("text-davinci-003"),
            "p50k_base"
        );
    }
}
