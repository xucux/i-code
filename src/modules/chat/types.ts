/**
 * 聊天模块类型定义
 *
 * 与后端 `src-tauri/src/modules/chat/types.rs` 对齐（字段 camelCase）。
 *
 * ## 数据边界
 *
 * - 持久化：程序目录 `chat/sessions.jsonl` + `chat/messages/{sessionId}.jsonl`
 * - 运行时：发送走 Tauri Command；流式结果走事件 `chat:stream-*`
 * - 协议：经本地网关 `POST /v1/chat/completions`（OpenAI 兼容）
 *
 * ## 关键字段语义
 *
 * | 概念 | 说明 |
 * |------|------|
 * | model | 路由模型 ID `{provider_slug}/{model_id}` |
 * | thinking | 助手推理过程，与 content 分离展示 |
 * | streaming | 助手消息是否仍在生成 |
 * | usage | 单条助手消息 token；顶栏汇总各条 total |
 */

/** 消息角色 */
export type ChatRole = 'system' | 'user' | 'assistant'

/**
 * 传输模式
 * - `sse`：流式，前端靠事件增量刷新气泡
 * - `http`：整包返回，仍发 done 事件统一收口
 */
export type ChatTransportMode = 'http' | 'sse'

/**
 * 网关入口协议
 * - `chat`：`POST /v1/chat/completions`（OpenAI 兼容，默认）
 * - `messages`：`POST /v1/messages`（Anthropic 兼容）
 * - `responses`：`POST /v1/responses`（OpenAI Responses API）
 */
export type ChatProtocol = 'chat' | 'messages' | 'responses'

/**
 * 提示词库条目（来自 `app_config_dir/prompt/*.md`）
 *
 * - `id`：文件名（如 `code-review.md`），读取详情的稳定键
 * - `title`：取自首个 `# ` 行；无则用文件名 stem
 */
export interface ChatPrompt {
  id: string
  title: string
}

/**
 * 提示词详情：正文已按 125000 字符截断
 */
export interface ChatPromptContent {
  id: string
  title: string
  content: string
  /** 是否因超过 125000 字符被截断 */
  truncated: boolean
}

/**
 * 附件类型
 * - `file`：全文并入请求 content，名称写入会话
 * - `image`：base64 → OpenAI `image_url`，名称写入会话
 */
export type ChatAttachmentKind = 'file' | 'image'

/** 已落库/展示用的附件（消息内） */
export interface ChatAttachment {
  id: string
  /** 原始文件名（界面小字 / 标签） */
  name: string
  kind: ChatAttachmentKind
  mimeType?: string
  /** 文本附件全文；图片为空 */
  textContent?: string
  /** 图片 data URL，用于气泡缩略图与协议 */
  dataUrl?: string
  size?: number
}

/** 单条助手消息的 token 用量（气泡下方小字） */
export interface ChatTokenUsage {
  promptTokens?: number
  completionTokens?: number
  totalTokens?: number
}

/** 会话内单条消息 */
export interface ChatMessage {
  id: string
  sessionId: string
  role: ChatRole
  /** 展示正文（用户含附件名提示；助手为回复；失败时为错误码+body） */
  content: string
  /** 模型思考/推理过程（助手消息可选） */
  thinking?: string
  attachments: ChatAttachment[]
  usage?: ChatTokenUsage
  /** 是否仍在流式生成（控制光标与思考块默认展开） */
  streaming: boolean
  /** 错误摘要 */
  error?: string
  /** HTTP 状态或协议错误码，如 `401` / `invalid_api_key` */
  errorCode?: string
  /** 上游/网关错误响应 body 原文 */
  errorBody?: string
  createdAt: string
  updatedAt: string
}

/** 会话列表项（不含消息体） */
export interface ChatSessionSummary {
  id: string
  title: string
  /** 路由模型 ID：`{provider_slug}/{model_id}` */
  model: string
  transportMode: ChatTransportMode
  /** 网关入口协议（旧数据缺失时默认 `chat`） */
  protocol: ChatProtocol
  messageCount: number
  createdAt: string
  updatedAt: string
}

/** 完整会话（含消息列表） */
export interface ChatSession {
  id: string
  title: string
  model: string
  transportMode: ChatTransportMode
  protocol: ChatProtocol
  messages: ChatMessage[]
  createdAt: string
  updatedAt: string
}

/** 创建会话入参 */
export interface CreateChatSessionInput {
  title?: string
  model: string
  transportMode?: ChatTransportMode
  protocol?: ChatProtocol
}

/** 更新会话入参（部分字段） */
export interface UpdateChatSessionInput {
  title?: string
  model?: string
  transportMode?: ChatTransportMode
  protocol?: ChatProtocol
}

/** 发送消息时提交的附件（前端已读内容/base64） */
export interface ChatAttachmentInput {
  name: string
  kind: ChatAttachmentKind
  mimeType?: string
  textContent?: string
  /** 图片 base64（可含或不含 data: 前缀） */
  base64?: string
  size?: number
}

/** 发送消息入参 */
export interface SendChatMessageInput {
  sessionId: string
  content: string
  attachments?: ChatAttachmentInput[]
  /** 可选覆盖本轮传输模式 */
  transportMode?: ChatTransportMode
  /** 可选覆盖本轮网关入口协议 */
  protocol?: ChatProtocol
  /** 当前应用 locale；首条消息时后端会注入为 system 消息 */
  locale?: string
}

/**
 * 发送即时回执：用户消息 + 占位助手消息。
 * 流式正文/思考/用量通过 `chat:stream-*` 事件后续更新。
 */
export interface SendChatMessageResult {
  session: ChatSessionSummary
  userMessage: ChatMessage
  assistantMessage: ChatMessage
  /** 用于 `chat_message_abort` 中断 */
  requestId?: string
}

/** 流式增量事件（`chat:stream-chunk`） */
export interface ChatStreamChunkEvent {
  sessionId: string
  messageId: string
  requestId: string
  /** 正文增量（可为空：仅思考增量） */
  delta: string
  /** 当前累计正文 */
  content: string
  /** 思考过程增量 */
  thinkingDelta?: string
  /** 当前累计思考过程 */
  thinking?: string
}

/** 流式/HTTP 完成事件（`chat:stream-done`） */
export interface ChatStreamDoneEvent {
  sessionId: string
  messageId: string
  requestId: string
  content: string
  /** 完整思考过程（若有） */
  thinking?: string
  usage?: ChatTokenUsage
}

/** 流式/HTTP 错误事件（`chat:stream-error`） */
export interface ChatStreamErrorEvent {
  sessionId: string
  messageId: string
  requestId: string
  /** 错误摘要 */
  error: string
  /** HTTP 状态或协议错误码 */
  errorCode?: string
  /** 完整错误 body */
  errorBody?: string
}

/** 中断请求结果 */
export interface AbortChatResult {
  aborted: boolean
  requestId: string
}

/**
 * 输入区待发送的本地附件预览（尚未写入会话 JSONL）。
 * 发送时映射为 `ChatAttachmentInput`。
 */
export interface PendingAttachment {
  /** 本地临时 ID（仅前端移除/列表 key） */
  localId: string
  name: string
  kind: ChatAttachmentKind
  mimeType?: string
  textContent?: string
  /** 图片预览 data URL */
  dataUrl?: string
  base64?: string
  size?: number
}
