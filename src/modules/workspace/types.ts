/**
 * 工作区模块类型定义
 *
 * 与 `docs/database.md` §4.21-§4.25 中的 `workspaces`、`workspace_cli_configs`、
 * `workspace_prompts`、`workspace_mcp_servers`、`workspace_skills` 表结构对齐。
 * 后端 Rust 通过 ts-rs 自动生成同名类型，前端禁止手写同步覆盖此文件。
 */

import type { Timestamp, SnowflakeId } from '@/core/types'
import type { CliProfile } from '@/modules/cli-management/types'

/**
 * 工作区
 * 对应 `workspaces` 表
 *
 * 通过工作区隔离 Prompts / MCP / Skill 配置，
 * 切换激活工作区并应用后才写入 CLI 实际配置文件。
 */
export interface Workspace {
  id: SnowflakeId
  /** 全局唯一路由标识 */
  slug: string
  displayName: string
  /** 本地工作区目录路径 */
  rootPath: string
  /** 当前激活工作区（同一时刻仅允许一个激活） */
  isActive: boolean
  /** 上次应用到 CLI 的时间 */
  lastAppliedAt?: Timestamp
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 工作区 × CLI 配置头
 * 对应 `workspace_cli_configs` 表
 *
 * 每个工作区对每个 CLI 一条配置头，隔离 Prompts / MCP / Skill。
 * (workspace_id, cli_profile_id) 唯一。
 */
export interface WorkspaceCliConfig {
  id: SnowflakeId
  workspaceId: SnowflakeId
  cliProfileId: SnowflakeId
  /** 是否已写入 CLI 实际配置文件 */
  isApplied: boolean
  /** 切换/修改后待应用 */
  pendingApply: boolean
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 工作区 Prompt
 * 对应 `workspace_prompts` 表
 */
export interface WorkspacePrompt {
  id: SnowflakeId
  workspaceCliConfigId: SnowflakeId
  name: string
  content: string
  sortOrder: number
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * MCP 传输方式
 * 对应 `workspace_mcp_servers.transport` 列
 */
export type McpTransport = 'stdio' | 'sse' | 'http'

/**
 * 工作区 MCP 服务器配置
 * 对应 `workspace_mcp_servers` 表
 *
 * `configJson` 包含完整的 MCP server 配置（命令、参数、环境变量、URL 等），
 * 结构与 MCP 协议规范一致，由 CLI 解析。
 */
export interface WorkspaceMcpServer {
  id: SnowflakeId
  workspaceCliConfigId: SnowflakeId
  name: string
  transport: McpTransport
  /** MCP server 完整配置 JSON（命令、参数、环境变量、URL 等） */
  configJson: string
  isEnabled: boolean
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 工作区 Skill 配置
 * 对应 `workspace_skills` 表
 *
 * Skill 可以来自本地文件（`sourcePath`）或内联内容（`content`）。
 */
export interface WorkspaceSkill {
  id: SnowflakeId
  workspaceCliConfigId: SnowflakeId
  name: string
  /** 本地 skill 文件路径 */
  sourcePath?: string
  /** 内联 skill 内容（与 sourcePath 二选一） */
  content?: string
  isEnabled: boolean
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 工作区子配置聚合视图
 * 前端展示与「应用」操作时使用，包含一个 WorkspaceCliConfig 下的所有子配置
 * 以及关联的 CLI 档案信息。
 */
export interface WorkspaceCliConfigAggregate {
  config: WorkspaceCliConfig
  profile: CliProfile
  prompts: WorkspacePrompt[]
  mcpServers: WorkspaceMcpServer[]
  skills: WorkspaceSkill[]
}

/**
 * 工作区聚合数据
 * 后端 `workspace_aggregate` 命令返回，包含工作区下所有 CLI 配置及其子配置。
 */
export interface WorkspaceAggregate {
  workspace: Workspace
  cliConfigs: WorkspaceCliConfigAggregate[]
}

/**
 * 工作区预览结果
 * 后端 `workspace_preview` 命令返回，包含单个 CLI 配置头将要生成的配置文件内容。
 */
export interface WorkspacePreviewResult {
  workspaceCliConfigId: SnowflakeId
  cliProfileId: SnowflakeId
  cliType: string
  content: string
}

/**
 * 创建工作区的输入参数
 */
export interface CreateWorkspaceInput {
  slug: string
  displayName: string
  rootPath: string
  isActive?: boolean
}

/**
 * 更新工作区的输入参数
 */
export interface UpdateWorkspaceInput {
  slug?: string
  displayName?: string
  rootPath?: string
}

/**
 * 创建 Prompt 的输入参数
 */
export interface CreateWorkspacePromptInput {
  workspaceCliConfigId: SnowflakeId
  name: string
  content: string
  sortOrder?: number
}

/**
 * 更新 Prompt 的输入参数
 */
export interface UpdateWorkspacePromptInput {
  name?: string
  content?: string
  sortOrder?: number
}

/**
 * 创建 MCP Server 的输入参数
 */
export interface CreateWorkspaceMcpServerInput {
  workspaceCliConfigId: SnowflakeId
  name: string
  transport: McpTransport
  configJson: string
  isEnabled?: boolean
}

/**
 * 更新 MCP Server 的输入参数
 */
export interface UpdateWorkspaceMcpServerInput {
  name?: string
  transport?: McpTransport
  configJson?: string
  isEnabled?: boolean
}

/**
 * 创建 Skill 的输入参数
 */
export interface CreateWorkspaceSkillInput {
  workspaceCliConfigId: SnowflakeId
  name: string
  sourcePath?: string
  content?: string
  isEnabled?: boolean
}

/**
 * 更新 Skill 的输入参数
 */
export interface UpdateWorkspaceSkillInput {
  name?: string
  sourcePath?: string
  content?: string
  isEnabled?: boolean
}

/**
 * 单个 CLI 配置的应用结果
 */
export interface ApplyCliResult {
  cliProfileId: SnowflakeId
  success: boolean
  error?: string
}

/**
 * 应用工作区结果
 * 后端 `workspace_apply` 命令返回
 */
export interface ApplyWorkspaceResult {
  workspaceId: SnowflakeId
  /** 成功应用的 CLI 配置数量 */
  appliedCount: number
  /** 应用失败的 CLI 配置数量 */
  failedCount: number
  /** 每个 CLI 档案的应用详情 */
  details: ApplyCliResult[]
}
