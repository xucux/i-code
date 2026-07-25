"use client"

import * as React from "react"
import { DayPicker } from "react-day-picker"
import type { DropdownOption } from "react-day-picker"

import { cn } from "@/lib/utils"
import { useDateLocale } from "@/modules/i18n/use-date-locale"
import { buttonVariants } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"

export type CalendarProps = React.ComponentProps<typeof DayPicker>

/** react-day-picker MonthsDropdown 接受的 props 类型 */
type MonthsDropdownProps = React.SelectHTMLAttributes<HTMLSelectElement> & {
  options?: DropdownOption[]
  value?: number
  onChange?: (e: React.ChangeEvent<HTMLSelectElement>) => void
}

/** react-day-picker YearsDropdown 接受的 props 类型 */
type YearsDropdownProps = React.SelectHTMLAttributes<HTMLSelectElement> & {
  options?: DropdownOption[]
  value?: number
  onChange?: (e: React.ChangeEvent<HTMLSelectElement>) => void
}

/**
 * 月份下拉选择器 —— 用项目 DropdownMenu 渲染 react-day-picker 的 MonthsDropdown
 */
function MonthsDropdown({ options, value, onChange, className, disabled }: MonthsDropdownProps) {
  const current = options?.find((o) => o.value === value)

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          disabled={disabled}
          className={cn(
            "rounded px-1.5 py-0.5 text-xs font-medium hover:bg-muted disabled:opacity-50",
            className
          )}
        >
          {current?.label ?? value}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="center"
        className="max-h-48 overflow-y-auto"
      >
        {options?.map((opt) => (
          <DropdownMenuItem
            key={opt.value}
            disabled={opt.disabled}
            onClick={() =>
              onChange?.({
                target: { value: String(opt.value) },
              } as unknown as React.ChangeEvent<HTMLSelectElement>)
            }
            className={cn(
              "justify-center text-xs",
              opt.value === value && "bg-muted font-medium"
            )}
          >
            {opt.label}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

/**
 * 年份下拉选择器 —— 用项目 DropdownMenu 渲染 react-day-picker 的 YearsDropdown
 */
function YearsDropdown({ options, value, onChange, className, disabled }: YearsDropdownProps) {
  const current = options?.find((o) => o.value === value)

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          disabled={disabled}
          className={cn(
            "rounded px-1.5 py-0.5 text-xs font-medium tabular-nums hover:bg-muted disabled:opacity-50",
            className
          )}
        >
          {current?.label ?? value}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="center"
        className="max-h-48 overflow-y-auto"
      >
        {options?.map((opt) => (
          <DropdownMenuItem
            key={opt.value}
            disabled={opt.disabled}
            onClick={() =>
              onChange?.({
                target: { value: String(opt.value) },
              } as unknown as React.ChangeEvent<HTMLSelectElement>)
            }
            className={cn(
              "justify-center text-xs tabular-nums",
              opt.value === value && "bg-muted font-medium"
            )}
          >
            {opt.label}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

/**
 * 日历组件
 *
 * 基于 react-day-picker v10，使用项目主题色与 Font Awesome 图标。
 * 支持单选、范围、多选模式；外层 Popover 组合成 Date Picker。
 * 年月导航改为点击弹出下拉选择，无左右箭头。
 */
function Calendar({
  className,
  classNames,
  showOutsideDays = true,
  ...props
}: CalendarProps) {
  const { reactDayPickerLocale } = useDateLocale()

  return (
    <DayPicker
      showOutsideDays={showOutsideDays}
      locale={reactDayPickerLocale}
      captionLayout="dropdown"
      className={cn("p-1.5", className)}
      classNames={{
        root: "p-1.5",
        months: "flex flex-col gap-2 sm:flex-row",
        month: "flex flex-col gap-2",
        month_caption: "flex items-center justify-center",
        caption_label: "hidden",
        nav: "hidden",
        dropdowns: "flex items-center justify-center gap-1",
        month_grid: "w-full border-collapse",
        weekdays: "flex",
        weekday:
          "w-7 rounded-md text-[0.65rem] font-normal text-muted-foreground",
        weeks: "",
        week: "mt-0.5 flex w-full",
        day: "relative h-7 w-7 p-0 text-center text-[0.75rem] focus-within:relative focus-within:z-20",
        day_button: cn(
          buttonVariants({ variant: "ghost" }),
          "size-7 p-0 text-[0.75rem] font-normal aria-selected:opacity-100"
        ),
        selected:
          "rounded-md bg-primary text-primary-foreground hover:bg-primary hover:text-primary-foreground focus:bg-primary focus:text-primary-foreground",
        range_start:
          "rounded-l-md rounded-r-none bg-primary text-primary-foreground",
        range_middle:
          "rounded-none bg-accent text-accent-foreground",
        range_end:
          "rounded-r-md rounded-l-none bg-primary text-primary-foreground",
        today: "rounded-md bg-accent text-accent-foreground",
        outside:
          "rounded-md text-muted-foreground opacity-50 aria-selected:bg-accent/50 aria-selected:text-muted-foreground aria-selected: opacity-30",
        disabled: "rounded-md text-muted-foreground opacity-50",
        hidden: "invisible",
        ...classNames,
      }}
      components={{
        MonthsDropdown,
        YearsDropdown,
      } as React.ComponentProps<typeof DayPicker>["components"]}
      {...props}
    />
  )
}
Calendar.displayName = "Calendar"

export { Calendar }
