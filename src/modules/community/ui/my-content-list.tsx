/**
 * 我的帖子 / 我的回复列表（个人栏入口切换的左列视图，§8.3）
 *
 * 两种内容共用滚动容器：
 * - posts：我的帖子（标题 + 摘要 + 回复数，点击进入详情）
 * - replies：我的回复（内容 + 所在帖子标题，点击进入详情）
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { useNavigate } from '@tanstack/react-router'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import {
  getCommunityMyPosts,
  getCommunityMyReplies,
  formatCommunityTime,
} from '@/hooks/use-community'
import type { MyPostItem, MyReplyItem } from '@/modules/community/types'
import { SectionBadge } from '@/modules/community/ui/section-badge'
import { LoadingRows } from './post-list'

/** 分页大小 */
const PAGE_SIZE = 20

export interface MyContentListProps {
  kind: 'posts' | 'replies'
}

export function MyContentList({ kind }: MyContentListProps) {
  const { t } = useTranslation('community')
  // 绝对路径导航，无需 from（路由重构为目录式后 index 路由 id 为 /community/）
  const navigate = useNavigate()

  const [posts, setPosts] = useState<MyPostItem[]>([])
  const [replies, setReplies] = useState<MyReplyItem[]>([])
  const [cursor, setCursor] = useState<string | null>(null)
  const [hasMore, setHasMore] = useState(false)
  const [loading, setLoading] = useState(true)
  const [loadingMore, setLoadingMore] = useState(false)
  // 避免开发模式 StrictMode 双执行重复请求
  const initialized = useRef(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      if (kind === 'posts') {
        const data = await getCommunityMyPosts(undefined, PAGE_SIZE)
        setPosts(data.posts)
        setCursor(data.nextCursor)
        setHasMore(data.nextCursor != null)
      } else {
        const data = await getCommunityMyReplies(undefined, PAGE_SIZE)
        setReplies(data.replies)
        setCursor(data.nextCursor)
        setHasMore(data.nextCursor != null)
      }
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [kind])

  useEffect(() => {
    if (initialized.current) return
    initialized.current = true
    void load()
  }, [load])

  const loadMore = useCallback(async () => {
    if (!cursor || loadingMore) return
    setLoadingMore(true)
    try {
      if (kind === 'posts') {
        const data = await getCommunityMyPosts(cursor, PAGE_SIZE)
        setPosts((prev) => [...prev, ...data.posts])
        setCursor(data.nextCursor)
        setHasMore(data.nextCursor != null)
      } else {
        const data = await getCommunityMyReplies(cursor, PAGE_SIZE)
        setReplies((prev) => [...prev, ...data.replies])
        setCursor(data.nextCursor)
        setHasMore(data.nextCursor != null)
      }
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setLoadingMore(false)
    }
  }, [kind, cursor, loadingMore])

  const gotoPost = (postId: number) => {
    void navigate({ to: '/community/post/$id', params: { id: String(postId) } })
  }

  if (loading) return <LoadingRows />

  if (kind === 'posts') {
    if (posts.length === 0) return <EmptyTip text={t('post.emptyMy')} />
    return (
      <div className="space-y-2">
        {posts.map((post) => (
          <button
            key={post.postId}
            type="button"
            onClick={() => gotoPost(post.postId)}
            className="hover:bg-accent/50 w-full rounded-lg border bg-card p-3 text-left transition-colors"
          >
            <div className="flex items-start justify-between gap-2">
              <span className="line-clamp-1 text-sm font-medium">{post.title}</span>
              <span className="text-muted-foreground mt-0.5 flex shrink-0 items-center gap-1.5 text-[11px] tabular-nums">
                <SectionBadge section={post.section} />
                {formatCommunityTime(post.createdAt, t)}
              </span>
            </div>
            <p className="text-muted-foreground mt-1 line-clamp-2 text-xs">{post.excerpt}</p>
            <div className="text-muted-foreground mt-2 flex items-center justify-end text-[11px] tabular-nums">
              <i className="fa-solid fa-comment mr-1 size-2.5" />
              {post.replyCount}
            </div>
          </button>
        ))}
        <LoadMoreFooter hasMore={hasMore} loadingMore={loadingMore} onLoadMore={loadMore} onRefresh={load} label={t('action.loadMore')} refreshLabel={t('action.refresh')} />
      </div>
    )
  }

  if (replies.length === 0) return <EmptyTip text={t('post.emptyMyReplies')} />
  return (
    <div className="space-y-2">
      {replies.map((reply) => (
        <button
          key={reply.replyId}
          type="button"
          onClick={() => gotoPost(reply.post.postId)}
          className="hover:bg-accent/50 w-full rounded-lg border bg-card p-3 text-left transition-colors"
        >
          <div className="flex items-center gap-1.5 text-[11px]">
            <i className="fa-solid fa-file-lines text-muted-foreground size-2.5" />
            <span className="text-muted-foreground max-w-52 truncate">{reply.post.title}</span>
            <SectionBadge section={reply.post.section} />
            <span className="text-muted-foreground ml-auto shrink-0 tabular-nums">
              {formatCommunityTime(reply.createdAt, t)}
            </span>
          </div>
          <p className="mt-1.5 line-clamp-3 text-xs leading-relaxed">{reply.content}</p>
        </button>
      ))}
      <LoadMoreFooter hasMore={hasMore} loadingMore={loadingMore} onLoadMore={loadMore} onRefresh={load} label={t('action.loadMore')} refreshLabel={t('action.refresh')} />
    </div>
  )
}

/** 空状态提示 */
function EmptyTip({ text }: { text: string }) {
  return (
    <div className="text-muted-foreground flex h-32 flex-col items-center justify-center gap-2 text-xs">
      <i className="fa-solid fa-inbox size-6 opacity-50" />
      {text}
    </div>
  )
}

/** 底部：加载更多 / 刷新 */
function LoadMoreFooter({
  hasMore,
  loadingMore,
  onLoadMore,
  onRefresh,
  label,
  refreshLabel,
}: {
  hasMore: boolean
  loadingMore: boolean
  onLoadMore: () => void
  onRefresh: () => void
  label: string
  refreshLabel: string
}) {
  if (!hasMore) {
    return (
      <div className="flex justify-center pt-1">
        <Button variant="ghost" size="sm" className="text-muted-foreground h-7 text-xs" onClick={onRefresh}>
          <i className="fa-solid fa-rotate mr-1.5 size-3" />
          {refreshLabel}
        </Button>
      </div>
    )
  }
  return (
    <div className="flex justify-center pt-1">
      <Button variant="outline" size="sm" className="h-7 text-xs" disabled={loadingMore} onClick={onLoadMore}>
        {loadingMore && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
        {label}
      </Button>
    </div>
  )
}
