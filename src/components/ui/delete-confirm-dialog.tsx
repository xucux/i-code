import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { useTranslation } from '@/modules/i18n/use-translation'

interface DeleteConfirmDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description?: string
  onConfirm: () => void
  /** 确认按钮文案，默认使用 common.delete */
  confirmText?: string
  /** 确认按钮变体，默认 destructive */
  confirmVariant?: 'destructive' | 'default'
}

/**
 * 通用删除确认对话框
 *
 * 用于各业务模块的删除二次确认，避免误操作。
 */
export function DeleteConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  onConfirm,
  confirmText,
  confirmVariant = 'destructive',
}: DeleteConfirmDialogProps) {
  const { t } = useTranslation()

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="text-base">{title}</DialogTitle>
          {description && <DialogDescription className="text-xs">{description}</DialogDescription>}
        </DialogHeader>
        <DialogFooter>
          <Button variant="ghost" size="sm" className="h-8 text-xs" onClick={() => onOpenChange(false)}>
            {t('common.cancel')}
          </Button>
          <Button variant={confirmVariant} size="sm" className="h-8 text-xs" onClick={onConfirm}>
            {confirmText ?? t('common.delete')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
