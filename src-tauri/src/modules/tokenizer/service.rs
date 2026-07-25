//! # Tokenizer 服务层
//!
//! 分词器注册表与统一调度中心。根据模型配置中的 `tokenizer` 字段
//! 选择对应分词策略，应用 `token_count_multiplier` 乘数，并处理降级。
//!
//! 移植自参考项目 `tokenizer/tokenizers.ts` 与 `service.ts` 的 `provideTokenCount`。
//!
//! ## 调用链
//!
//! ```text
//! 外部调用 provide_token_count(model_id, text, tokenizer, multiplier)
//!   → resolve_tokenizer_id() 解析策略 ID
//!   → 按策略分派到对应分词函数
//!   → 失败时 fallback 到 char4
//!   → 应用乘数 → ceil(base × multiplier)
//! ```

use super::char4::count_tokens_char4;
use super::conservative::{count_tokens_conservative, count_tokens_conservative_text};
use super::deepseek::{
    count_tokens_deepseek_messages, count_tokens_deepseek_text, is_deepseek_model,
};
use super::openai::{count_tokens_openai_messages, count_tokens_openai_text};
use super::types::{
    ChatMessage, TokenCountResult, TokenizerId, TokenizerInfo,
    DEFAULT_TOKEN_COUNT_MULTIPLIER, DEFAULT_TOKENIZER_ID,
};

// ===== 分词器注册表 =====

/// 分词器描述信息列表
///
/// 对标参考项目 `TOKENIZERS` 注册表，供前端 UI 展示选择器。
pub fn tokenizer_list() -> Vec<TokenizerInfo> {
    vec![
        TokenizerInfo {
            id: TokenizerId::Default.as_str().to_string(),
            label: "default".to_string(),
            description: "VS Code 官方近似算法（约 4 字符/token）".to_string(),
        },
        TokenizerInfo {
            id: TokenizerId::Char4.as_str().to_string(),
            label: "char4".to_string(),
            description: "约 4 字符/token 近似估算，速度快但精度低".to_string(),
        },
        TokenizerInfo {
            id: TokenizerId::Conservative.as_str().to_string(),
            label: "conservative".to_string(),
            description: "基于 UTF-8 字节的保守估算（3 字节/token），确保不超上下文窗口".to_string(),
        },
        TokenizerInfo {
            id: TokenizerId::Openai.as_str().to_string(),
            label: "openai".to_string(),
            description: "OpenAI tiktoken BPE 精确分词，按模型自动选择编码".to_string(),
        },
        TokenizerInfo {
            id: TokenizerId::Deepseek.as_str().to_string(),
            label: "deepseek".to_string(),
            description: "DeepSeek 官方 BPE 分词（deepseek_v3 编码）".to_string(),
        },
    ]
}

// ===== Token 计数主入口 =====

/// 估算纯文本的 token 数（服务层统一入口）
///
/// 按指定分词器策略估算，失败时降级到 char4，最终应用乘数。
///
/// # 参数
///
/// - `model_id`：模型 ID（如 `openai/gpt-4o`），提取模型名部分用于编码选择
/// - `text`：要估算的纯文本
/// - `tokenizer_id`：分词器策略（None 则使用默认值或按模型自动推断）
/// - `multiplier`：token 计数乘数（None 则使用默认值 1.0）
///
/// # 返回
///
/// `TokenCountResult` 包含最终 token 数、使用的分词器 ID 和乘数
pub fn provide_token_count(
    model_id: &str,
    text: &str,
    tokenizer_id: Option<&str>,
    multiplier: Option<f64>,
) -> TokenCountResult {
    // 解析模型名：从 `provider_slug/model_id` 格式中提取模型名
    let model_name = extract_model_name(model_id);

    // 解析分词器 ID：显式指定 → 自动推断 → 默认
    let resolved_tokenizer = if let Some(id_str) = tokenizer_id {
        TokenizerId::from_str_lossy(id_str)
    } else {
        auto_detect_tokenizer(&model_name)
    };

    // 解析乘数
    let resolved_multiplier = resolve_multiplier(multiplier);

    // 按策略执行分词
    let base_count = match resolved_tokenizer {
        TokenizerId::Default | TokenizerId::Char4 => {
            count_tokens_char4(text)
        }
        TokenizerId::Conservative => {
            count_tokens_conservative_text(text)
        }
        TokenizerId::Openai => {
            count_tokens_openai_text(&model_name, text)
        }
        TokenizerId::Deepseek => {
            count_tokens_deepseek_text(&model_name, text)
        }
    };

    // 应用乘数
    let final_count = apply_multiplier(base_count, resolved_multiplier);

    TokenCountResult {
        token_count: final_count,
        tokenizer_id: resolved_tokenizer.as_str().to_string(),
        multiplier: resolved_multiplier,
    }
}

