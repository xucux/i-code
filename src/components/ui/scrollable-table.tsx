import * as React from 'react'
import { cn } from '@/lib/utils'
import { TableDensityContext, type TableDensity } from '@/components/ui/table'

/** 视图模式 */
export type ViewMode = 'compact' | 'scroll'

export interface ScrollableTableProps
  extends React.TableHTMLAttributes<HTMLTableElement> {
  /** 视图模式：compact 自适应换行 / scroll 固定列宽横向滚动 */
  viewMode?: ViewMode
  /** 表格密度 */
  density?: TableDensity
  /** 容器高度，由 useAvailableHeight 计算后传入 */
  style?: React.CSSProperties
  /** 是否加载中 */
  loading?: boolean
  /** 加载提示文案；不传时仅显示转圈图标（避免硬编码中文） */
  loadingText?: string
}

/**
 * 可滚动表格容器
 *
 * 使用原生 overflow-auto 实现可靠的横向 + 纵向滚动，
 * 专为列数多、需要固定列宽横向滚动的统计表格设计。
 *
 * 结构：
 * - 外层：高度约束、w-full/min-w-0、loading 遮罩（不参与滚动）
 * - 内层：overflow-auto 滚动容器
 * - table：compact 自适应；scroll 时 min-w-max 撑开列宽
 *
 * 与 ScrollPage / Radix ScrollArea 的区别：
 * - 原生滚动条对宽表格溢出检测更稳定可预测
 *
 * 用法：
 * ```tsx
 * <ScrollableTable viewMode="scroll" style={{ height: tableHeight }} density="compact">
 *   <TableHeader className="sticky top-0 z-10 bg-muted">...</TableHeader>
 *   <TableBody>...</TableBody>
 * </ScrollableTable>
 * ```
 */
export const ScrollableTable = React.forwardRef<HTMLTableElement, ScrollableTableProps>(
  (
    {
      viewMode = 'compact',
      density = 'default',
      style,
      loading = false,
      loadingText,
      className,
      children,
      ...props
    },
    ref
  ) => {
    const scroll = viewMode === 'scroll'

    return (
      <div className="relative h-full w-full min-w-0" style={style}>
        <div className="h-full max-h-full w-full min-w-0 overflow-auto rounded-md border">
          <TableDensityContext.Provider value={density}>
            <table
              ref={ref}
              className={cn(
                // border-separate 提升 thead/th sticky 在 WebView 中的兼容性
                'w-full caption-bottom border-separate border-spacing-0 text-sm',
                scroll && 'min-w-max',
                className
              )}
              {...props}
            >
              {children}
            </table>
          </TableDensityContext.Provider>
        </div>

        {loading && (
          <div className="absolute inset-0 z-20 flex items-center justify-center rounded-md bg-background/50 text-xs text-muted-foreground">
            <i className="fa-solid fa-circle-notch fa-spin mr-1.5" />
            {loadingText}
          </div>
        )}
      </div>
    )
  }
)
ScrollableTable.displayName = 'ScrollableTable'