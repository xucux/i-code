import { cn } from '@/lib/utils'
import { formatCompactCount } from '@/core/utils'

export interface TitleBarInfoItem {
  /** Font Awesome 图标名称，不需要 fa- 前缀 */
  icon: string
  /** 信息项标签 */
  label: string
  /** 信息项数值；大整数建议以 string 或 bigint 传入以避免精度丢失 */
  value: string | number | bigint
  /** 是否高亮显示 */
  active?: boolean
}

interface TitleBarInfoProps {
  /** 要在标题栏展示的信息项列表 */
  items: TitleBarInfoItem[]
  className?: string
}

/**
 * 标题栏信息展示组件
 * 用于在自定义标题栏中间区域展示紧凑的实时信息（如 token 消耗总数）。
 * 每项包含图标、标签和数值，整体保持与标题栏一致的高度和视觉风格。
 */
export function TitleBarInfo({ items, className }: TitleBarInfoProps) {
  if (items.length === 0) return null

  return (
    <div
      className={cn(
        'flex items-center gap-3 rounded-full border bg-background/90 px-2.5 py-0.5 text-[10px] backdrop-blur-sm',
        className
      )}
    >
      {items.map((item, index) => (
        <div key={`${item.label}-${index}`} className="flex items-center gap-1">
          <i
            className={cn(
              'fa-solid h-3 w-3',
              `fa-${item.icon}`,
              item.active ? 'text-primary' : 'text-muted-foreground'
            )}
          />
          <span className="text-muted-foreground">{item.label}</span>
          <span className={cn('font-medium tabular-nums', item.active && 'text-primary')}>
            {/* 数字类型使用紧凑计数格式化，便于展示 token 等可能很大的数值 */}
            {typeof item.value === 'number' ? formatCompactCount(item.value) : item.value}
          </span>
        </div>
      ))}
    </div>
  )
}
