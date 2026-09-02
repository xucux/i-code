//! # 聊天模块 DTO
//!
//! 会话与消息以 JSONL 持久化到程序目录 `chat/`。
//! 字段统一 `#[serde(rename_all = "camelCase")]`，与前端 `src/modules/chat/types.ts` 对齐。
//!
//! ## 逻辑要点
//!
//! - `ChatMessage.thinking`：与 `content` 分离，供 UI 折叠展示推理过程
//! - `ChatMessage.streaming`：占位助手消息为 true，完成后写回 false
//! - `SendChatMessageResult.request_id`：对应 `active_requests` 与中断 Command
//! - 流式事件同时带 delta 与累计 content/thinking，前端可直接覆盖本地状态

use serde::{Deserialize, Serialize};

/// 消息角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

impl ChatRole {
    #[expect(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

/// 传输模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChatTransportMode {
    /// 非流式 HTTP
    Http,
    /// SSE 流式
    #[default]
    Sse,
}

/// 网关入口协议
///
/// 决定向本地网关哪个端点发送请求：
/// - `Chat`：`POST /v1/chat/completions`（OpenAI 兼容，默认）
/// - `Messages`：`POST /v1/messages`（Anthropic 兼容）
/// - `Responses`：`POST /v1/responses`（OpenAI Responses API）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChatProtocol {
    #[default]
    Chat,
    Messages,
    Responses,
}

/// 附件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatAttachmentKind {
    /// 普通文本/代码等文件，内容并入 content
    File,
    /// 图片，转 base64 放入 image_url
    Image,
}

/// 会话中保存的附件元数据（名称 + 类型；图片另存 data URL）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachment {
    /// 附件 ID
    pub id: String,
    /// 原始文件名
    pub name: String,
    /// 附件类型
    pub kind: ChatAttachmentKind,
    /// MIME 类型（图片必填）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// 文本附件的完整内容；图片附件为空
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_content: Option<String>,
    /// 图片 data URL（`data:image/...;base64,...`）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_url: Option<String>,
    /// 文件大小（字节）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// 提示词库条目（来自 `app_config_dir/prompt/*.md`）
///
/// - `id`：文件名（如 `code-review.md`），作为读取详情的稳定键
/// - `title`：取自首个 `# ` 行；若无则用文件名 stem
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPrompt {
    pub id: String,
    pub title: String,
}

/// 提示词详情：正文（已按 125000 字符截断）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPromptContent {
    pub id: String,
    pub title: String,
    pub content: String,
    /// 是否因超过 125000 字符被截断
    pub truncated: bool,
}

/// 提示词正文最大字符数（超出截断并标记）
pub const CHAT_PROMPT_MAX_CHARS: usize = 125_000;

/// Token 用量
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTokenUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<i64>,
}

/// 单条聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: ChatRole,
    /// 展示用纯文本（用户输入原文 / 助手回复）
    pub content: String,
    /// 模型思考/推理过程（助手消息可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    /// 附件列表（名称与图片/文件内容）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<ChatAttachment>,
    /// Token 用量（助手消息）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ChatTokenUsage>,
    /// 生成该助手消息的模型 ID（`provider_slug/model_id`），仅助手消息
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// 是否仍在流式生成
    #[serde(default)]
    pub streaming: bool,
    /// 错误摘要（失败时；气泡内优先展示结构化错误）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// 错误码：HTTP 状态（如 `502`）或协议码（如 `invalid_api_key` / `GATEWAY`）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// 上游/网关错误响应 body 原文（不截断）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_body: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 会话摘要（列表项）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSessionSummary {
    pub id: String,
    pub title: String,
    /// 路由模型 ID：`{provider_slug}/{model_id}`
    pub model: String,
    pub transport_mode: ChatTransportMode,
    /// 网关入口协议（旧数据缺失时默认 `chat`）
    #[serde(default)]
    pub protocol: ChatProtocol,
    pub message_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

/// 完整会话（含消息）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub model: String,
    pub transport_mode: ChatTransportMode,
    #[serde(default)]
    pub protocol: ChatProtocol,
    pub messages: Vec<ChatMessage>,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建会话
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChatSessionInput {
    #[serde(default)]
    pub title: Option<String>,
    /// 路由模型 ID
    pub model: String,
    #[serde(default)]
    pub transport_mode: Option<ChatTransportMode>,
    #[serde(default)]
    pub protocol: Option<ChatProtocol>,
}

/// 更新会话
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChatSessionInput {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub transport_mode: Option<ChatTransportMode>,
    #[serde(default)]
    pub protocol: Option<ChatProtocol>,
}

/// 前端提交的附件（发送前已读取内容 / base64）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachmentInput {
    pub name: String,
    pub kind: ChatAttachmentKind,
    #[serde(default)]
    pub mime_type: Option<String>,
    /// 文本附件完整内容
    #[serde(default)]
    pub text_content: Option<String>,
    /// 图片 base64（不含 data: 前缀）或完整 data URL
    #[serde(default)]
    pub base64: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

/// 发送消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendChatMessageInput {
    pub session_id: String,
    /// 用户输入文本
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<ChatAttachmentInput>,
    /// 可选覆盖本轮传输模式
    #[serde(default)]
    pub transport_mode: Option<ChatTransportMode>,
    /// 可选覆盖本轮网关入口协议
    #[serde(default)]
    pub protocol: Option<ChatProtocol>,
    /// 当前应用 locale；首条消息时后端会注入为 system 消息
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// 本轮推理力度（reasoning_effort），如 `low` / `medium` / `high` / `none`；
    /// 非空时按协议注入请求体（chat → reasoning_effort，responses → reasoning.effort）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_effort: Option<String>,
}

/// 发送消息后的即时回执（用户消息 + 占位助手消息）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendChatMessageResult {
    pub session: ChatSessionSummary,
    pub user_message: ChatMessage,
    pub assistant_message: ChatMessage,
    /// 流式请求 ID，用于中断
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// 流式增量事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamChunkEvent {
    pub session_id: String,
    pub message_id: String,
    pub request_id: String,
    /// 正文增量（可为空：仅思考增量）
    #[serde(default)]
    pub delta: String,
    /// 当前累计正文
    pub content: String,
    /// 思考过程增量
    #[serde(default)]
    pub thinking_delta: String,
    /// 当前累计思考过程
    #[serde(default)]
    pub thinking: String,
}

/// 流式/HTTP 完成事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamDoneEvent {
    pub session_id: String,
    pub message_id: String,
    pub request_id: String,
    pub content: String,
    /// 完整思考过程（若有）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ChatTokenUsage>,
    /// 生成该条消息的模型 ID（`provider_slug/model_id`）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// 流式/HTTP 错误事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamErrorEvent {
    pub session_id: String,
    pub message_id: String,
    pub request_id: String,
    /// 错误摘要（兼容旧前端）
    pub error: String,
    /// 错误码（HTTP 状态或协议 code）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// 完整错误 body
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_body: Option<String>,
}

/// 中断结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbortChatResult {
    pub aborted: bool,
    pub request_id: String,
}
