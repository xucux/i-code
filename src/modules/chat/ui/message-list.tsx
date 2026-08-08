/**
 * 聊天消息列表（右侧上方）
 *
 * ## 界面描述
 *
 * - 可滚动消息流；空状态居中提示。
 * - **用户气泡**：右对齐，`bg-primary` / `text-primary-foreground`。
 * - **助手气泡**：左对齐，`bg-muted`；上方可挂思考块，下方可挂 token 用量小字。
 * - **系统气泡**：`bg-accent`（较少使用）。
 * - 用户消息附件：图片缩略图或文件名标签，展示在气泡下方。
 * - 流式中气泡末尾脉冲光标 `▍`。
 * - **调用失败**：助手气泡内直接展示错误码 + 响应 body（红色边框/底色），会话回看可复现。
 *
 * ## 逻辑描述
 *
 * - 消息变化后平滑滚到底部。
 * - 思考过程取自 `message.thinking`；仅助手且非空时渲染 `ThinkingBlock`。
 * - 思考块：流式默认展开，结束后默认折叠；用户手动切换后不再自动改写。
 * - Token 仅在助手消息完成（`!streaming`）且有 `usage` 时展示。
 * - 气泡正文为空且仍在流式时：有思考显示「生成中」，否则「思考中」。
 * - 错误优先读 `errorCode` / `errorBody`；缺省时回退 `error` 或 content。
 */

import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useTranslation } from '@/modules/i18n/use-translation'
import type { ChatMessage, ChatTokenUsage } from '@/modules/chat/types'

export interface MessageListProps {
  messages: ChatMessage[]
  /** 无消息时的提示文案；默认 i18n `messages.empty` */
  emptyHint?: string
  /** 父级计算的可用高度等样式（可选） */
  style?: React.CSSProperties
  /** 删除单条消息回调（仅助手气泡三点菜单触发） */
  onDeleteMessage?: (messageId: string) => void
}

export function MessageList({ messages, emptyHint, style, onDeleteMessage }: MessageListProps) {
  const { t } = useTranslation('chat')
  const bottomRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' })
  }, [messages])

  return (
    <div className="min-h-0 flex-1 overflow-hidden" style={style}>
      <ScrollArea className="h-full">
        <div className="flex flex-col gap-3 px-3 py-3">
          {messages.length === 0 && (
            <p className="py-10 text-center text-xs text-muted-foreground">
              {emptyHint ?? t('messages.empty')}
            </p>
          )}
          {messages.map((msg) => (
            <MessageBubble
              key={msg.id}
              message={msg}
              onDeleteMessage={onDeleteMessage}
            />
          ))}
          <div ref={bottomRef} />
        </div>
      </ScrollArea>
    </div>
  )
}

/**
 * 单条消息气泡
 *
 * 结构（助手）：思考块 → 正文/错误气泡 → token 小字 → 三点菜单
 * 结构（用户）：正文气泡 → 附件预览
 */