/// 估算消息列表的 token 数（服务层统一入口）
///
/// 按指定分词器策略估算，失败时降级到 char4/conservative，最终应用乘数。
///
/// # 参数
///
/// - `model_id`：模型 ID
/// - `messages`：OpenAI 格式的聊天消息列表
/// - `tokenizer_id`：分词器策略（None 则自动推断）
/// - `multiplier`：token 计数乘数（None 则使用默认值 1.0）
///
/// # 返回
///
/// `TokenCountResult` 包含最终 token 数、使用的分词器 ID 和乘数
pub fn provide_message_token_count(
    model_id: &str,
    messages: &[ChatMessage],
    tokenizer_id: Option<&str>,
    multiplier: Option<f64>,
) -> TokenCountResult {
    let model_name = extract_model_name(model_id);

    // 解析分词器 ID
    let resolved_tokenizer = if let Some(id_str) = tokenizer_id {
        TokenizerId::from_str_lossy(id_str)
    } else {
        auto_detect_tokenizer(&model_name)
    };

    let resolved_multiplier = resolve_multiplier(multiplier);

    // 按策略执行分词
    let base_count = match resolved_tokenizer {
        TokenizerId::Default | TokenizerId::Char4 => {
            // char4 对消息列表：提取文本后按 char4 估算 + extra_tokens
            let input = super::content::collect_tokenized_input(messages);
            count_tokens_char4(&input.text_content) + input.extra_tokens
        }
        TokenizerId::Conservative => {
            count_tokens_conservative(messages)
        }
        TokenizerId::Openai => {
            count_tokens_openai_messages(&model_name, messages)
        }
        TokenizerId::Deepseek => {
            count_tokens_deepseek_messages(&model_name, messages)
        }
    };

    let final_count = apply_multiplier(base_count, resolved_multiplier);

    TokenCountResult {
        token_count: final_count,
        tokenizer_id: resolved_tokenizer.as_str().to_string(),
        multiplier: resolved_multiplier,
    }
}

// ===== 辅助函数 =====

/// 从模型路由 ID 中提取模型名
///
/// 输入格式：`provider_slug/model_id` 或纯 `model_id`
/// - `openai/gpt-4o` → `gpt-4o`
/// - `gpt-4o` → `gpt-4o`
fn extract_model_name(model_id: &str) -> String {
    if let Some(pos) = model_id.find('/') {
        model_id[pos + 1..].to_string()
    } else {
        model_id.to_string()
    }
}

/// 按模型名自动推断最合适的分词器
///
/// 推断规则：
/// - DeepSeek 系列模型 → deepseek
/// - OpenAI GPT / o 系列模型 → openai
/// - 其他 → default (char4)
fn auto_detect_tokenizer(model_name: &str) -> TokenizerId {
    if is_deepseek_model(model_name) {
        return TokenizerId::Deepseek;
    }

    // OpenAI 系列模型前缀
    let normalized = model_name.to_lowercase();
    const OPENAI_PREFIXES: &[&str] = &[
        "gpt-4", "gpt-3.5", "gpt-4o", "gpt-4.1", "gpt-4.5", "gpt-5",
        "o1", "o3", "o4", "chatgpt", "text-davinci", "code-davinci",
        "babbage", "davinci", "gpt2",
    ];
    if OPENAI_PREFIXES
        .iter()
        .any(|p| normalized.starts_with(p))
    {
        return TokenizerId::Openai;
    }

    DEFAULT_TOKENIZER_ID
}

