/**
 * 社区帖子列表（左列主视图，§8.3）
 *
 * 卡片式列表项：标题 + 摘要 + 作者/头像 + 回复数 + 时间；
 * 游标分页，底部「加载更多」。
 */

import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { useNavigate } from '@tanstack/react-router'
import { getCommunityAvatar } from '@/modules/community/avatars'
import { SectionBadge } from '@/modules/community/ui/section-badge'
import { formatCommunityTime } from '@/hooks/use-community'
import type { PostSummary } from '@/modules/community/types'

export interface PostListProps {
  posts: PostSummary[]
  loading: boolean
  loadingMore: boolean
  hasMore: boolean
  onRefresh: () => void
  onLoadMore: () => void
}

export function PostList({ posts, loading, loadingMore, hasMore, onRefresh, onLoadMore }: PostListProps) {
  const { t } = useTranslation('community')
  // 绝对路径导航，无需 from（路由重构为目录式后 index 路由 id 为 /community/）
  const navigate = useNavigate()

  if (loading) {
    return <LoadingRows />
  }

  if (posts.length === 0) {
    return (
      <div className="text-muted-foreground flex h-32 flex-col items-center justify-center gap-2 text-xs">
        <i className="fa-solid fa-inbox size-6 opacity-50" />
        {t('post.empty')}
      </div>
    )
  }

  return (
    <div className="space-y-2 overflow-y-auto">
      {posts.map((post) => (
        <button
          key={post.postId}
          type="button"
          onClick={() => void navigate({ to: '/community/post/$id', params: { id: String(post.postId) } })}
          className="hover:bg-accent/50 w-full rounded-lg border bg-card p-3 text-left transition-colors"
        >
          <div className="flex items-start justify-between gap-2">
            <span className="flex min-w-0 items-center gap-1.5">
              <span className="line-clamp-1 text-sm font-medium">{post.title}</span>
            </span>
            <span className="text-muted-foreground mt-0.5 flex shrink-0 items-center gap-1.5 text-[11px] tabular-nums">
              <SectionBadge section={post.section} />
              {formatCommunityTime(post.createdAt, t)}
            </span>
          </div>
          <p className="text-muted-foreground mt-1 line-clamp-2 text-xs leading-relaxed">{post.excerpt}</p>
          <div className="text-muted-foreground mt-2 flex items-center gap-1.5 text-[11px]">
            <span className="text-sm leading-none">{getCommunityAvatar(post.author.avatarIndex)}</span>
            <span className="max-w-28 truncate">{post.author.nickname}</span>
            <span className="ml-auto flex items-center gap-1 tabular-nums">
              <i className="fa-solid fa-comment size-2.5" />
              {post.replyCount}
            </span>
          </div>
        </button>
      ))}

      {/* 分页：加载更多 / 刷新 */}
      <div className="flex justify-center pt-1">
        {hasMore ? (
          <Button variant="outline" size="sm" className="h-7 text-xs" disabled={loadingMore} onClick={onLoadMore}>
            {loadingMore && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
            {t('action.loadMore')}
          </Button>
        ) : (
          <Button variant="ghost" size="sm" className="text-muted-foreground h-7 text-xs" onClick={onRefresh}>
            <i className="fa-solid fa-rotate mr-1.5 size-3" />
            {t('action.refresh')}
          </Button>
        )}
      </div>
    </div>
  )
}

/** 列表加载占位（骨架行） */
export function LoadingRows({ rows = 4 }: { rows?: number }) {
  return (
    <div className="space-y-2" aria-busy>
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="animate-pulse rounded-lg border bg-card p-3">
          <div className="bg-muted h-4 w-2/5 rounded" />
          <div className="bg-muted mt-2 h-3 w-full rounded" />
          <div className="bg-muted mt-1.5 h-3 w-3/4 rounded" />
        </div>
      ))}
    </div>
  )
}
