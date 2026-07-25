import { cn } from '@/lib/utils'

export interface LoadingOverlayProps {
  /** 是否显示遮罩 */
  open?: boolean
  /** 加载提示文案 */
  message?: string
  className?: string
  style?: React.CSSProperties
}

/**
 * 全屏遮罩加载组件
 *
 * 用于需要阻塞用户操作并等待后台完成的场景（如备份恢复后自动重启）。
 * 居中显示旋转图标与可选文案，覆盖整个视口。
 */
export function LoadingOverlay({ open = true, message, className, style }: LoadingOverlayProps) {
  if (!open) return null

  return (
    <div
      className={cn(
        'fixed inset-0 z-50 flex flex-col items-center justify-center gap-3 bg-background/80 backdrop-blur-sm',
        className
      )}
      style={style}
    >
      <i className="fa-solid fa-circle-notch fa-spin text-primary text-2xl" />
      {message && <p className="text-muted-foreground text-sm">{message}</p>}
    </div>
  )
}
