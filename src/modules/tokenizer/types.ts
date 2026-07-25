/**
 * Tokenizer 模块类型定义
 *
 * 与后端 `src-tauri/src/modules/tokenizer/types.rs` 对齐。
 * 后端 Rust 通过 serde 序列化，前端类型定义保持 camelCase。
 */

// ===== 分词器标识 =====

/** 分词器策略标识 */
export type TokenizerId = 'default' | 'char4' | 'conservative' | 'openai' | 'deepseek'

// ===== 消息内容类型 =====

/** 聊天消息角色 */
export type ChatRole = 'system' | 'user' | 'assistant' | 'tool'

/** 图片 URL 结构 */
export interface ImageUrl {
  /** 图片 URL 或 data URI */
  url: string
  /** 图片细节级别：low / high / auto */
  detail: string
}

/** 消息内容部分 */
export type ContentPart =
  | { type: 'text'; text: string }
  | { type: 'image_url'; imageUrl: ImageUrl }

/** 消息内容：字符串或内容部分数组 */
export type MessageContent = string | ContentPart[]

/** 工具调用中的函数信息 */
export interface FunctionCall {
  /** 函数名 */
  name: string
  /** 函数参数（JSON 字符串） */
  arguments: string
}

/** 工具调用 */
export interface ToolCall {
  /** 工具调用 ID */
  id: string
  /** 工具类型，通常为 "function" */
  type: string
  /** 函数调用信息 */
  function: FunctionCall
}

/** 聊天消息（对齐 OpenAI ChatCompletion API 格式） */
export interface ChatMessage {
  /** 消息角色 */
  role: ChatRole
  /** 消息内容 */
  content: MessageContent
  /** 工具调用 ID（role=tool 时） */
  toolCallId?: string
  /** 工具调用列表（role=assistant 时） */
  toolCalls?: ToolCall[]
}

// ===== 分词结果 =====

/** Token 计数结果 */
export interface TokenCountResult {
  /** 估算的 token 数 */
  tokenCount: number
  /** 使用的分词器 ID */
  tokenizerId: string
  /** 应用的乘数 */
  multiplier: number
}

/** 分词器描述信息 */
export interface TokenizerInfo {
  /** 分词器 ID */
  id: string
  /** 显示标签 */
  label: string
  /** 描述说明 */
  description: string
}

// ===== Command 入参 =====

/** Token 计数 Command 入参 */
export interface TokenCountInput {
  /** 模型 ID（格式：provider_slug/model_id） */
  modelId: string
  /** 要计算 token 的文本内容 */
  text: string
  /** 指定分词器（覆盖模型配置中的 tokenizer） */
  tokenizer?: string
  /** 指定乘数（覆盖模型配置中的 tokenCountMultiplier） */
  multiplier?: number
}

/** 消息列表 Token 计数 Command 入参 */
export interface MessageTokenCountInput {
  /** 模型 ID（格式：provider_slug/model_id） */
  modelId: string
  /** 聊天消息列表 */
  messages: ChatMessage[]
  /** 指定分词器（覆盖模型配置中的 tokenizer） */
  tokenizer?: string
  /** 指定乘数（覆盖模型配置中的 tokenCountMultiplier） */
  multiplier?: number
}
