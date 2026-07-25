/**
 * 调用记录模块类型定义
 *
 * 与后端 `src-tauri/src/modules/call_records/types.rs` 对齐。
 * V006 增强：增加 routeMode 分组、错误分布、聚合表类型。
 */

/** 请求来源入口 */
export type CallSource = 'cli' | 'gateway' | 'internal'

/** 模型调用记录（与后端 ModelCallLog 对齐） */
export interface ModelCallLog {
  id: string
  providerId: string
  gatewayModelId?: string
  modelId: string
  requestId?: string
  requestedAt: string
  completedAt?: string
  durationMs?: number
  statusCode?: number
  errorMessage?: string
  promptTokens?: number
  completionTokens?: number
  totalTokens?: number
  cachedTokens?: number
  cacheHit: boolean
  routeMode: number
  source: CallSource
  timeToFirstTokenMs?: number
  pricePer1mTokens?: number
  /** 请求使用的 Gateway API Key 明文，V013 新增 */
  apiKeySecretId?: string
}

/** 列出调用记录的查询输入 */
export interface ListModelCallLogsInput {
  providerId?: string
  modelId?: string
  /** 请求发起时间起始过滤（ISO 8601，含） */
  startAt?: string
  /** 请求发起时间结束过滤（ISO 8601，含） */
  endAt?: string
  limit?: number
  offset?: number
  /** 按请求使用的 Gateway API Key 过滤，V013 新增 */
  apiKeySecretId?: string
}

/** 模型调用统计查询输入 */
export interface ModelCallStatsInput {
  /** 统计起始时间（ISO 8601），默认 24 小时前 */
  startAt?: string
  /** 统计结束时间，默认当前 */
  endAt?: string
  /** 按入口过滤：cli / gateway / internal */
  source?: CallSource
  /** 按供应商过滤 */
  providerId?: string
  /** 按模型过滤 */
  modelId?: string
  /** 按路由模式过滤：1=直连，2=虚拟故障转移 */
  routeMode?: number
  /** 按请求使用的 Gateway API Key 过滤，V013 新增 */
  apiKeySecretId?: string
}

/** 模型调用统计输出行 */
export interface ModelCallStatsRow {
  providerId: string
  providerName: string
  modelId: string
  source: CallSource
  /** 路由模式：1=直连，2=虚拟故障转移 */
  routeMode: number
  /** 请求使用的 Gateway API Key 明文，空字符串表示无/未识别，V013 新增 */
  apiKeySecretId: string
  requestCount: number
  successCount: number
  successRate: number
  totalTokens: number
  cachedTokens: number
  cacheHitRate: number
  /** 总花费金额（CNY，元） */
  costCny: number
  costRatio: number
  /** 每百万 Token 成本（CNY / 1M tokens） */
  costPer1mTokens: number
  avgDurationMs: number
  avgTimeToFirstTokenMs: number
  avgTokensPerSecond: number
  /** 4xx 错误数 */
  errorCount4xx: number
  /** 5xx 错误数 */
  errorCount5xx: number
}

// ===== 聚合表类型 =====

/** 聚合时间粒度 */
export type StatsGranularity = 'thirtySeconds' | 'oneMinute' | 'tenMinutes' | 'thirtyMinutes' | 'hourly' | 'daily'

/** 聚合统计查询输入 */
export interface AggregatedStatsInput {
  /** 时间粒度 */
  granularity: StatsGranularity
  /** 开始时间 */
  startAt?: string
  /** 结束时间 */
  endAt?: string
  /** 请求来源过滤 */
  source?: CallSource
  /** 供应商过滤 */
  providerId?: string
  /** 模型过滤 */
  modelId?: string
  /** 路由模式过滤 */
  routeMode?: number
  /** 按请求使用的 Gateway API Key 过滤，V013 新增 */
  apiKeySecretId?: string
}

/** 聚合统计输出行 */
export interface AggregatedStatsRow {
  providerId: string
  /** 供应商显示名称（由 provider_id 反查 providers 表得到） */
  providerName: string
  modelId: string
  source: CallSource
  routeMode: number
  /** 请求使用的 Gateway API Key 明文，空字符串表示无/未识别，V013 新增 */
  apiKeySecretId: string
  /** 时间桶（整点/整天对齐的 ISO 8601） */
  timeBucket: string
  requestCount: number
  successCount: number
  successRate: number
  errorCount4xx: number
  errorCount5xx: number
  errorRate4xx: number
  errorRate5xx: number
  totalTokens: number
  cachedTokens: number
  cacheHitRate: number
  /** 总花费金额（CNY，元） */
  costCny: number
  avgDurationMs: number
  avgTimeToFirstTokenMs: number
  avgTokensPerSecond: number
}

/** 清空统计数据输入参数 */
export interface ClearStatsInput {
  /** 开始时间（RFC3339），为空则清空全部 */
  startAt?: string
  /** 结束时间（RFC3339），为空则清空全部 */
  endAt?: string
}
