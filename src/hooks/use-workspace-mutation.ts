import { invokeCommand } from '@/hooks/use-command'
import type {
  Workspace,
  WorkspacePrompt,
  WorkspaceMcpServer,
  WorkspaceSkill,
  ApplyCliResult,
  ApplyWorkspaceResult,
  WorkspacePreviewResult,
  CreateWorkspaceInput,
  UpdateWorkspaceInput,
  CreateWorkspacePromptInput,
  UpdateWorkspacePromptInput,
  CreateWorkspaceMcpServerInput,
  UpdateWorkspaceMcpServerInput,
  CreateWorkspaceSkillInput,
  UpdateWorkspaceSkillInput,
} from '@/modules/workspace/types'

/**
 * 工作区模块写入操作集合
 *
 * 封装对工作区（Workspace）及其子配置（Prompt / MCP / Skill）的增删改命令调用。
 */

// ===== 工作区 =====

export async function createWorkspace(input: CreateWorkspaceInput): Promise<Workspace | null> {
  try {
    return await invokeCommand<Workspace>('workspace_create', { input })
  } catch {
    return null
  }
}

export async function updateWorkspace(
  id: string,
  input: UpdateWorkspaceInput
): Promise<Workspace | null> {
  try {
    return await invokeCommand<Workspace>('workspace_update', { id, input })
  } catch {
    return null
  }
}

export async function deleteWorkspace(id: string): Promise<boolean> {
  try {
    await invokeCommand<void>('workspace_delete', { id })
    return true
  } catch {
    return false
  }
}

export async function switchWorkspace(id: string): Promise<Workspace | null> {
  try {
    return await invokeCommand<Workspace>('workspace_switch', { id })
  } catch {
    return null
  }
}

// ===== Prompt =====

export async function createWorkspacePrompt(
  input: CreateWorkspacePromptInput
): Promise<WorkspacePrompt | null> {
  try {
    return await invokeCommand<WorkspacePrompt>('workspace_prompt_create', { input })
  } catch {
    return null
  }
}

export async function updateWorkspacePrompt(
  id: string,
  input: UpdateWorkspacePromptInput
): Promise<WorkspacePrompt | null> {
  try {
    return await invokeCommand<WorkspacePrompt>('workspace_prompt_update', { id, input })
  } catch {
    return null
  }
}

export async function deleteWorkspacePrompt(id: string): Promise<boolean> {
  try {
    await invokeCommand<void>('workspace_prompt_delete', { id })
    return true
  } catch {
    return false
  }
}

// ===== MCP Server =====

export async function createWorkspaceMcpServer(
  input: CreateWorkspaceMcpServerInput
): Promise<WorkspaceMcpServer | null> {
  try {
    return await invokeCommand<WorkspaceMcpServer>('workspace_mcp_server_create', { input })
  } catch {
    return null
  }
}

export async function updateWorkspaceMcpServer(
  id: string,
  input: UpdateWorkspaceMcpServerInput
): Promise<WorkspaceMcpServer | null> {
  try {
    return await invokeCommand<WorkspaceMcpServer>('workspace_mcp_server_update', { id, input })
  } catch {
    return null
  }
}

export async function deleteWorkspaceMcpServer(id: string): Promise<boolean> {
  try {
    await invokeCommand<void>('workspace_mcp_server_delete', { id })
    return true
  } catch {
    return false
  }
}

// ===== Skill =====

export async function createWorkspaceSkill(
  input: CreateWorkspaceSkillInput
): Promise<WorkspaceSkill | null> {
  try {
    return await invokeCommand<WorkspaceSkill>('workspace_skill_create', { input })
  } catch {
    return null
  }
}

export async function updateWorkspaceSkill(
  id: string,
  input: UpdateWorkspaceSkillInput
): Promise<WorkspaceSkill | null> {
  try {
    return await invokeCommand<WorkspaceSkill>('workspace_skill_update', { id, input })
  } catch {
    return null
  }
}

export async function deleteWorkspaceSkill(id: string): Promise<boolean> {
  try {
    await invokeCommand<void>('workspace_skill_delete', { id })
    return true
  } catch {
    return false
  }
}

// ===== 应用与预览 =====

export async function applyWorkspace(workspaceId: string): Promise<ApplyWorkspaceResult | null> {
  try {
    return await invokeCommand<ApplyWorkspaceResult>('workspace_apply', { id: workspaceId })
  } catch {
    return null
  }
}

export async function applyCliConfig(workspaceCliConfigId: string): Promise<ApplyCliResult | null> {
  try {
    return await invokeCommand<ApplyCliResult>('workspace_apply_cli_config', {
      workspaceCliConfigId,
    })
  } catch {
    return null
  }
}

export async function previewCliConfig(
  workspaceCliConfigId: string
): Promise<WorkspacePreviewResult | null> {
  try {
    return await invokeCommand<WorkspacePreviewResult>('workspace_preview', {
      workspaceCliConfigId,
    })
  } catch {
    return null
  }
}
