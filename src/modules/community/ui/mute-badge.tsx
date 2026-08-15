/**
 * 禁言徽章（D12）
 *
 * 在作者昵称旁展示「已禁言」标识。仅当 `muted = true` 时渲染；
 * 提供 `until`（到期时间）时，悬停提示剩余时长（永久禁言 until 为 null）。
 */

import { useTranslation } from '@/modules/i18n/use-translation'
import { Badge } from '@/components/ui/badge'

export interface MuteBadgeProps {
  /** 是否处于禁言状态 */
  muted: boolean
  /** 禁言到期时间（UTC ISO）；null = 永久禁言 */
  until?: string | null
}

/** 计算到期剩余的人类可读文案（直到格式化为「永久」或「X 天 X 小时」） */
function formatUntilUntil(until: string | null | undefined, t: (k: string, o?: Record<string, unknown>) => string): string {
  if (!until) return t('badge.muteForever')
  const end = new Date(until).getTime()
  if (Number.isNaN(end)) return t('badge.muteForever')
  const diff = end - Date.now()
  if (diff <= 0) return t('badge.muteForever')
  const hour = 60 * 60 * 1000
  const day = 24 * hour
  const days = Math.floor(diff / day)
  const hours = Math.floor((diff % day) / hour)
  if (days > 0) return t('badge.muteRemainDays', { days, hours })
  return t('badge.muteRemainHours', { hours })
}

export function MuteBadge({ muted, until }: MuteBadgeProps) {
  const { t } = useTranslation('community')
  if (!muted) return null
  return (
    <Badge
      variant="outline"
      className="border-amber-500/40 bg-amber-500/10 text-amber-600 h-4 shrink-0 gap-0.5 px-1 text-[10px]"
      title={t('badge.muteTip', { remain: formatUntilUntil(until, t) })}
    >
      <i className="fa-solid fa-volume-xmark size-2" />
      {t('badge.muted')}
    </Badge>
  )
}