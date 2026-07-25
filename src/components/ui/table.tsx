import * as React from 'react'

import { cn } from '@/lib/utils'

/** 表格密度 */
type TableDensity = 'compact' | 'default'

/**
 * 表格密度上下文
 *
 * 由 Table 组件提供，TableHead / TableCell / TableRow 消费，
 * 根据 density 自动调整行高、内边距与字号。
 */
const TableDensityContext = React.createContext<TableDensity>('default')

function useTableDensity(): TableDensity {
  return React.useContext(TableDensityContext)
}

/**
 * 表格根组件
 *
 * 基于 HTML <table>，外层包裹可滚动容器，支持：
 * - density：紧凑/默认密度模式，影响子组件行高与内边距
 * - style：传入 useAvailableHeight 计算的高度以启用垂直滚动
 * - overflow：是否开启外层滚动容器，默认 true；在 ScrollPage 等外层滚动容器内
 *   需要由父级统一控制横向滚动时，可设为 false，避免重复滚动条
 * - 水平方向自动滚动（列过多时）
 */
const Table = React.forwardRef<
  HTMLTableElement,
  React.HTMLAttributes<HTMLTableElement> & {
    /** 表格密度：compact 紧凑 / default 默认 */
    density?: TableDensity
    /** 是否开启外层滚动容器，默认 true */
    overflow?: boolean
  }
>(({ className, density = 'default', overflow = true, ...props }, ref) => (
  <TableDensityContext.Provider value={density}>
    <div className={cn('relative w-full', overflow && 'overflow-auto')}>
      <table
        ref={ref}
        className={cn('w-full caption-bottom text-sm', className)}
        {...props}
      />
    </div>
  </TableDensityContext.Provider>
))
Table.displayName = 'Table'

const TableHeader = React.forwardRef<
  HTMLTableSectionElement,
  React.HTMLAttributes<HTMLTableSectionElement>
>(({ className, ...props }, ref) => (
  <thead ref={ref} className={cn('[&_tr]:border-b', className)} {...props} />
))
TableHeader.displayName = 'TableHeader'

const TableBody = React.forwardRef<
  HTMLTableSectionElement,
  React.HTMLAttributes<HTMLTableSectionElement>
>(({ className, ...props }, ref) => (
  <tbody
    ref={ref}
    className={cn('[&_tr:last-child]:border-0', className)}
    {...props}
  />
))
TableBody.displayName = 'TableBody'

const TableFooter = React.forwardRef<
  HTMLTableSectionElement,
  React.HTMLAttributes<HTMLTableSectionElement>
>(({ className, ...props }, ref) => (
  <tfoot
    ref={ref}
    className={cn(
      'border-t bg-muted/50 font-medium [&>tr]:last:border-b-0',
      className
    )}
    {...props}
  />
))
TableFooter.displayName = 'TableFooter'

const TableRow = React.forwardRef<
  HTMLTableRowElement,
  React.HTMLAttributes<HTMLTableRowElement>
>(({ className, ...props }, ref) => {
  const density = useTableDensity()
  return (
    <tr
      ref={ref}
      className={cn(
        'border-b transition-colors hover:bg-muted/50 data-[state=selected]:bg-muted',
        density === 'compact' && 'text-xs',
        className
      )}
      {...props}
    />
  )
})
TableRow.displayName = 'TableRow'

const TableHead = React.forwardRef<
  HTMLTableCellElement,
  React.ThHTMLAttributes<HTMLTableCellElement>
>(({ className, ...props }, ref) => {
  const density = useTableDensity()
  return (
    <th
      ref={ref}
      className={cn(
        'text-left align-middle font-medium text-muted-foreground [&:has([role=checkbox])]:pr-0 [&>[role=checkbox]]:translate-y-[2px]',
        density === 'compact'
          ? 'h-7 px-2 text-[10px]'
          : 'h-10 px-2',
        className
      )}
      {...props}
    />
  )
})
TableHead.displayName = 'TableHead'

const TableCell = React.forwardRef<
  HTMLTableCellElement,
  React.TdHTMLAttributes<HTMLTableCellElement>
>(({ className, ...props }, ref) => {
  const density = useTableDensity()
  return (
    <td
      ref={ref}
      className={cn(
        'align-middle [&:has([role=checkbox])]:pr-0 [&>[role=checkbox]]:translate-y-[2px]',
        density === 'compact'
          ? 'px-2 py-1'
          : 'p-2',
        className
      )}
      {...props}
    />
  )
})
TableCell.displayName = 'TableCell'

const TableCaption = React.forwardRef<
  HTMLTableCaptionElement,
  React.HTMLAttributes<HTMLTableCaptionElement>
>(({ className, ...props }, ref) => (
  <caption
    ref={ref}
    className={cn('mt-4 text-sm text-muted-foreground', className)}
    {...props}
  />
))
TableCaption.displayName = 'TableCaption'

export {
  Table,
  TableHeader,
  TableBody,
  TableFooter,
  TableHead,
  TableRow,
  TableCell,
  TableCaption,
  /** 表格密度类型，供外部类型引用 */
  type TableDensity,
  /** 表格密度上下文，供 ScrollableTable 等扩展组件消费 */
  TableDensityContext,
}
