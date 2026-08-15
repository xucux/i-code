/**
 * 举报弹层（§6 内容治理：任意用户可举报帖子 / 回复）
 *
 * 仅收集可选原因（选填），提交后由管理员在举报列表处理。
 */

import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { reportCommunityContent } from '@/hooks/use-community'

/** 原因字数上限（防灌水） */
const REASON_MAX = 200

export interface ReportDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 举报目标类型 */
  targetType: 'post' | 'reply'
  /** 举报目标 ID */
  targetId: number | null
  /** 目标内容预览（弹层展示，便于确认） */
  preview?: string
}

export function ReportDialog({ open, onOpenChange, targetType, targetId, preview }: ReportDialogProps) {
  const { t } = useTranslation('community')
  const [reason, setReason] = useState('')
  const [submitting, setSubmitting] = useState(false)

  // 每次打开重置原因
  useEffect(() => {
    if (open) setReason('')
  }, [open])

  const handleSubmit = async () => {
    if (targetId == null || submitting) return
    setSubmitting(true)
    try {
      await reportCommunityContent({
        targetType,
        targetId,
        reason: reason.trim() || undefined,
      })
      toast.success(t('success.report'))
      onOpenChange(false)
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="text-sm">
            {t('report.title')} · {targetType === 'post' ? t('report.post') : t('report.reply')}
          </DialogTitle>
          <DialogDescription className="text-xs">{t('report.desc')}</DialogDescription>
        </DialogHeader>

        {preview && (
          <div className="bg-muted/50 max-h-20 overflow-hidden rounded-md border p-2">
            <p className="text-muted-foreground line-clamp-3 text-xs">{preview}</p>
          </div>
        )}

        <div className="space-y-1">
          <div className="flex items-center justify-between">
            <Label htmlFor="report-reason" className="text-xs">
              {t('report.reason')}
            </Label>
            <span className="text-muted-foreground text-[10px] tabular-nums">
              {reason.length}/{REASON_MAX}
            </span>
          </div>
          <Textarea
            id="report-reason"
            value={reason}
            maxLength={REASON_MAX}
            placeholder={t('report.reasonPlaceholder')}
            onChange={(e) => setReason(e.target.value)}
            className="min-h-20 text-xs"
          />
        </div>

        <DialogFooter>
          <Button variant="outline" size="sm" className="h-8 text-xs" onClick={() => onOpenChange(false)}>
            {t('post.cancel')}
          </Button>
          <Button variant="destructive" size="sm" className="h-8 text-xs" disabled={submitting} onClick={handleSubmit}>
            {submitting && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
            {t('report.submit')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
