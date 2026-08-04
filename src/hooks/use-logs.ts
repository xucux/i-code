import { useCallback, useEffect, useState } from 'react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { invokeCommand } from '@/hooks/use-command'
import { useAutoRefresh } from '@/components/ui/auto-refresh'
import { BACKEND_EVENTS } from '@/core/events'
import type { LogEntry as LoggerLogEntry, LogFilter, LogSource } from '@/modules/logger/types'
import type { LogEntry as UiLogEntry, LogLevel as UiLogLevel } from '@/components/ui/log-viewer'

/**
 * 日志内存缓冲区默认大小（条）
 *
 * 与后端 LogRollingConfig 默认值保持一致，确保前端展示不会超出缓冲容量。
 */
export const DEFAULT_LOG_BUFFER_SIZE = 5000

/**
 * 自动刷新间隔（毫秒）
 */
const DEFAULT_REFRESH_INTERVAL = 3000

interface UseLogsOptions {
  /** 查询过滤条件 */
  filter: LogFilter
  /** 内存缓冲队列最大条数 */
  bufferSize?: number
  /** 是否启用自动刷新 */
  autoRefresh?: boolean
  /** 自动刷新间隔（毫秒） */
  refreshInterval?: number
}

interface UseLogsResult {
  /** 已适配为 UI 组件格式的日志列表 */
  logs: UiLogEntry[]
  /** 是否正在加载 */
  loading: boolean
  /** 错误信息 */
  error: string | null
  /** 手动刷新 */
  refresh: () => Promise<void>
  /** 清空后端日志缓冲 */
  clear: () => Promise<void>
}

/**
 * 日志查询与实时监听 Hook
 *
 * 功能：
 * - 调用后端 `log_list` 命令按 filter 查询日志
 * - 监听 `log:new-entry` 事件，实时追加符合当前过滤条件的新日志
 * - 支持自动刷新与手动刷新
 * - 超过 bufferSize 时丢弃旧日志，避免前端内存无限增长
 */
export function useLogs({
  filter,
  bufferSize = DEFAULT_LOG_BUFFER_SIZE,
  autoRefresh = true,
  refreshInterval = DEFAULT_REFRESH_INTERVAL,
}: UseLogsOptions): UseLogsResult {
  const [logs, setLogs] = useState<UiLogEntry[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  /** 调用后端查询日志 */
  const fetchLogs = useCallback(async () => {
    try {
      setLoading(true)
      const result = await invokeCommand<LoggerLogEntry[]>('log_list', { filter })
      // 后端已按时间倒序返回（最新在前），保留最新的 bufferSize 条
      setLogs(result.map(adaptLogEntry).slice(0, bufferSize))
      setError(null)
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }, [filter, bufferSize])

  /** 清空后端日志 */
  const clearLogs = useCallback(async () => {
    try {
      setLoading(true)
      await invokeCommand<void>('log_clear')
      setLogs([])
      setError(null)
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    let unlistenNewEntry: UnlistenFn | undefined
    let unlistenCleared: UnlistenFn | undefined

    // 初始加载
    fetchLogs()

    // 监听后端推送的新日志
    listen<LoggerLogEntry>(BACKEND_EVENTS.LOG_NEW_ENTRY, (event) => {
      const entry = event.payload
      if (matchesFilter(entry, filter)) {
        setLogs((prev) => {
          // 新日志为最新条目，插入数组头部以保持时间倒序
          const next = [adaptLogEntry(entry), ...prev]
          return next.slice(0, bufferSize)
        })
      }
    }).then((fn) => {
      unlistenNewEntry = fn
    })

    // 监听日志清空事件
    listen(BACKEND_EVENTS.LOG_CLEARED, () => {
      setLogs([])
    }).then((fn) => {
      unlistenCleared = fn
    })

    return () => {
      if (unlistenNewEntry) unlistenNewEntry()
      if (unlistenCleared) unlistenCleared()
    }
  }, [fetchLogs, filter, bufferSize])

  // 自动刷新轮询
  useAutoRefresh({
    onRefresh: fetchLogs,
    intervalMs: autoRefresh ? refreshInterval : null,
  })

  return { logs, loading, error, refresh: fetchLogs, clear: clearLogs }
}

/**
 * 将后端 LoggerLogEntry 转换为 UI LogViewer 所需格式
 *
 * message 由 method、url、statusCode、durationMs、errorMessage 拼接而成，
 * 详细字段放入 context 供后续展开查看。
 */
function adaptLogEntry(entry: LoggerLogEntry): UiLogEntry {
  const parts: string[] = []
  if (entry.method) parts.push(entry.method)
  if (entry.url) parts.push(entry.url)
  if (entry.statusCode !== undefined) parts.push(String(entry.statusCode))
  if (entry.durationMs !== undefined) parts.push(`${entry.durationMs}ms`)
  if (entry.errorMessage) parts.push(entry.errorMessage)

  const message = parts.length > 0 ? parts.join(' ') : entry.requestId ?? entry.id

  return {
    id: entry.id,
    level: entry.level.toLowerCase() as UiLogLevel,
    timestamp: entry.timestamp,
    source: entry.source,
    message,
    context: {
      method: entry.method ?? null,
      url: entry.url ?? null,
      statusCode: entry.statusCode ?? null,
      durationMs: entry.durationMs ?? null,
      promptTokens: entry.promptTokens ?? null,
      completionTokens: entry.completionTokens ?? null,
      totalTokens: entry.totalTokens ?? null,
      cachedTokens: entry.cachedTokens ?? null,
      errorMessage: entry.errorMessage ?? null,
      requestId: entry.requestId ?? null,
      modelId: entry.modelId ?? null,
      requestHeaders: entry.requestHeaders ?? null,
      requestBody: entry.requestBody ?? null,
      responseBody: entry.responseBody ?? null,
      tags: entry.tags && entry.tags.length > 0 ? entry.tags.join(', ') : null,
      fileName: entry.fileName ?? null,
      lineNumber: entry.lineNumber ?? null,
    },
  }
}

/**
 * 判断新推送的日志是否匹配当前过滤条件
 *
 * 与后端 LoggerService::matches 逻辑保持一致：
 * 级别、来源、关键词（URL / errorMessage）、时间范围、请求 ID。
 */
function matchesFilter(entry: LoggerLogEntry, filter: LogFilter): boolean {
  if (filter.levels?.length && !filter.levels.includes(entry.level)) {
    return false
  }
  if (filter.sources?.length && !filter.sources.includes(entry.source)) {
    return false
  }
  if (filter.keyword) {
    const kw = filter.keyword.toLowerCase()
    const urlMatch = entry.url?.toLowerCase().includes(kw) ?? false
    const errMatch = entry.errorMessage?.toLowerCase().includes(kw) ?? false
    if (!urlMatch && !errMatch) {
      return false
    }
  }
  if (filter.timeRange?.from && entry.timestamp < filter.timeRange.from) {
    return false
  }
  if (filter.timeRange?.to && entry.timestamp > filter.timeRange.to) {
    return false
  }
  if (filter.requestId && entry.requestId !== filter.requestId) {
    return false
  }
  return true
}

/**
 * 构造按来源筛选的 LogFilter
 */
export function buildSourceFilter(sources: LogSource[]): LogFilter {
  return { sources }
}
