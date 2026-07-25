/**
 * 额度监控模块类型定义
 *
 * 与 `docs/database.md` §5.10 中的 `BalanceConfig` / `BalanceSnapshot` 对齐。
 * 后端 Rust 通过 ts-rs 自动生成同名类型，前端禁止手写同步覆盖此文件。
 *
 * 独立模块，被 `ai-gateway` 和 `gateway-runtime` 依赖。
 */

/**
 * 额度监控方法枚举
 * 对应 database.md §5.10，多态联合类型的判别字段
 *
 * - `none`：不监控
 * - `moonshot-ai`：Moonshot AI（Kimi 国内站）
 * - `kimi-code`：Kimi Code（Kimi 国际站）
 * - `newapi`：New API（OneAPI 系分支，需 userId 与 systemToken）
 * - `deepseek`：DeepSeek
 * - `openrouter`：OpenRouter
 * - `siliconflow`：硅基流动
 * - `aihubmix`：AIHubMix
 * - `claude-relay-service`：Claude Relay Service
 * - `antigravity`：Google Antigravity
 * - `gemini-cli`：Gemini CLI
 * - `codex`：OpenAI Codex
 * - `synthetic`：合成数据（测试用）
 * - `minimax`：MiniMax
 */
export type BalanceMethod =
  | 'none'
  | 'moonshot-ai'
  | 'kimi-code'
  | 'newapi'
  | 'deepseek'
  | 'openrouter'
  | 'siliconflow'
  | 'aihubmix'
  | 'claude-relay-service'
  | 'antigravity'
  | 'gemini-cli'
  | 'codex'
  | 'synthetic'
  | 'minimax'

/**
 * New API 额度转换配置
 * 用于将原始 quota 数值转换为标准额度值
 */
export interface NewApiQuotaTransform {
  /** 主额度字段名，默认 `quota` */
  quotaField?: string
  /** 额外累加额度字段名列表 */
  extraQuotaFields?: string[]
  /** 原始额度转换除数，默认 500000 */
  divisor?: number
  /** 除法后乘数，默认 1 */
  multiplier?: number
}

/**
 * 额度监控配置（多态联合类型）
 * 对应 `providers.balance_provider_json` 与 `cli_providers.balance_json` 中的配置部分
 *
 * 由 `method` 字段区分不同供应商的查询参数：
 * - `newapi`：可选 userId / systemToken / quotaTransform
 * - `claude-relay-service`：可选 baseUrl（自定义 apiStats API 地址）
 * - 其他方法：仅需 method 字段
 */
export type BalanceConfig =
  | { method: 'none' }
  | { method: 'moonshot-ai' }
  | { method: 'kimi-code' }
  | {
      method: 'newapi'
      userId?: string
      /** 系统 token，明文或 `$SECRET:{snowflake_id}$` 引用 */
      systemToken?: string
      quotaTransform?: NewApiQuotaTransform
    }
  | { method: 'deepseek' }
  | { method: 'openrouter' }
  | { method: 'siliconflow' }
  | { method: 'aihubmix' }
  | { method: 'claude-relay-service'; baseUrl?: string }
  | { method: 'antigravity' }
  | { method: 'gemini-cli' }
  | { method: 'codex' }
  | { method: 'synthetic' }
  | { method: 'minimax' }

/**
 * 额度指标时间范围
 * - `current`：当前周期
 * - `month`：自然月
 * - `day`：自然日
 * - `week`：自然周
 * - `total`：累计
 */
export type BalancePeriod = 'current' | 'month' | 'day' | 'week' | 'total'

/**
 * 额度指标公共基座字段
 * 所有 `BalanceMetric` 子类型都包含这些字段
 */
export interface BalanceMetricBase {
  /** 指标唯一标识，如 `balance` / `tokens` / `expires` / `status` */
  id: string
  /** 指标类型，用于联合类型判别 */
  type: 'amount' | 'integer' | 'token' | 'percent' | 'time' | 'status'
  /** 时间范围 */
  period?: BalancePeriod
  /** 自定义周期标签（覆盖 period） */
  periodLabel?: string
  /** 作用域，如 `account` / `model` */
  scope?: string
  /** 是否主指标，UI 中高亮显示 */
  primary?: boolean
  /** UI 展示标签 */
  label?: string
}

/**
 * 金额类指标
 */
export interface AmountBalanceMetric extends BalanceMetricBase {
  type: 'amount'
  /** 方向：剩余 / 已用 / 上限 */
  direction: 'remaining' | 'used' | 'limit'
  /** 金额数值（不使用 number 以避免大数精度丢失） */
  value: number | string
  /** 货币符号，如 `$` / `¥` */
  currencySymbol?: string
}

/**
 * 整数类指标
 */
export interface IntegerBalanceMetric extends BalanceMetricBase {
  type: 'integer'
  direction: 'remaining' | 'used' | 'limit'
  value: number | string
}

