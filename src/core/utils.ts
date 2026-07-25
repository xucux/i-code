/**
 * 纯函数工具集
 *
 * 所有函数均为纯函数，无副作用，可被任意模块引用。
 * 业务模块禁止在此文件中引入业务类型，仅可使用 core 内部类型。
 */

import { v4 as uuidv4 } from 'uuid'
import { SECRET_PREFIX, SECRET_SUFFIX } from './constants'

/**
 * 生成 UUID v4 字符串
 * 仅用于非持久化的临时标识（如本地占位、请求 ID），业务表主键请走后端雪花 ID。
 */
export function uuid(): string {
  return uuidv4()
}

/**
 * 深拷贝对象
 * 仅适用于可被 JSON 序列化的数据（无函数、Symbol、循环引用）。
 * 业务配置对象（Provider、ModelConfig 等）均满足此约束。
 */
export function deepClone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value))
}

/**
 * 类型守卫：联合类型 exhaustiveness 检查
 * 在 switch/if 处理完所有联合类型分支后，调用此函数确保没有遗漏。
 *
 * @example
 * ```ts
 * switch (auth.method) {
 *   case 'api-key': return ...
 *   case 'oauth2': return ...
 *   case 'none': return ...
 *   default: return assertNever(auth)  // 编译期保证所有分支已处理
 * }
 * ```
 */
export function assertNever(value: never): never {
  throw new Error(`Unexpected value: ${String(value)}`)
}

/**
 * 将 Date / 字符串 / 数字格式化为 ISO 8601 UTC 字符串
 * 后端 Rust 通过 chrono::Utc::now() 生成，前端按字符串处理。
 */
export function formatDate(date: Date | string | number): string {
  const d = typeof date === 'string' || typeof date === 'number' ? new Date(date) : date
  return d.toISOString()
}

/**
 * 将 ISO 8601 时间字符串格式化为本地时间
 * 格式固定为 `yyyy-MM-dd HH:mm:ss`，便于列表中展示。
 * 输入非法时返回原字符串。
 */
export function formatDateTime(iso: string | undefined | null): string {
  if (!iso) return ''
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso

  const pad = (n: number) => n.toString().padStart(2, '0')
  const year = d.getFullYear()
  const month = pad(d.getMonth() + 1)
  const day = pad(d.getDate())
  const hour = pad(d.getHours())
  const minute = pad(d.getMinutes())
  const second = pad(d.getSeconds())
  return `${year}-${month}-${day} ${hour}:${minute}:${second}`
}

/**
 * 判断字符串是否为 Secret 引用（`$SECRET:{snowflake_id}$`）
 * 用于区分配置中的明文值与加密引用。
 */
export function isSecretRef(value: string): boolean {
  return (
    typeof value === 'string' &&
    value.startsWith(SECRET_PREFIX) &&
    value.endsWith(SECRET_SUFFIX)
  )
}

/**
 * 从 Secret 引用字符串中提取雪花 ID
 * 若不是合法的引用格式，返回 null。
 */
export function extractSecretId(ref: string): string | null {
  if (!isSecretRef(ref)) return null
  // 形如 `$SECRET:{snowflake_id}$`，去掉前缀 `$SECRET:` 与后缀 `$`
  return ref.slice(SECRET_PREFIX.length, -SECRET_SUFFIX.length)
}

/**
 * 将雪花 ID 包装为 Secret 引用字符串
 */
export function buildSecretRef(id: string): string {
  return `${SECRET_PREFIX}${id}${SECRET_SUFFIX}`
}

/**
 * Promise 延时工具，用于重试退避等场景
 */
export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

/**
 * 将进程物理内存（KB）格式化为友好单位
 * - 小于 1 MB 时展示 KB
 * - 1 MB ~ 1 GB 之间展示 MB
 * - 大于等于 1 GB 时展示 GB
 *
 * 后端 Rust 通过 sysinfo 返回 KB，前端使用此函数统一格式化。
 */
