/**
 * 轻量化下拉选择组件（无边框、无胶囊）
 *
 * ## 界面描述
 *
 * 触发器为 ghost 风格文本按钮：仅展示当前选中文案 + 小号箭头，
 * 无边框、无背景胶囊；悬浮时以 `bg-accent` 轻量高亮。
 * 弹层为紧凑选项列表，选中项带 ✓ 指示。
 *
 * ## 逻辑描述
 *
 * - 受控组件：`value` + `onChange`；选中后自动收起弹层。
 * - 通用组件，禁止引入业务模块类型；适合工具栏等紧凑场景
 *   （如聊天输入区的传输模式 / 协议切换）。
 * - 主题色全部走 CSS 变量。
 */

import { useMemo, useState } from 'react'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/utils'

/** 轻量下拉选项 */
export interface LightSelectOption {
  /** 选项唯一标识 */
  value: string
  /** 展示文案 */
  label: string
  /** 是否禁用该选项 */
  disabled?: boolean
}

export interface LightSelectProps {
  /** 选项列表 */
  options: LightSelectOption[]
  /** 当前选中的值 */
  value: string
  /** 选中变化回调 */
  onChange: (value: string) => void
  /** 占位文案（value 未匹配任何选项时展示） */
  placeholder?: string
  /** 是否禁用 */
  disabled?: boolean
  /** 弹层对齐方式（相对触发器，默认 start） */
  align?: 'start' | 'center' | 'end'
  /** 触发器自定义类名 */
  className?: string
  /** 弹层自定义类名 */
  contentClassName?: string
}

/**
 * 轻量化下拉选择（无边框、无胶囊）
 */
export function LightSelect({
  options,
  value,
  onChange,
  placeholder,
  disabled,
  align = 'start',
  className,
  contentClassName,
}: LightSelectProps) {
  const [open, setOpen] = useState(false)

  const selected = useMemo(
    () => options.find((opt) => opt.value === value),
    [options, value],
  )

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          disabled={disabled}
          className={cn(
            'inline-flex h-7 items-center gap-1 rounded px-1.5 text-xs text-foreground transition-colors',
            'hover:bg-accent focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring',
            'disabled:cursor-not-allowed disabled:opacity-50',
            className,
          )}
          title={selected?.label || placeholder}
        >
          <span className="min-w-0 max-w-[140px] truncate">
            {selected?.label ?? placeholder}
          </span>
          <i
            className={cn(
              'fa-solid fa-chevron-down shrink-0 text-[9px] text-muted-foreground transition-transform',
              open && 'rotate-180',
            )}
          />
        </button>
      </PopoverTrigger>
      <PopoverContent align={align} sideOffset={6} className={cn('min-w-[var(--radix-popover-trigger-width)] p-1', contentClassName)}>
        <div className="max-h-[260px] overflow-y-auto">
          {options.length === 0 ? (
            <p className="px-2 py-2 text-center text-xs text-muted-foreground">{placeholder}</p>
          ) : (
            options.map((opt) => {
              const active = opt.value === value
              return (
                <button
                  key={opt.value}
                  type="button"
                  disabled={opt.disabled}
                  className={cn(
                    'flex w-full items-center gap-1.5 rounded px-2 py-1.5 text-left text-xs transition-colors',
                    'hover:bg-accent focus-visible:bg-accent focus-visible:outline-none',
                    active && 'bg-accent',
                    opt.disabled && 'pointer-events-none opacity-50',
                  )}
                  onClick={() => {
                    onChange(opt.value)
                    setOpen(false)
                  }}
                >
                  <span className="min-w-0 flex-1 truncate">{opt.label}</span>
                  {active && <i className="fa-solid fa-check shrink-0 text-[10px] text-primary" />}
                </button>
              )
            })
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}
