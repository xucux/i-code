import * as React from 'react'
import * as ScrollAreaPrimitive from '@radix-ui/react-scroll-area'

import { cn } from '@/lib/utils'
import { TableDensityContext, type TableDensity } from '@/components/ui/table'

/** 视图模式 */
export type RadixScrollableTableViewMode = 'compact' | 'expanded'

export interface RadixScrollableTableProps
  extends React.TableHTMLAttributes<HTMLTableElement> {
  /** 视图模式：compact 紧凑自适应 / expanded 展开固定列宽并启用横向滚动 */
  viewMode?: RadixScrollableTableViewMode
  /** 容器高度，由 useAvailableHeight 计算后传入以启用垂直滚动 */
  style?: React.CSSProperties
  /** 是否加载中 */
  loading?: boolean
  /** 加载提示文案 */
  loadingText?: string
}

/**
 * 基于 Radix ScrollArea 的可滚动表格容器
 *
 * 特点：
 * - 内部使用 Radix ScrollArea 实现横纵向自定义滚动条
 * - compact 模式下表格宽度自适应容器，列名和单元格值自动换行，整体排布在可视区域内
 * - expanded 模式下表格使用 min-w-max 禁止换行，列宽之和超出容器时通过底部滚动条横向滚动
 * - 表头可通过 `<TableHeader className="sticky top-0 ...">` 实现滚动吸附
 * - 高度由父级通过 style 传入，未传入时尝试占满父级高度（h-full）
 *
 * 用法：
 * ```tsx
 * <RadixScrollableTable viewMode="expanded" style={{ height: tableHeight }}>
 *   <TableHeader className="sticky top-0 z-10 bg-muted/50">...</TableHeader>
 *   <TableBody>...</TableBody>
 * </RadixScrollableTable>
 * ```
 */
export const RadixScrollableTable = React.forwardRef<
  HTMLTableElement,
  RadixScrollableTableProps
>(
  (
    {
      viewMode = 'compact',
      style,
      loading = false,
      loadingText = '加载中...',
      className,
      children,
      ...props
    },
    ref
  ) => {
    const expanded = viewMode === 'expanded'
    // 行高保持紧凑，expanded 仅改变列宽与换行行为
    const density: TableDensity = 'compact'

    return (
      <div
        className="relative h-full w-full overflow-hidden rounded-md border"
        style={style}
      >
        <ScrollAreaPrimitive.Root
          // expanded 模式下滚动条常显，避免用户看不到可横向拖动
        //   type={expanded ? 'always' : 'auto'}
          className="h-full w-full overflow-hidden rounded-[inherit]"
        >
          <ScrollAreaPrimitive.Viewport className="h-full w-full rounded-[inherit]">
            <TableDensityContext.Provider value={density}>
              <table
                ref={ref}
                className={cn(
                  'w-full caption-bottom text-sm',
                  expanded
                    ? 'min-w-max whitespace-nowrap'
                    : 'break-words [&_th]:min-w-0 [&_td]:min-w-0',
                  className
                )}
                {...props}
              >
                {children}
              </table>
            </TableDensityContext.Provider>
          </ScrollAreaPrimitive.Viewport>

          <ScrollAreaPrimitive.ScrollAreaScrollbar
            orientation="vertical"
            className="flex h-full w-2.5 touch-none select-none border-l border-l-transparent p-[1px] transition-colors"
          >
            <ScrollAreaPrimitive.ScrollAreaThumb className="relative flex-1 rounded-full bg-border" />
          </ScrollAreaPrimitive.ScrollAreaScrollbar>

          <ScrollAreaPrimitive.ScrollAreaScrollbar
            orientation="horizontal"
            className="flex h-2.5 touch-none select-none flex-col border-t border-t-transparent p-[1px] transition-colors"
          >
            <ScrollAreaPrimitive.ScrollAreaThumb className="relative flex-1 rounded-full bg-border" />
          </ScrollAreaPrimitive.ScrollAreaScrollbar>

          <ScrollAreaPrimitive.Corner />
        </ScrollAreaPrimitive.Root>

        {loading && (
          <div className="absolute inset-0 flex items-center justify-center bg-background/50 text-xs text-muted-foreground">
            <i className="fa-solid fa-circle-notch fa-spin mr-1.5" />
            {loadingText}
          </div>
        )}
      </div>
    )
  }
)
RadixScrollableTable.displayName = 'RadixScrollableTable'
