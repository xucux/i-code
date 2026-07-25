"use client"

import * as React from "react"

import { cn } from "@/lib/utils"
import { CodeEditor } from "@/components/preview/code-editor"
import type { LogEntry } from "@/components/ui/log-viewer"

/**
 * 单条日志格式化为文本行
 */
function formatLogLine(entry: LogEntry): string {
  const time = new Date(entry.timestamp).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
  const ms = String(new Date(entry.timestamp).getMilliseconds()).padStart(3, "0")
  return `[${time}.${ms}] [${entry.level.toUpperCase()}] [${entry.source}] ${entry.message}`
}

export interface LogPanelProps {
  /** 日志缓冲队列 */
  logs: LogEntry[]
  /** 缓冲队列最大长度 */
  bufferSize?: number
  /** 自定义类名 */
  className?: string
  /** 编辑器最小高度 */
  minHeight?: string
  /** 空状态占位文案 */
  emptyText?: string
}

/**
 * 基于编辑器的日志展示面板
 *
 * 仅展示缓冲队列中的最新日志，超出 bufferSize 的旧日志会被丢弃。
 * 日志以纯文本形式渲染在 CodeMirror 编辑器中，便于复制与主题融合。
 */
export function LogPanel({
  logs,
  bufferSize = 200,
  className,
  minHeight = "240px",
  emptyText = "// 暂无日志数据",
}: LogPanelProps) {
  // 仅保留缓冲队列范围内的最新日志
  const bufferedLogs = React.useMemo(() => {
    if (logs.length <= bufferSize) return logs
    return logs.slice(logs.length - bufferSize)
  }, [logs, bufferSize])

  const value = React.useMemo(() => {
    if (bufferedLogs.length === 0) return emptyText
    return bufferedLogs.map(formatLogLine).join("\n")
  }, [bufferedLogs, emptyText])

  return (
    <div className={cn("space-y-2", className)}>
      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <span>
          缓冲队列：{bufferedLogs.length} / {bufferSize}
        </span>
        <span>仅展示最新 {bufferSize} 条日志</span>
      </div>
      <CodeEditor
        value={value}
        language="text"
        readOnly
        minHeight={minHeight}
      />
    </div>
  )
}
