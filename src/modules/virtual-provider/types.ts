/**
 * 虚拟供应商与故障转移模块类型定义
 *
 * 对应 `docs/development.md` §5.16。允许用户创建「虚拟供应商」与「虚拟模型 ID」，
 * 每个虚拟模型 ID 背后关联一组真实供应商的真实模型，按优先级与故障转移策略自动选择。
 *
 * 依赖方向：
 * - 依赖 `ai-gateway`：读取真实供应商/模型
 * - 依赖 `gateway-runtime`：在转发时执行故障转移
 * - 为 `cli-management` 提供可选的虚拟 CLI 供应商绑定
 *
 * **典型场景**：用户同时拥有多个 OpenAI 渠道、NVIDIA、DeepSeek 等。
 * 当某渠道额度耗尽或网络不稳定时，虚拟供应商自动切换到下一个可用模型，
 * Agent 客户端只需固定配置 AI Gateway 地址、API Key 与虚拟模型 ID。
 */

import type { Timestamp, SnowflakeId } from '@/core/types'

/**
 * 故障转移策略
 * 与后端 `VirtualProviderStrategy` 对齐，使用 snake_case 字符串。
 *
 * - `fallback`：按优先级顺序尝试，失败则切换下一条（默认）
 * - `on_all`：同时请求所有可用路由（v0.1 未实现）
 * - `load_balance`：按权重轮询（v0.1 未实现）
 */
export type FailoverStrategy =
  | 'fallback'
  | 'on_all'
  | 'load_balance'

/**
 * 虚拟供应商
 * 对应数据库表 `virtual_providers`
 *
 * 对外表现为一个普通供应商，拥有自己的 alias、名称与故障转移策略。
 * 客户端看到的供应商列表中，虚拟供应商与其他真实供应商并列。
 */
