import { useTranslation } from '@/modules/i18n/use-translation'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import type { WorkspacePrompt } from '@/modules/workspace/types'
import type { CliProfile } from '@/modules/cli-management/types'

interface PromptListProps {
  prompts: Array<{ item: WorkspacePrompt; profile: CliProfile }>
  onEdit: (prompt: WorkspacePrompt) => void
  onDelete: (prompt: WorkspacePrompt) => void
  onPreview: (prompt: WorkspacePrompt) => void
}

/**
 * 提示词列表
 *
 * 「提示词」Tab 下展示当前工作区跨 CLI 聚合的所有 Prompt。
 * 每行显示所属 CLI 档案，并提供编辑、删除、预览快捷操作。
 */
export function PromptList({ prompts, onEdit, onDelete, onPreview }: PromptListProps) {
  const { t } = useTranslation()

  if (prompts.length === 0) {
    return (
      <div className="text-muted-foreground flex h-40 items-center justify-center text-sm">
        {t('workspace.empty.noPrompts')}
      </div>
    )
  }

  return (
    <div className="space-y-2">
      {prompts.map(({ item, profile }) => (
        <div
          key={item.id}
          className="group flex items-start justify-between rounded-md border p-2.5"
        >
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-medium">{item.name}</p>
            <p className="text-muted-foreground line-clamp-2 text-xs">{item.content}</p>
            <div className="mt-1 flex items-center gap-2">
              <span className="text-muted-foreground text-[10px]">
                {t('workspace.actions.belongsTo')}: {profile.displayName}
              </span>
              <Badge variant="secondary" className="text-[10px]">
                {profile.cliType}
              </Badge>
            </div>
          </div>
          <div className="ml-2 flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100">
            <Button variant="ghost" size="icon" className="size-6" onClick={() => onPreview(item)}>
              <i className="fa-solid fa-eye text-[10px]" />
            </Button>
            <Button variant="ghost" size="icon" className="size-6" onClick={() => onEdit(item)}>
              <i className="fa-solid fa-pen text-[10px]" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-6 text-destructive hover:text-destructive"
              onClick={() => onDelete(item)}
            >
              <i className="fa-solid fa-trash text-[10px]" />
            </Button>
          </div>
        </div>
      ))}
    </div>
  )
}
