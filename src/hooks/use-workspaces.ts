import { useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import type {
  Workspace,
  WorkspaceCliConfig,
  WorkspacePrompt,
  WorkspaceMcpServer,
  WorkspaceSkill,
} from '@/modules/workspace/types'

/**
 * 获取工作区列表
 *
 * 调用后端 `workspace_list` 命令。
 */
export function useWorkspaces(): {
  workspaces: Workspace[]
  loading: boolean
  refetch: () => void
} {
  const [workspaces, setWorkspaces] = useState<Workspace[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    setLoading(true)
    try {
      const result = await invokeCommand<Workspace[]>('workspace_list')
      setWorkspaces(result)
    } catch {
      setWorkspaces([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [])

  return { workspaces, loading, refetch: load }
}

/**
 * 获取指定工作区下的 CLI 配置头列表
 *
 * 调用后端 `workspace_cli_config_list` 命令。
 */
export function useWorkspaceCliConfigs(workspaceId: string | null): {
  configs: WorkspaceCliConfig[]
  loading: boolean
  refetch: () => void
} {
  const [configs, setConfigs] = useState<WorkspaceCliConfig[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    if (!workspaceId) {
      setConfigs([])
      return
    }
    setLoading(true)
    try {
      const result = await invokeCommand<WorkspaceCliConfig[]>('workspace_cli_config_list', {
        workspaceId,
      })
      setConfigs(result)
    } catch {
      setConfigs([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [workspaceId])

  return { configs, loading, refetch: load }
}

/**
 * 获取指定 CLI 配置头下的 Prompts
 */
export function useWorkspacePrompts(configId: string | null): {
  prompts: WorkspacePrompt[]
  loading: boolean
  refetch: () => void
} {
  const [prompts, setPrompts] = useState<WorkspacePrompt[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    if (!configId) {
      setPrompts([])
      return
    }
    setLoading(true)
    try {
      const result = await invokeCommand<WorkspacePrompt[]>('workspace_prompt_list', {
        workspaceCliConfigId: configId,
      })
      setPrompts(result)
    } catch {
      setPrompts([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [configId])

  return { prompts, loading, refetch: load }
}

/**
 * 获取指定 CLI 配置头下的 MCP Servers
 */
export function useWorkspaceMcpServers(configId: string | null): {
  servers: WorkspaceMcpServer[]
  loading: boolean
  refetch: () => void
} {
  const [servers, setServers] = useState<WorkspaceMcpServer[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    if (!configId) {
      setServers([])
      return
    }
    setLoading(true)
    try {
      const result = await invokeCommand<WorkspaceMcpServer[]>('workspace_mcp_server_list', {
        workspaceCliConfigId: configId,
      })
      setServers(result)
    } catch {
      setServers([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [configId])

  return { servers, loading, refetch: load }
}

/**
 * 获取指定 CLI 配置头下的 Skills
 */
export function useWorkspaceSkills(configId: string | null): {
  skills: WorkspaceSkill[]
  loading: boolean
  refetch: () => void
} {
  const [skills, setSkills] = useState<WorkspaceSkill[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    if (!configId) {
      setSkills([])
      return
    }
    setLoading(true)
    try {
      const result = await invokeCommand<WorkspaceSkill[]>('workspace_skill_list', {
        workspaceCliConfigId: configId,
      })
      setSkills(result)
    } catch {
      setSkills([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [configId])

  return { skills, loading, refetch: load }
}


