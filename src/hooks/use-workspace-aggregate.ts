import { useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import type { Workspace, WorkspaceAggregate } from '@/modules/workspace/types'

/**
 * 工作区聚合数据 Hook
 *
 * 加载工作区列表，并默认选中第一个工作区；选中工作区变化时调用
 * 后端 `workspace_aggregate` 一次性加载该工作区下所有 CLI 配置及其子配置。
 */
export function useWorkspaceAggregate(): {
  workspaces: Workspace[]
  selectedWorkspaceId: string | null
  setSelectedWorkspaceId: (id: string | null) => void
  aggregate: WorkspaceAggregate | null
  loading: boolean
  refetchWorkspaces: () => Promise<void>
  refetchAggregate: () => Promise<void>
} {
  const [workspaces, setWorkspaces] = useState<Workspace[]>([])
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState<string | null>(null)
  const [aggregate, setAggregate] = useState<WorkspaceAggregate | null>(null)
  const [loading, setLoading] = useState(false)

  const loadWorkspaces = async () => {
    try {
      const result = await invokeCommand<Workspace[]>('workspace_list')
      setWorkspaces(result)
      // 首次加载且未选中的情况下，默认选中第一个工作区
      setSelectedWorkspaceId((prev) => {
        if (prev) return prev
        return result[0]?.id ?? null
      })
    } catch {
      setWorkspaces([])
    }
  }

  const loadAggregate = async () => {
    if (!selectedWorkspaceId) {
      setAggregate(null)
      return
    }
    setLoading(true)
    try {
      const result = await invokeCommand<WorkspaceAggregate>('workspace_aggregate', {
        id: selectedWorkspaceId,
      })
      setAggregate(result)
    } catch {
      setAggregate(null)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void loadWorkspaces()
  }, [])

  useEffect(() => {
    void loadAggregate()
  }, [selectedWorkspaceId])

  return {
    workspaces,
    selectedWorkspaceId,
    setSelectedWorkspaceId,
    aggregate,
    loading,
    refetchWorkspaces: loadWorkspaces,
    refetchAggregate: loadAggregate,
  }
}
