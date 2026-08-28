/**
 * 消息通知列表弹层（2026-08-23 通知迭代）
 *
 * - 打开时自动「全部已读」（需求 2：点击消息按钮进入通知列表即解除所有未读），
 *   并通过 `onUnreadCleared` 通知父级清空小红点；
 * - reply / tip 类通知可点击跳转到对应帖子（/community/post/:id）；ban / mute 类仅展示；
 * - 列表游标分页，底部「加载更多」。
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { cn } from '@/lib/utils'
import { formatDateTime } from '@/core/utils'
import {
  formatCommunityTime,
  getCommunityNotifications,
  markCommunityNotificationsRead,
} from '@/hooks/use-community'
import type { NotificationItem } from '@/modules/community/types'

/** 通知类型 → 图标与配色 */
function notificationVisual(type: NotificationItem['type']): { icon: string; className: string } {
  switch (type) {
    case 'reply':
      return { icon: 'fa-reply', className: 'text-primary' }
    case 'ban':
      return { icon: 'fa-ban', className: 'text-destructive' }
    case 'mute':
      return { icon: 'fa-volume-xmark', className: 'text-muted-foreground' }
    case 'tip':
      // 打赏迭代（2026-08-28）：礼物图标，主题色
      return { icon: 'fa-gift', className: 'text-primary' }
  }
}

/** 单条通知行：图标 + 标题 + 正文 + 时间；未读展示圆点；reply / tip 类可跳转 */
function NotificationRow({
  item,
  onNavigate,
}: {
  item: NotificationItem
  /** 点击可跳转通知前回调（父级用于关闭通知弹窗） */
  onNavigate?: () => void
}) {
  const { t } = useTranslation('community')
  const { icon, className } = notificationVisual(item.type)
  const time = formatCommunityTime(item.createdAt, t)

  // 标题：reply = 回复人昵称 + 帖子标题；tip = 打赏人昵称 + 金额（+ 帖子标题）；ban / mute = 固定文案
  const title =
    item.type === 'reply'
      ? item.postTitle
        ? t('notifications.replyTitle', { name: item.actorNickname ?? '', post: item.postTitle })
        : t('notifications.replyTitleNoPost', { name: item.actorNickname ?? '' })
      : item.type === 'tip'
        ? item.postTitle
          ? t('notifications.tipTitle', { name: item.actorNickname ?? '', post: item.postTitle, amount: item.content ?? '0' })
          : t('notifications.tipTitleNoPost', { name: item.actorNickname ?? '', amount: item.content ?? '0' })
        : item.type === 'ban'
          ? t('notifications.banTitle')
          : t('notifications.muteTitle')

  // 正文：reply = 回复预览；tip = 无（金额已入标题）；ban / mute = 原因 + 解除信息
  const body =
    item.type === 'reply'
      ? item.content ?? ''
      : item.type === 'tip'
        ? ''
        : item.type === 'ban'
          ? item.content
            ? t('notifications.reason', { reason: item.content })
            : ''
          : [
              item.content ? t('notifications.reason', { reason: item.content }) : '',
              item.until
                ? t('notifications.muteUntil', { time: formatDateTime(item.until) })
                : t('notifications.muteForever'),
            ]
              .filter(Boolean)
              .join('，')

  const inner = (
    <div className="flex min-w-0 items-start gap-2">
      <i className={cn('fa-solid mt-0.5 size-3', icon, className)} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span
            className={cn(
              'min-w-0 flex-1 truncate text-xs',
              item.isRead ? 'text-muted-foreground' : 'font-medium text-foreground'
            )}
          >
            {title}
          </span>
          {!item.isRead && <span className="size-1.5 shrink-0 rounded-full bg-primary" />}
        </div>
        {body && (
          <p className="text-muted-foreground mt-0.5 line-clamp-2 break-words text-[11px] leading-relaxed">{body}</p>
        )}
        <p className="text-muted-foreground mt-0.5 text-[10px] tabular-nums">{time}</p>
      </div>
    </div>
  )

  // reply / tip 类且帖子仍存在 → 点击跳转详情（先关闭通知弹窗）；否则纯展示
  if ((item.type === 'reply' || item.type === 'tip') && item.postId != null) {
    return (
      <Link
        to="/community/post/$id"
        params={{ id: String(item.postId) }}
        onClick={() => onNavigate?.()}
        className="block rounded-md px-1.5 py-1.5 transition-colors hover:bg-muted"
      >
        {inner}
      </Link>
    )
  }
  return <div className="px-1.5 py-1.5">{inner}</div>
}

