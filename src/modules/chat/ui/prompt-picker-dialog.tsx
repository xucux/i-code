/**
 * 提示词选择弹窗
 *
 * ## 界面描述
 *
 * - 紧凑列表：每行仅标题占一整行，标题过长时**自动横向滚动**（跑马灯），
 *   右侧固定一个【应用】按钮。
 * - 空/加载态走 i18n。
 * - 应用：调用 `getChatPrompt(id)` 取正文（后端已截断 125000 字符），
 *   若 `truncated` 则 toast 警告，随后回调父级将正文写入输入框。
 *
 * ## 来源
 *
 * 提示词来自用户软件目录（与数据库同目录，即 `app_config_dir/prompt/`）下
 * 的 `*.md` 文件；标题取自文件首个 `# ` 行。
 */

import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useTranslation } from '@/modules/i18n/use-translation'
import { getChatPrompt, listChatPrompts } from '@/hooks/use-chat'
import type { ChatPrompt } from '@/modules/chat/types'
import { cn } from '@/lib/utils'

export interface PromptPickerDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 应用提示词：将正文交回父级写入输入框 */
  onApply: (content: string) => void
}

/** 单行标题跑马灯：内容溢出时自动横向滚动 */
function MarqueeTitle({ title }: { title: string }) {
  const textRef = useRef<HTMLSpanElement>(null)
  const [overflow, setOverflow] = useState(0)

  useLayoutEffect(() => {
    const el = textRef.current
    if (!el) return
    const measure = () => {
      const diff = el.scrollWidth - el.clientWidth
      setOverflow(diff > 1 ? diff : 0)
    }
    measure()
    // 标题变化或容器尺寸变化后重测
    const ro = new ResizeObserver(measure)
    ro.observe(el)
    return () => ro.disconnect()
  }, [title])

  return (
    <span
      className="relative block overflow-hidden whitespace-nowrap text-xs font-medium"
      style={{ ['--marquee-shift' as string]: overflow }}
    >
      <span
        ref={textRef}
        className={cn('inline-block', overflow > 0 && 'marquee-anim')}
        title={title}
      >
        {title}
      </span>
    </span>
  )
}

export function PromptPickerDialog({
  open,
  onOpenChange,
  onApply,
}: PromptPickerDialogProps) {
  const { t } = useTranslation('chat')
  const [prompts, setPrompts] = useState<ChatPrompt[]>([])
  const [loading, setLoading] = useState(false)
  const [applyingId, setApplyingId] = useState<string | null>(null)
  // `t` 在每次渲染都是新引用，放进 ref 以免 effect 依赖它形成死循环
  const tRef = useRef(t)
  tRef.current = t

  useEffect(() => {
    if (!open) return
    let cancelled = false
    setLoading(true)
    void (async () => {
      try {
        const list = await listChatPrompts()
        if (!cancelled) setPrompts(list)
      } catch {
        if (!cancelled) toast.error(tRef.current('prompts.loadFailed'))
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
    // 仅依赖 open：每次打开弹窗加载一次即可
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  const handleApply = async (id: string) => {
    setApplyingId(id)
    try {
      const detail = await getChatPrompt(id)
      if (detail.truncated) {
        toast.warning(t('prompts.truncatedWarn'))
      }
      onApply(detail.content)
      onOpenChange(false)
    } catch {
      toast.error(t('prompts.loadFailed'))
    } finally {
      setApplyingId(null)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md gap-0 p-0 sm:rounded-lg">
        <DialogHeader className="border-b px-3 py-2 text-left">
          <DialogTitle className="flex items-center gap-1.5 text-sm">
            <i className="fa-solid fa-bookmark text-xs text-primary" />
            {t('prompts.title')}
          </DialogTitle>
          <DialogDescription className="text-[10px]">
            {t('prompts.description')}
          </DialogDescription>
        </DialogHeader>

        <ScrollArea className="max-h-[60vh]">
          <ul className="divide-y">
            {loading ? (
              <li className="px-3 py-3 text-center text-xs text-muted-foreground">
                <i className="fa-solid fa-circle-notch fa-spin mr-1" />
                {t('prompts.loading')}
              </li>
            ) : prompts.length === 0 ? (
              <li className="px-3 py-4 text-center text-xs text-muted-foreground">
                {t('prompts.empty')}
              </li>
            ) : (
              prompts.map((p) => (
                <li
                  key={p.id}
                  className="group flex items-center gap-2 px-3 py-1.5 transition-colors hover:bg-accent/50"
                >
                  <div className="min-w-0 flex-1">
                    <MarqueeTitle title={p.title} />
                  </div>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-6 shrink-0 px-2 text-[11px] opacity-70 transition-opacity group-hover:opacity-100"
                    disabled={applyingId === p.id}
                    onClick={() => void handleApply(p.id)}
                  >
                    {applyingId === p.id ? (
                      <i className="fa-solid fa-circle-notch fa-spin" />
                    ) : (
                      <>
                        <i className="fa-solid fa-arrow-right-to-bracket mr-1 text-[10px]" />
                        {t('prompts.apply')}
                      </>
                    )}
                  </Button>
                </li>
              ))
            )}
          </ul>
        </ScrollArea>
      </DialogContent>
    </Dialog>
  )
}
