import { useState, useCallback } from 'react'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { TitleBarInfo, type TitleBarInfoItem } from '@/components/ui/title-bar-info'
import { MemoryInfo } from '@/components/ui/memory-info'

// 配置项初始数据：模拟标题栏可展示的信息项
const initialItems: TitleBarInfoItem[] = [
  { icon: 'coins', label: 'Tokens', value: 12846, active: true },
  { icon: 'bolt', label: 'RPM', value: 42, active: false },
  { icon: 'clock', label: 'Latency', value: '120ms', active: false },
]

/**
 * 标题栏信息配置组件（组件库示例）
 * 用于演示如何配置标题栏中间展示的信息项，不依赖业务数据。
 * 提供启用/禁用、修改数值、重置等交互能力。
 */
export function TitleBarConfig() {
  const [items, setItems] = useState<TitleBarInfoItem[]>(initialItems)
  // 是否启用内存信息展示
  const [memoryEnabled, setMemoryEnabled] = useState(true)

  // 切换信息项的启用状态
  const toggleItem = useCallback((index: number) => {
    setItems((prev) =>
      prev.map((item, i) => (i === index ? { ...item, active: !item.active } : item))
    )
  }, [])

  // 修改信息项的展示数值
  const updateValue = useCallback((index: number, value: string) => {
    setItems((prev) =>
      prev.map((item, i) => (i === index ? { ...item, value } : item))
    )
  }, [])

  // 重置为初始状态
  const resetItems = useCallback(() => {
    setItems(initialItems)
    setMemoryEnabled(true)
  }, [])

  // 过滤出当前启用的信息项，用于标题栏展示
  const activeItems = items.filter((item) => item.active)

  return (
    <div className="space-y-4">
      {/* 标题栏信息展示预览：普通信息项 + 内存信息 */}
      <div className="flex h-9 items-center justify-center gap-4 rounded-md border bg-muted/30">
        <TitleBarInfo items={activeItems} />
        <MemoryInfo enabled={memoryEnabled} />
      </div>

      {/* 配置面板 */}
      <div className="space-y-3 rounded-md border p-3">
        <div className="flex items-center justify-between">
          <h4 className="text-sm font-medium">标题栏信息配置</h4>
          <Button variant="ghost" size="sm" className="h-7 text-xs" onClick={resetItems}>
            重置
          </Button>
        </div>

        {items.map((item, index) => (
          <div key={item.label} className="flex items-center gap-3">
            <Switch
              id={`title-bar-info-${index}`}
              checked={item.active}
              onCheckedChange={() => toggleItem(index)}
            />
            <Label
              htmlFor={`title-bar-info-${index}`}
              className={cn('flex flex-1 items-center gap-2 text-xs', !item.active && 'opacity-50')}
            >
              <i className={cn('fa-solid', `fa-${item.icon}`, 'size-3.5')} />
              {item.label}
            </Label>
            <Input
              value={String(item.value)}
              onChange={(e) => updateValue(index, e.target.value)}
              disabled={!item.active}
              className="h-7 w-28 text-xs"
            />
          </div>
        ))}

        {/* 内存信息开关 */}
        <div className="flex items-center gap-3">
          <Switch
            id="title-bar-memory"
            checked={memoryEnabled}
            onCheckedChange={setMemoryEnabled}
          />
          <Label
            htmlFor="title-bar-memory"
            className={cn('flex flex-1 items-center gap-2 text-xs', !memoryEnabled && 'opacity-50')}
          >
            <i className="fa-solid fa-memory size-3.5" />
            内存占用
          </Label>
        </div>
      </div>
    </div>
  )
}
