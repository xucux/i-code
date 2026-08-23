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
import { Textarea } from '@/components/ui/textarea'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { MarkdownContent } from '@/components/ui/markdown-content'

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
      <DialogContent className="h-[80vh] max-w-lg">
        <DialogHeader>
          <DialogTitle className="text-sm">
            <i className="fa-solid fa-reply mr-1.5 size-3" />
            {target ? t('comment.replyTo', { name: target.nickname }) : t('comment.replyTitle')}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-2 py-1">
          {/* Markdown 编辑 / 预览双 Tab */}
          <Tabs defaultValue="edit">
            <TabsList className="h-6">
              <TabsTrigger
                value="edit"
                className="text-muted-foreground h-4 px-2 text-[11px] data-[state=active]:text-foreground"
              >
                <i className="fa-solid fa-pen mr-1 size-2.5" />
                {t('post.editTab')}
              </TabsTrigger>
              <TabsTrigger
                value="preview"
                className="text-muted-foreground h-4 px-2 text-[11px] data-[state=active]:text-foreground"
              >
                <i className="fa-solid fa-eye mr-1 size-2.5" />
                {t('post.previewTab')}
              </TabsTrigger>
            </TabsList>
            <TabsContent value="edit" className="mt-1.5">
              <Textarea
                value={draft}
                maxLength={REPLY_MAX}
                autoFocus
                placeholder={t('comment.placeholder')}
                onChange={(e) => setDraft(e.target.value)}
                onKeyDown={(e) => {
                  // Ctrl/Cmd + Enter 快捷发送
                  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') void handleSubmit()
                }}
                className="h-[45vh] resize-none text-xs leading-relaxed"
              />
            </TabsContent>
            <TabsContent value="preview" className="mt-1.5">
              {/* 预览区与编辑区等高，内容超高时内部滚动 */}
              <div className="h-[45vh] overflow-y-auto rounded-md border p-3">
                {draft.trim() ? (
                  <MarkdownContent content={draft} />
                ) : (
                  <p className="text-muted-foreground text-xs">{t('post.previewEmpty')}</p>
                )}
              </div>
            </TabsContent>
          </Tabs>
        </div>

        <DialogFooter>
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
