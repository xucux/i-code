/**
 * 帖子打赏信息折叠区域（2026-08-28 打赏迭代）
 *
 * 仅当帖子存在打赏（tipCount > 0）时展示，默认折叠；
 * 折叠态头部显示「X 人打赏 · 共 Y 积分」，展开后按需拉取打赏人列表
 * （实名展示：头像 / 昵称 / 禁言态 + 金额 + 时间，游标分页）。
 * 不随分享直链（/s/:pid）展示。
 */

import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { getCommunityAvatar } from '@/modules/community/avatars'
import { formatCommunityTime, getCommunityPostTips } from '@/hooks/use-community'
import { MuteBadge } from './mute-badge'
import { cn } from '@/lib/utils'
import type { PostTipListData } from '@/modules/community/types'

export interface PostTipSectionProps {
  postId: number
  /** 打赏人数（来自帖子详情 posts.tip_count） */
  tipCount: number
  /** 打赏总额（来自帖子详情 posts.tip_amount） */
  tipAmount: number
}

/** 打赏人列表单页条数（与 Worker 分页默认一致） */
const TIPS_PAGE_SIZE = 20

export function PostTipSection({ postId, tipCount, tipAmount }: PostTipSectionProps) {
  const { t } = useTranslation('community')
  // 默认折叠（需求：打赏信息区域默认折叠）
  const [expanded, setExpanded] = useState(false)
  // 打赏人列表（展开后按需加载）
  const [data, setData] = useState<PostTipListData | null>(null)
  const [loading, setLoading] = useState(false)
  const [loadingMore, setLoadingMore] = useState(false)

  /** 拉取打赏列表第一页（成功即覆盖；失败保留旧数据 + 静默，展开态下次自动重试） */
  const load = useCallback(async () => {
    if (data) return
    setLoading(true)
    try {
      setData(await getCommunityPostTips(postId, undefined, TIPS_PAGE_SIZE))
    } catch {
      // 拉取失败静默（头部汇总信息仍可看，展开区显示占位）
    } finally {
      setLoading(false)
    }
  }, [postId, data])

  // 展开时按需加载第一页
  useEffect(() => {
    if (expanded) void load()
  }, [expanded, load])

  const loadMore = async () => {
    if (!data?.nextCursor || loadingMore) return
    setLoadingMore(true)
    try {
      const next = await getCommunityPostTips(postId, data.nextCursor, TIPS_PAGE_SIZE)
      setData({ items: [...data.items, ...next.items], nextCursor: next.nextCursor })
    } catch {
      toast.error(t('loadError.listing'))
    } finally {
      setLoadingMore(false)
    }
  }

  // 折叠区无内容（tipCount=0）时不渲染
  if (tipCount <= 0) return null

  return (
    <div className="mt-2.5 rounded-md border border-dashed bg-muted/30">
      {/* 折叠头（点击展开 / 收起） 将元素基线与父元素基线对齐‌ */}
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full  align-baseline gap-2 px-2 py-2 text-left transition-colors hover:bg-muted/60"
      >
        <i className="fa-solid fa-hand-holding-heart text-muted-foreground size-3 shrink-0 mr-2" />
        <span className="text-xs font-medium">
          {t('tips.summary', { count: tipCount, amount: tipAmount })}
        </span>
        {/* 向下箭头icon */}
        <i
          className={cn(
            'text-muted-foreground ml-auto size-2.5 mr-1 transition-transform',
            expanded ? 'fa-solid fa-chevron-up' : 'fa-solid fa-chevron-down'
          )}
        />
      </button>

      {/* 展开内容：打赏人列表（按需加载） */}
      {expanded && (
        <div className="border-t border-dashed border-muted px-2.5 pb-2 pt-1.5">
          {loading && !data ? (
            <div className="text-muted-foreground flex h-12 items-center justify-center gap-2 text-xs">
              <i className="fa-solid fa-spinner fa-spin size-3 mr-2" />
              {t('loadError.loading')}
            </div>
          ) : data && data.items.length === 0 ? (
            <p className="text-muted-foreground py-2 text-center text-[11px]">{t('tips.empty')}</p>
          ) : (
            <>
              <div className="space-y-1.5">
                {(data?.items ?? []).map((item) => (
                  <div key={item.author.userId} className="flex items-center gap-1.5 text-xs">
                    <span className="text-sm leading-none">{getCommunityAvatar(item.author.avatarIndex)}</span>
                    <span className="max-w-28 truncate font-medium">{item.author.nickname}</span>
                    <MuteBadge muted={item.author.muted} />
                    <span className="text-muted-foreground text-[10px] tabular-nums">
                      {formatCommunityTime(item.createdAt, t)}
                    </span>
                    <span className="ml-auto flex items-center gap-1 font-medium tabular-nums">
                      <i className="fa-solid fa-coins text-muted-foreground size-3 shrink-0" />
                      {item.amount}
                    </span>
                  </div>
                ))}
              </div>
              {data?.nextCursor && (
                <button
                  type="button"
                  onClick={() => void loadMore()}
                  disabled={loadingMore}
                  className="text-muted-foreground hover:text-foreground mx-auto mt-1.5 flex items-center gap-1 text-[11px] transition-colors"
                >
                  {loadingMore ? (
                    <i className="fa-solid fa-spinner fa-spin size-2.5" />
                  ) : (
                    <i className="fa-solid fa-ellipsis size-2.5" />
                  )}
                  {t('action.loadMore')}
                </button>
              )}
            </>
          )}
        </div>
      )}
    </div>
  )
}