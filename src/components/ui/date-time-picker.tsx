"use client"

import * as React from "react"
import { format, set } from "date-fns"

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

export interface DateTimePickerProps {
  /** 当前选中的日期时间 */
  value?: Date
  /** 日期时间变化回调 */
  onChange?: (date: Date | undefined) => void
  /** 占位文案 */
  placeholder?: string
  /** 是否禁用 */
  disabled?: boolean
  /** 自定义类名 */
  className?: string
  /** 对齐方式 */
  align?: "start" | "center" | "end"
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

/**
 * 日期时间选择器
 *
 * 在日期选择器基础上增加时/分/秒滚动选择，输出完整 Date 对象。
 * 适合需要精确到秒的场景，如日志时间范围筛选。
 */
export function DateTimePicker({
  value,
  onChange,
  placeholder,
  disabled,
  className,
  align = "start",
}: DateTimePickerProps) {
  const [open, setOpen] = React.useState(false)
  const { t } = useTranslation('date')
  const { t: tCommon } = useTranslation('common')
  const { dateFnsLocale } = useDateLocale()
  const finalPlaceholder = placeholder ?? t('pickDateTime')

  const hours = value ? value.getHours() : 0
  const minutes = value ? value.getMinutes() : 0
  const seconds = value ? value.getSeconds() : 0

  const handleTimeChange = (
    part: "hours" | "minutes" | "seconds",
    num: number
  ) => {
    const base = value ?? new Date()
    const next = set(base, {
      hours: part === "hours" ? num : base.getHours(),
      minutes: part === "minutes" ? num : base.getMinutes(),
      seconds: part === "seconds" ? num : base.getSeconds(),
    })
    onChange?.(next)
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          disabled={disabled}
          className={cn(
            "h-8 w-auto min-w-[150px] justify-start gap-2 px-2 text-xs font-normal",
            !value && "text-muted-foreground",
            className
          )}
        >
          <i className="fa-regular fa-calendar size-3.5" />
          {value
            ? format(value, "yyyy-MM-dd HH:mm:ss", { locale: dateFnsLocale })
            : finalPlaceholder}
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-auto p-0" align={align}>
        <Calendar
          mode="single"
          selected={value}
          onSelect={(date) => {
            if (!date) {
              onChange?.(undefined)
              return
            }
            // 保留当前已选时间，未选则默认 00:00:00
            const base = value ?? new Date()
            const next = set(date, {
              hours: base.getHours(),
              minutes: base.getMinutes(),
              seconds: base.getSeconds(),
            })
            onChange?.(next)
          }}
        />
        <div className="flex items-center justify-center gap-1 border-t p-1.5">
          <TimeUnit
            value={hours}
            onChange={(v) => handleTimeChange("hours", v)}
            max={23}
          />
          <span className="text-[0.65rem] text-muted-foreground">:</span>
          <TimeUnit
            value={minutes}
            onChange={(v) => handleTimeChange("minutes", v)}
            max={59}
          />
          <span className="text-[0.65rem] text-muted-foreground">:</span>
          <TimeUnit
            value={seconds}
            onChange={(v) => handleTimeChange("seconds", v)}
            max={59}
          />
          <Button
            size="sm"
            className="ml-1 h-6 px-2 text-[10px]"
            onClick={() => setOpen(false)}
          >
            {tCommon('confirm')}
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  )
}
