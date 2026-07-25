//! # Tokenizer Tauri Commands
//!
//! 向前端暴露的分词器 Command 接口。
//!
//! ## Command 列表
//!
//! | Command | 说明 |
//! |--------|------|
//! | `tokenizer_list` | 获取所有可用分词器信息列表 |
//! | `tokenizer_count` | 估算纯文本的 token 数 |
//! | `tokenizer_count_messages` | 估算消息列表的 token 数 |

use super::service;
use super::types::{MessageTokenCountInput, TokenCountInput, TokenCountResult, TokenizerInfo};

/// 获取所有可用分词器信息列表
///
/// 供前端模型配置页面的 tokenizer 选择器使用。
#[tauri::command]
pub fn tokenizer_list() -> Vec<TokenizerInfo> {
    service::tokenizer_list()
}

/// 估算纯文本的 token 数
///
/// 入参 `TokenCountInput` 包含模型 ID、文本、可选分词器覆盖与乘数覆盖。
#[tauri::command]
pub fn tokenizer_count(input: TokenCountInput) -> TokenCountResult {
    service::provide_token_count(
        &input.model_id,
        &input.text,
        input.tokenizer.as_deref(),
        input.multiplier,
    )
}

/// 估算消息列表的 token 数
///
/// 入参 `MessageTokenCountInput` 包含模型 ID、消息列表、可选分词器覆盖与乘数覆盖。
#[tauri::command]
pub fn tokenizer_count_messages(input: MessageTokenCountInput) -> TokenCountResult {
    service::provide_message_token_count(
        &input.model_id,
        &input.messages,
        input.tokenizer.as_deref(),
        input.multiplier,
    )
}
