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

use tauri::State;

use crate::error::IcodeResult;

use super::service::ChatServiceHandle;
use super::types::{
    AbortChatResult, ChatSession, ChatSessionSummary, CreateChatSessionInput,
    SendChatMessageInput, SendChatMessageResult, UpdateChatSessionInput,
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
