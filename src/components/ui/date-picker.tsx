"use client"

import * as React from "react"
import { format } from "date-fns"

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

export interface DatePickerProps {
  /** 当前选中的日期 */
  value?: Date
  /** 日期变化回调 */
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

/**
 * 日期选择器
 *
 * 组合 Button + Popover + Calendar，输出原生 Date 对象。
 * 使用 yyyy-MM-dd 格式展示，符合项目紧凑风格。
 */
export function DatePicker({
  value,
  onChange,
  placeholder,
  disabled,
  className,
  align = "start",
}: DatePickerProps) {
  const [open, setOpen] = React.useState(false)
  const { t } = useTranslation('date')
  const { dateFnsLocale } = useDateLocale()
  const finalPlaceholder = placeholder ?? t('pickDate')

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          disabled={disabled}
          className={cn(
            "h-8 w-auto min-w-[110px] justify-start gap-2 px-2 text-xs font-normal",
            !value && "text-muted-foreground",
            className
          )}
        >
          <i className="fa-regular fa-calendar size-3.5" />
          {value ? format(value, "yyyy-MM-dd", { locale: dateFnsLocale }) : finalPlaceholder}
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-auto p-0" align={align}>
        <Calendar
          mode="single"
          selected={value}
          onSelect={(date) => {
            onChange?.(date)
            if (date) setOpen(false)
          }}
        />
      </PopoverContent>
    </Popover>
  )
}
