/**
 * CLI 管理模块类型定义
 *
 * 与 `docs/database.md` §4.18-§4.20 中的 `cli_profiles`、`cli_providers`、
 * `cli_model_mappings` 表结构对齐。
 * 后端 Rust 通过 ts-rs 自动生成同名类型，前端禁止手写同步覆盖此文件。
 */

import type { Timestamp, SnowflakeId } from '@/core/types'
import type { BalanceSnapshot } from '@/modules/balance/types'

/**
 * CLI 级代理配置
 * 对应 cli_profiles.proxy_json 字段
 *
 * 与供应商级代理 `ProxyConfig` 区分：CLI 代理保留「直连」「自定义」「系统代理」
 * 「VS Code 代理」四种策略，便于生成各 CLI 配置文件时独立处理。
 */
export interface CliProxyConfig {
  type: 'direct' | 'custom' | 'system' | 'vscode'
  /** 自定义代理 URL（仅 `custom` 类型生效） */
  url?: string
  /** 代理认证凭据或 `$SECRET:{snowflake_id}$` 引用 */
  authorization?: string
  /** 是否严格校验 SSL 证书（默认 true） */
  strictSSL?: boolean
  /** 不走代理的主机列表（NO_PROXY 环境变量等价物） */
  noProxy?: string[]
}

/**
 * 受管 CLI 类型
 * 对应 database.md §5.2
 */
export type CliType =
  | 'claude-code'
  | 'codex'
  | 'opencode'
  | 'gemini-cli'
  | 'cursor-agent'
  | 'custom'

/**
 * CLI 模型映射输入模式
 * 对应 `cli_model_mappings.input_mode` 列
 * - `select`：从已暴露的 Gateway 模型列表选择
 * - `manual`：用户手动输入真实模型 ID
 */
export type CliModelMappingInputMode = 'select' | 'manual'

/**
 * CLI 配置档案
 * 对应 `cli_profiles` 表
 *
 * 每个受管 CLI（Claude Code、Codex、Gemini CLI 等）一条记录。
 */