export interface NotificationListDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 全部已读成功后回调（父级清空小红点计数） */
  onUnreadCleared: () => void
}

export function NotificationListDialog({
  open,
  onOpenChange,
  onUnreadCleared,
}: NotificationListDialogProps) {
  const { t } = useTranslation('community')
  const [items, setItems] = useState<NotificationItem[]>([])
  const [cursor, setCursor] = useState<string | null>(null)
  const [hasMore, setHasMore] = useState(false)
  const [loading, setLoading] = useState(false)
  const [loadingMore, setLoadingMore] = useState(false)
  const [error, setError] = useState<string | null>(null)
  // 全部已读标记（每次打开执行一次），避免 StrictMode 双执行重复请求
  const clearedRef = useRef(false)

  /** 加载更多（游标分页） */
  const loadMore = useCallback(async () => {
    if (!cursor || loadingMore) return
    setLoadingMore(true)
    try {
      const data = await getCommunityNotifications(cursor)
      setItems((prev) => [...prev, ...data.items])
      setCursor(data.nextCursor)
      setHasMore(data.nextCursor != null)
    } catch {
      // 加载更多失败静默（可再次点击重试）
    } finally {
      setLoadingMore(false)
    }
  }, [cursor, loadingMore])

  // 打开时：先「全部已读」（需求 2：进入通知列表即解除所有未读），再加载列表保证未读圆点一致
  useEffect(() => {
    if (!open) return
    let cancelled = false
    clearedRef.current = false
    const init = async () => {
      if (!clearedRef.current) {
        clearedRef.current = true
        try {
          const updated = await markCommunityNotificationsRead()
          if (updated > 0 && !cancelled) onUnreadCleared()
        } catch {
          // 已读标记失败不阻塞浏览（下次打开重试）
        }
      }
      if (cancelled) return
      setLoading(true)
      setError(null)
      try {
        const data = await getCommunityNotifications()
        if (cancelled) return
        setItems(data.items)
        setCursor(data.nextCursor)
        setHasMore(data.nextCursor != null)
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e))
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    void init()
    return () => {
      cancelled = true
    }
  }, [open, onUnreadCleared])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex h-[65vh] max-w-md flex-col gap-3 p-4">
        <DialogHeader>
          <DialogTitle className="text-sm">
            <i className="fa-solid fa-bell mr-1.5 size-3" />
            {t('notifications.title')}
          </DialogTitle>
        </DialogHeader>

        <div className="custom-scrollbar min-h-0 flex-1 overflow-y-auto pr-1">
          {loading ? (
            <div className="text-muted-foreground flex h-24 items-center justify-center gap-2 text-xs">
              <i className="fa-solid fa-spinner fa-spin size-3.5" />
              {t('loadError.loading')}
            </div>
          ) : error ? (
            <p className="text-muted-foreground text-center text-xs">{error}</p>
          ) : items.length === 0 ? (
            <div className="text-muted-foreground flex h-24 flex-col items-center justify-center gap-1.5 text-xs">
              <i className="fa-solid fa-bell-slash size-4" />
              {t('notifications.empty')}
            </div>
          ) : (
            <div className="space-y-0.5">
              {items.map((item) => (
                <NotificationRow
                  key={item.notificationId}
                  item={item}
                  onNavigate={() => onOpenChange(false)}
                />
              ))}
              {hasMore && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="mt-1 h-7 w-full text-xs"
                  disabled={loadingMore}
                  onClick={() => void loadMore()}
                >
                  {loadingMore && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
                  {t('notifications.loadMore')}
                </Button>
              )}
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" size="sm" className="h-8 text-xs" onClick={() => onOpenChange(false)}>
            {t('post.cancel')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
