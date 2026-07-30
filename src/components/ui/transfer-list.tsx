"use client"

import * as React from "react"

import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { ScrollPage } from "@/components/ui/scroll-page"

/**
 * 穿梭框条目数据
 */
export interface TransferListItem {
  /** 条目唯一标识 */
  id: string
  /** 展示文本 */
  label: string
  /** 辅助描述（第一行小字） */
  description?: string
  /** 第二行辅助小字（如供应商名称） */
  secondary?: string
  /** 是否禁用 */
  disabled?: boolean
  /** 分组键（可用于排序或视觉分组） */
  group?: string
}

export interface TransferListProps {
  /** 候选池全部条目 */
  items: TransferListItem[]
  /** 已选中的条目 ID 列表 */
  selectedIds: string[]
  /** 选择变化回调，返回新的已选 ID 列表 */
  onChange: (selectedIds: string[]) => void
  /** 左侧标题 */
  titleLeft?: string
  /** 右侧标题 */
  titleRight?: string
  /** 左侧搜索占位符 */
  leftSearchPlaceholder?: string
  /** 右侧搜索占位符 */
  rightSearchPlaceholder?: string
  /** 自定义类名 */
  className?: string
  /** 列表区域高度 */
  listHeight?: string
}

/**
 * 通用穿梭框组件
 *
 * 左右两栏布局：左侧为候选池，右侧为已选项。
 * 支持关键词搜索、单选/多选、批量移入/移出。
 * 变更结果通过 `onChange` 以已选 ID 数组形式返回。
 */
