import { useTranslation } from '@/modules/i18n/use-translation'
import { Badge } from '@/components/ui/badge'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import type { WorkspaceCliConfigAggregate } from '@/modules/workspace/types'

interface CliConfigSelectDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  configs: WorkspaceCliConfigAggregate[]
  onSelect: (config: WorkspaceCliConfigAggregate) => void
}

/**
 * CLI 配置头选择对话框
 *
 * 新建 Prompt / Skill / MCP 时，若当前工作区存在多个 CLI 配置头，
 * 先弹出此对话框让用户选择要关联到哪个 CLI 档案。
 * 只有一个配置头时，组件外部应直接选中而不弹出。
 */
export function CliConfigSelectDialog({
  open,
  onOpenChange,
  configs,
  onSelect,
}: CliConfigSelectDialogProps) {
  const { t } = useTranslation()

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="text-base">{t('workspace.cliSelect.title')}</DialogTitle>
          <DialogDescription className="text-xs">
            {t('workspace.cliSelect.description')}
          </DialogDescription>
        </DialogHeader>
        <div className="mt-2 space-y-2">
          {configs.map((cfg) => (
            <button
              key={cfg.config.id}
              type="button"
              onClick={() => onSelect(cfg)}
              className="flex w-full items-center justify-between rounded-md border p-3 text-left transition-colors hover:bg-muted/50"
            >
              <div>
                <p className="text-sm font-medium">{cfg.profile.displayName}</p>
                <p className="text-muted-foreground text-xs font-mono">{cfg.profile.slug}</p>
              </div>
              <Badge variant="secondary" className="text-[10px]">
                {cfg.profile.cliType}
              </Badge>
            </button>
          ))}
          {configs.length === 0 && (
            <p className="text-muted-foreground py-4 text-center text-sm">
              {t('workspace.empty.noCliConfig')}
            </p>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
