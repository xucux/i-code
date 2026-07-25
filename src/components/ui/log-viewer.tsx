"use client"

import { useState } from "react"
import { cn } from "@/lib/utils"
import { Badge } from "@/components/ui/badge"
import { ScrollArea } from "@/components/ui/scroll-area"
import { useTranslation } from "@/modules/i18n/use-translation"

/**
 * 日志级别
 */
export type LogLevel = "debug" | "info" | "warn" | "error"

/**
 * 单条日志条目
 */
export interface LogEntry {
  /** 唯一标识 */
  id: string
  /** 日志级别 */
  level: LogLevel
  /** ISO 8601 时间戳 */
  timestamp: string
  /** 日志来源模块 */
  source: string
  /** 日志消息 */
  message: string
  /** 额外上下文（请求状态码、耗时、请求体、响应体等） */
  context?: Record<string, string | number | null>
}

export interface LogViewerProps {
  /** 日志列表 */
  logs: LogEntry[]
  /** 自定义类名 */
  className?: string
  /** 自定义行内样式 */
  style?: React.CSSProperties
  /** 空状态提示 */
  emptyText?: string
  /** 点击某条日志时的回调（用于展开详情） */
  onRowClick?: (entry: LogEntry) => void
  /** 当前高亮的日志 ID */
  activeId?: string
}

/**
 * 根据日志级别返回对应的颜色样式
 */
function levelVariant(level: LogLevel): string {
  switch (level) {
    case "debug":
      return "bg-muted text-muted-foreground hover:bg-muted/80"
    case "info":
      return "bg-blue-100 text-blue-700 hover:bg-blue-100/80 dark:bg-blue-900/30 dark:text-blue-300"
    case "warn":
      return "bg-yellow-100 text-yellow-700 hover:bg-yellow-100/80 dark:bg-yellow-900/30 dark:text-yellow-300"
    case "error":
      return "bg-red-100 text-red-700 hover:bg-red-100/80 dark:bg-red-900/30 dark:text-red-300"
    default:
      return "bg-muted text-muted-foreground"
  }
}

/**
 * 格式化时间戳为可读形式
 *
 * 统一输出：MM-dd HH:mm:ss（月-日 时:分:秒）
 */
function formatTimestamp(ts: string): string {
  // 处理 yyyy-MM-dd HH:mm:ss.SSS 格式
  if (ts.includes('-') && ts.includes(' ') && ts.includes('.')) {
    // 格式：2026-07-17 08:30:15.123 → 07-17 08:30:15
    const [datePart, timePart] = ts.split(' ')
    const [_, month, day] = datePart.split('-')
    const timeWithoutMs = timePart.split('.')[0]
    return `${month}-${day} ${timeWithoutMs}`
  }
  // 兼容 ISO 8601 格式
  try {
    const d = new Date(ts)
    const mo = String(d.getMonth() + 1).padStart(2, '0')
    const da = String(d.getDate()).padStart(2, '0')
    const h = String(d.getHours()).padStart(2, '0')
    const m = String(d.getMinutes()).padStart(2, '0')
    const s = String(d.getSeconds()).padStart(2, '0')
    return `${mo}-${da} ${h}:${m}:${s}`
  } catch {
    return ts
  }
}

/** Token 相关字段 */
const TOKEN_KEYS = ['promptTokens', 'completionTokens', 'totalTokens', 'cachedTokens']

/**
 * 日志浏览组件
 *
 * 以表格/列表形式展示日志，支持级别颜色区分、滚动浏览、点击展开详情。
 * 当日志条目包含 requestBody / responseBody 时，展开显示请求/响应体内容。
 * 纯展示组件，过滤、导出、实时追加由调用方控制。
 */
