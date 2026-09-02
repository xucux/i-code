/**
 * 聊天输入区（右侧下方）
 *
 * ## 界面描述
 *
 * 自上而下三层：
 * 1. **附件预览条**（有附件时显示）
 *    - 普通文件：小字文件名 + 回形针图标 + 移除按钮
 *    - 图片：约 40×40 缩略图 + 文件名 + hover 移除
 * 2. **工具栏**：模型选择（浮层内含思考强度子面板）、轻量化传输模式/协议下拉、选文件、选图片
 * 3. **输入行**：多行 Textarea + 发送按钮；发送中切换为红色「中断」
 *
 * 主题色全部走 CSS 变量（`bg-card` / `border` / `muted-foreground` 等）。
 *
 * ## 逻辑描述
 *
 * - 发送条件：未禁用、未发送中、已选模型、且文本非空或有附件。
 * - Enter 发送，Shift+Enter 换行。
 * - 附件由 `readFileAsPendingAttachment` 在前端读入：
 *   - 图片 → data URL + base64（协议 `image_url`）
 *   - 其它 → 全文文本（后端并入 content）
 * - 本组件只持有预览态 `PendingAttachment`，真正落库与协议组装在后端 Service。
 * - 模型/模式变更回调由父级写回当前会话（若已有 `activeId`）。
 */

import { useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { LightSelect } from '@/components/ui/light-select'
import { useTranslation } from '@/modules/i18n/use-translation'
import type { ChatProtocol, ChatTransportMode, PendingAttachment } from '@/modules/chat/types'
import { readFileAsPendingAttachment } from '@/hooks/use-chat'
import { cn } from '@/lib/utils'
import { PromptPickerDialog } from './prompt-picker-dialog'
import { BenchmarkDialog } from './benchmark-dialog'
import { ModelThinkingPicker, type ModelThinkingPickerModel } from './model-thinking-picker'

/** 输入区 Props：受控文本/附件 + 模型与传输模式 + 发送/中断 */
export interface ChatInputProps {
  /** 输入框文本 */
  value: string
  onChange: (value: string) => void
  /** 待发送附件预览列表 */
  attachments: PendingAttachment[]
  onAttachmentsChange: (items: PendingAttachment[]) => void
  transportMode: ChatTransportMode
  onTransportModeChange: (mode: ChatTransportMode) => void
  /** 网关入口协议：Chat / Messages / Responses */
  protocol: ChatProtocol
  onProtocolChange: (protocol: ChatProtocol) => void
  /** 可选模型：`value` 为路由 ID，`thinkingJson` 为模型思考配置 */
  models: ModelThinkingPickerModel[]
  selectedModel: string
  onModelChange: (model: string) => void
  /** 是否开启思考（推理） */
  thinkingEnabled: boolean
  onThinkingEnabledChange: (enabled: boolean) => void
  /** 本轮推理力度（reasoning_effort），空串表示不指定 */
  thinkingEffort: string
  onThinkingEffortChange: (effort: string) => void
  /** 是否正在等待助手回复（显示中断按钮） */
  sending: boolean
  disabled?: boolean
  onSend: () => void
  onAbort: () => void
  /** 发送检测题目（点击题目表「发送」后回调，参数为题目文本） */
  onSendQuestion?: (question: string) => void
  /** 导出当前会话为 HTML */
  onExportHtml?: () => void
}

export function ChatInput({
  value,
  onChange,
  attachments,
  onAttachmentsChange,
  transportMode,
  onTransportModeChange,
  protocol,
  onProtocolChange,
  models,
  selectedModel,
  onModelChange,
  thinkingEnabled,
  onThinkingEnabledChange,
  thinkingEffort,
  onThinkingEffortChange,
  sending,
  disabled,
  onSend,
  onAbort,
  onSendQuestion,
  onExportHtml,
}: ChatInputProps) {
  const { t } = useTranslation('chat')
  const fileRef = useRef<HTMLInputElement>(null)
  const imageRef = useRef<HTMLInputElement>(null)
  const [promptOpen, setPromptOpen] = useState(false)
  /** 检测题目弹窗开关 */
  const [benchmarkOpen, setBenchmarkOpen] = useState(false)

  /** 应用提示词：追加到输入框（已有内容则换行分隔） */
  const handleApplyPrompt = (content: string) => {
    const trimmed = content
    onChange(value.trim().length > 0 ? `${value}\n\n${trimmed}` : trimmed)
  }

  /** 可否发送：有模型 + 文本或附件，且非发送中 */
  const canSend =
    !disabled &&
    !sending &&
    !!selectedModel &&
    (value.trim().length > 0 || attachments.length > 0)

  /** Enter 发送，Shift+Enter 换行 */
  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      if (canSend) onSend()
    }
  }

  /**
   * 选择本地文件：读入 PendingAttachment 后交给父级。
   * @param imageOnly true 时跳过非 image/*（图片按钮）
   */
  const pickFiles = async (files: FileList | null, imageOnly: boolean) => {
    if (!files?.length) return
    const next: PendingAttachment[] = [...attachments]
    for (const file of Array.from(files)) {
      if (imageOnly && !file.type.startsWith('image/')) continue
      try {
        const item = await readFileAsPendingAttachment(file)
        next.push(item)
      } catch {
        // 忽略单文件失败，不打断其余文件
      }
    }
    onAttachmentsChange(next)
  }

  /** 从预览条移除某附件（仅本地状态） */
  const removeAttachment = (localId: string) => {
    onAttachmentsChange(attachments.filter((a) => a.localId !== localId))
  }

  return (
    <div className="border-t bg-card px-3 py-2">
      {/* 附件预览：输入框上方；图片缩略图 + 文件名小字 */}
      {attachments.length > 0 && (
        <div className="mb-2 flex flex-wrap items-center gap-1.5">
          {attachments.map((att) =>
            att.kind === 'image' && att.dataUrl ? (
              <div key={att.localId} className="group relative">
                <img
                  src={att.dataUrl}
                  alt={att.name}
                  title={att.name}
                  className="size-10 rounded border object-cover"
                />
                <button
                  type="button"
                  className="absolute -right-1 -top-1 flex size-4 items-center justify-center rounded-full bg-destructive text-[8px] text-destructive-foreground opacity-0 transition-opacity group-hover:opacity-100"
                  onClick={() => removeAttachment(att.localId)}
                >
                  <i className="fa-solid fa-xmark" />
                </button>
                <p className="mt-0.5 max-w-10 truncate text-center text-[9px] text-muted-foreground">
                  {att.name}
                </p>
              </div>
            ) : (
              <span
                key={att.localId}
                className="inline-flex items-center gap-1 rounded border bg-background px-1.5 py-0.5 text-[10px] text-muted-foreground"
              >
                <i className="fa-solid fa-paperclip text-[9px]" />
                <span className="max-w-[120px] truncate">{att.name}</span>
                <button
                  type="button"
                  className="ml-0.5 text-muted-foreground hover:text-destructive"
                  onClick={() => removeAttachment(att.localId)}
                >
                  <i className="fa-solid fa-xmark text-[9px]" />
                </button>
              </span>
            )
          )}
        </div>
      )}

      <div className="mb-1.5 flex flex-wrap items-center gap-1.5">
        {/* 模型选择 + 思考强度子级面板（选中模型后浮层内自动展示思考面板） */}
        <ModelThinkingPicker
          models={models}
          selectedModel={selectedModel}
          onModelChange={onModelChange}
          thinkingEnabled={thinkingEnabled}
          onThinkingEnabledChange={onThinkingEnabledChange}
          thinkingEffort={thinkingEffort}
          onThinkingEffortChange={onThinkingEffortChange}
          disabled={sending || disabled}
        />

        {/* 传输模式：轻量化下拉（无边框无胶囊） */}
        <LightSelect
          value={transportMode}
          onChange={(v) => onTransportModeChange(v as ChatTransportMode)}
          disabled={sending || disabled}
          options={[
            { value: 'sse', label: 'SSE' },
            { value: 'http', label: 'HTTP' },
          ]}
        />

        {/* 网关入口协议：轻量化下拉（无边框无胶囊） */}
        <LightSelect
          value={protocol}
          onChange={(v) => onProtocolChange(v as ChatProtocol)}
          disabled={sending || disabled}
          options={[
            { value: 'chat', label: t('input.protocolChat') },
            { value: 'messages', label: t('input.protocolMessages') },
            { value: 'responses', label: t('input.protocolResponses') },
          ]}
        />

        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-7 px-2 text-xs"
          disabled={sending || disabled}
          onClick={() => setPromptOpen(true)}
          title={t('input.prompts')}
        >
          <i className="fa-solid fa-bookmark" />
        </Button>

        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-7 px-2 text-xs"
          disabled={sending || disabled}
          onClick={() => setBenchmarkOpen(true)}
          title={t('benchmark.title')}
        >
          <i className="fa-solid fa-clipboard-question" />
        </Button>

        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-7 px-2 text-xs"
          disabled={sending || disabled}
          onClick={() => fileRef.current?.click()}
          title={t('input.attachFile')}
        >
          <i className="fa-solid fa-paperclip" />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-7 px-2 text-xs"
          disabled={sending || disabled}
          onClick={() => imageRef.current?.click()}
          title={t('input.attachImage')}
        >
          <i className="fa-solid fa-image" />
        </Button>

        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-7 px-2 text-xs"
          disabled={disabled}
          onClick={() => onExportHtml?.()}
          title={t('input.exportHtml')}
        >
          <i className="fa-solid fa-file-export" />
        </Button>

        <input
          ref={fileRef}
          type="file"
          className="hidden"
          multiple
          onChange={(e) => {
            void pickFiles(e.target.files, false)
            e.target.value = ''
          }}
        />
        <input
          ref={imageRef}
          type="file"
          accept="image/*"
          className="hidden"
          multiple
          onChange={(e) => {
            void pickFiles(e.target.files, true)
            e.target.value = ''
          }}
        />
      </div>

      <div className="flex items-end gap-2">
        <Textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t('input.placeholder')}
          disabled={disabled}
          className={cn('min-h-[64px] max-h-[140px] flex-1 resize-none text-xs')}
          rows={3}
        />
        {sending ? (
          <Button
            type="button"
            variant="destructive"
            size="sm"
            className="h-8 shrink-0 text-xs"
            onClick={onAbort}
          >
            <i className="fa-solid fa-stop mr-1" />
            {t('input.abort')}
          </Button>
        ) : (
          <Button
            type="button"
            size="sm"
            className="h-8 shrink-0 text-xs"
            disabled={!canSend}
            onClick={onSend}
          >
            <i className="fa-solid fa-paper-plane mr-1" />
            {t('input.send')}
          </Button>
        )}
      </div>

      <PromptPickerDialog
        open={promptOpen}
        onOpenChange={setPromptOpen}
        onApply={handleApplyPrompt}
      />

      {/* 检测题目弹窗：发送动作交由父级走会话发送链路 */}
      <BenchmarkDialog
        open={benchmarkOpen}
        onOpenChange={setBenchmarkOpen}
        onSend={(question) => onSendQuestion?.(question)}
        disabled={sending || disabled}
      />
    </div>
  )
}
