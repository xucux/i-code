import { createFileRoute } from '@tanstack/react-router'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { WorkspaceManager } from '@/modules/workspace/ui/workspace-manager'

/**
 * 工作区首页
 *
 * 按项目/目录隔离 Prompts、MCP Servers、Skills 配置，并支持一键应用。
 * 页面使用 `useAvailableHeight` 计算内容区可用高度，并传入 WorkspaceManager。
 */
function WorkspacesIndexPage() {
  const [height, pageRef] = useAvailableHeight()

  return (
    <div ref={pageRef} className="flex h-full flex-col p-6">
      <div className="min-h-0 flex-1">
        <WorkspaceManager height={height - 80} />
      </div>
    </div>
  )
}

export const Route = createFileRoute('/workspaces/')({
  component: WorkspacesIndexPage,
})
