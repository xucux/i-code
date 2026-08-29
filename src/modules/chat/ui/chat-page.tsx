/**
 * 聊天主页面（路由 `/chat`）
 *
 * ## 界面描述
 *
 * 左右分栏布局（紧凑适配 900×700）：
 *
 * ```
 * ┌────────────┬──────────────────────────────────────┐
 * │ 会话列表    │ 顶栏：会话标题 · 网关状态 · 累计 Token │
 * │ SessionList│──────────────────────────────────────│
 * │ 新建/删除  │ 消息区 MessageList（气泡 + 思考 + 用量）│
 * │ 点击切换   │──────────────────────────────────────│
 * │            │ 输入区 ChatInput（附件预览/模型/SSE/发送）│
 * └────────────┴──────────────────────────────────────┘
 * ```
 *
 * - **左侧**：会话记录；新建、点击进入、删除（二次确认）。
 * - **右侧顶栏**：当前标题、网关运行状态、模型 ID、会话累计 Token。
 * - **右侧中部**：聊天气泡；助手支持思考折叠块与 token 小字。
 * - **右侧底部**：文本输入、附件/图片、传输模式、发送/中断。
 *
 * ## 逻辑描述
 *
 * 1. **数据源**：`useChatSessions` 管列表；`useChatSession(activeId)` 管当前消息与流式。
 * 2. **模型列表**：来自网关已暴露模型（`useExposedModels`），ID 为 `{slug}/{model}`。
 * 3. **草稿会话**：未点「新建」却在输入框打字或加附件时，静默 `create` 并选中。
 * 4. **发送链路**：校验网关运行 → 确保会话 → 可选同步 model/mode → `send`；
 *    新建后若 hook 尚未对齐 `activeId`，用 `pendingSendRef` 延迟发送。
 * 5. **中断**：`activeRequestId` 存在时调用 `abort`，后端取消 oneshot。
 * 6. **Token 汇总**：遍历当前消息的 `usage`，顶栏展示 total（hover 看 prompt/completion）。
 *
 * 高度：`useAvailableHeight` 实测后传入左侧列表与右侧列，避免 flex 嵌套高度失效。
 */

import { useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { useChatSession, useChatSessions, exportChatHtml, revealChatFile } from '@/hooks/use-chat'
import { useCatalogModels } from '@/hooks/use-catalog'
import { useGatewayStatus } from '@/hooks/use-gateway-status'
import { DeleteConfirmDialog } from '@/components/ui/delete-confirm-dialog'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { toIcodeError } from '@/core/errors'
import { buildModelId } from '@/core/utils'
import type { ChatMessage, ChatProtocol, ChatTransportMode, PendingAttachment } from '@/modules/chat/types'
import { SessionList } from './session-list'
import { MessageList } from './message-list'
import { ChatInput } from './chat-input'

/**
 * 新建会话后待发送的消息快照。
 * 等 `activeId` 与 `useChatSession` 对齐后再真正调用 send，避免 reload 冲掉流式状态。
 */
interface PendingSend {
  sessionId: string
  content: string
  attachments: PendingAttachment[]
  transportMode: ChatTransportMode
  protocol: ChatProtocol
}

/**
 * 聊天页根组件：组装会话列表、消息区、输入区与删除确认。
 */
export function ChatPage() {
  const { t } = useTranslation('chat')
  const { status: gatewayStatus } = useGatewayStatus()
  const { models: exposedModels } = useCatalogModels()
  const {
    sessions,
    loading: sessionsLoading,
    create,
    remove,
    update,
    refetch: refetchSessions,
  } = useChatSessions()

  const [activeId, setActiveId] = useState<string | null>(null)
  const [deleteId, setDeleteId] = useState<string | null>(null)
  /** 导出 HTML 结果弹窗：null=关闭；success=成功；error=失败 */
  const [exportResult, setExportResult] = useState<{
    success: boolean
    path?: string
    error?: string
  } | null>(null)
  const [input, setInput] = useState('')
  const [attachments, setAttachments] = useState<PendingAttachment[]>([])
  const [transportMode, setTransportMode] = useState<ChatTransportMode>('sse')
  const [protocol, setProtocol] = useState<ChatProtocol>('chat')
  const [selectedModel, setSelectedModel] = useState('')
  const pendingSendRef = useRef<PendingSend | null>(null)
  /** 正在自动创建草稿会话，避免输入过程中重复 create */
  const ensuringSessionRef = useRef(false)
  /** 输入触发的草稿创建 Promise，发送时可 await 同一结果 */
  const ensureSessionPromiseRef = useRef<Promise<string | null> | null>(null)

  const {
    session,
    messages,
    sending,
    activeRequestId,
    abort,
    send,
    deleteMessage,
  } = useChatSession(activeId)

  const [pageHeight, pageRef] = useAvailableHeight()

  const modelOptions = useMemo(() => {
    return exposedModels.map((m) => {
      const value = buildModelId(m.providerSlug, m.modelId)
      const label = m.displayName ? `${m.displayName} (${value})` : value
      return { value, label }
    })
  }, [exposedModels])

  const effectiveModel = selectedModel || session?.model || modelOptions[0]?.value || ''
  const effectiveMode = transportMode || session?.transportMode || 'sse'
  const effectiveProtocol = protocol || session?.protocol || 'chat'

  /** 当前会话累计 Token（汇总各助手消息 usage） */
  const tokenTotals = useMemo(() => {
    let prompt = 0
    let completion = 0
    let total = 0
    let hasAny = false
    for (const m of messages) {
      if (!m.usage) continue
      hasAny = true
      if (m.usage.promptTokens != null) prompt += m.usage.promptTokens
      if (m.usage.completionTokens != null) completion += m.usage.completionTokens
      if (m.usage.totalTokens != null) {
        total += m.usage.totalTokens
      } else {
        total += (m.usage.promptTokens ?? 0) + (m.usage.completionTokens ?? 0)
      }
    }
    return { prompt, completion, total, hasAny }
  }, [messages])

  useEffect(() => {
    if (!selectedModel && modelOptions[0]?.value) {
      setSelectedModel(modelOptions[0].value)
    }
  }, [modelOptions, selectedModel])

  // 新建会话后延迟发送：等 activeId 对齐 hook 再发，避免 reload 冲掉流式状态
  useEffect(() => {
    const pending = pendingSendRef.current
    if (!pending || !activeId || pending.sessionId !== activeId) return
    pendingSendRef.current = null
    void (async () => {
      const ok = await send(pending.content, pending.attachments, pending.transportMode, pending.protocol)
      if (!ok) {
        toast.error(t('errors.sendFailed'))
        setInput(pending.content)
        setAttachments(pending.attachments)
      } else {
        void refetchSessions()
      }
    })()
  }, [activeId, send, t, refetchSessions])

  /**
   * 确保存在当前会话：无选中会话时自动创建并选中。
   * 用于「未点新建、直接在输入框打字/加附件」场景。
   */
  const ensureActiveSession = async (options?: {
    /** 失败时是否 toast（输入时静默，发送时提示） */
    silent?: boolean
  }): Promise<string | null> => {
    if (activeId) return activeId
    if (ensureSessionPromiseRef.current) {
      return ensureSessionPromiseRef.current
    }

    const silent = options?.silent ?? false
    const model = effectiveModel || modelOptions[0]?.value
    if (!model) {
      if (!silent) toast.error(t('errors.noModel'))
      return null
    }

    ensuringSessionRef.current = true
    const promise = (async () => {
      try {
        const created = await create({
          model,
          transportMode: effectiveMode,
          protocol: effectiveProtocol,
        })
        if (!created) {
          if (!silent) toast.error(t('errors.createFailed'))
          return null
        }
        setActiveId(created.id)
        setSelectedModel(created.model)
        setTransportMode(created.transportMode)
        setProtocol(created.protocol)
        return created.id
      } finally {
        ensuringSessionRef.current = false
        ensureSessionPromiseRef.current = null
      }
    })()

    ensureSessionPromiseRef.current = promise
    return promise
  }

  /** 输入变化：无会话时自动建草稿并选中，不打断当前输入内容 */
  const handleInputChange = (value: string) => {
    setInput(value)
    if (!activeId && !ensuringSessionRef.current && value.trim().length > 0) {
      void ensureActiveSession({ silent: true })
    }
  }

  /** 附件变化：无会话时同样自动建草稿 */
  const handleAttachmentsChange = (items: PendingAttachment[]) => {
    setAttachments(items)
    if (!activeId && !ensuringSessionRef.current && items.length > 0) {
      void ensureActiveSession({ silent: true })
    }
  }

  /** 顶栏/侧栏「新建」：显式创建空会话并清空输入 */
  const handleCreate = async () => {
    const model = effectiveModel || modelOptions[0]?.value
    if (!model) {
      toast.error(t('errors.noModel'))
      return
    }
    const created = await create({
      model,
      transportMode: effectiveMode,
      protocol: effectiveProtocol,
    })
    if (!created) {
      toast.error(t('errors.createFailed'))
      return
    }
    setActiveId(created.id)
    setSelectedModel(created.model)
    setTransportMode(created.transportMode)
    setProtocol(created.protocol)
    setInput('')
    setAttachments([])
  }

  /** 点击左侧会话：切换 activeId，同步模型/模式，清空未发送草稿 */
  const handleSelect = (id: string) => {
    setActiveId(id)
    const found = sessions.find((s) => s.id === id)
    if (found) {
      setSelectedModel(found.model)
      setTransportMode(found.transportMode)
      setProtocol(found.protocol)
    }
    setInput('')
    setAttachments([])
  }

  /** 删除确认后：后端删 JSONL；若删的是当前会话则取消选中 */
  const handleDeleteConfirm = async () => {
    if (!deleteId) return
    const ok = await remove(deleteId)
    if (ok) {
      if (activeId === deleteId) {
        setActiveId(null)
      }
      toast.success(t('sessions.deleted'))
    } else {
      toast.error(t('errors.deleteFailed'))
    }
    setDeleteId(null)
  }

  /** 模型变更：有会话则写回 JSONL 摘要 */
  const handleModelChange = async (model: string) => {
    setSelectedModel(model)
    if (activeId) {
      const updated = await update(activeId, { model })
      if (!updated) toast.error(t('errors.updateFailed'))
      else void refetchSessions()
    }
  }

  /** 传输模式变更：SSE / HTTP，有会话则持久化 */
  const handleModeChange = async (mode: ChatTransportMode) => {
    setTransportMode(mode)
    if (activeId) {
      const updated = await update(activeId, { transportMode: mode })
      if (!updated) toast.error(t('errors.updateFailed'))
      else void refetchSessions()
    }
  }

  /** 入口协议变更：Chat / Messages / Responses，有会话则持久化 */
  const handleProtocolChange = async (p: ChatProtocol) => {
    setProtocol(p)
    if (activeId) {
      const updated = await update(activeId, { protocol: p })
      if (!updated) toast.error(t('errors.updateFailed'))
      else void refetchSessions()
    }
  }

  /**
   * 发送消息
   *
   * 1. 网关须运行；2. 须有模型与内容/附件；3. 无会话则 ensure 后挂 pendingSend；
   * 4. 有会话则必要时更新 model/mode，再 `send`；失败回填输入。
   */
  const handleSend = async () => {
    if (!gatewayStatus.isRunning) {
      toast.error(t('errors.gatewayStopped'))
      return
    }

    const model = effectiveModel || modelOptions[0]?.value
    if (!model) {
      toast.error(t('errors.noModel'))
      return
    }
    if (!input.trim() && attachments.length === 0) return
    if (sending) return

    const content = input
    const atts = attachments

    // 无会话（或输入时正在建草稿）：确保会话存在并选中
    let sessionId = activeId
    if (!sessionId) {
      sessionId = await ensureActiveSession({ silent: false })
      if (!sessionId) return
      setInput('')
      setAttachments([])
      pendingSendRef.current = {
        sessionId,
        content,
        attachments: atts,
        transportMode: effectiveMode,
        protocol: effectiveProtocol,
      }
      // activeId 可能已由 ensure 设置；若尚未对齐 hook，pending 发送 effect 会接手
      setActiveId(sessionId)
      return
    }

    if (session?.model !== model || session?.transportMode !== effectiveMode || session?.protocol !== effectiveProtocol) {
      await update(sessionId, { model, transportMode: effectiveMode, protocol: effectiveProtocol })
    }

    setInput('')
    setAttachments([])
    const ok = await send(content, atts, effectiveMode, effectiveProtocol)
    if (!ok) {
      toast.error(t('errors.sendFailed'))
      setInput(content)
      setAttachments(atts)
    } else {
      void refetchSessions()
    }
  }

  /** 中断当前 requestId 对应的网关请求 */
  const handleAbort = async () => {
    if (activeRequestId) {
      await abort()
    }
    toast.message(t('input.aborted'))
  }

  /** 删除单条助手消息：后端移除 + 前端同步，直接删除并 toast */
  const handleDeleteMessage = async (messageId: string) => {
    const ok = await deleteMessage(messageId)
    if (ok) {
      toast.success(t('messages.deleteSuccess'))
      void refetchSessions()
    } else {
      toast.error(t('messages.deleteFailed'))
    }
  }

  /** 导出当前会话为 HTML 文件：读取主题色内联渲染，写入 exports/ 目录，弹窗展示结果 */
  const handleExportHtml = async () => {
    if (!session || messages.length === 0) {
      setExportResult({ success: false, error: t('input.exportEmpty') })
      return
    }
    try {
      const html = buildChatHtml(session.title, messages)
      const safeTitle = session.title.replace(/[\\/:*?"<>|]/g, '_').slice(0, 60) || 'chat'
      const filename = `${safeTitle}-${Date.now()}.html`
      const path = await exportChatHtml(html, filename)
      setExportResult({ success: true, path })
    } catch (e) {
      setExportResult({ success: false, error: toIcodeError(e)?.message ?? t('input.exportFailed') })
    }
  }

  /** 在系统文件浏览器中显示导出的文件 */
  const handleRevealFile = async (path: string) => {
    try {
      await revealChatFile(path)
    } catch (e) {
      toast.error(toIcodeError(e)?.message ?? t('input.exportFailed'))
    }
  }

  const deletingSession = sessions.find((s) => s.id === deleteId)

  return (
    <div ref={pageRef} className="flex h-full min-h-0 overflow-hidden">
      <SessionList
        sessions={sessions}
        activeId={activeId}
        loading={sessionsLoading}
        onSelect={handleSelect}
        onDelete={(id) => setDeleteId(id)}
        onCreate={() => void handleCreate()}
        style={{ width: 200, height: pageHeight || undefined }}
      />

      <div
        className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden"
        style={{ height: (pageHeight - 41) || undefined }}
      >
        <div className="flex h-12 items-center justify-between gap-2 border-b px-3 py-2">
          <div className="min-w-0 flex-1">
            <h1 className="truncate text-sm font-medium">
              {session?.title ?? t('page.title')}
            </h1>
            <p className="truncate text-[10px] text-muted-foreground">
              {gatewayStatus.isRunning
                ? t('page.gatewayRunning')
                : t('page.gatewayStopped')}
              {session?.model
                ? ` · ${session.model}`
                : effectiveModel
                  ? ` · ${effectiveModel}`
                  : ''}
            </p>
          </div>
          {/* 当前会话累计 Token */}
          <div
            className="shrink-0 rounded-md border bg-muted/40 px-2 py-1 text-right"
            title={
              tokenTotals.hasAny
                ? `${t('messages.promptTokens')} ${tokenTotals.prompt} · ${t('messages.completionTokens')} ${tokenTotals.completion}`
                : t('page.tokenTotalEmpty')
            }
          >
            <p className="text-[9px] text-muted-foreground">{t('page.tokenTotal')}</p>
            <p className="text-xs font-medium tabular-nums text-foreground">
              {tokenTotals.hasAny ? tokenTotals.total.toLocaleString() : '—'}
            </p>
          </div>
        </div>

        <MessageList messages={messages} onDeleteMessage={(id) => void handleDeleteMessage(id)} />

        <ChatInput
          value={input}
          onChange={handleInputChange}
          attachments={attachments}
          onAttachmentsChange={handleAttachmentsChange}
          transportMode={effectiveMode}
          onTransportModeChange={(m) => void handleModeChange(m)}
          protocol={effectiveProtocol}
          onProtocolChange={(p) => void handleProtocolChange(p)}
          models={modelOptions}
          selectedModel={effectiveModel}
          onModelChange={(m) => void handleModelChange(m)}
          sending={sending}
          disabled={false}
          onSend={() => void handleSend()}
          onAbort={() => void handleAbort()}
          onExportHtml={() => void handleExportHtml()}
        />
      </div>

      <DeleteConfirmDialog
        open={!!deleteId}
        onOpenChange={(open) => {
          if (!open) setDeleteId(null)
        }}
        title={t('sessions.deleteTitle')}
        description={
          deletingSession
            ? t('sessions.deleteDesc', { title: deletingSession.title })
            : undefined
        }
        onConfirm={() => void handleDeleteConfirm()}
      />

      {/* 导出 HTML 结果弹窗：成功展示路径 + 打开文件夹；失败展示原因 */}
      <Dialog
        open={!!exportResult}
        onOpenChange={(open) => {
          if (!open) setExportResult(null)
        }}
      >
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-sm">
              <i
                className={
                  exportResult?.success
                    ? 'fa-solid fa-circle-check text-green-500'
                    : 'fa-solid fa-circle-exclamation text-destructive'
                }
              />
              {exportResult?.success
                ? t('input.exportSuccessTitle')
                : t('input.exportFailedTitle')}
            </DialogTitle>
            <DialogDescription className="text-xs">
              {exportResult?.success
                ? exportResult.path
                : exportResult?.error || t('input.exportFailed')}
            </DialogDescription>
          </DialogHeader>
          {exportResult?.success && exportResult.path && (
            <DialogFooter>
              <Button
                type="button"
                variant="default"
                size="sm"
                onClick={() => void handleRevealFile(exportResult.path!)}
              >
                <i className="fa-solid fa-folder-open" />
                {t('input.openFolder')}
              </Button>
            </DialogFooter>
          )}
        </DialogContent>
      </Dialog>
    </div>
  )
}

/**
 * i-code Logo SVG（内联到导出 HTML 中，无需外部依赖）
 *
 * 圆角矩形背景 + 代码符号，与项目 "AI 编程助手" 定位一致。
 */
const APP_LOGO_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" width="32" height="32">' +
  '<rect width="32" height="32" rx="7" ry="7" fill="currentColor"/>' +
  '<path d="M10 12 L6 16 L10 20" stroke="white" stroke-width="2.4" fill="none" stroke-linecap="round" stroke-linejoin="round"/>' +
  '<path d="M22 12 L26 16 L22 20" stroke="white" stroke-width="2.4" fill="none" stroke-linecap="round" stroke-linejoin="round"/>' +
  '<line x1="18" y1="11" x2="14" y2="21" stroke="white" stroke-width="2.4" stroke-linecap="round"/>' +
  '</svg>'

/**
 * GitHub 仓库地址
 */
const APP_GITHUB_URL = 'https://github.com/xucux/i-code'
const APP_VERSION = '0.2.14'

/**
 * 构建单会话 HTML 导出文档
 *
 * - 读取当前主题 CSS 变量内联到 `<style>`，保证离线打开时配色与界面一致
 * - 用户气泡靠右（primary）、助手气泡靠左（muted）
 * - 助手含 thinking 折叠块、正文、token 用量与模型 id
 * - 图片附件内嵌 base64；其他附件仅展示名称与大小
 * - 顶部带项目 Logo、标题、GitHub 链接；宽度 800px 居中
 */
function buildChatHtml(title: string, messages: ChatMessage[]): string {
  const root = document.documentElement
  const cssVar = (name: string): string => {
    const v = getComputedStyle(root).getPropertyValue(name).trim()
    return v || '#000000'
  }
  const bg = cssVar('--background')
  const fg = cssVar('--foreground')
  const primary = cssVar('--primary')
  const primaryFg = cssVar('--primary-foreground')
  const muted = cssVar('--muted')
  const mutedFg = cssVar('--muted-foreground')
  const border = cssVar('--border')
  const accent = cssVar('--accent')
  const destructive = cssVar('--destructive')
  const card = cssVar('--card')
  const cardFg = cssVar('--card-foreground')

  /**
   * 检测主题深浅：解析 HSL 格式 `h s% l%` 的 L 分量。
   * 浅色主题下 `--card` 与 `--background` 颜色几乎相同，
   * 需要通过 filter 拉开助手气泡与背景的对比度。
   */
  const parseLuminance = (hsl: string): number => {
    const parts = hsl.trim().split(/\s+/)
    if (parts.length >= 3) {
      const l = Number.parseFloat(parts[2].replace('%', ''))
      return Number.isNaN(l) ? 50 : l
    }
    return 50
  }
  const bgL = parseLuminance(bg)
  const cardL = parseLuminance(card)
  const isLightTheme = bgL >= 50
  const bgCardSimilar = Math.abs(bgL - cardL) < 2
  // 计算助手气泡背景色：当 card 与背景接近时，通过调整亮度拉开对比度
  let assistantBg = card
  if (bgCardSimilar) {
    const parts = card.trim().split(/\s+/)
    if (parts.length >= 3) {
      const h = parts[0]
      const s = parts[1]
      let l = Number.parseFloat(parts[2].replace('%', ''))
      l = isLightTheme ? l * 0.94 : Math.min(l * 1.08, 100)
      assistantBg = `${h} ${s} ${l.toFixed(1)}%`
    }
  }

  const esc = (s: string): string =>
    s
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')

  const formatSize = (bytes?: number): string => {
    if (bytes == null) return ''
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  }

  const renderUsage = (m: ChatMessage): string => {
    if (!m.usage) return ''
    const parts: string[] = []
    if (m.usage.promptTokens != null) parts.push(`输入 ${m.usage.promptTokens}`)
    if (m.usage.completionTokens != null) parts.push(`输出 ${m.usage.completionTokens}`)
    if (m.usage.totalTokens != null) parts.push(`合计 ${m.usage.totalTokens}`)
    if (parts.length === 0) return ''
    const model = m.model ? ` <span class="model">(${esc(m.model)})</span>` : ''
    return `<div class="usage">${esc(parts.join(' · '))}${model}</div>`
  }

  const renderAttachments = (m: ChatMessage): string => {
    if (!m.attachments || m.attachments.length === 0) return ''
    const items = m.attachments.map((att) => {
      if (att.kind === 'image' && att.dataUrl) {
        return `<img src="${esc(att.dataUrl)}" alt="${esc(att.name)}" class="att-img" />`
      }
      const size = formatSize(att.size)
      return `<span class="att-file">📎 ${esc(att.name)}${size ? ` · ${esc(size)}` : ''}</span>`
    })
    return `<div class="attachments">${items.join('')}</div>`
  }

  const bubbles = messages
    .map((m) => {
      const isUser = m.role === 'user'
      const cls = isUser ? 'bubble user' : 'bubble assistant'
      const thinking = m.thinking?.trim()
        ? `<details class="thinking"><summary>思考过程</summary><div>${esc(m.thinking)}</div></details>`
        : ''
      const content = `<div class="content">${esc(m.content) || ' '}</div>`
      const usage = isUser ? '' : renderUsage(m)
      const atts = isUser ? renderAttachments(m) : ''
      return `<div class="row ${isUser ? 'row-user' : 'row-assistant'}">
  <div class="${cls}">${thinking}${content}${atts}${usage}</div>
</div>`
    })
    .join('\n')

  const titleEsc = esc(title)
  const githubSvg =
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="16" height="16" fill="currentColor">' +
    '<path d="M12 .297C5.37.297 0 5.67 0 12.297c0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61-.546-1.385-1.333-1.755-1.333-1.755-1.09-.745.084-.729.084-.729 1.205.084 1.84 1.236 1.84 1.236 1.07 1.835 2.805 1.305 3.49.998.108-.776.418-1.305.762-1.605-2.665-.305-5.467-1.335-5.467-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.957-.266 1.98-.398 3-.403 1.02.005 2.045.137 3 .403 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.62-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297 24 5.67 18.627.297 12 .297z"/>' +
    '</svg>'

  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>${titleEsc} - i-code</title>
<style>
  :root {
    --background: ${bg};
    --foreground: ${fg};
    --primary: ${primary};
    --primary-foreground: ${primaryFg};
    --muted: ${muted};
    --muted-foreground: ${mutedFg};
    --border: ${border};
    --accent: ${accent};
    --destructive: ${destructive};
    --card: ${card};
    --card-foreground: ${cardFg};
    --assistant-bg: ${assistantBg};
  }
  * { box-sizing: border-box; }
  html, body { margin: 0; padding: 0; }
  body {
    background: var(--background);
    color: var(--foreground);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
    font-size: 13px;
    line-height: 1.6;
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 32px 16px;
  }
  /* 容器：800px 居中，白色背景，圆角阴影 */
  .container {
    width: 100%;
    max-width: 800px;
    background: var(--background);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 2px 16px rgba(0, 0, 0, 0.06);
    overflow: hidden;
  }
  /* 顶部 Header */
  .header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 20px 24px;
    border-bottom: 1px solid var(--border);
    background: linear-gradient(to bottom, ${muted}, var(--background));
  }
  .header .logo {
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    color: ${primary};
  }
  .header .title-area { flex: 1; min-width: 0; }
  .header .app-name {
    font-size: 15px;
    font-weight: 600;
    color: var(--foreground);
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .header .app-name .version {
    font-size: 11px;
    font-weight: 400;
    color: var(--muted-foreground);
    background: var(--muted);
    padding: 1px 6px;
    border-radius: 4px;
  }
  .header .session-title {
    font-size: 13px;
    color: var(--muted-foreground);
    margin-top: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .header .actions { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }
  .header .github-link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--muted-foreground);
    text-decoration: none;
    font-size: 12px;
    padding: 4px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    transition: all 0.2s;
  }
  .header .github-link:hover {
    color: var(--primary);
    border-color: var(--primary);
    background: var(--accent);
  }
  /* 消息区 */
  .content-area {
    padding: 24px;
  }
  .row { display: flex; margin-bottom: 16px; }
  .row-user { justify-content: flex-end; }
  .row-assistant { justify-content: flex-start; }
  .bubble {
    max-width: 80%;
    border-radius: 10px;
    padding: 10px 14px;
    word-break: break-word;
    white-space: pre-wrap;
  }
  .bubble.user {
    background: var(--primary);
    color: var(--primary-foreground);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.08);
  }
  .bubble.assistant {
    background: hsl(var(--assistant-bg));
    color: var(--card-foreground);
    border: 1px solid var(--border);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.08);
  }
  .thinking {
    margin-bottom: 8px;
    border: 1px dashed var(--border);
    border-radius: 8px;
    padding: 8px 10px;
    background: var(--accent);
    opacity: 0.9;
  }
  .thinking summary {
    cursor: pointer;
    font-size: 11px;
    color: var(--muted-foreground);
    user-select: none;
  }
  .thinking summary:hover { color: var(--foreground); }
  .thinking div {
    margin-top: 8px;
    font-size: 12px;
    color: var(--muted-foreground);
    white-space: pre-wrap;
  }
  .content { white-space: pre-wrap; }
  .attachments { margin-top: 8px; display: flex; flex-wrap: wrap; gap: 8px; }
  .att-img {
    width: 80px;
    height: 80px;
    object-fit: cover;
    border-radius: 6px;
    border: 1px solid var(--border);
    display: block;
  }
  .att-file {
    font-size: 11px;
    color: var(--muted-foreground);
    background: var(--background);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 3px 8px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .usage {
    margin-top: 6px;
    font-size: 10px;
    color: var(--muted-foreground);
    font-variant-numeric: tabular-nums;
  }
  .usage .model { opacity: 0.7; margin-left: 4px; }
  /* 底部 Footer */
  .footer {
    padding: 16px 24px;
    border-top: 1px solid var(--border);
    text-align: center;
    font-size: 11px;
    color: var(--muted-foreground);
  }
  .footer a {
    color: var(--primary);
    text-decoration: none;
  }
  .footer a:hover { text-decoration: underline; }
</style>
</head>
<body>
<div class="container">
  <!-- Header -->
  <div class="header">
    <div class="logo">${APP_LOGO_SVG}</div>
    <div class="title-area">
      <div class="app-name">i-code <span class="version">${APP_VERSION}</span></div>
      <div class="session-title">${titleEsc}</div>
    </div>
    <div class="actions">
      <a class="github-link" href="${APP_GITHUB_URL}" target="_blank" rel="noopener noreferrer">
        ${githubSvg}
        GitHub
      </a>
    </div>
  </div>
  <!-- Content -->
  <div class="content-area">
    ${bubbles}
  </div>
  <!-- Footer -->
  <div class="footer">
    Exported by <a href="${APP_GITHUB_URL}" target="_blank" rel="noopener noreferrer">i-code</a> · Local AI Gateway & CLI Config Manager
  </div>
</div>
</body>
</html>`
}
