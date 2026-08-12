"use client"

import * as React from "react"
import { format } from "date-fns"
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

export interface DateRangePickerProps {
  /** 当前选中的日期范围 */
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
}

/**
 * 日期范围选择器
 *
 * 组合 Button + Popover + Calendar(mode=range)，输出 { from, to }。
 * 适合按天粒度的统计/过滤场景。
 */
export function DateRangePicker({
  value,
  onChange,
  placeholder,
  disabled,
  className,
  align = "start",
}: DateRangePickerProps) {
  const [open, setOpen] = React.useState(false)
  const { t } = useTranslation('date')
  const { dateFnsLocale } = useDateLocale()
  const finalPlaceholder = placeholder ?? t('pickDateRange')

  const label = React.useMemo(() => {
    if (value?.from && value?.to) {
      return `${format(value.from, "yyyy-MM-dd", { locale: dateFnsLocale })} ~ ${format(
        value.to,
        "yyyy-MM-dd",
        { locale: dateFnsLocale }
      )}`
    }
    if (value?.from) {
      return `${format(value.from, "yyyy-MM-dd", { locale: dateFnsLocale })} ~ ...`
    }
    return finalPlaceholder
  }, [value, finalPlaceholder, dateFnsLocale])

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          disabled={disabled}
          className={cn(
            "h-8 w-auto min-w-[170px] justify-start gap-2 px-2 text-xs font-normal",
            !value?.from && "text-muted-foreground",
            className
          )}
        >
          <i className="fa-regular fa-calendar size-3.5" />
          {label}
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-auto p-0" align={align}>
        <div className="flex items-center justify-end px-1 pt-1">
          {value?.from && (
            <Button
              variant="ghost"
              size="sm"
              className="h-5 px-1.5 text-[10px] text-muted-foreground hover:text-foreground"
              onClick={() => {
                onChange?.(undefined)
                setOpen(false)
              }}
            >
              <i className="fa-solid fa-xmark mr-1" />
              {t('clear')}
            </Button>
          )}
        </div>
        <Calendar
          mode="range"
          selected={value}
          onSelect={(range) => {
            onChange?.(range)
            // 选择完整起止后自动关闭
            if (range?.from && range?.to) {
              setOpen(false)
            }
          }}
          numberOfMonths={2}
        />
      </PopoverContent>
    </Popover>
  )
}
