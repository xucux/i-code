/**
 * 签到排行列表（个人栏「签到排行」入口切换的左列视图）
 *
 * 顶部在「累计签到 / 连续签到」两个子维度间切换，各自展示对应排行的
 * 前段用户（offset 分页，共用同一分页状态）。数据来自 Worker 侧
 * `checkins/leaderboard`（累计 = check_ins 计数；连续 = 当前仍连续天数），
 * 均过滤封禁用户，禁言用户仍展示。
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { getCommunityCheckInLeaderboard } from '@/hooks/use-community'
import { getCommunityAvatar } from '@/modules/community/avatars'
import type { CheckInLeaderboardItem } from '@/modules/community/types'
import { LoadingRows } from './post-list'

/** 分页大小 */
const PAGE_SIZE = 20

/** 排行维度（累计签到 / 连续签到） */
type CheckInDimension = 'total' | 'streak'

export function CheckInLeaderboardList() {
  const { t } = useTranslation('community')

  const [dimension, setDimension] = useState<CheckInDimension>('total')
  const [total, setTotal] = useState<CheckInLeaderboardItem[]>([])
  const [streak, setStreak] = useState<CheckInLeaderboardItem[]>([])
  const [nextOffset, setNextOffset] = useState<number | null>(null)
  const [hasMore, setHasMore] = useState(false)
  const [loading, setLoading] = useState(true)
  const [loadingMore, setLoadingMore] = useState(false)
  // 避免开发模式 StrictMode 双执行重复请求
  const initialized = useRef(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const data = await getCommunityCheckInLeaderboard(0, PAGE_SIZE)
      setTotal(data.total)
      setStreak(data.streak)
      setNextOffset(data.nextOffset)
      setHasMore(data.nextOffset != null)
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    if (initialized.current) return
    initialized.current = true
    void load()
  }, [load])

  const loadMore = useCallback(async () => {
    if (nextOffset == null || loadingMore) return
    setLoadingMore(true)
    try {
      const data = await getCommunityCheckInLeaderboard(nextOffset, PAGE_SIZE)
      setTotal((prev) => [...prev, ...data.total])
      setStreak((prev) => [...prev, ...data.streak])
      setNextOffset(data.nextOffset)
      setHasMore(data.nextOffset != null)
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setLoadingMore(false)
    }
  }, [nextOffset, loadingMore])

  if (loading) return <LoadingRows />

  // 当前维度有效数据（累计 / 连续任一非空即视为有排行）
  const items = dimension === 'total' ? total : streak

  if (items.length === 0) {
    return (
      <div className="text-muted-foreground flex h-32 flex-col items-center justify-center gap-2 text-xs">
        <i className="fa-solid fa-calendar-check size-6 opacity-50" />
        {t('leaderboard.empty')}
      </div>
    )
  }

  return (
    <div className="space-y-2 pb-10">
      {/* 子维度切换：累计签到 / 连续签到 */}
      <div className="flex gap-1.5">
        {(['total', 'streak'] as const).map((d) => (
          <button
            key={d}
            type="button"
            onClick={() => setDimension(d)}
            className={cn(
              'flex h-8 flex-1 items-center justify-center gap-1.5 rounded-md border px-2 text-xs transition-colors',
              dimension === d
                ? 'border-primary bg-primary text-primary-foreground'
                : 'bg-background text-muted-foreground hover:bg-muted hover:text-foreground'
            )}
          >
            <i className={cn('size-2.5', d === 'total' ? 'fa-solid fa-calendar-check' : 'fa-solid fa-fire')} />
            <div className='flex items-center gap-1.5'>
            {t(d === 'total' ? 'checkin.total' : 'checkin.streak')}
            {d === 'streak' && (
              <span className="text-[9px]  leading-none opacity-80">{t('checkin.streakHint')}</span>
            )}
            </div>

          </button>
        ))}
      </div>

      {items.map((item) => (
        <div key={item.userId} className="hover:bg-accent/50 flex items-center gap-2.5 rounded-lg border bg-card p-2.5">
          {/* 名次徽标（前 3 名高亮） */}
          <span className={rankBadgeClass(item.rank)} title={t('leaderboard.rank', { rank: item.rank })}>
            {item.rank}
          </span>
          <div className="bg-muted flex size-8 shrink-0 items-center justify-center rounded-full border text-base">
            {getCommunityAvatar(item.avatarIndex)}
          </div>
          <div className="min-w-0 flex-1">
            <p className="truncate text-xs font-medium">{item.nickname}</p>
            <p className="text-muted-foreground text-[11px] tabular-nums">
              {dimension === 'total'
                ? t('checkin.totalValue', { count: item.totalCheckIns ?? 0 })
                : t('checkin.streakValue', { count: item.streakDays ?? 0 })}
            </p>
          </div>
        </div>
      ))}

      {hasMore ? (
        <div className="flex justify-center pt-1">
          <Button variant="outline" size="sm" className="h-7 text-xs" disabled={loadingMore} onClick={() => void loadMore()}>
            {loadingMore && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
            {t('action.loadMore')}
          </Button>
        </div>
      ) : (
        <div className="flex justify-center pt-1">
          <Button variant="ghost" size="sm" className="text-muted-foreground h-7 text-xs" onClick={() => void load()}>
            <i className="fa-solid fa-rotate mr-1.5 size-3" />
            {t('action.refresh')}
          </Button>
        </div>
      )}
    </div>
  )
}

/** 名次徽标样式：前 3 名按金银铜高亮，其余 muted */
function rankBadgeClass(rank: number): string {
  const base = 'flex size-6 shrink-0 items-center justify-center rounded-full text-[11px] font-semibold tabular-nums '
  if (rank === 1) return base + 'bg-amber-100 text-amber-700'
  if (rank === 2) return base + 'bg-slate-200 text-slate-600'
  if (rank === 3) return base + 'bg-orange-100 text-orange-700'
  return base + 'bg-muted text-muted-foreground'
}