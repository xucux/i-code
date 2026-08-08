/**
 * 聊天模块 hooks 与 Command 封装
 *
 * ## 职责
 *
 * | 导出 | 说明 |
 * |------|------|
 * | `list/get/create/update/delete/send/abort` | 薄封装 `invokeCommand` |
 * | `useChatSessions` | 左侧会话列表状态与 CRUD |
 * | `useChatSession` | 当前会话消息、发送/中断、流式事件合并 |
 * | `readFileAsPendingAttachment` | 浏览器 File → 待发送附件 |
 *
 * ## 逻辑描述
 *
 * 1. **列表**：挂载时 `chat_session_list`，变更后本地乐观更新再可选 refetch。
 * 2. **当前会话**：`sessionId` 变化时 `chat_session_get`；若本地已有 streaming 消息则不覆盖。
 * 3. **发送**：`chat_message_send` 立即带回用户消息 + 占位助手消息 + `requestId`。
 * 4. **流式**：全局 `listen` 三事件，按 `sessionId` 过滤后合并到对应 `messageId`。
 * 5. **中断**：`chat_message_abort(requestId)`；完成/错误事件会清 `sending`。
 *
 * 业务组件勿直接 `invoke`，统一走本文件。
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invokeCommand } from '@/hooks/use-command'
import { getLocale } from '@/modules/i18n/i18n'
import type {
  AbortChatResult,
  ChatMessage,
  ChatPrompt,
  ChatPromptContent,
  ChatProtocol,
  ChatSession,
  ChatSessionSummary,
  ChatStreamChunkEvent,
  ChatStreamDoneEvent,
  ChatStreamErrorEvent,
  ChatTransportMode,
  CreateChatSessionInput,
  PendingAttachment,
  SendChatMessageResult,
  UpdateChatSessionInput,
} from '@/modules/chat/types'

/** 后端 `app.emit` 事件名，与 `service.rs` 常量一致 */
export const CHAT_EVENTS = {
  STREAM_CHUNK: 'chat:stream-chunk',
  STREAM_DONE: 'chat:stream-done',
  STREAM_ERROR: 'chat:stream-error',
} as const

export async function listChatSessions(): Promise<ChatSessionSummary[]> {
  return invokeCommand<ChatSessionSummary[]>('chat_session_list')
}

export async function getChatSession(id: string): Promise<ChatSession> {
  return invokeCommand<ChatSession>('chat_session_get', { id })
}

export async function createChatSession(
  input: CreateChatSessionInput
): Promise<ChatSessionSummary> {
  return invokeCommand<ChatSessionSummary>('chat_session_create', { input })
}

export async function updateChatSession(
  id: string,
  input: UpdateChatSessionInput
): Promise<ChatSessionSummary> {
  return invokeCommand<ChatSessionSummary>('chat_session_update', { id, input })
}

export async function deleteChatSession(id: string): Promise<void> {
  await invokeCommand<void>('chat_session_delete', { id })
}

export async function sendChatMessage(input: {
  sessionId: string
  content: string
  attachments?: PendingAttachment[]
  transportMode?: ChatTransportMode
  protocol?: ChatProtocol
}): Promise<SendChatMessageResult> {
  const attachments = (input.attachments ?? []).map((a) => ({
    name: a.name,
    kind: a.kind,
    mimeType: a.mimeType,
    textContent: a.textContent,
    base64: a.base64 ?? a.dataUrl,
    size: a.size,
  }))
  return invokeCommand<SendChatMessageResult>('chat_message_send', {
    input: {
      sessionId: input.sessionId,
      content: input.content,
      attachments,
      transportMode: input.transportMode,
      protocol: input.protocol,
      locale: getLocale(),
    },
  })
}

export async function abortChatMessage(requestId: string): Promise<AbortChatResult> {
  return invokeCommand<AbortChatResult>('chat_message_abort', { requestId })
}

/** 删除单条消息（从会话 JSONL 移除并回写摘要计数） */
export async function deleteChatMessage(sessionId: string, messageId: string): Promise<void> {
  await invokeCommand<void>('chat_message_delete', { sessionId, messageId })
}

