/**
 * 积分排行列表（个人栏「积分排行」入口切换的左列视图）
 *
 * 展示所有用户按累计积分降序的排行（offset 分页），支持加载更多 / 刷新。
 * Worker 侧聚合 `points_ledger`，过滤封禁用户，禁言用户仍展示。
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { getCommunityPointsLeaderboard } from '@/hooks/use-community'
import { getCommunityAvatar } from '@/modules/community/avatars'
import type { PointsLeaderboardItem } from '@/modules/community/types'
import { LoadingRows } from './post-list'

/** 分页大小 */
const PAGE_SIZE = 20

export function LeaderboardList() {
  const { t } = useTranslation('community')

  const [items, setItems] = useState<PointsLeaderboardItem[]>([])
  const [nextOffset, setNextOffset] = useState<number | null>(null)
  const [hasMore, setHasMore] = useState(false)
  const [loading, setLoading] = useState(true)
  const [loadingMore, setLoadingMore] = useState(false)
  // 避免开发模式 StrictMode 双执行重复请求
  const initialized = useRef(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const data = await getCommunityPointsLeaderboard(0, PAGE_SIZE)
      setItems(data.items)
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
      const data = await getCommunityPointsLeaderboard(nextOffset, PAGE_SIZE)
      setItems((prev) => [...prev, ...data.items])
      setNextOffset(data.nextOffset)
      setHasMore(data.nextOffset != null)
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setLoadingMore(false)
    }
  }, [nextOffset, loadingMore])

  if (loading) return <LoadingRows />

  if (items.length === 0) {
    return (
      <div className="text-muted-foreground flex h-32 flex-col items-center justify-center gap-2 text-xs">
        <i className="fa-solid fa-trophy size-6 opacity-50" />
        {t('leaderboard.empty')}
      </div>
    )
  }

  return (
    <div className="space-y-2">
      {items.map((item) => (
        <div key={item.userId} className="hover:bg-accent/50 flex items-center gap-2.5 rounded-lg border bg-card p-2.5">
          {/* 名次徽标（前 3 名高亮） */}
          <span
            className={rankBadgeClass(item.rank)}
            title={t('leaderboard.rank', { rank: item.rank })}
          >
            {item.rank}
          </span>
          <div className="bg-muted flex size-8 shrink-0 items-center justify-center rounded-full border text-base">
            {getCommunityAvatar(item.avatarIndex)}
          </div>
          <div className="min-w-0 flex-1">
            <p className="truncate text-xs font-medium">{item.nickname}</p>
            <p className="text-muted-foreground text-[11px] tabular-nums">
              {t('leaderboard.points', { points: item.points })}
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