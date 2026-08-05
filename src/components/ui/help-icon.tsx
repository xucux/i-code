import * as React from 'react'
import { Tooltip, TooltipTrigger, TooltipContent, TooltipProvider } from '@/components/ui/tooltip'
import { Popover, PopoverTrigger, PopoverContent } from '@/components/ui/popover'

interface HelpIconProps {
  type?: 'tooltip' | 'popover'
  trigger?: 'hover' | 'click'
  children: React.ReactNode
  side?: 'top' | 'bottom' | 'left' | 'right'
  align?: 'start' | 'center' | 'end'
  contentClassName?: string
  ariaLabel?: string
  /**
   * 触发按钮与图标尺寸档位：
   * - `sm`：按钮 size-5 / 图标 text-xs（紧凑表单标签旁使用）
   * - `md`（默认）：按钮 size-7 / 图标 text-sm
   * - `lg`：按钮 size-9 / 图标 text-base
   */
  size?: 'sm' | 'md' | 'lg'
}

const sizeMap = {
  sm: { btn: 'size-5', icon: 'text-xs' },
  md: { btn: 'size-7', icon: 'text-sm' },
  lg: { btn: 'size-9', icon: 'text-base' },
} as const

function HelpIcon({
  type = 'tooltip',
  trigger,
  children,
  side = 'bottom',
  align = 'end',
  contentClassName,
  ariaLabel,
  size = 'md',
}: HelpIconProps) {
  const isClick = trigger === 'click' || (trigger === undefined && type === 'popover')
  const { btn, icon } = sizeMap[size]

  const triggerEl = (
    <button
      type="button"
      className={`flex ${btn} items-center justify-center rounded text-muted-foreground transition-colors hover:bg-muted hover:text-foreground`}
      aria-label={ariaLabel}
    >
      <i className={`fa-regular fa-circle-question ${icon}`} />
    </button>
  )

  if (isClick) {
    return (
      <Popover>
        <PopoverTrigger asChild>
          {triggerEl}
        </PopoverTrigger>
        <PopoverContent side={side} align={align} className={contentClassName}>
          {children}
        </PopoverContent>
      </Popover>
    )
  }

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          {triggerEl}
        </TooltipTrigger>
        <TooltipContent side={side} align={align} className={contentClassName}>
          {children}
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}

export { HelpIcon }
export type { HelpIconProps }