function MessageBubble({
  message,
  onDeleteMessage,
}: {
  message: ChatMessage
  onDeleteMessage?: (messageId: string) => void
}) {
  const { t } = useTranslation('chat')
  const isUser = message.role === 'user'
  const isAssistant = message.role === 'assistant'
  const thinkingText = message.thinking?.trim() ?? ''
  const hasThinking = thinkingText.length > 0
  const hasError =
    !!message.error ||
    !!message.errorCode ||
    !!message.errorBody ||
    (!message.streaming &&
      isAssistant &&
      !!message.content &&
      (message.content.startsWith('错误码:') || message.content.includes('响应 Body:')))

  /** 复制助手消息正文到剪贴板 */
  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(message.content)
      toast.success(t('messages.copySuccess'))
    } catch {
      toast.error(t('messages.copyFailed'))
    }
  }

  /** 删除当前助手消息（直接删除 + toast 由父级处理） */
  const handleDelete = () => {
    onDeleteMessage?.(message.id)
  }

  /** 是否展示三点菜单：仅助手且非流式 */
  const showActions = isAssistant && !message.streaming

  return (
    <div className={cn('flex w-full', isUser ? 'justify-end' : 'justify-start')}>
      <div className={cn('flex max-w-[85%] flex-col gap-1', isUser ? 'items-end' : 'items-start')}>
        {/* 思考过程：仅助手消息且有内容时展示 */}
        {isAssistant && hasThinking && (
          <ThinkingBlock
            text={thinkingText}
            streaming={message.streaming}
          />
        )}

        {/* 调用失败：错误码 + body 直接写在气泡内 */}
        {isAssistant && hasError && !message.streaming ? (
          <ErrorBubble message={message} />
        ) : (
          /* 正文：主题色气泡；流式时空正文用占位文案 + 光标 */
          <div
            className={cn(
              'rounded-lg px-3 py-2 text-xs leading-relaxed whitespace-pre-wrap break-words',
              isUser && 'bg-primary text-primary-foreground',
              isAssistant && 'bg-muted text-foreground',
              message.role === 'system' && 'bg-accent text-accent-foreground'
            )}
          >
            {message.content ||
              (message.streaming
                ? hasThinking
                  ? t('messages.generating')
                  : t('messages.thinking')
                : '')}
            {message.streaming && (
              <span className="ml-1 inline-block animate-pulse text-muted-foreground">▍</span>
            )}
          </div>
        )}

        {/* 助手气泡三点菜单：复制 / 删除（非流式时显示，定位气泡右下） */}
        {showActions && (
          <div className="flex w-full justify-end">
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <button
                  type="button"
                  className="flex size-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                  title={t('messages.actions')}
                >
                  <i className="fa-solid fa-ellipsis text-[10px]" />
                </button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="text-xs">
                <DropdownMenuItem onClick={handleCopy}>
                  <i className="fa-solid fa-copy text-[10px]" />
                  {t('messages.copy')}
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={handleDelete}
                  className="text-destructive focus:text-destructive"
                >
                  <i className="fa-solid fa-trash text-[10px]" />
                  {t('messages.delete')}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        )}

        {/* 附件/图片展示（用户消息）：名称进入会话，图片可预览 */}
        {message.attachments?.length > 0 && (
          <div className="flex max-w-full flex-wrap gap-1.5">
            {message.attachments.map((att) =>
              att.kind === 'image' && att.dataUrl ? (
                <img
                  key={att.id}
                  src={att.dataUrl}
                  alt={att.name}
                  title={att.name}
                  className="size-12 rounded border object-cover"
                />
              ) : (
                <span
                  key={att.id}
                  className="inline-flex items-center gap-1 rounded border bg-background px-1.5 py-0.5 text-[10px] text-muted-foreground"
                >
                  <i className="fa-solid fa-paperclip text-[9px]" />
                  {att.name}
                </span>
              )
            )}
          </div>
        )}

        {/* 单条用量：完成后展示，供顶栏汇总 */}
        {isAssistant && !message.streaming && message.usage && (
          <TokenUsageText usage={message.usage} model={message.model} />
        )}
      </div>
    </div>
  )
}

/**
 * 助手调用失败气泡
 *
 * 展示错误码与完整响应 body，写入会话消息便于回看。
 */
function ErrorBubble({ message }: { message: ChatMessage }) {
  const { t } = useTranslation('chat')
  const code = message.errorCode?.trim()
  const body = message.errorBody?.trim()
  // 后端已把「错误码 + Body」写入 content 时，优先直接展示 content
  const structuredInContent =
    !!message.content &&
    (message.content.startsWith('错误码:') || message.content.includes('响应 Body:'))

  return (
    <div
      className={cn(
        'w-full rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs leading-relaxed text-destructive'
      )}
      role="alert"
    >
      <div className="mb-1 flex items-center gap-1.5 font-medium">
        <i className="fa-solid fa-circle-exclamation text-[10px]" />
        <span>{t('messages.callError')}</span>
        {code && (
          <span className="rounded border border-destructive/30 bg-background/60 px-1 py-0.5 font-mono text-[10px] tabular-nums text-destructive">
            {code}
          </span>
        )}
      </div>
      {structuredInContent ? (
        <pre className="max-h-48 overflow-auto whitespace-pre-wrap break-words font-mono text-[10px] text-destructive/90">
          {message.content}
        </pre>
      ) : (
        <>
          {body ? (
            <pre className="max-h-48 overflow-auto whitespace-pre-wrap break-words font-mono text-[10px] text-destructive/90">
              {body}
            </pre>
          ) : (
            <p className="whitespace-pre-wrap break-words text-[11px]">
              {message.error || message.content || t('messages.callErrorUnknown')}
            </p>
          )}
        </>
      )}
    </div>
  )
}

