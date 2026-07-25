"use client"

import * as React from "react"

import type { DateRange } from "react-day-picker"

import { cn } from "@/lib/utils"
import { useTranslation } from "@/modules/i18n/use-translation"
import { DateTimeRangePicker } from "@/components/ui/date-time-range-picker"
import { Button } from "@/components/ui/button"

export interface LogTimeRangeValue {
  /** 开始时间（ISO 8601 字符串） */
  from?: string
  /** 结束时间（ISO 8601 字符串） */
  to?: string
}

export interface LogTimeRangeFilterProps {
  /** 当前时间范围值 */
  value?: LogTimeRangeValue
  /** 时间范围变化回调 */
  onChange?: (value: LogTimeRangeValue | undefined) => void
  /** 自定义类名 */
  className?: string
}

/**
 * 将 LogFilter 中的时间范围字符串转换为 DateRange
 *
 * 后端使用 ISO 8601 字符串进行比较，组件层使用 Date 对象交互。
 */
function toDateRange(value?: LogTimeRangeValue): DateRange | undefined {
  if (!value?.from && !value?.to) return undefined
  return {
    from: value.from ? new Date(value.from) : undefined,
    to: value.to ? new Date(value.to) : undefined,
  }
}

/**
 * 将 DateRange 转换为 ISO 8601 字符串范围
 */
function toLogTimeRange(range?: DateRange): LogTimeRangeValue | undefined {
  if (!range?.from && !range?.to) return undefined
  return {
    from: range.from?.toISOString(),
    to: range.to?.toISOString(),
  }
}

/**
 * 日志时间范围筛选器
 *
 * 使用组件库 DateTimeRangePicker，其面板内置 9 组常用快捷范围预设；
 * 支持精确到秒的起止时间选择，输出 ISO 8601 字符串，与 LogFilter.timeRange 格式保持一致。
 */
export function LogTimeRangeFilter({
  value,
  onChange,
  className,
}: LogTimeRangeFilterProps) {
  const { t } = useTranslation()
  const range = React.useMemo(() => toDateRange(value), [value])

  const handleClear = () => {
    onChange?.(undefined)
  }

  return (
    <div className={cn("flex flex-wrap items-center gap-2", className)}>
      <DateTimeRangePicker
        value={range}
        onChange={(next) => onChange?.(toLogTimeRange(next))}
        placeholder={t('logger.timeRange.placeholder')}
        align="start"
      />

      <Button
        variant="ghost"
        size="sm"
        className="h-8 px-2 text-xs text-muted-foreground"
        onClick={handleClear}
      >
        {t('logger.timeRange.clear')}
      </Button>
    </div>
  )
}
