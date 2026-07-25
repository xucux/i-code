/**
 * 自研内存 Logger 全局工具接口
 *
 * 与 `console.log` / `tauri-plugin-log` 完全隔离，
 * 通过 `log_message` Command 写入后端内存环形缓冲区，实时同步到「日志」页面。
 *
 * 适用于需要出现在应用内日志面板、供运维诊断的业务日志。
 * 开发调试信息仍建议使用 `console.log` 或后端 `log::` 宏。
 */
import { invokeCommand } from '@/hooks/use-command'
import type { LogLevel } from './types'

interface LogMessagePayload {
  level: LogLevel
  message: string
  fileName?: string
  lineNumber?: number
}

function buildPayload(
  level: LogLevel,
  message: string,
  fileName?: string,
  lineNumber?: number,
): LogMessagePayload {
  return { level, message, fileName, lineNumber }
}

async function send(payload: LogMessagePayload): Promise<void> {
  try {
    await invokeCommand<void>('log_message', payload)
  } catch {
    // 日志写入失败不应打断业务逻辑；开发环境可在 DevTools 中查看原始报错
  }
}

/**
 * 自研内存日志工具
 *
 * 所有方法均为异步非阻塞调用，失败静默忽略。
 */
export const logger = {
  /** DEBUG 级别日志 */
  debug: (message: string, fileName?: string, lineNumber?: number) =>
    send(buildPayload('DEBUG', message, fileName, lineNumber)),
  /** INFO 级别日志 */
  info: (message: string, fileName?: string, lineNumber?: number) =>
    send(buildPayload('INFO', message, fileName, lineNumber)),
  /** WARN 级别日志 */
  warn: (message: string, fileName?: string, lineNumber?: number) =>
    send(buildPayload('WARN', message, fileName, lineNumber)),
  /** ERROR 级别日志 */
  error: (message: string, fileName?: string, lineNumber?: number) =>
    send(buildPayload('ERROR', message, fileName, lineNumber)),
} as const
