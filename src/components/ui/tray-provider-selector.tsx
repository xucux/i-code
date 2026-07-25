import { useState } from 'react'
import { cn } from '@/lib/utils'

export interface TrayProvider {
  id: string
  name: string
}

interface TrayProviderSelectorProps {
  /** 可选供应商列表 */
  providers: TrayProvider[]
  /** 当前选中的供应商 ID */
  value?: string
  /** 选中变化回调 */
  onChange?: (providerId: string) => void
  className?: string
}

/**
 * 托盘供应商选择组件
 * 模拟系统托盘菜单中的供应商切换列表，纯 UI 组件，数据与回调由外部传入。
 */
export function TrayProviderSelector({
  providers,
  value,
  onChange,
  className,
}: TrayProviderSelectorProps) {
  const [selected, setSelected] = useState(value ?? providers[0]?.id)

  const handleSelect = (id: string) => {
    setSelected(id)
    onChange?.(id)
  }

  return (
    <div className={cn('w-56 rounded-md border bg-popover p-1 text-popover-foreground shadow-md', className)}>
      {providers.map((provider) => (
        <button
          key={provider.id}
          type="button"
          onClick={() => handleSelect(provider.id)}
          className={cn(
            'flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-xs transition-colors',
            selected === provider.id
              ? 'bg-accent text-accent-foreground'
              : 'hover:bg-muted'
          )}
        >
          <i
            className={cn(
              'fa-solid fa-check h-3 w-3',
              selected === provider.id ? 'opacity-100' : 'opacity-0'
            )}
          />
          {provider.name}
        </button>
      ))}
    </div>
  )
}