export interface VirtualProvider {
  id: SnowflakeId
  /** 名称 */
  name: string
  /** 别名（唯一），用于路由 */
  alias: string
  /** 展示名称（可选） */
  displayName?: string
  /** 是否启用 */
  isEnabled: boolean
  /** 故障转移策略 */
  strategy: FailoverStrategy
  /** 错误重试次数（默认 3） */
  maxRetries: number
  /** 重试间隔毫秒数（默认 1000） */
  retryIntervalMs: number
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 虚拟模型
 * 对应数据库表 `virtual_models`
 *
 * 虚拟供应商对外暴露的模型标识，例如 `smart-failover-gpt`。
 * 该 ID 在 AI Gateway 内部被解析为一组真实模型路由。
 */
export interface VirtualModel {
  id: SnowflakeId
  /** 所属虚拟供应商 ID */
  virtualProviderId: SnowflakeId
  /** 对外虚拟模型 ID，如 `smart-failover-gpt` */
  modelId: string
  /** 展示名称 */
  displayName?: string
  /** 是否启用 */
  isEnabled: boolean
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 虚拟模型路由
 * 对应数据库表 `virtual_model_routes`
 *
 * 一条从虚拟模型 ID 到真实模型实例的映射。
 * 同一虚拟模型下的路由按 priority ASC 排序，priority 越小优先级越高。
 */
export interface VirtualModelRoute {
  id: SnowflakeId
  /** 所属虚拟模型 ID */
  virtualModelId: SnowflakeId
  /** 目标真实供应商 ID */
  targetProviderId: SnowflakeId
  /** 目标真实模型 ID */
  targetModelId: string
  /** 优先级，数值越小优先级越高 */
  priority: number
  /** 是否启用该路由 */
  enabled: boolean
  /** 该路由最大重试次数 */
  maxRetries: number
  /** 该路由重试间隔毫秒数 */
  retryIntervalMs: number
  /** 该路由超时时间（毫秒） */
  timeoutMs?: number
  /** 是否健康（健康检查结果） */
  isHealthy: boolean
  /** 上次健康检查通过时间 */
  lastHealthyAt?: Timestamp
  /** 该路由专属的额外请求头（覆盖目标供应商级别配置） */
  extraHeaders?: Record<string, string>
  /** 该路由专属的额外请求体参数 */
  extraBody?: Record<string, unknown>
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 路由尝试记录
 * 用于 `VirtualProviderResolveResult.attempts`，记录每次故障转移的尝试详情
 */
export interface RouteAttempt {
  /** 尝试的路由 ID */
  routeId: SnowflakeId
  /** 目标供应商 ID */
  targetProviderId: SnowflakeId
  /** 目标模型 ID */
  targetModelId: string
  /** 是否成功 */
  success: boolean
  /** HTTP 状态码 */
  statusCode?: number
  /** 失败原因 */
  errorMessage?: string
  /** 该路由耗时（毫秒） */
  durationMs: number
  /** 尝试时间戳（毫秒） */
  attemptedAt: number
}

/**
 * 虚拟供应商解析结果
 * 由 `virtual-provider.service.resolve()` 返回
 *
 * 包含最终选中的供应商与模型信息、实际请求 URL、合并后的请求头，
 * 以及本次路由尝试历史 `attempts`。
 */
export interface VirtualProviderResolveResult {
  /** 是否解析成功 */
  resolved: boolean
  /** 最终选中的供应商 ID */
  providerId?: SnowflakeId
  /** 最终选中的模型 ID */
  modelId?: string
  /** 实际请求 URL */
  actualRequestUrl?: string
  /** 合并后的请求头 */
  headers?: Record<string, string>
  /** 路由尝试历史（按时间顺序） */
  attempts: RouteAttempt[]
  /** 错误信息（全部路由失败时） */
  error?: string
}

/**
 * 虚拟模型映射图节点
 * 前端展示组件 `VirtualModelGraph` 使用
 *
 * 中心节点为「虚拟模型 ID」，通过连线指向多个「真实供应商模型」子节点。
 */
export interface VirtualModelGraphNode {
  /** 虚拟模型 ID */
  virtualModelId: SnowflakeId
  /** 虚拟模型显示名 */
  virtualModelLabel: string
  /** 子节点列表（真实供应商模型） */
  children: Array<{
    /** 路由 ID */
    routeId: SnowflakeId
    /** 目标供应商 ID */
    providerId: SnowflakeId
    /** 目标供应商显示名 */
    providerLabel: string
    /** 目标模型 ID */
    modelId: string
    /** 优先级 */
    priority: number
    /** 是否启用 */
    enabled: boolean
    /** 是否健康 */
    healthy: boolean
    /** 额度进度（0-100，可选） */
    quotaPercent?: number
    /** 上次健康检查时间 */
    lastHealthyAt?: Timestamp
  }>
}

/**
 * 创建虚拟供应商的输入参数
 * 与后端 `CreateVirtualProviderInput` 对齐。
 */
export interface CreateVirtualProviderInput {
  name: string
  alias: string
  displayName?: string
  isEnabled?: boolean
  strategy?: FailoverStrategy
  /** 错误重试次数，默认 3 */
  maxRetries?: number
  /** 重试间隔毫秒数，默认 1000 */
  retryIntervalMs?: number
}

/**
 * 更新虚拟供应商的输入参数
 * 与后端 `UpdateVirtualProviderInput` 对齐。
 */
export interface UpdateVirtualProviderInput {
  name?: string
  alias?: string
  displayName?: string
  isEnabled?: boolean
  strategy?: FailoverStrategy
  maxRetries?: number
  retryIntervalMs?: number
}

/**
 * 创建虚拟模型的输入参数
 */
export interface CreateVirtualModelInput {
  virtualProviderId: SnowflakeId
  modelId: string
  displayName?: string
  isEnabled?: boolean
}

/**
 * 更新虚拟模型的输入参数
 */
export interface UpdateVirtualModelInput {
  modelId?: string
  displayName?: string
  isEnabled?: boolean
}

/**
 * 虚拟模型路由的输入参数（创建/更新）
 */
export interface VirtualModelRouteInput {
  id?: SnowflakeId
  targetProviderId: SnowflakeId
  targetModelId: string
  priority: number
  enabled: boolean
  maxRetries?: number
  retryIntervalMs?: number
  timeoutMs?: number
  extraHeaders?: Record<string, string>
  extraBody?: Record<string, unknown>
}

/**
 * 保存虚拟模型时携带的单条子级路由输入
 * 与后端 `SaveVirtualModelRouteInput` 对齐。
 */
export interface SaveVirtualModelRouteInput {
  targetProviderId: string
  targetModelId: string
  priority: number
  enabled: boolean
  isHealthy: boolean
  maxRetries: number
  retryIntervalMs: number
}

/**
 * 保存虚拟模型完整输入（包含子级路由）
 * 与后端 `SaveVirtualModelInput` 对齐。
 */
export interface SaveVirtualModelInput {
  id?: string
  virtualProviderId: string
  modelId: string
  displayName?: string
  isEnabled: boolean
  routes: SaveVirtualModelRouteInput[]
}

/**
 * 健康检查结果
 * 由后台健康检查调度器周期性更新
 */
export interface HealthCheckStatus {
  routeId: SnowflakeId
  isHealthy: boolean
  lastHealthyAt?: Timestamp
  /** 上次检查的错误信息 */
  lastError?: string
  /** 上次检查耗时（毫秒） */
  lastCheckDurationMs?: number
  /** 连续失败次数 */
  consecutiveFailures: number
}
