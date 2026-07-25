"use client"

import * as React from "react"
import { format, set, subDays, subHours, subMonths, subYears } from "date-fns"
import type { DateRange } from "react-day-picker"

import { cn } from "@/lib/utils"
import { useTranslation } from "@/modules/i18n/use-translation"
import { useDateLocale } from "@/modules/i18n/use-date-locale"
import { Button } from "@/components/ui/button"
import { Calendar } from "@/components/ui/calendar"

import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"

/** 单个快捷范围配置 */
export interface DateTimeRangePreset {
  /** 显示文案 */
  label: string
  /** 返回对应的日期时间范围 */
  getValue: () => DateRange
}

export interface DateTimeRangePickerProps {
  /** 当前选中的日期时间范围 */
  value?: DateRange
  /** 范围变化回调 */
  onChange?: (range: DateRange | undefined) => void
  /** 占位文案 */
  placeholder?: string
  /** 是否禁用 */
  disabled?: boolean
  /** 自定义类名 */
  className?: string
  /** 对齐方式 */
  align?: "start" | "center" | "end"
  /**
   * 快捷范围预设列表；传空数组则不显示预设面板。
   * 不传时使用默认的 9 组常用日志/统计时间范围。
   */
  presets?: DateTimeRangePreset[]
}

function pad2(n: number): string {
  return String(n).padStart(2, "0")
}

/**
 * 单个时间单元（时/分/秒）的滚动选择器
 *
 * 上下箭头调整数值，点击数字可直接输入。
 */
function TimeUnit({
  value,
  onChange,
  max,
}: {
  value: number
  onChange: (v: number) => void
  max: number
}) {
  const [editing, setEditing] = React.useState(false)
  const inputRef = React.useRef<HTMLInputElement>(null)

  React.useEffect(() => {
    if (editing) inputRef.current?.select()
  }, [editing])

  const clamp = (n: number) => (n < 0 ? max : n > max ? 0 : n)

  if (editing) {
    return (
      <input
        ref={inputRef}
        type="text"
        defaultValue={pad2(value)}
        className="h-5 w-7 rounded border border-input bg-background text-center text-[0.75rem] tabular-nums outline-none focus:border-primary"
        maxLength={2}
        onBlur={(e) => {
          const num = Number.parseInt(e.target.value.replace(/\D/g, ""), 10)
          if (!Number.isNaN(num)) onChange(Math.min(Math.max(num, 0), max))
          setEditing(false)
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") (e.target as HTMLInputElement).blur()
          if (e.key === "Escape") setEditing(false)
        }}
        autoFocus
      />
    )
  }

  return (
    <div className="flex flex-col items-center gap-0.5">
      <button
        type="button"
        onClick={() => onChange(clamp(value + 1))}
        className="flex size-4 items-center justify-center rounded text-[0.5rem] text-muted-foreground hover:bg-muted hover:text-foreground"
      >
        <i className="fa-solid fa-chevron-up" />
      </button>
      <button
        type="button"
        onClick={() => setEditing(true)}
        className="h-5 w-7 rounded text-[0.75rem] tabular-nums hover:bg-muted"
      >
        {pad2(value)}
      </button>
      <button
        type="button"
        onClick={() => onChange(clamp(value - 1))}
        className="flex size-4 items-center justify-center rounded text-[0.5rem] text-muted-foreground hover:bg-muted hover:text-foreground"
      >
        <i className="fa-solid fa-chevron-down" />
      </button>
    </div>
  )
}

/** 获取默认快捷范围预设 */
function useDefaultPresets(): DateTimeRangePreset[] {
  const { t } = useTranslation()
  return React.useMemo(() => {
    const now = new Date()
    return [
      {
        label: t("dateTimeRangePresets.last1Hour"),
        getValue: () => ({ from: subHours(now, 1), to: now }),
      },
      {
        label: t("dateTimeRangePresets.last12Hours"),
        getValue: () => ({ from: subHours(now, 12), to: now }),
      },
      {
        label: t("dateTimeRangePresets.last1Day"),
        getValue: () => ({ from: subHours(now, 24), to: now }),
      },
      {
        label: t("dateTimeRangePresets.last7Days"),
        getValue: () => ({ from: subDays(now, 7), to: now }),
      },
      {
        label: t("dateTimeRangePresets.last15Days"),
        getValue: () => ({ from: subDays(now, 15), to: now }),
      },
      {
        label: t("dateTimeRangePresets.lastHalfMonth"),
        getValue: () => ({ from: subDays(now, 15), to: now }),
      },
      {
        label: t("dateTimeRangePresets.last1Month"),
        getValue: () => ({ from: subDays(now, 30), to: now }),
      },
      {
        label: t("dateTimeRangePresets.lastHalfYear"),
        getValue: () => ({ from: subMonths(now, 6), to: now }),
      },
      {
        label: t("dateTimeRangePresets.last1Year"),
        getValue: () => ({ from: subYears(now, 1), to: now }),
      },
    ]
  }, [t])
}

/**
 * 日期时间范围选择器
 *
 * 在日期范围基础上为起止日期分别增加时/分/秒输入，
 * 并在面板左侧提供可配置的快捷范围预设（默认覆盖 1 小时到 1 年），
 * 输出完整 { from, to } Date 对象，适合日志等精确时间范围场景。
 */