/// 解析乘数，无效值回退到默认
fn resolve_multiplier(multiplier: Option<f64>) -> f64 {
    match multiplier {
        Some(m) if m.is_finite() && m > 0.0 => m,
        _ => DEFAULT_TOKEN_COUNT_MULTIPLIER,
    }
}

/// 应用乘数并向上取整
fn apply_multiplier(base: usize, multiplier: f64) -> usize {
    if multiplier == 1.0 {
        return base; // 乘数为 1 时直接返回，避免浮点精度问题
    }
    let result = base as f64 * multiplier;
    if result.is_finite() && result > 0.0 {
        result.ceil() as usize
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_model_name() {
        assert_eq!(extract_model_name("openai/gpt-4o"), "gpt-4o");
        assert_eq!(extract_model_name("deepseek/deepseek-v3"), "deepseek-v3");
        assert_eq!(extract_model_name("gpt-4o"), "gpt-4o");
        assert_eq!(extract_model_name("anthropic/claude-3"), "claude-3");
    }

    #[test]
    fn test_auto_detect_tokenizer() {
        assert_eq!(auto_detect_tokenizer("deepseek-v3"), TokenizerId::Deepseek);
        assert_eq!(auto_detect_tokenizer("gpt-4o"), TokenizerId::Openai);
        assert_eq!(auto_detect_tokenizer("gpt-4"), TokenizerId::Openai);
        assert_eq!(auto_detect_tokenizer("o1-mini"), TokenizerId::Openai);
        assert_eq!(auto_detect_tokenizer("claude-3"), TokenizerId::Default);
        assert_eq!(auto_detect_tokenizer("llama-3"), TokenizerId::Default);
    }

    #[test]
    fn test_resolve_multiplier() {
        assert_eq!(resolve_multiplier(None), 1.0);
        assert_eq!(resolve_multiplier(Some(1.5)), 1.5);
        assert_eq!(resolve_multiplier(Some(0.0)), 1.0);   // 非正 → 默认
        assert_eq!(resolve_multiplier(Some(-1.0)), 1.0);  // 非正 → 默认
        assert_eq!(resolve_multiplier(Some(f64::NAN)), 1.0); // 非有限 → 默认
        assert_eq!(resolve_multiplier(Some(f64::INFINITY)), 1.0);
    }

    #[test]
    fn test_apply_multiplier() {
        assert_eq!(apply_multiplier(100, 1.0), 100);
        assert_eq!(apply_multiplier(100, 1.5), 150);
        assert_eq!(apply_multiplier(100, 0.5), 50);
        assert_eq!(apply_multiplier(7, 1.5), 11); // ceil(10.5) = 11
    }

    #[test]
    fn test_provide_token_count_default() {
        let result = provide_token_count("openai/gpt-4o", "hello world", None, None);
        assert!(result.token_count > 0);
        // 未指定分词器时，gpt-4o 应自动推断为 openai
        assert_eq!(result.tokenizer_id, "openai");
    }

    #[test]
    fn test_provide_token_count_explicit_tokenizer() {
        let result = provide_token_count(
            "openai/gpt-4o",
            "hello world",
            Some("char4"),
            None,
        );
        assert_eq!(result.tokenizer_id, "char4");
        // "hello world" = 11 字符 → ceil(11/4) = 3
        assert_eq!(result.token_count, 3);
    }

    #[test]
    fn test_provide_token_count_with_multiplier() {
        let result = provide_token_count(
            "openai/gpt-4o",
            "hello world",
            Some("char4"),
            Some(2.0),
        );
        // "hello world" = 11 字符 → ceil(11/4) = 3, × 2.0 = 6
        assert_eq!(result.token_count, 6);
    }

    #[test]
    fn test_tokenizer_list() {
        let list = tokenizer_list();
        assert_eq!(list.len(), 5);
        assert_eq!(list[0].id, "default");
        assert_eq!(list[4].id, "deepseek");
    }
}
