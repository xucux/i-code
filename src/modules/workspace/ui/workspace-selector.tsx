import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { cn } from '@/lib/utils'
import type { Workspace } from '@/modules/workspace/types'

interface WorkspaceSelectorProps {
  workspaces: Workspace[]
  selectedWorkspaceId: string | null
  onSelect: (id: string | null) => void
  onCreate: () => void
}

/**
 * 工作区选择器
 *
 * 页面顶部最左侧的下拉选择框 + 新建工作区按钮。
 * 工作区列表按激活状态优先、显示名称排序。
 */
export function WorkspaceSelector({
  workspaces,
  selectedWorkspaceId,
  onSelect,
  onCreate,
}: WorkspaceSelectorProps) {
  const { t } = useTranslation()

  return (
    <div className="flex items-center gap-2">
      <Select value={selectedWorkspaceId ?? ''} onValueChange={(v) => onSelect(v || null)}>
        <SelectTrigger className="h-8 w-[200px] text-xs">
          <SelectValue placeholder={t('workspace.selector.placeholder')} />
        </SelectTrigger>
        <SelectContent>
          {workspaces.map((workspace) => (
            <SelectItem key={workspace.id} value={workspace.id} className="text-xs">
              <span className="flex items-center gap-2">
                {workspace.isActive && (
                  <i className={cn('fa-solid fa-circle-check', 'text-primary', 'text-[10px]')} />
                )}
                <span className="truncate">{workspace.displayName}</span>
              </span>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <Button size="icon" className="size-8" onClick={onCreate} title={t('workspace.selector.new')}>
        <i className="fa-solid fa-plus text-xs" />
      </Button>
    </div>
  )
}