export function formatMemory(kb: number | null): string {
  if (kb === null || kb < 0) return '—'
  if (kb < 1024) return `${kb} KB`
  if (kb < 1024 * 1024) return `${(kb / 1024).toFixed(1)} MB`
  return `${(kb / 1024 / 1024).toFixed(1)} GB`
}

/**
 * 将大数字格式化为紧凑计数（K / W / B）
 * - K = 千（1,000）
 * - W = 万（10,000）
 * - B = 亿（100,000,000）
 *
 * 常用于 token 消耗、额度等可能快速增长的数值展示。
 * 支持 number、bigint 与字符串，避免大整数超过 JS Number 安全范围时丢失精度。
 */
export function formatCompactCount(value: number | bigint | string | null): string {
  if (value === null) return '—'

  // 统一转换为 bigint 进行安全比较与运算
  let n: bigint
  try {
    n = typeof value === 'bigint' ? value : BigInt(value)
  } catch {
    return '—'
  }

  if (n < 0n) return '—'
  if (n < 1_000n) return String(n)
  if (n < 10_000n) return `${(Number(n) / 1_000).toFixed(1)}K`
  if (n < 100_000_000n) return `${(Number(n) / 10_000).toFixed(1)}W`
  return `${(Number(n) / 100_000_000).toFixed(2)}B`
}

/**
 * 金额格式化
 * @param amountMin 最小货币单位（如美分），避免浮点误差
 * @param currencySymbol 货币符号，例如 `$`、`¥`
 * @returns 形如 `$12.34` 的字符串
 */
export function formatAmount(amountMin: number | bigint | string | null, currencySymbol = '$'): string {
  if (amountMin === null) return '—'
  try {
    const n = typeof amountMin === 'bigint' ? amountMin : BigInt(amountMin)
    const whole = n / 100n
    const cents = n % 100n
    return `${currencySymbol}${whole}.${cents.toString().padStart(2, '0')}`
  } catch {
    return '—'
  }
}

/**
 * 拼接 Gateway 监听地址
 * @param host 主机，例如 `127.0.0.1`
 * @param port 端口，例如 `54321`
 * @returns 形如 `127.0.0.1:54321` 的字符串
 */
export function formatGatewayAddress(host: string, port: number): string {
  // IPv6 地址需要用方括号包裹
  if (host.includes(':')) {
    return `[${host}]:${port}`
  }
  return `${host}:${port}`
}

/**
 * 解析外部模型 ID（`{provider_slug}/{model_id}`）
 * @returns `[providerSlug, modelId]` 元组；若格式非法返回 null
 */
export function parseModelId(modelId: string): [string, string] | null {
  const idx = modelId.indexOf('/')
  if (idx <= 0 || idx >= modelId.length - 1) return null
  return [modelId.slice(0, idx), modelId.slice(idx + 1)]
}

/**
 * 拼接对外暴露的模型 ID（`{provider_slug}/{model_id}`）
 */
export function buildModelId(providerSlug: string, modelId: string): string {
  return `${providerSlug}/${modelId}`
}

/**
 * 截断字符串到指定长度，超出部分以 `…` 替代
 * 用于 UI 中展示长文本时的兜底处理。
 */
export function truncate(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text
  return text.slice(0, Math.max(0, maxLength - 1)) + '…'
}

/**
 * 判断当前是否运行在 Tauri 桌面环境中
 *
 * Tauri 运行时会在 window 上注入 `__TAURI_INTERNALS__` 对象，
 * web 预览模式下该对象不存在，直接调用 Tauri API 会导致
 * "Cannot read properties of undefined (reading 'metadata')" 错误。
 *
 * 所有使用 Tauri API 的组件/钩子都应先通过此函数判断环境。
 */
export function isTauri(): boolean {
  return '__TAURI_INTERNALS__' in window
}

/**
 * 校验 slug 是否合法
 * 规则：小写字母、数字、连字符；长度 1-64；不能以连字符开头/结尾。
 */
export function isValidSlug(slug: string): boolean {
  return /^[a-z0-9]([a-z0-9-]{0,62}[a-z0-9])?$/.test(slug)
}