/** 导出 HTML 到应用配置目录 exports/，返回写入文件绝对路径 */
export async function exportChatHtml(html: string, filename: string): Promise<string> {
  return invokeCommand<string>('chat_export_html', { html, filename })
}

// ===== 提示词库（prompt 目录下 *.md） =====

/** 列出所有提示词（标题取自首个 `# ` 行） */
export async function listChatPrompts(): Promise<ChatPrompt[]> {
  return invokeCommand<ChatPrompt[]>('chat_prompt_list')
}

/** 读取提示词正文（超过 125000 字符自动截断） */
export async function getChatPrompt(id: string): Promise<ChatPromptContent> {
  return invokeCommand<ChatPromptContent>('chat_prompt_get', { id })
}

/**
 * 会话列表 hook
 *
 * 驱动左侧 `SessionList`：加载、新建、删除、更新（模型/模式/标题）。
 */
export function useChatSessions(): {
  sessions: ChatSessionSummary[]
  loading: boolean
  refetch: () => Promise<void>
  create: (input: CreateChatSessionInput) => Promise<ChatSessionSummary | null>
  remove: (id: string) => Promise<boolean>
  update: (id: string, input: UpdateChatSessionInput) => Promise<ChatSessionSummary | null>
} {
  const [sessions, setSessions] = useState<ChatSessionSummary[]>([])
  const [loading, setLoading] = useState(false)

  const refetch = useCallback(async () => {
    setLoading(true)
    try {
      const list = await listChatSessions()
      setSessions(list)
    } catch {
      setSessions([])
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refetch()
  }, [refetch])

  const create = useCallback(
    async (input: CreateChatSessionInput) => {
      try {
        const session = await createChatSession(input)
        setSessions((prev) => [session, ...prev.filter((s) => s.id !== session.id)])
        return session
      } catch {
        return null
      }
    },
    []
  )

  const remove = useCallback(async (id: string) => {
    try {
      await deleteChatSession(id)
      setSessions((prev) => prev.filter((s) => s.id !== id))
      return true
    } catch {
      return false
    }
  }, [])

  const update = useCallback(async (id: string, input: UpdateChatSessionInput) => {
    try {
      const session = await updateChatSession(id, input)
      setSessions((prev) =>
        prev
          .map((s) => (s.id === id ? session : s))
          .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
      )
      return session
    } catch {
      return null
    }
  }, [])

  return { sessions, loading, refetch, create, remove, update }
}

/*
 * @param sessionId 选中会话；null 时清空消息且不请求后端
 *
 * 注意：新建会话后立刻 send 时，父级需等 `sessionId` 传入本 hook 后再调 `send`，
 * 或使用 `ChatPage` 的 `pendingSendRef` 模式。
 **
 * 当前会话消息 + 流式更新
 */
export function useChatSession(sessionId: string | null): {
  session: ChatSession | null
  messages: ChatMessage[]
  loading: boolean
  activeRequestId: string | null
  sending: boolean
  reload: (id?: string) => Promise<void>
  send: (content: string, attachments: PendingAttachment[], transportMode?: ChatTransportMode, protocol?: ChatProtocol) => Promise<boolean>
  abort: () => Promise<void>
  /** 删除单条消息：后端移除 JSONL 条目，前端同步 state */
  deleteMessage: (messageId: string) => Promise<boolean>
  applySessionSummary: (summary: ChatSessionSummary) => void
  /** 外部调用 sendChatMessage 后同步本地消息与 requestId */
  applySendResult: (result: SendChatMessageResult) => void
} {
  const [session, setSession] = useState<ChatSession | null>(null)
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [loading, setLoading] = useState(false)
  const [sending, setSending] = useState(false)
  const [activeRequestId, setActiveRequestId] = useState<string | null>(null)
  const sessionIdRef = useRef(sessionId)
  sessionIdRef.current = sessionId

  const reload = useCallback(async (id?: string) => {
    const targetId = id ?? sessionId
    if (!targetId) {
      setSession(null)
      setMessages([])
      return
    }
    setLoading(true)
    try {
      const data = await getChatSession(targetId)
      setSession(data)
      setMessages(data.messages)
    } catch {
      setSession(null)
      setMessages([])
    } finally {
      setLoading(false)
    }
  }, [sessionId])

  useEffect(() => {
    let cancelled = false
    setActiveRequestId(null)
    setSending(false)

    const load = async () => {
      if (!sessionId) {
        setSession(null)
        setMessages([])
        return
      }
      setLoading(true)
      try {
        const data = await getChatSession(sessionId)
        if (cancelled) return
        setSession(data)
        // 若本地已有同会话更新内容，避免异步 reload 覆盖掉刚发送/流式中的消息
        setMessages((prev) => {
          const sameSession = prev.length > 0 && prev.every((m) => m.sessionId === sessionId)
          if (!sameSession) return data.messages
          const hasLocalStreaming = prev.some((m) => m.streaming)
          if (hasLocalStreaming) return prev
          if (prev.length >= data.messages.length) return prev
          return data.messages
        })
      } catch {
        if (!cancelled) {
          setSession(null)
          setMessages([])
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    }

    void load()
    return () => {
      cancelled = true
    }
  }, [sessionId])

  // 监听流式事件
  useEffect(() => {
    const unlisteners: UnlistenFn[] = []
    let cancelled = false

    const setup = async () => {
      const u1 = await listen<ChatStreamChunkEvent>(CHAT_EVENTS.STREAM_CHUNK, (event) => {
        const payload = event.payload
        if (payload.sessionId !== sessionIdRef.current) return
        setMessages((prev) =>
          prev.map((m) =>
            m.id === payload.messageId
              ? {
                  ...m,
                  content: payload.content,
                  thinking: payload.thinking || m.thinking,
                  streaming: true,
                }
              : m
          )
        )
      })
      const u2 = await listen<ChatStreamDoneEvent>(CHAT_EVENTS.STREAM_DONE, (event) => {
        const payload = event.payload
        if (payload.sessionId !== sessionIdRef.current) return
        setMessages((prev) =>
        prev.map((m) =>
          m.id === payload.messageId
            ? {
                ...m,
                content: payload.content,
                thinking: payload.thinking || m.thinking,
                usage: payload.usage,
                model: payload.model ?? m.model,
                streaming: false,
                error: undefined,
                errorCode: undefined,
                errorBody: undefined,
              }
            : m
        )
      )
        setActiveRequestId((cur) => (cur === payload.requestId ? null : cur))
        setSending(false)
      })
      const u3 = await listen<ChatStreamErrorEvent>(CHAT_EVENTS.STREAM_ERROR, (event) => {
        const payload = event.payload
        if (payload.sessionId !== sessionIdRef.current) return
        // 气泡直接展示错误码 + body（后端已写入 content；前端再兜底组装）
        const bubbleContent = formatChatErrorBubbleContent(payload)
        setMessages((prev) =>
          prev.map((m) =>
            m.id === payload.messageId
              ? {
                  ...m,
                  streaming: false,
                  error: payload.error,
                  errorCode: payload.errorCode,
                  errorBody: payload.errorBody,
                  content: bubbleContent || m.content || '',
                }
              : m
          )
        )
        setActiveRequestId((cur) => (cur === payload.requestId ? null : cur))
        setSending(false)
      })
      if (cancelled) {
        u1()
        u2()
        u3()
        return
      }
      unlisteners.push(u1, u2, u3)
    }

    void setup()
    return () => {
      cancelled = true
      unlisteners.forEach((u) => u())
    }
  }, [])

  const send = useCallback(
    async (
      content: string,
      attachments: PendingAttachment[],
      transportMode?: ChatTransportMode,
      protocol?: ChatProtocol
    ) => {
      if (!sessionId) return false
      setSending(true)
      try {
        const result = await sendChatMessage({
          sessionId,
          content,
          attachments,
          transportMode,
          protocol,
        })
        setSession((prev) =>
          prev
            ? {
                ...prev,
                title: result.session.title,
                model: result.session.model,
                transportMode: result.session.transportMode,
                protocol: result.session.protocol,
                updatedAt: result.session.updatedAt,
              }
            : prev
        )
        setMessages((prev) => {
          const withoutDup = prev.filter(
            (m) => m.id !== result.userMessage.id && m.id !== result.assistantMessage.id
          )
          return [...withoutDup, result.userMessage, result.assistantMessage]
        })
        if (result.requestId) {
          setActiveRequestId(result.requestId)
        } else {
          setSending(false)
        }
        return true
      } catch {
        setSending(false)
        return false
      }
    },
    [sessionId]
  )

  const abort = useCallback(async () => {
    if (!activeRequestId) return
    try {
      await abortChatMessage(activeRequestId)
    } catch {
      // 忽略
    }
  }, [activeRequestId])

  const deleteMessage = useCallback(
    async (messageId: string) => {
      if (!sessionId) return false
      try {
        await deleteChatMessage(sessionId, messageId)
        setMessages((prev) => prev.filter((m) => m.id !== messageId))
        return true
      } catch {
        return false
      }
    },
    [sessionId]
  )

  const applySessionSummary = useCallback((summary: ChatSessionSummary) => {
    setSession((prev) =>
      prev
        ? {
            ...prev,
            title: summary.title,
            model: summary.model,
            transportMode: summary.transportMode,
            updatedAt: summary.updatedAt,
          }
        : prev
    )
  }, [])

  const applySendResult = useCallback((result: SendChatMessageResult) => {
    setSession((prev) =>
      prev && prev.id === result.session.id
        ? {
            ...prev,
            title: result.session.title,
            model: result.session.model,
            transportMode: result.session.transportMode,
            protocol: result.session.protocol,
            updatedAt: result.session.updatedAt,
          }
        : prev ?? {
            id: result.session.id,
            title: result.session.title,
            model: result.session.model,
            transportMode: result.session.transportMode,
            protocol: result.session.protocol,
            messages: [],
            createdAt: result.session.createdAt,
            updatedAt: result.session.updatedAt,
          }
    )
    setMessages((prev) => {
      const withoutDup = prev.filter(
        (m) => m.id !== result.userMessage.id && m.id !== result.assistantMessage.id
      )
      return [...withoutDup, result.userMessage, result.assistantMessage]
    })
    if (result.requestId) {
      setActiveRequestId(result.requestId)
      setSending(true)
    } else {
      setActiveRequestId(null)
      setSending(false)
    }
  }, [])

  return {
    session,
    messages,
    loading,
    activeRequestId,
    sending,
    reload,
    send,
    abort,
    deleteMessage,
    applySessionSummary,
    applySendResult,
  }
}

/**
 * 将流式错误事件格式化为气泡正文（错误码 + body）
 * 与后端 `format_error_bubble_content` 对齐，作前端兜底。
 */
export function formatChatErrorBubbleContent(payload: {
  error: string
  errorCode?: string
  errorBody?: string
}): string {
  const lines: string[] = []
  const code = payload.errorCode?.trim()
  if (code) lines.push(`错误码: ${code}`)
  const body = payload.errorBody?.trim()
  if (body) {
    lines.push(`响应 Body:\n${body}`)
  } else if (payload.error?.trim()) {
    lines.push(payload.error.trim())
  }
  return lines.join('\n')
}

/**
 * 将浏览器 `File` 读成 `PendingAttachment`（输入区预览用）
 *
 * - 图片：`readAsDataURL` → `dataUrl` + 纯 `base64` 字段
 * - 其它：`readAsText` → `textContent` 全文（后端拼进 content）
 */
export async function readFileAsPendingAttachment(file: File): Promise<PendingAttachment> {
  const localId = crypto.randomUUID()
  const isImage = file.type.startsWith('image/')

  if (isImage) {
    const dataUrl = await readFileAsDataUrl(file)
    const base64 = dataUrl.includes(',') ? dataUrl.split(',')[1] : dataUrl
    return {
      localId,
      name: file.name,
      kind: 'image',
      mimeType: file.type || 'image/png',
      dataUrl,
      base64,
      size: file.size,
    }
  }

  const textContent = await readFileAsText(file)
  return {
    localId,
    name: file.name,
    kind: 'file',
    mimeType: file.type || undefined,
    textContent,
    size: file.size,
  }
}

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result ?? ''))
    reader.onerror = () => reject(reader.error ?? new Error('read image failed'))
    reader.readAsDataURL(file)
  })
}

function readFileAsText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result ?? ''))
    reader.onerror = () => reject(reader.error ?? new Error('read file failed'))
    reader.readAsText(file)
  })
}
