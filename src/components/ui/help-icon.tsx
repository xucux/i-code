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
}

function HelpIcon({
  type = 'tooltip',
  trigger,
  children,
  side = 'bottom',
  align = 'end',
  contentClassName,
  ariaLabel,
}: HelpIconProps) {
  const isClick = trigger === 'click' || (trigger === undefined && type === 'popover')

  const triggerEl = (
    <button
      type="button"
      className="flex size-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
      aria-label={ariaLabel}
    >
      <i className="fa-regular fa-circle-question text-sm" />
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
