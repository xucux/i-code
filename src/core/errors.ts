/**
 * 业务错误基类与具体错误类型
 *
 * 与后端 `src-tauri/src/error.rs` 中的 `IcodeError` 对齐。
 * 后端错误通过 Tauri invoke 抛出，前端通过 try/catch 接收后包装为本类型。
 *
 * 错误码规范：
 * - UNKNOWN：未分类的未知错误
 * - VALIDATION：表单/参数校验失败
 * - NOT_FOUND：资源不存在
 * - AUTH：认证失败或会话过期
 * - FORBIDDEN：无权限访问资源
 * - CONFLICT：唯一约束冲突（如 slug 重复）
 * - GATEWAY：网关请求转发异常
 * - DATABASE：数据库操作失败
 * - INTERNAL：后端内部错误（不应暴露给用户）
 */

export type ErrorCode =
  | 'UNKNOWN'
  | 'VALIDATION'
  | 'NOT_FOUND'
  | 'AUTH'
  | 'FORBIDDEN'
  | 'CONFLICT'
  | 'GATEWAY'
  | 'DATABASE'
  | 'INTERNAL'

/**
 * 业务错误基类
 * 所有抛出到 UI 层的错误都应继承此类或直接使用其子类。
 */
export class IcodeError extends Error {
  readonly code: ErrorCode
  readonly details?: Record<string, unknown>

  constructor(
    message: string,
    code: ErrorCode = 'UNKNOWN',
    details?: Record<string, unknown>
  ) {
    super(message)
    this.name = 'IcodeError'
    this.code = code
    this.details = details
  }
}

/**
 * 参数/表单校验失败错误
 * 用于前端表单校验、Repository 序列化失败等场景。
 */
export class ValidationError extends IcodeError {
  constructor(message: string, details?: Record<string, unknown>) {
    super(message, 'VALIDATION', details)
    this.name = 'ValidationError'
  }
}

/**
 * 资源不存在错误
 * 通常对应后端 404，例如查询的供应商 ID 不存在。
 */
export class NotFoundError extends IcodeError {
  constructor(resource: string, id?: string) {
    super(`${resource}${id ? `(${id})` : ''} 不存在`, 'NOT_FOUND', { resource, id })
    this.name = 'NotFoundError'
  }
}

/**
 * 认证失败或会话过期错误
 * 用于 Gateway API Key 校验失败、供应商 OAuth Token 过期等场景。
 */
export class AuthError extends IcodeError {
  constructor(message = '认证失败或已过期') {
    super(message, 'AUTH')
    this.name = 'AuthError'
  }
}

/**
 * 权限不足错误
 * 例如用户尝试访问不属于自己的资源。
 */
export class ForbiddenError extends IcodeError {
  constructor(message = '权限不足') {
    super(message, 'FORBIDDEN')
    this.name = 'ForbiddenError'
  }
}

/**
 * 唯一约束冲突错误
 * 例如创建供应商时 slug 已存在。
 */
export class ConflictError extends IcodeError {
  constructor(message: string, details?: Record<string, unknown>) {
    super(message, 'CONFLICT', details)
    this.name = 'ConflictError'
  }
}

/**
 * Gateway 请求转发异常
 * 用于上游供应商返回错误、流式响应中断、超时等场景。
 */
export class GatewayError extends IcodeError {
  constructor(message: string, details?: Record<string, unknown>) {
    super(message, 'GATEWAY', details)
    this.name = 'GatewayError'
  }
}

/**
 * 数据库操作失败错误
 * 通常由 Repository 层抛出，例如 SQLite 锁定、迁移失败。
 */
export class DatabaseError extends IcodeError {
  constructor(message: string, details?: Record<string, unknown>) {
    super(message, 'DATABASE', details)
    this.name = 'DatabaseError'
  }
}

/**
 * 将任意异常转换为 IcodeError
 * 主要用于 try/catch 中兜底，确保 UI 层接收到的都是统一错误类型。
 *
 * 后端通过 Tauri Command 返回的错误已序列化为 `{ code, message, details }` 结构体，
 * 此处优先按标准结构解析，避免前端展示 `[object Object]`。
 */
export function toIcodeError(e: unknown): IcodeError {
  if (e instanceof IcodeError) return e

  if (e && typeof e === 'object') {
    const obj = e as { code?: unknown; message?: unknown; details?: unknown }
    if (typeof obj.message === 'string') {
      const code = typeof obj.code === 'string' ? (obj.code as ErrorCode) : 'INTERNAL'
      const details =
        obj.details && typeof obj.details === 'object'
          ? (obj.details as Record<string, unknown>)
          : undefined
      return new IcodeError(obj.message, code, details)
    }
    if ('message' in e && typeof (e as Error).message === 'string') {
      return new IcodeError((e as Error).message, 'INTERNAL')
    }
  }

  if (e instanceof Error) return new IcodeError(e.message, 'INTERNAL')
  return new IcodeError(String(e), 'INTERNAL')
}