/**
 * 思考过程折叠块
 *
 * - 流式生成中：默认展开，便于实时查看
 * - 完成后：默认折叠，可点击展开
 * - 用户手动切换后，不再自动改写展开状态
 */
function ThinkingBlock({ text, streaming }: { text: string; streaming: boolean }) {
  const { t } = useTranslation('chat')
  const [open, setOpen] = useState(streaming)
  const userToggledRef = useRef(false)
  const wasStreamingRef = useRef(streaming)

  useEffect(() => {
    // 从流式结束瞬间：若用户未手动操作，则自动折叠
    if (wasStreamingRef.current && !streaming && !userToggledRef.current) {
      setOpen(false)
    }
    // 新一轮流式开始：默认展开（用户未手动操作时）
    if (!wasStreamingRef.current && streaming && !userToggledRef.current) {
      setOpen(true)
    }
    wasStreamingRef.current = streaming
  }, [streaming])

  const preview = text.length > 48 ? `${text.slice(0, 48)}…` : text

  return (
    <Collapsible
      open={open}
      onOpenChange={(next) => {
        userToggledRef.current = true
        setOpen(next)
      }}
      className="w-full min-w-[160px] max-w-full"
    >
      <CollapsibleTrigger asChild>
        <button
          type="button"
          className={cn(
            'flex w-full items-center gap-1.5 rounded-md border border-border/60 bg-background/60 px-2 py-1 text-left text-[10px] text-muted-foreground transition-colors hover:bg-accent/50 hover:text-foreground'
          )}
        >
          <i
            className={cn(
              'fa-solid size-3 shrink-0 text-[10px]',
              streaming
                ? 'fa-brain animate-pulse text-primary'
                : open
                  ? 'fa-chevron-down'
                  : 'fa-chevron-right'
            )}
          />
          <span className="shrink-0 font-medium">
            {streaming ? t('messages.thinkingInProgress') : t('messages.thinkingDone')}
          </span>
          {!open && (
            <span className="min-w-0 flex-1 truncate opacity-80">{preview}</span>
          )}
        </button>
      </CollapsibleTrigger>
      <CollapsibleContent className="mt-1">
        <div className="max-h-40 overflow-y-auto rounded-md border border-dashed border-border/70 bg-muted/40 px-2.5 py-2 text-[11px] leading-relaxed text-muted-foreground whitespace-pre-wrap break-words">
          {text}
          {streaming && (
            <span className="ml-0.5 inline-block animate-pulse">▍</span>
          )}
        </div>
      </CollapsibleContent>
    </Collapsible>
  )
}

/** 气泡下方 token 用量小字（prompt / completion / total），右侧括号显示模型 id */
function TokenUsageText({ usage, model }: { usage: ChatTokenUsage; model?: string }) {
  const { t } = useTranslation('chat')
  const parts: string[] = []
  if (usage.promptTokens != null) {
    parts.push(`${t('messages.promptTokens')} ${usage.promptTokens}`)
  }
  if (usage.completionTokens != null) {
    parts.push(`${t('messages.completionTokens')} ${usage.completionTokens}`)
  }
  if (usage.totalTokens != null) {
    parts.push(`${t('messages.totalTokens')} ${usage.totalTokens}`)
  }
  if (parts.length === 0) return null
  const modelLabel = model ? formatModelLabel(model) : null
  return (
    <p className="text-[10px] text-muted-foreground tabular-nums">
      {parts.join(' · ')}
      {modelLabel && <span className="ml-1 text-muted-foreground/70">({modelLabel})</span>}
    </p>
  )
}

/**
 * 模型 id 展示：`provider_slug/model_id` 超长时省略前缀为 `…/model_id`。
 */
function formatModelLabel(model: string): string {
  const idx = model.indexOf('/')
  if (idx < 0) return model
  if (model.length <= 24) return model
  return `…/${model.slice(idx + 1)}`
}
