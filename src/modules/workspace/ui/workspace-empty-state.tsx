import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

interface WorkspaceEmptyStateProps {
  onCreate: () => void
}

/**
 * 工作区空状态
 *
 * 当没有工作区时展示，引导用户创建第一个工作区。
 */
export function WorkspaceEmptyState({ onCreate }: WorkspaceEmptyStateProps) {
  const { t } = useTranslation()

  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-4 rounded-md border border-dashed p-8">
      <i className={cn('fa-solid fa-briefcase', 'text-muted-foreground', 'text-4xl')} />
      <p className="text-muted-foreground text-center text-sm">{t('workspace.empty.noWorkspace')}</p>
      <Button size="sm" onClick={onCreate}>
        <i className={cn('fa-solid fa-plus', 'mr-1.5')} />
        {t('workspace.selector.new')}
      </Button>
    </div>
  )
}