export interface CliProfile {
  id: SnowflakeId
  /** 全局唯一路由标识，如 `claude-code` */
  slug: string
  displayName: string
  cliType: CliType
  /** CLI 实际配置文件路径，应用工作区时写入此文件 */
  configFilePath?: string
  /** CLI 级代理配置 JSON（ProxyConfig 序列化） */
  proxyJson?: string
  isEnabled: boolean
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * CLI 配置档案 DTO（前端表单使用）
 * 将 JSON 字段反序列化为强类型对象
 */
export interface CliProfileDto {
  id: SnowflakeId
  slug: string
  displayName: string
  cliType: CliType
  configFilePath?: string
  proxy?: CliProxyConfig
  isEnabled: boolean
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * CLI 绑定的供应商
 * 对应 `cli_providers` 表
 *
 * 支持两种路由模式：
 * - `routeMode = true`：走本地网关代理，`gatewayBaseUrl` 为空时运行时从 `app_settings` 动态拼接
 * - `routeMode = false`：直连 `directBaseUrl`，必须填写
 */
export interface CliProvider {
  id: SnowflakeId
  cliProfileId: SnowflakeId
  /** 绑定的 Gateway 供应商 ID；路由模式时必填，可空（DELETE 时 SET NULL） */
  providerId?: SnowflakeId
  displayName: string
  /** 1=走本地网关代理；0=直连 */
  routeMode: number
  /** 路由模式下的网关地址；为空时运行时从 `app_settings.gateway_host:gateway_port` 动态拼接 */
  gatewayBaseUrl?: string
  /** 非路由模式直连地址；`routeMode=0` 时必填 */
  directBaseUrl?: string
  /** CLI 侧认证 JSON（可与 Gateway 分离） */
  authJson?: string
  /** 额度展示缓存 JSON（BalanceSnapshot 序列化） */
  balanceJson?: string
  sortOrder: number
  isDefault: boolean
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * CLI 供应商 DTO（前端展示与表单使用）
 * 将 JSON 字段反序列化为强类型对象
 */
export interface CliProviderDto {
  id: SnowflakeId
  cliProfileId: SnowflakeId
  providerId?: SnowflakeId
  displayName: string
  routeMode: number
  gatewayBaseUrl?: string
  directBaseUrl?: string
  /** CLI 侧认证配置（结构待定，先保留为 JSON 字符串） */
  authJson?: string
  /** 额度快照 */
  balance?: BalanceSnapshot
  sortOrder: number
  isDefault: boolean
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * CLI 模型映射
 * 对应 `cli_model_mappings` 表
 *
 * 支持两种输入模式：
 * - `select`：从 Gateway 已暴露模型列表选择（填充 `gatewayModelId`）
 * - `manual`：手动输入真实模型 ID（填充 `rawModelId`）
 */
export interface CliModelMapping {
  id: SnowflakeId
  /** 所属 CLI 供应商 ID（注意：是 cli_provider_id，不是 cli_profile_id） */
  cliProviderId: SnowflakeId
  /** CLI 内使用的模型名别名，如 `gpt-4o` */
  cliModelAlias: string
  /** 路由模式下指向 Gateway 模型 ID（`{provider_slug}/{model_id}` 格式） */
  gatewayModelId?: string
  /** 非路由模式真实模型 ID */
  rawModelId?: string
  inputMode: CliModelMappingInputMode
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 创建 CLI 配置档案的输入参数
 */
export interface CreateCliProfileInput {
  slug: string
  displayName: string
  cliType: CliType
  configFilePath?: string
  proxyJson?: string
  isEnabled?: boolean
}

/**
 * 更新 CLI 配置档案的输入参数
 */
export interface UpdateCliProfileInput {
  slug?: string
  displayName?: string
  cliType?: CliType
  configFilePath?: string
  proxyJson?: string
  isEnabled?: boolean
}

/**
 * 创建 CLI 供应商绑定的输入参数
 */
export interface CreateCliProviderInput {
  cliProfileId: SnowflakeId
  providerId?: SnowflakeId
  displayName: string
  /** 1=走本地网关代理；0=直连 */
  routeMode: number
  gatewayBaseUrl?: string
  directBaseUrl?: string
  authJson?: string
  sortOrder?: number
  isDefault?: boolean
}

/**
 * 更新 CLI 供应商绑定的输入参数
 */
export interface UpdateCliProviderInput {
  providerId?: SnowflakeId
  displayName?: string
  routeMode?: number
  gatewayBaseUrl?: string
  directBaseUrl?: string
  authJson?: string
  sortOrder?: number
  isDefault?: boolean
}

/**
 * 创建 CLI 模型映射的输入参数
 */
export interface CreateCliModelMappingInput {
  cliProviderId: SnowflakeId
  cliModelAlias: string
  gatewayModelId?: string
  rawModelId?: string
  inputMode: CliModelMappingInputMode
}

/**
 * 更新 CLI 模型映射的输入参数
 */
export interface UpdateCliModelMappingInput {
  cliModelAlias?: string
  gatewayModelId?: string
  rawModelId?: string
  inputMode?: CliModelMappingInputMode
}

export type CliConfigFormat = 'json' | 'jsonc' | 'toml'
export type CliConfigParseStatus = 'missing' | 'valid' | 'invalid'

/** CLI 配置文件只读探测结果；后端不会返回文件正文。 */
export interface CliConfigFileInspection {
  cliType: CliType
  configuredPath?: string
  suggestedPath: string
  resolvedPath: string
  format: CliConfigFormat
  exists: boolean
  isFile: boolean
  readable: boolean
  parseStatus: CliConfigParseStatus
  issue?: 'not-file' | 'unreadable' | 'invalid-syntax'
  /** 对应客户端 CLI 是否在 PATH 中可用 */
  clientAvailable: boolean
}

/** CLI 配置文件内容（前端编辑回写） */
export interface CliConfigFileContent {
  cliType: CliType
  resolvedPath: string
  format: CliConfigFormat
  content: string
}
