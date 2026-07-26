/**
 * 脚本模板状态徽章
 */

import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'

export interface ScriptTemplateStatusBadgeProps {
  status: string
  className?: string
  /** 文案映射 */
  labels?: {
    draft?: string
    active?: string
    disabled?: string
  }
}

export function ScriptTemplateStatusBadge({
  status,
  className,
  labels,
}: ScriptTemplateStatusBadgeProps) {
  const label =
    status === 'active'
      ? (labels?.active ?? '启用')
      : status === 'disabled'
        ? (labels?.disabled ?? '禁用')
        : (labels?.draft ?? '草稿')

  return (
    <Badge
      variant="outline"
      className={cn(
        'text-[10px] font-normal',
        status === 'active' && 'border-emerald-500/40 text-emerald-600 dark:text-emerald-400',
        status === 'disabled' && 'border-destructive/40 text-destructive',
        status === 'draft' && 'text-muted-foreground',
        className
      )}
    >
      {label}
    </Badge>
  )
}