export function DateTimeRangePicker({
  value,
  onChange,
  placeholder,
  disabled,
  className,
  align = "start",
  presets: presetsProp,
}: DateTimeRangePickerProps) {
  const [open, setOpen] = React.useState(false)
  const { t } = useTranslation()
  const { t: tDate } = useTranslation('date')
  const { dateFnsLocale } = useDateLocale()
  const defaultPresets = useDefaultPresets()
  const presets = presetsProp === undefined ? defaultPresets : presetsProp
  const finalPlaceholder = placeholder ?? tDate('pickDateTimeRange')

  const label = React.useMemo(() => {
    if (value?.from && value?.to) {
      return `${format(value.from, "yyyy-MM-dd HH:mm:ss", {
        locale: dateFnsLocale,
      })} ~ ${format(value.to, "yyyy-MM-dd HH:mm:ss", { locale: dateFnsLocale })}`
    }
    if (value?.from) {
      return `${format(value.from, "yyyy-MM-dd HH:mm:ss", {
        locale: dateFnsLocale,
      })} ~ ...`
    }
    return finalPlaceholder
  }, [value, finalPlaceholder, dateFnsLocale])

  const updateTime = (
    point: "from" | "to",
    part: "hours" | "minutes" | "seconds",
    num: number
  ) => {
    const base = value?.[point] ?? new Date()
    const next = set(base, {
      hours: part === "hours" ? num : base.getHours(),
      minutes: part === "minutes" ? num : base.getMinutes(),
      seconds: part === "seconds" ? num : base.getSeconds(),
    })
    onChange?.(
      point === "from"
        ? { from: next, to: value?.to }
        : { from: value?.from, to: next }
    )
  }

  const fromHours = value?.from ? value.from.getHours() : 0
  const fromMinutes = value?.from ? value.from.getMinutes() : 0
  const fromSeconds = value?.from ? value.from.getSeconds() : 0
  const toHours = value?.to ? value.to.getHours() : 23
  const toMinutes = value?.to ? value.to.getMinutes() : 59
  const toSeconds = value?.to ? value.to.getSeconds() : 59

  const TimeField = ({ point }: { point: "from" | "to" }) => {
    const isFrom = point === "from"
    return (
      <div className="flex items-center gap-1">
        <span className="w-7 text-[10px] text-muted-foreground">
          {isFrom ? tDate('start') : tDate('end')}
        </span>
        <TimeUnit
          value={isFrom ? fromHours : toHours}
          onChange={(v) => updateTime(point, "hours", v)}
          max={23}
        />
        <span className="text-[0.65rem] text-muted-foreground">:</span>
        <TimeUnit
          value={isFrom ? fromMinutes : toMinutes}
          onChange={(v) => updateTime(point, "minutes", v)}
          max={59}
        />
        <span className="text-[0.65rem] text-muted-foreground">:</span>
        <TimeUnit
          value={isFrom ? fromSeconds : toSeconds}
          onChange={(v) => updateTime(point, "seconds", v)}
          max={59}
        />
      </div>
    )
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          disabled={disabled}
          className={cn(
            "h-8 w-auto min-w-[220px] justify-start gap-2 px-2 text-xs font-normal",
            !value?.from && "text-muted-foreground",
            className
          )}
        >
          <i className="fa-regular fa-calendar size-3.5" />
          <span className="truncate">{label}</span>
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-auto p-0" align={align}>
        <div className="flex">
          {presets.length > 0 && (
            <div className="flex min-w-[5.5rem] flex-col gap-0.5 border-r p-1.5">
              {presets.map((preset) => (
                <Button
                  key={preset.label}
                  variant="ghost"
                  size="sm"
                  className="h-6 justify-start px-1.5 text-[10px]"
                  onClick={() => onChange?.(preset.getValue())}
                >
                  {preset.label}
                </Button>
              ))}
            </div>
          )}
          <div>
            <Calendar
              mode="range"
              selected={value}
              onSelect={(range) => {
                if (!range) {
                  onChange?.(undefined)
                  return
                }
                const prevFrom = value?.from
                const prevTo = value?.to
                const next: DateRange = {
                  from: range.from
                    ? set(range.from, {
                        hours: prevFrom?.getHours() ?? 0,
                        minutes: prevFrom?.getMinutes() ?? 0,
                        seconds: prevFrom?.getSeconds() ?? 0,
                      })
                    : undefined,
                  to: range.to
                    ? set(range.to, {
                        hours: prevTo?.getHours() ?? 23,
                        minutes: prevTo?.getMinutes() ?? 59,
                        seconds: prevTo?.getSeconds() ?? 59,
                      })
                    : undefined,
                }
                onChange?.(next)
                // 日期时间范围选择器不自动关闭，用户需继续调整时间后点确定
              }}
              numberOfMonths={2}
            />
            <div className="flex flex-row gap-1 border-t p-1.5 items-center">
              <TimeField point="from" />
              <TimeField point="to" />
              <div className="flex justify-end">
                <Button
                  size="sm"
                  className="h-6 px-2 text-[10px]"
                  onClick={() => setOpen(false)}
                >
                  {t("dateTimeRangePresets.confirm")}
                </Button>
              </div>
            </div>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  )
}
