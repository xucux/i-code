/**
 * 全局基础类型定义
 *
 * 所有业务模块均可依赖此文件中的类型，但 core 自身禁止引入任何业务模块类型。
 * 与后端 Rust 通过 ts-rs 生成的类型保持一致（驼峰命名 + camelCase 序列化）。
 */

/**
 * 雪花 ID 字符串，作为所有业务表主键
 */
export type SnowflakeId = string

/**
 * 路由用供应商标识，全局唯一
 * 仅允许小写字母、数字、连字符，例：`openai-main`
 */
export type Slug = string

/**
 * ISO 8601 UTC 时间字符串，例：`2026-07-15T08:00:00Z`
 * 后端 Rust 通过 chrono 生成，前端按字符串处理避免时区错乱。
 */
export type Timestamp = string

/**
 * 排序方向
 */
export type SortDirection = 'asc' | 'desc'

/**
 * 通用排序参数
 */
export interface SortParams<T extends string = string> {
  field: T
  direction: SortDirection
}

/**
 * 分页查询参数
 */
export interface PagingParams {
  page: number
  pageSize: number
}

/**
 * 分页查询结果
 */
export interface PagingResult<T> {
  items: T[]
  total: number
  page: number
  pageSize: number
}

/**
 * 通用 Result 类型，用于显式错误处理流程
 * 与 Rust 端 `Result<T, IcodeError>` 对齐
 */
export type Result<T, E = Error> =
  | { ok: true; value: T }
  | { ok: false; error: E }

/** 构造成功结果 */
export function ok<T>(value: T): Result<T, never> {
  return { ok: true, value }
}

/** 构造失败结果 */
export function err<E>(error: E): Result<never, E> {
  return { ok: false, error }
}

/**
 * 应用主题枚举
 * 六种主题对应不同品牌风格（Claude/DeepSeek/默认）
 */
export type Theme = 'light' | 'dark' | 'claude-light' | 'claude-dark' | 'deepseek-light' | 'deepseek-dark'

/**
 * 应用语言枚举
 */
export type Locale = 'zh-CN' | 'en'

/** 兼容性别名 */
export type AppTheme = Theme
export type AppLocale = Locale

/**
 * 内部 CLI 豁免认证的 Header 名
 * 当请求头中包含此键且值匹配应用生成的内部 token 时，Gateway 跳过 API Key 校验。
 */
export const INTERNAL_CLI_HEADER = 'X-iCode-Internal'
