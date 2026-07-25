/**
 * 本地网关运行时模块类型定义
 *
 * 对应 `docs/development.md` §5.8 / §5.13。前端仅维护运行时状态视图，
 * 实际状态存储在 Tauri State 中，不写库。
 *
 * `gateway-runtime` 启动本地 HTTP 服务，接收来自内部 CLI 或外部客户端的
 * OpenAI 兼容请求，并路由到真实供应商。所有请求经过拦截器链采集日志与调用记录。
 */

import type { Timestamp } from '@/core/types'

/**
 * 网关运行时状态
 * 仅在内存中维护，不持久化到数据库
 */
export interface GatewayRuntimeState {
  /** 是否正在运行 */
  isRunning: boolean
  /** 绑定的监听地址，如 `127.0.0.1` */
  boundHost?: string
  /** 绑定的监听端口，如 `54321` */
  boundPort?: number
  /** 启动时间戳（ISO 8601） */
  startedAt?: Timestamp
  /** 最近一次错误信息（如启动失败、端口占用等） */
  lastError?: string
  /** 已处理的请求总数（统计计数） */
  totalRequests?: number
  /** 当前活跃请求数（并发计数） */
  activeRequests?: number
}

/**
 * 网关请求封装
 * 标准化的请求对象，用于拦截器链传递
 */
export interface GatewayRequest {
  /** 网关生成的唯一请求追踪 ID */
  requestId: string
  /** HTTP 方法 */
  method: string
  /** 完整请求 URL */
  url: string
  /** 请求头（已脱敏，不包含 Authorization 等敏感字段） */
  headers: Record<string, string>
  /** 请求体（可选，仅记录元信息，不记录完整内容以保护隐私） */
  body?: unknown
  /** 请求开始时间戳（毫秒） */
  startedAt?: number
}

/**
 * 网关响应封装
 * 标准化的响应对象，用于拦截器链传递
 */
export interface GatewayResponse {
  requestId: string
  /** HTTP 状态码 */
  statusCode: number
  /** 响应头（已脱敏） */
  headers: Record<string, string>
  /** 响应体（可选） */
  body?: unknown
  /** 请求耗时（毫秒） */
  durationMs: number
}

/**
 * 启动网关的输入参数
 * 为空时使用 `app_settings` 中的配置
 */
export interface StartGatewayInput {
  host?: string
  port?: number
}

/**
 * 启动网关的结果
 */
export interface StartGatewayResult {
  success: boolean
  host: string
  port: number
  /** 启动失败时的错误信息 */
  error?: string
}

/**
 * 健康检查状态
 * 对应 `GET /health` 与 `GET /readyz` 接口
 */
export interface HealthCheckResult {
  /** 是否存活（HTTP Server 是否响应） */
  alive: boolean
  /** 是否就绪（数据库与上游供应商是否可达） */
  ready: boolean
  /** 数据库连接是否正常 */
  databaseOk: boolean
  /** 上游供应商可达性检查结果 */
  upstreamChecks?: Array<{
    providerId: string
    reachable: boolean
    latencyMs?: number
    error?: string
  }>
  /** 检查时间戳（毫秒） */
  checkedAt: number
}

/**
 * 网关请求来源类型
 * 用于认证豁免判断
 * - `internal-cli`：内部 CLI（可豁免 API Key 校验）
 * - `external`：外部客户端（必须校验 Gateway Key）
 */
export type GatewayRequestSource = 'internal-cli' | 'external'

/**
 * 拦截器尝试记录
 * 用于虚拟供应商故障转移时的尝试历史
 */
export interface GatewayAttemptRecord {
  /** 目标供应商 ID */
  providerId: string
  /** 目标模型 ID */
  modelId: string
  /** 是否成功 */
  success: boolean
  /** HTTP 状态码 */
  statusCode?: number
  /** 失败原因 */
  errorMessage?: string
  /** 该路由耗时（毫秒） */
  durationMs: number
}
