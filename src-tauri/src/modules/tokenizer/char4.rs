//! # char4 分词策略
//!
//! 近似算法：约 4 个 Unicode 字符 ≈ 1 token。
//!
//! 这是 VS Code 官方使用的近似估算方式，速度快但精度低，
//! 对中文等多字节语言偏差较大（中文约 1.5-2 字符/token）。
//!
//! 移植自参考项目 `tokenizer/char4.ts`。

use super::types::CHAR4_CHARS_PER_TOKEN;

/// 使用 char4 策略估算纯文本的 token 数
///
/// 算法：`ceil(unicode字符数 / 4)`
///
/// # 参数
///
/// - `text`：要估算的纯文本
///
/// # 返回
///
/// 估算的 token 数（最少 0）
pub fn count_tokens_char4(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let char_count = text.chars().count();
    (char_count + CHAR4_CHARS_PER_TOKEN - 1) / CHAR4_CHARS_PER_TOKEN // 向上取整
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        assert_eq!(count_tokens_char4(""), 0);
    }

    #[test]
    fn test_short_text() {
        // 1-4 字符 → 1 token
        assert_eq!(count_tokens_char4("a"), 1);
        assert_eq!(count_tokens_char4("ab"), 1);
        assert_eq!(count_tokens_char4("abc"), 1);
        assert_eq!(count_tokens_char4("abcd"), 1);
    }

    #[test]
    fn test_medium_text() {
        // 5-8 字符 → 2 tokens
        assert_eq!(count_tokens_char4("abcde"), 2);
        assert_eq!(count_tokens_char4("abcdefgh"), 2);
    }

    #[test]
    fn test_chinese() {
        // 中文字符也按 Unicode 字符数计算
        // "你好世界" = 4 字符 → 1 token（char4 低估中文）
        assert_eq!(count_tokens_char4("你好世界"), 1);
        // "你好世界测试文本" = 8 字符 → 2 tokens
        assert_eq!(count_tokens_char4("你好世界测试文本"), 2);
    }

    #[test]
    fn test_mixed() {
        // 混合 ASCII 与 CJK
        let text = "Hello你好World世界";
        assert_eq!(text.chars().count(), 14);
        assert_eq!(count_tokens_char4(text), 4); // ceil(14/4) = 4
    }
}