export function TransferList({
  items,
  selectedIds,
  onChange,
  titleLeft = "可选",
  titleRight = "已选",
  leftSearchPlaceholder = "搜索...",
  rightSearchPlaceholder = "搜索...",
  className,
  listHeight = "h-56",
}: TransferListProps) {
  const selectedSet = React.useMemo(() => new Set(selectedIds), [selectedIds])

  const leftItems = React.useMemo(
    () => items.filter((item) => !selectedSet.has(item.id)),
    [items, selectedSet]
  )
  const rightItems = React.useMemo(
    () => items.filter((item) => selectedSet.has(item.id)),
    [items, selectedSet]
  )

  const [leftSearch, setLeftSearch] = React.useState("")
  const [rightSearch, setRightSearch] = React.useState("")

  const [leftChecked, setLeftChecked] = React.useState<Set<string>>(new Set())
  const [rightChecked, setRightChecked] = React.useState<Set<string>>(new Set())

  const matchesSearch = (item: TransferListItem, term: string) => {
    if (!term) return true
    const lower = term.toLowerCase()
    return (
      item.id.toLowerCase().includes(lower) ||
      item.label.toLowerCase().includes(lower) ||
      (item.description?.toLowerCase().includes(lower) ?? false) ||
      (item.group?.toLowerCase().includes(lower) ?? false)
    )
  }

  const filteredLeft = React.useMemo(() => {
    const term = leftSearch.trim().toLowerCase()
    if (!term) return leftItems
    return leftItems.filter((item) => matchesSearch(item, term))
  }, [leftItems, leftSearch])

  const filteredRight = React.useMemo(() => {
    const term = rightSearch.trim().toLowerCase()
    if (!term) return rightItems
    return rightItems.filter((item) => matchesSearch(item, term))
  }, [rightItems, rightSearch])

  const toggleCheck = (
    set: React.Dispatch<React.SetStateAction<Set<string>>>,
    id: string
  ) => {
    set((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const moveToRight = (ids?: Set<string>) => {
    const idsToMove = ids ? Array.from(ids) : filteredLeft.map((i) => i.id)
    const availableIds = new Set(filteredLeft.map((i) => i.id))
    const validIds = idsToMove.filter((id) => availableIds.has(id))
    if (validIds.length === 0) return
    onChange([...selectedIds, ...validIds])
    setLeftChecked((prev) => {
      const next = new Set(prev)
      validIds.forEach((id) => next.delete(id))
      return next
    })
  }

  const moveToLeft = (ids?: Set<string>) => {
    const idsToRemove = ids ? Array.from(ids) : filteredRight.map((i) => i.id)
    const removeSet = new Set(idsToRemove)
    if (removeSet.size === 0) return
    onChange(selectedIds.filter((id) => !removeSet.has(id)))
    setRightChecked((prev) => {
      const next = new Set(prev)
      idsToRemove.forEach((id) => next.delete(id))
      return next
    })
  }

  const ListBox = ({
    items: listItems,
    checked,
    onToggle,
  }: {
    items: TransferListItem[]
    checked: Set<string>
    onToggle: (id: string) => void
  }) => (
    <div className={cn("space-y-1", listHeight)}>
      {listItems.length === 0 && (
        <div className="text-muted-foreground py-6 text-center text-xs">
          暂无数据
        </div>
      )}
      {listItems.map((item) => (
        <label
          key={item.id}
          className={cn(
            "flex cursor-pointer items-center gap-2 rounded-md border px-2 py-1.5 transition-colors",
            "hover:bg-accent",
            item.disabled && "pointer-events-none opacity-50"
          )}
        >
          <Checkbox
            checked={checked.has(item.id)}
            onCheckedChange={() => onToggle(item.id)}
          />
          <div className="min-w-0 flex-1">
            <div className="truncate text-xs font-medium">{item.label}</div>
            {item.description && (
              <div className="text-muted-foreground truncate text-[10px]">
                {item.description}
              </div>
            )}
            {item.secondary && (
              <div className="text-muted-foreground/70 truncate text-[10px]">
                {item.secondary}
              </div>
            )}
          </div>
        </label>
      ))}
    </div>
  )

  return (
    <div className={cn("grid grid-cols-[1fr_auto_1fr] gap-2", className)}>
      {/* 左侧候选池 */}
      <div className="flex flex-col gap-2 rounded-md border bg-card p-2">
        <div className="flex items-center justify-between px-1">
          <span className="text-xs font-medium">{titleLeft}</span>
          <span className="text-muted-foreground text-[10px]">
            {filteredLeft.length}/{leftItems.length}
          </span>
        </div>
        <Input
          value={leftSearch}
          onChange={(e) => setLeftSearch(e.target.value)}
          placeholder={leftSearchPlaceholder}
          className="h-7 text-xs"
        />
        <ScrollPage variant="borderless" scrollbarVisible="auto" className="flex-1">
          <ListBox
            items={filteredLeft}
            checked={leftChecked}
            onToggle={(id) => toggleCheck(setLeftChecked, id)}
          />
        </ScrollPage>
      </div>

      {/* 中间操作按钮 */}
      <div className="flex flex-col justify-center gap-1">
        <Button
          type="button"
          variant="outline"
          size="icon"
          className="size-7"
          onClick={() => moveToRight(leftChecked)}
          disabled={leftChecked.size === 0}
        >
          <i className="fa-solid fa-angle-right text-xs" />
        </Button>
        <Button
          type="button"
          variant="outline"
          size="icon"
          className="size-7"
          onClick={() => moveToLeft(rightChecked)}
          disabled={rightChecked.size === 0}
        >
          <i className="fa-solid fa-angle-left text-xs" />
        </Button>
      </div>

      {/* 右侧已选项 */}
      <div className="flex flex-col gap-2 rounded-md border bg-card p-2">
        <div className="flex items-center justify-between px-1">
          <span className="text-xs font-medium">{titleRight}</span>
          <span className="text-muted-foreground text-[10px]">
            {filteredRight.length}/{rightItems.length}
          </span>
        </div>
        <Input
          value={rightSearch}
          onChange={(e) => setRightSearch(e.target.value)}
          placeholder={rightSearchPlaceholder}
          className="h-7 text-xs"
        />
        <ScrollPage variant="borderless" scrollbarVisible="auto" className="flex-1">
          <ListBox
            items={filteredRight}
            checked={rightChecked}
            onToggle={(id) => toggleCheck(setRightChecked, id)}
          />
        </ScrollPage>
      </div>
    </div>
  )
}