export function LogViewer({
  logs,
  className,
  style,
  emptyText,
  onRowClick,
  activeId,
}: LogViewerProps) {
  const { t } = useTranslation('logger')
  const [expandedId, setExpandedId] = useState<string | null>(null)

  const defaultEmptyText = emptyText ?? t('logViewer.empty', 'No logs')
  const fieldLabels: Record<string, string> = {
    method: t('logViewer.method'),
    url: 'URL',
    statusCode: t('logViewer.statusCode'),
    durationMs: t('logViewer.durationMs'),
    tokenInfo: t('logViewer.tokenInfo'),
    errorMessage: t('logViewer.errorMessage'),
    requestId: t('logViewer.requestId'),
    modelId: t('logViewer.modelId'),
    requestBody: t('logViewer.requestBody'),
    responseBody: t('logViewer.responseBody'),
    tags: t('logViewer.tags'),
    fileName: t('logViewer.fileName'),
    lineNumber: t('logViewer.lineNumber'),
  }

  return (
    <div
      style={style}
      className={cn(
        "flex h-full flex-col rounded-md border bg-background text-xs",
        className
      )}
    >
      {/* 表头 */}
      <div className="grid grid-cols-[4rem_7rem_1fr] gap-2 border-b bg-muted/50 px-3 py-1.5 font-medium text-[10px] text-muted-foreground sm:grid-cols-[5rem_8rem_8rem_1fr]">
        <span>{t('logViewer.level')}</span>
        <span>{t('logViewer.time')}</span>
        <span className="hidden sm:inline">{t('logViewer.source')}</span>
        <span>{t('logViewer.message')}</span>
      </div>

      {/* 日志列表 — min-h-0 确保 flex 子项可收缩，ScrollArea 才能正常滚动 */}
      <ScrollArea className="min-h-0 flex-1">
        {logs.length === 0 ? (
          <div className="flex h-32 items-center justify-center text-muted-foreground">
            {defaultEmptyText}
          </div>
        ) : (
          <div className="divide-y">
            {logs.map((entry) => {
              const isExpanded = expandedId === entry.id

              return (
                <div key={entry.id}>
                  <button
                    type="button"
                    onClick={() => {
                      onRowClick?.(entry)
                      setExpandedId(isExpanded ? null : entry.id)
                    }}
                    className={cn(
                      "grid w-full grid-cols-[4rem_7rem_1fr] gap-2 px-3 py-1 text-left transition-colors",
                      "hover:bg-accent/50 focus:bg-accent/50 focus:outline-none",
                      "sm:grid-cols-[5rem_8rem_8rem_1fr]",
                      activeId === entry.id && "bg-accent"
                    )}
                  >
                    <span className="flex items-center">
                      <Badge variant="secondary" className={cn("text-[10px]", levelVariant(entry.level))}>
                        {entry.level.toUpperCase()}
                      </Badge>
                    </span>
                    <span className="flex items-center text-[10px] tabular-nums text-muted-foreground">
                      {formatTimestamp(entry.timestamp)}
                    </span>
                    <span className="hidden items-center truncate text-[10px] text-muted-foreground sm:flex">
                      {entry.source}
                    </span>
                    <span className="flex items-center gap-1.5">
                      <i className={cn(
                        "fa-solid fa-chevron-right text-[8px] text-muted-foreground transition-transform",
                        isExpanded && "rotate-90"
                      )} />
                      <span className="truncate">{entry.message}</span>
                      {entry.context?.tags && typeof entry.context.tags === 'string' && (
                        <span className="ml-1 flex shrink-0 gap-1">
                          {entry.context.tags.split(',').map((tag) => (
                            <Badge
                              key={tag}
                              variant="outline"
                              className="h-4 px-1 text-[9px] font-normal text-muted-foreground"
                            >
                              {tag.trim()}
                            </Badge>
                          ))}
                        </span>
                      )}
                    </span>
                  </button>

                  {/* 展开的详细内容 */}
                  {isExpanded && (
                    <div className="border-t bg-muted/30 px-3 py-2 text-[10px]">
                      <table className="w-full">
                        <tbody>
                          {/* Token 信息：合并为一个字段，JSON 格式展示 */}
                          {entry.context && (() => {
                            const tokenObj: Record<string, number> = {}
                            for (const key of TOKEN_KEYS) {
                              const v = entry.context[key]
                              if (v != null && v !== '') tokenObj[key] = v as number
                            }
                            if (Object.keys(tokenObj).length === 0) return null
                            return (
                              <tr key="tokenInfo">
                                <td className="w-24 py-0.5 pr-2 font-medium text-muted-foreground align-top whitespace-nowrap">
                                  {fieldLabels.tokenInfo}
                                </td>
                                <td className="py-0.5">
                                  <pre className="whitespace-pre-wrap break-all rounded bg-background p-2 font-mono text-[10px] leading-tight">
                                    {JSON.stringify(tokenObj, null, 2)}
                                  </pre>
                                </td>
                              </tr>
                            )
                          })()}
                          {/* 其他字段：排除 token 字段和请求/响应体 */}
                          {entry.context && Object.entries(entry.context)
                            .filter(([key, v]) =>
                              v != null && v !== '' && !TOKEN_KEYS.includes(key)
                            )
                            .map(([key, value]) => {
                              const isLongText = typeof value === 'string' && value.length > 100
                              return (
                                <tr key={key}>
                                  <td className="w-24 py-0.5 pr-2 font-medium text-muted-foreground align-top whitespace-nowrap">
                                    {fieldLabels[key] || key}
                                  </td>
                                  <td className="py-0.5">
                                    {isLongText ? (
                                      <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-all rounded bg-background p-2 font-mono text-[10px] leading-tight">
                                        {String(value)}
                                      </pre>
                                    ) : (
                                      <span>{String(value)}</span>
                                    )}
                                  </td>
                                </tr>
                              )
                            })}
                        </tbody>
                      </table>
                    </div>
                  )}
                </div>
              )
            })}
          </div>
        )}
      </ScrollArea>
    </div>
  )
}
