import { cn } from '@/lib/utils'
import { formatMemory } from '@/core/utils'
import { useMemoryUsage } from '@/modules/system/use-memory-usage'

interface MemoryInfoProps {
  /** 是否启用内存监控；关闭时不渲染任何内容（彻底隐藏） */
  enabled?: boolean
  className?: string
}

/**
 * 应用内存信息展示组件
 *
 * 使用 useMemoryUsage Hook 获取当前应用进程的物理内存占用，
 * 以胶囊样式展示，与 TitleBarInfo（tokens、rpm 等）视觉风格统一。
 * enabled=false 时不渲染任何内容，不留残余图标。
 */
export function MemoryInfo({ enabled = true, className }: MemoryInfoProps) {
  const { memoryKb } = useMemoryUsage({ enabled })

  // 关闭时彻底隐藏，不留图标或占位
  if (!enabled) return null

  return (
    <div
      className={cn(
        'flex items-center gap-1 rounded-full border bg-background/90 px-2.5 py-0.5 text-[10px] backdrop-blur-sm',
        className
      )}
    >
      <i className="fa-solid fa-memory h-3 w-3 text-primary" />
      <span className="text-muted-foreground">内存</span>
      <span className="font-medium tabular-nums text-primary">{formatMemory(memoryKb)}</span>
    </div>
  )
}
