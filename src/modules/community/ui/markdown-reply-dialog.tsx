/**
 * 一级评论回复弹层（支持 Markdown 编辑 / 预览，2026-08-23 通知迭代）
 *
 * 用于「发表顶层评论」与「回复顶层评论（一级）」；与 CreatePostDialog 同款
 * 编辑 / 预览双 Tab。二级（楼中楼）回复仍使用原纯文本弹窗（见 post-detail.tsx）。
 */

import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { MarkdownEditor } from './markdown-editor'
import { useDialogFullscreen } from './use-dialog-fullscreen'
import { cn } from '@/lib/utils'

/** 回复字数上限（与 Worker / Rust 侧一致） */
const REPLY_MAX = 1000

export interface MarkdownReplyDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 回复目标（顶层评论）；null = 发表新顶层评论 */
  target?: { replyId: number; nickname: string } | null
  /** 提交回复（调用方执行 create_reply），成功后由调用方关闭并刷新；失败抛错由本组件 toast */
  onSubmit: (content: string) => Promise<unknown>
}

export function MarkdownReplyDialog({
  open,
  onOpenChange,
  target,
  onSubmit,
}: MarkdownReplyDialogProps) {
  const { t } = useTranslation('community')
  const [draft, setDraft] = useState('')
  const [submitting, setSubmitting] = useState(false)
  // 系统全屏（Fullscreen API + CSS 回退），放大整个弹窗 / 关闭时自动退出
  const { expanded, toggleFullscreen } = useDialogFullscreen(open)

  // 每次打开重置草稿（回到编辑 Tab）
  useEffect(() => {
    if (open) setDraft('')
  }, [open])

  const canSend = draft.trim().length > 0 && draft.length <= REPLY_MAX && !submitting

  const handleSubmit = async () => {
    if (!canSend) return
    setSubmitting(true)
    try {
      await onSubmit(draft.trim())
      setDraft('')
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className={cn(
          'flex h-[80vh] max-w-lg flex-col',
          expanded &&
            '!fixed !inset-0 !left-0 !top-0 !h-screen !w-screen !max-h-none !max-w-none !translate-x-0 !translate-y-0 !rounded-none border-0'
        )}
      >
        {/* 整窗全屏放大 / 还原（置于 close 按钮左侧，参考脚本编辑器） */}
        <button
          type="button"
          title={expanded ? t('editor.restore') : t('editor.expand')}
          aria-label={expanded ? t('editor.restore') : t('editor.expand')}
          onClick={() => void toggleFullscreen()}
          className="absolute right-11 top-4 rounded-sm opacity-70 ring-offset-background transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
        >
          <i className={cn('fa-solid h-4 w-4', expanded ? 'fa-compress' : 'fa-expand')} />
        </button>

        <DialogHeader className="shrink-0 pr-14">
          <DialogTitle className="text-sm">
            <i className="fa-solid fa-reply mr-1.5 size-3" />
            {target ? t('comment.replyTo', { name: target.nickname }) : t('comment.replyTitle')}
          </DialogTitle>
        </DialogHeader>

        <div className={cn('min-h-0', expanded ? 'flex flex-1 flex-col overflow-hidden' : 'space-y-1.5 py-1')}>
          {/* Markdown 编辑 / 预览双 Tab（含语法工具栏与代码折叠） */}
          <MarkdownEditor
            value={draft}
            onChange={setDraft}
            maxLength={REPLY_MAX}
            autoFocus
            placeholder={t('comment.placeholder')}
            heightClass="h-[45vh]"
            fill={expanded}
            onKeyDown={(e) => {
              // Ctrl/Cmd + Enter 快捷发送
              if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') void handleSubmit()
            }}
          />
        </div>

        <DialogFooter className="shrink-0">
          <span className="text-muted-foreground mr-auto text-[10px] tabular-nums">
            {draft.length}/{REPLY_MAX}
          </span>
          <Button variant="outline" size="sm" className="h-8 text-xs" onClick={() => onOpenChange(false)}>
            {t('post.cancel')}
          </Button>
          <Button size="sm" className="h-8 text-xs" disabled={!canSend} onClick={handleSubmit}>
            {submitting && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
            {t('action.reply')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
