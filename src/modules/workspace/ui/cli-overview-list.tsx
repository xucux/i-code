import { useTranslation } from '@/modules/i18n/use-translation'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { WorkspaceCliConfigAggregate } from '@/modules/workspace/types'

interface CliOverviewListProps {
  configs: WorkspaceCliConfigAggregate[]
  onApply: (configId: string) => void
  onPreview: (configId: string) => void
}

/**
 * CLI 概览列表
 *
 * 主 Tab 下展示当前工作区已配置的所有 CLI 档案，含名称、类型、应用状态。
 * 每行提供「应用」「预览」快捷操作。
 */
export function CliOverviewList({ configs, onApply, onPreview }: CliOverviewListProps) {
  const { t } = useTranslation()

  if (configs.length === 0) {
    return (
      <div className="text-muted-foreground flex h-40 items-center justify-center text-sm">
        {t('workspace.empty.noCliConfig')}
      </div>
    )
  }

  return (
    <div className="space-y-2">
      {configs.map((cfg) => {
        const pending = cfg.config.pendingApply
        const applied = cfg.config.isApplied && !pending
        return (
          <div
            key={cfg.config.id}
            className="group flex items-center justify-between rounded-md border p-3"
          >
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <p className="truncate text-sm font-medium">{cfg.profile.displayName}</p>
                <Badge variant="secondary" className="text-[10px]">
                  {cfg.profile.cliType}
                </Badge>
                {applied ? (
                  <Badge variant="outline" className="text-[10px]">
                    {t('workspace.status.applied')}
                  </Badge>
                ) : (
                  <Badge variant="destructive" className="text-[10px]">
                    {t('workspace.status.pending')}
                  </Badge>
                )}
              </div>
              <p className="text-muted-foreground truncate text-xs font-mono">
                {cfg.profile.slug}
              </p>
            </div>
            <div className="ml-2 flex items-center gap-1">
              <Button
                variant="ghost"
                size="sm"
                className="h-7 text-xs"
                onClick={() => onPreview(cfg.config.id)}
              >
                {t('workspace.actions.preview')}
              </Button>
              <Button
                size="sm"
                className={cn('h-7 text-xs', pending && 'animate-pulse')}
                onClick={() => onApply(cfg.config.id)}
              >
                {t('workspace.actions.apply')}
              </Button>
            </div>
          </div>
        )
      })}
    </div>
  )
}
