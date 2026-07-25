"use client"

import * as React from "react"

import { cn } from "@/lib/utils"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Button } from "@/components/ui/button"

/**
 * 单个下拉选项的数据结构
 */
export interface DropdownOption {
  /** 选项唯一标识 */
  value: string
  /** 展示文案 */
  label: string
  /** 左侧 Font Awesome 图标类名，例如 "fa-solid fa-robot" */
  icon?: string
  /** 是否禁用该选项 */
  disabled?: boolean
}

export interface DropdownProps {
  /** 选项列表 */
  options: DropdownOption[]
  /** 当前选中的值 */
  value?: string
  /** 选中变化回调 */
  onChange?: (value: string) => void
  /** 占位文案 */
  placeholder?: string
  /** 是否禁用整个下拉框 */
  disabled?: boolean
  /** 自定义类名 */
  className?: string
  /** 弹层面板类名 */
  contentClassName?: string
  /** 对齐方式 */
  align?: "start" | "center" | "end"
  /** 尺寸 */
  size?: "sm" | "default" | "lg"
}

/**
 * 基础下拉选择组件
 *
 * 基于 Popover 封装，支持图标、禁用项、占位文案等。
 * 用于替代原生 select，提供与主题一致的样式。
 */
export function Dropdown({
  options,
  value,
  onChange,
  placeholder = "请选择...",
  disabled = false,
  className,
  contentClassName,
  align = "start",
  size = "default",
}: DropdownProps) {
  const [open, setOpen] = React.useState(false)

  const selected = React.useMemo(
    () => options.find((opt) => opt.value === value),
    [options, value]
  )

  const handleSelect = (opt: DropdownOption) => {
    if (opt.disabled) return
    onChange?.(opt.value)
    setOpen(false)
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          disabled={disabled}
          className={cn(
            "w-full justify-between font-normal",
            !selected && "text-muted-foreground",
            size === "sm" && "h-8 px-2.5 text-xs",
            size === "lg" && "h-10 px-4",
            className
          )}
        >
          <span className="flex items-center gap-2 truncate">
            {selected?.icon && (
              <i className={cn(selected.icon, "size-4 shrink-0")} />
            )}
            <span className="truncate">{selected?.label ?? placeholder}</span>
          </span>
          <i className="fa-solid fa-chevron-down size-4 shrink-0 opacity-50" />
        </Button>
      </PopoverTrigger>
      <PopoverContent
        align={align}
        className={cn("w-[var(--radix-popover-trigger-width)] p-0", contentClassName)}
      >
        <div className="max-h-60 overflow-auto py-1">
          {options.length === 0 ? (
            <div className="px-3 py-2 text-sm text-muted-foreground">暂无选项</div>
          ) : (
            options.map((opt) => (
              <button
                key={opt.value}
                type="button"
                disabled={opt.disabled}
                onClick={() => handleSelect(opt)}
                className={cn(
                  "relative flex w-full items-center gap-2 px-3 py-2 text-sm outline-none transition-colors",
                  "hover:bg-accent hover:text-accent-foreground",
                  "focus:bg-accent focus:text-accent-foreground",
                  opt.value === value && "bg-accent text-accent-foreground",
                  opt.disabled && "pointer-events-none opacity-50"
                )}
              >
                {opt.icon && <i className={cn(opt.icon, "size-4 shrink-0")} />}
                <span className="flex-1 text-left">{opt.label}</span>
                {opt.value === value && (
                  <i className="fa-solid fa-check size-4 shrink-0" />
                )}
              </button>
            ))
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}
