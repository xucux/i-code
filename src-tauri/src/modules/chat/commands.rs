//! # 聊天模块 Tauri Command 声明
//!
//! 前端经 `invokeCommand` / `use-chat` 调用，禁止业务组件直接 `invoke`。
//!
//! | Command | 作用 |
//! |---------|------|
//! | `chat_session_list` | 会话列表 |
//! | `chat_session_get` | 完整会话（含消息） |
//! | `chat_session_create` / `update` / `delete` | 会话 CRUD |
//! | `chat_message_send` | 发送；立即返回占位，流式走事件 |
//! | `chat_message_abort` | 按 `request_id` 中断 |
//! | `chat_message_delete` | 删除单条消息 |
//! | `chat_export_html` | 导出 HTML 到 `exports/` 目录 |

use tauri::State;

use crate::error::IcodeResult;

use super::service::ChatServiceHandle;
use super::types::{
    AbortChatResult, ChatPrompt, ChatPromptContent, ChatSession, ChatSessionSummary,
    CreateChatSessionInput, SendChatMessageInput, SendChatMessageResult, UpdateChatSessionInput,
};

/// 列出会话
#[tauri::command]
pub async fn chat_session_list(
    state: State<'_, ChatServiceHandle>,
) -> IcodeResult<Vec<ChatSessionSummary>> {
    state.service().list_sessions()
}

/// 获取完整会话（含消息）
#[tauri::command]
pub async fn chat_session_get(
    state: State<'_, ChatServiceHandle>,
    id: String,
) -> IcodeResult<ChatSession> {
    state.service().get_session(&id)
}

/// 创建会话
#[tauri::command]
pub async fn chat_session_create(
    state: State<'_, ChatServiceHandle>,
    input: CreateChatSessionInput,
) -> IcodeResult<ChatSessionSummary> {
    state.service().create_session(input)
}

/// 更新会话
#[tauri::command]
pub async fn chat_session_update(
    state: State<'_, ChatServiceHandle>,
    id: String,
    input: UpdateChatSessionInput,
) -> IcodeResult<ChatSessionSummary> {
    state.service().update_session(&id, input)
}

/// 删除会话
#[tauri::command]
pub async fn chat_session_delete(
    state: State<'_, ChatServiceHandle>,
    id: String,
) -> IcodeResult<()> {
    state.service().delete_session(&id)
}

/// 发送消息（立即返回占位，流式结果走事件）
#[tauri::command]
pub async fn chat_message_send(
    state: State<'_, ChatServiceHandle>,
    input: SendChatMessageInput,
) -> IcodeResult<SendChatMessageResult> {
    let service = state.clone_inner();
    service.send_message(input)
}

/// 中断进行中的请求
#[tauri::command]
pub async fn chat_message_abort(
    state: State<'_, ChatServiceHandle>,
    request_id: String,
) -> IcodeResult<AbortChatResult> {
    state.service().abort_request(&request_id)
}

/// 删除单条消息（从会话 JSONL 中移除并回写摘要计数）
#[tauri::command]
pub async fn chat_message_delete(
    state: State<'_, ChatServiceHandle>,
    session_id: String,
    message_id: String,
) -> IcodeResult<()> {
    state.service().delete_message(&session_id, &message_id)
}

/// 导出 HTML 到应用配置目录 `exports/`，返回写入文件的绝对路径
#[tauri::command]
pub async fn chat_export_html(
    state: State<'_, ChatServiceHandle>,
    html: String,
    filename: String,
) -> IcodeResult<String> {
    state.service().export_html(&html, &filename)
}

// ===== 提示词库（prompt 目录下 *.md 文件） =====

/// 列出所有提示词：读取 `app_config_dir/prompt/*.md`，标题取自首个 `# ` 行
#[tauri::command]
pub async fn chat_prompt_list(
    state: State<'_, ChatServiceHandle>,
) -> IcodeResult<Vec<ChatPrompt>> {
    state.service().list_prompts()
}

/// 读取提示词正文：超过 125000 字符自动截断并标记 `truncated`
#[tauri::command]
pub async fn chat_prompt_get(
    state: State<'_, ChatServiceHandle>,
    id: String,
) -> IcodeResult<ChatPromptContent> {
    state.service().get_prompt(&id)
}