/**
 * Token 用量指标
 */
export interface TokenBalanceMetric extends BalanceMetricBase {
  type: 'token'
  used?: number | string
  limit?: number | string
  remaining?: number | string
}

/**
 * 百分比指标
 */
export interface PercentBalanceMetric extends BalanceMetricBase {
  type: 'percent'
  /** 0-100 之间的百分比数值 */
  value: number
  /** 基准：剩余 / 已用 */
  basis?: 'remaining' | 'used'
}

/**
 * 时间类指标
 */
export interface TimeBalanceMetric extends BalanceMetricBase {
  type: 'time'
  /** 时间类型：过期时间 / 重置时间 */
  kind: 'expiresAt' | 'resetAt'
  /** ISO 8601 时间字符串 */
  value: string
  /** 毫秒时间戳（便于排序与过期判断） */
  timestampMs?: number
}

/**
 * 状态类指标
 */
export interface StatusBalanceMetric extends BalanceMetricBase {
  type: 'status'
  /** 状态值 */
  value: 'ok' | 'unlimited' | 'exhausted' | 'error' | 'unavailable'
  /** 状态描述消息 */
  message?: string
}

/**
 * 额度指标联合类型
 */
export type BalanceMetric =
  | AmountBalanceMetric
  | IntegerBalanceMetric
  | TokenBalanceMetric
  | PercentBalanceMetric
  | TimeBalanceMetric
  | StatusBalanceMetric

/**
 * 额度快照
 *
 * 对应 `cli_providers.balance_json` 与运行时额度缓存。
 * 由 `balance/service.queryBalance()` 调用供应商 API 后生成，
 * 写入 `cli_providers.balance_json` 避免频繁请求。
 */
export interface BalanceSnapshot {
  /** 快照更新时间戳（毫秒） */
  updatedAt: number
  /** 额度指标数组 */
  items: BalanceMetric[]
}

/**
 * 额度刷新结果
 * 后端 `balance_refresh` 命令返回
 */
export interface BalanceRefreshResult {
  cliProviderId: string
  snapshot: BalanceSnapshot
  /** 刷新过程中发生的错误（非致命，部分指标可能仍可获取） */
  warnings?: string[]
}

/**
 * 额度警告事件
 * 当额度低于阈值时触发 `BALANCE_REFRESHED` 事件
 */
export interface BalanceWarning {
  cliProviderId: string
  /** 触发警告的指标 ID */
  metricId: string
  /** 警告消息 */
  message: string
  /** 触发时间戳（毫秒） */
  triggeredAt: number
}

/**
 * 供应商额度快照行
 *
 * 后端 `balance_list_snapshots` 命令返回，关联 providers 表与
 * provider_balance_snapshots 表，供前端供应商列表与系统托盘展示。
 */
export interface ProviderBalanceSnapshotRow {
  /** 供应商 ID */
  providerId: string
  /** 供应商展示名 */
  displayName: string
  /** 供应商 slug */
  slug: string
  /** 额度监控方法（如 'deepseek' / 'openrouter'），无配置时为 null */
  balanceMethod: string | null
  /** 额度快照 */
  snapshot: BalanceSnapshot
  /** 快照更新时间（ISO 8601） */
  updatedAt: string
}

/**
 * 额度百分比摘要
 *
 * 从 BalanceSnapshot 中提取的展示用结构，按 period（周/月）分组。
 */
export interface BalancePercentSummary {
  /** 周限额剩余百分比（0-100） */
  weekPercent?: number
  /** 月限额剩余百分比（0-100） */
  monthPercent?: number
  /** 其他百分比指标（无明确周期时） */
  otherPercent?: number
  /** 周限额重置时间（ISO 8601），无则 undefined */
  weekResetAt?: string
  /** 月限额重置时间（ISO 8601），无则 undefined */
  monthResetAt?: string
}

/**
 * 从额度快照中提取百分比摘要
 *
 * 解析 percent 类型指标按 period 分组，并匹配对应的 time 类型重置时间。
 */
export function extractPercentSummary(snapshot: BalanceSnapshot | undefined | null): BalancePercentSummary | null {
  if (!snapshot || !snapshot.items || snapshot.items.length === 0) return null

  const result: BalancePercentSummary = {}
  let hasAny = false

  for (const item of snapshot.items) {
    if (item.type === 'percent') {
      // PercentBalanceMetric.value 始终为 number 类型
      const val = item.value
      if (Number.isFinite(val)) {
        hasAny = true
        if (item.period === 'week') result.weekPercent = val
        else if (item.period === 'month') result.monthPercent = val
        else if (result.otherPercent === undefined) result.otherPercent = val
      }
    } else if (item.type === 'time' && item.kind === 'resetAt') {
      if (item.period === 'week') result.weekResetAt = item.value
      else if (item.period === 'month') result.monthResetAt = item.value
    }
  }

  return hasAny ? result : null
}
