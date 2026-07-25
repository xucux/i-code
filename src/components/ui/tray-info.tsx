import { cn } from '@/lib/utils'
import { Progress } from '@/components/ui/progress'

interface TrayInfoProps {
  /** 当前使用的供应商名称 */
  provider?: string
  /** 当前使用的模型名称 */
  model?: string
  /** 额度信息：已用、总额、单位 */
  quota?: {
    used: number
    total: number
    unit?: string
  }
  className?: string
}

/**
 * 托盘信息展示组件
 * 用于在系统托盘弹出内容或配置预览中展示当前供应商、模型与额度使用情况。
 * 纯展示组件，数据由外部传入，不依赖业务逻辑。
 */
export function TrayInfo({ provider, model, quota, className }: TrayInfoProps) {
  const percent =
    quota && quota.total > 0 ? Math.round((quota.used / quota.total) * 100) : undefined

  return (
    <div className={cn('w-56 space-y-3 rounded-md border bg-popover p-3 text-popover-foreground shadow-md', className)}>
      {/* 供应商 */}
      <div className="flex items-center gap-2">
        <i className="fa-solid fa-building-user h-4 w-4 text-muted-foreground" />
        <div className="min-w-0 flex-1">
          <p className="text-[10px] text-muted-foreground">供应商</p>
          <p className="truncate text-xs font-medium">{provider ?? '—'}</p>
        </div>
      </div>

      {/* 模型 */}
      <div className="flex items-center gap-2">
        <i className="fa-solid fa-robot h-4 w-4 text-muted-foreground" />
        <div className="min-w-0 flex-1">
          <p className="text-[10px] text-muted-foreground">模型</p>
          <p className="truncate text-xs font-medium">{model ?? '—'}</p>
        </div>
      </div>

      {/* 额度 */}
      {quota && (
        <div className="space-y-1.5">
          <div className="flex items-center justify-between text-[10px]">
            <span className="text-muted-foreground">额度</span>
            <span className="tabular-nums">
              {quota.used} / {quota.total} {quota.unit ?? ''}
            </span>
          </div>
          <Progress value={percent} className="h-1.5" />
        </div>
      )}
    </div>
  )
}
