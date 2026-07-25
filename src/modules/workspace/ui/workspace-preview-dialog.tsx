import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { ScrollPage } from '@/components/ui/scroll-page'
import type { WorkspacePreviewResult } from '@/modules/workspace/types'

interface WorkspacePreviewDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  result: WorkspacePreviewResult | null
  loading: boolean
}

/**
 * 工作区配置预览对话框
 *
 * 只读展示指定 CLI 配置头将要生成的配置文件内容。
 * 当前为统一 JSON 格式，后续可按 CLI 类型生成对应原生格式。
 */
export function WorkspacePreviewDialog({
  open,
  onOpenChange,
  result,
  loading,
}: WorkspacePreviewDialogProps) {
  const { t } = useTranslation()

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-w-2xl flex-col">
        <DialogHeader>
          <DialogTitle className="text-base">{t('workspace.preview.title')}</DialogTitle>
          {result && (
            <DialogDescription className="text-xs">
              {result.cliType} · {result.cliProfileId}
            </DialogDescription>
          )}
        </DialogHeader>
        <ScrollPage className="mt-2 min-h-[300px] flex-1" variant="default">
          {loading ? (
            <div className="text-muted-foreground flex h-40 items-center justify-center text-sm">
              {t('common.loading')}
            </div>
          ) : result ? (
            <pre className="whitespace-pre-wrap break-all p-4 font-mono text-xs">{result.content}</pre>
          ) : (
            <div className="text-muted-foreground flex h-40 items-center justify-center text-sm">
              {t('workspace.preview.empty')}
            </div>
          )}
        </ScrollPage>
        <div className="mt-4 flex justify-end">
          <Button size="sm" className="h-8 text-xs" onClick={() => onOpenChange(false)}>
            {t('common.close')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
