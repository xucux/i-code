/**
 * 自动刷新组件
 *
 * 提供两种变体：
 * - AutoRefreshToggle：单开关，启用/禁用固定间隔刷新
 * - AutoRefreshSelect：下拉选择刷新间隔，支持多种预设间隔
 *
 * 设计原则：极简紧凑，不占用过多空间。
 */

import { useEffect, useCallback } from 'react'
import { cn } from '@/lib/utils'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

// ===== 公共类型 =====

/** 刷新间隔（毫秒），null 表示关闭 */
export type RefreshInterval = number | null

/** 获取当前语言下的预设刷新间隔选项 */
export function useRefreshOptions(): { label: string; value: RefreshInterval }[] {
  const { t } = useTranslation('autoRefresh')
  return [
    { label: t('off'), value: null },
    { label: t('seconds', { count: 2 }), value: 2000 },
    { label: t('seconds', { count: 5 }), value: 5000 },
    { label: t('seconds', { count: 15 }), value: 15000 },
    { label: t('seconds', { count: 30 }), value: 30000 },
    { label: t('minutes', { count: 1 }), value: 60000 },
  ]
}

// ===== Hook: useAutoRefresh =====

interface UseAutoRefreshOptions {
  /** 刷新回调 */
  onRefresh: () => void | Promise<void>
  /** 刷新间隔（毫秒），null/0/undefined 关闭 */
  intervalMs?: RefreshInterval
  /** 是否启用（仅用于 toggle 模式） */
  enabled?: boolean
}

/**
 * 自动刷新 Hook
 *
 * 管理定时器生命周期，intervalMs/enabled 变化时自动重置。
 */
export function useAutoRefresh({
  onRefresh,
  intervalMs,
  enabled = true,
}: UseAutoRefreshOptions) {
  const refresh = useCallback(() => {
    void onRefresh()
  }, [onRefresh])

  useEffect(() => {
    if (!enabled || !intervalMs || intervalMs <= 0) return
    const timer = setInterval(refresh, intervalMs)
    return () => clearInterval(timer)
  }, [refresh, intervalMs, enabled])

  return { refresh }
}

// ===== AutoRefreshToggle =====

export interface AutoRefreshToggleProps {
  /** 是否启用自动刷新 */
  checked: boolean
  /** 切换回调 */
  onCheckedChange: (checked: boolean) => void
  /** 自定义类名 */
  className?: string
  /** 标签文字，默认"自动刷新" */
  label?: string
}

/**
 * 自动刷新单开关组件
 *
 * 极简设计：刷新图标 + Switch，一行展示。
 * 适用于日志页等场景。
 */
export function AutoRefreshToggle({
  checked,
  onCheckedChange,
  className,
  label,
}: AutoRefreshToggleProps) {
  const { t } = useTranslation('autoRefresh')
  const finalLabel = label ?? t('label')
  return (
    <div className={cn('flex items-center gap-1.5', className)}>
      <i
        className={cn(
          'fa-solid fa-rotate text-[10px] transition-colors',
          checked ? 'text-primary' : 'text-muted-foreground'
        )}
      />
      <Switch
        id="auto-refresh-toggle"
        checked={checked}
        onCheckedChange={onCheckedChange}
        className="scale-75"
      />
      <Label htmlFor="auto-refresh-toggle" className="text-xs cursor-pointer">
        {finalLabel}
      </Label>
    </div>
  )
}

// ===== AutoRefreshSelect =====

export interface AutoRefreshSelectProps {
  /** 当前刷新间隔（毫秒），null 表示关闭 */
  value: RefreshInterval
  /** 间隔变更回调 */
  onValueChange: (value: RefreshInterval) => void
  /** 可选选项列表 */
  options?: { label: string; value: RefreshInterval }[]
  /** 自定义类名 */
  className?: string
}

/**
 * 自动刷新下拉选择组件
 *
 * 极简设计：刷新图标 + 紧凑 Select，仅占用 ~24px 高度。
 * 适用于统计页等需要灵活选择间隔的场景。
 */
export function AutoRefreshSelect({
  value,
  onValueChange,
  options,
  className,
}: AutoRefreshSelectProps) {
  const defaultOptions = useRefreshOptions()
  const finalOptions = options ?? defaultOptions

  return (
    <div className={cn('flex items-center gap-1', className)}>
      {/* <i
        className={cn(
          'fa-solid fa-rotate text-[10px] transition-colors',
          value != null && value > 0 ? 'text-primary' : 'text-muted-foreground'
        )}
      /> */}
      <Select
        value={value?.toString() ?? 'null'}
        onValueChange={(v) => {
          onValueChange(v === 'null' ? null : Number(v))
        }}
      >
        <SelectTrigger className="h-7 w-[84px] gap-1 border-none bg-muted/50 px-1.5 text-xs shadow-none hover:bg-muted">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {finalOptions.map((opt) => (
            <SelectItem
              key={opt.label}
              value={opt.value?.toString() ?? 'null'}
              className="text-xs"
            >
              {opt.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  )
}
