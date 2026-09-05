/**
 * 我的帖子 / 我的回复列表（个人栏入口切换的左列视图，§8.3）
 *
 * 两种内容共用滚动容器：
 * - posts：我的帖子（标题 + 摘要 + 回复数，点击进入详情），支持编辑 / 删除
 * - replies：我的回复（内容 + 所在帖子标题，点击进入详情），支持编辑（行内）/ 删除
 *
 * 编辑 / 删除仅作用于本人内容（Worker 侧校验归属）。
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { useNavigate } from '@tanstack/react-router'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { HelpIcon } from '@/components/ui/help-icon'
import { cn } from '@/lib/utils'
import { MarkdownEditor } from './markdown-editor'
import { useDialogFullscreen } from './use-dialog-fullscreen'
import {
  deleteCommunityMyPost,
  deleteCommunityMyReply,
  getCommunityMyPosts,
  getCommunityMyReplies,
  getCommunityPost,
  formatCommunityTime,
  updateCommunityMyPost,
  updateCommunityMyReply,
} from '@/hooks/use-community'
import {
  COMMUNITY_SECTIONS,
  type CommunitySection,
  type MyPostItem,
  type MyReplyItem,
} from '@/modules/community/types'
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

  // 帖子：编辑弹窗目标 / 待确认删除的帖子
  const [editingPostId, setEditingPostId] = useState<number | null>(null)
  const [deletingPostId, setDeletingPostId] = useState<number | null>(null)
  // 回复：行内编辑目标 / 待确认删除的回复
  const [editingReplyId, setEditingReplyId] = useState<number | null>(null)
  const [deletingReplyId, setDeletingReplyId] = useState<number | null>(null)
  const [acting, setActing] = useState(false)

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

  /** 删除帖子（内联二次确认） */
  const handleDeletePost = async () => {
    if (deletingPostId == null || acting) return
    setActing(true)
    try {
      await deleteCommunityMyPost(deletingPostId)
      toast.success(t('my.postDeleted'))
      setDeletingPostId(null)
      await load()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setActing(false)
    }
  }

  /** 删除回复（内联二次确认；顶层评论级联楼中楼） */
  const handleDeleteReply = async () => {
    if (deletingReplyId == null || acting) return
    setActing(true)
    try {
      await deleteCommunityMyReply(deletingReplyId)
      toast.success(t('my.replyDeleted'))
      setDeletingReplyId(null)
      await load()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setActing(false)
    }
  }

  if (loading) return <LoadingRows />

  if (kind === 'posts') {
    if (posts.length === 0) return <EmptyTip text={t('post.emptyMy')} />
    return (
      <>
        <div className="space-y-2">
          {posts.map((post) => (
          <div key={post.postId} className="hover:bg-accent/50 rounded-lg border bg-card p-3 transition-colors">
            <button
              type="button"
              onClick={() => gotoPost(post.postId)}
              className="w-full text-left"
            >
              <div className="flex items-start justify-between gap-2">
                <span className="flex min-w-0 items-center gap-1.5">
                  {post.pinned && (
                    <i className="fa-solid fa-thumbtack text-primary size-2.5" title={t('pinned')} />
                  )}
                  <span className="line-clamp-1 text-sm font-medium">{post.title}</span>
                </span>
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
            <div className="mt-2 flex items-center justify-end gap-1 border-t pt-1.5">
              <Button
                variant="ghost"
                size="sm"
                className="text-muted-foreground hover:text-foreground h-6 px-2 text-[11px]"
                onClick={() => setEditingPostId(post.postId)}
              >
                <i className="fa-solid fa-pen mr-1 size-2.5" />
                {t('my.edit')}
              </Button>
              {deletingPostId === post.postId ? (
                <>
                  <Button
                    variant="destructive"
                    size="sm"
                    className="h-6 px-2 text-[11px]"
                    disabled={acting}
                    onClick={() => void handleDeletePost()}
                  >
                    {t('my.deleteConfirm')}
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-muted-foreground h-6 px-2 text-[11px]"
                    onClick={() => setDeletingPostId(null)}
                  >
                    {t('post.cancel')}
                  </Button>
                </>
              ) : (
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-muted-foreground hover:text-destructive h-6 px-2 text-[11px]"
                  onClick={() => {
                    setDeletingPostId(post.postId)
                    setDeletingReplyId(null)
                  }}
                >
                  <i className="fa-solid fa-trash mr-1 size-2.5" />
                  {t('my.delete')}
                </Button>
              )}
            </div>
          </div>
        ))}
        <LoadMoreFooter hasMore={hasMore} loadingMore={loadingMore} onLoadMore={loadMore} onRefresh={load} label={t('action.loadMore')} refreshLabel={t('action.refresh')} />
        </div>
        {/* 编辑我的帖子弹窗 */}
        <EditMyPostDialog
          postId={editingPostId}
          onOpenChange={(open) => !open && setEditingPostId(null)}
          onSaved={() => void load()}
        />
      </>
    )
  }

  if (replies.length === 0) return <EmptyTip text={t('post.emptyMyReplies')} />
  return (
    <div className="space-y-2">
      {replies.map((reply) => (
        <div key={reply.replyId} className="hover:bg-accent/50 rounded-lg border bg-card p-3 transition-colors">
          <button
            type="button"
            onClick={() => gotoPost(reply.post.postId)}
            className="w-full text-left"
          >
            <div className="flex items-center gap-1.5 text-[11px]">
              <i className="fa-solid fa-file-lines text-muted-foreground size-2.5" />
              <span className="text-muted-foreground max-w-52 truncate">{reply.post.title}</span>
              <SectionBadge section={reply.post.section} />
              <span className="text-muted-foreground ml-auto shrink-0 tabular-nums">
                {formatCommunityTime(reply.createdAt, t)}
              </span>
            </div>
            {editingReplyId === reply.replyId ? null : (
              <p className="mt-1.5 line-clamp-3 text-xs leading-relaxed">{reply.content}</p>
            )}
          </button>

          {editingReplyId === reply.replyId ? (
            <ReplyEditInline
              reply={reply}
              onCancel={() => setEditingReplyId(null)}
              onSaved={async () => {
                setEditingReplyId(null)
                await load()
              }}
            />
          ) : (
            <div className="mt-2 flex items-center justify-end gap-1 border-t pt-1.5">
              <Button
                variant="ghost"
                size="sm"
                className="text-muted-foreground hover:text-foreground h-6 px-2 text-[11px]"
                onClick={() => {
                  setEditingReplyId(reply.replyId)
                  setDeletingReplyId(null)
                }}
              >
                <i className="fa-solid fa-pen mr-1 size-2.5" />
                {t('my.edit')}
              </Button>
              {deletingReplyId === reply.replyId ? (
                <>
                  <Button
                    variant="destructive"
                    size="sm"
                    className="h-6 px-2 text-[11px]"
                    disabled={acting}
                    onClick={() => void handleDeleteReply()}
                  >
                    {t('my.deleteConfirm')}
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-muted-foreground h-6 px-2 text-[11px]"
                    onClick={() => setDeletingReplyId(null)}
                  >
                    {t('post.cancel')}
                  </Button>
                </>
              ) : (
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-muted-foreground hover:text-destructive h-6 px-2 text-[11px]"
                  title={t('my.deleteCascadeHint')}
                  onClick={() => {
                    setDeletingReplyId(reply.replyId)
                    setDeletingPostId(null)
                  }}
                >
                  <i className="fa-solid fa-trash mr-1 size-2.5" />
                  {t('my.delete')}
                </Button>
              )}
            </div>
          )}
        </div>
      ))}
      <LoadMoreFooter hasMore={hasMore} loadingMore={loadingMore} onLoadMore={loadMore} onRefresh={load} label={t('action.loadMore')} refreshLabel={t('action.refresh')} />
    </div>
  )
}

/** 回复行内编辑（保存走后端更新接口，成功后整页刷新列表） */
function ReplyEditInline({
  reply,
  onCancel,
  onSaved,
}: {
  reply: MyReplyItem
  onCancel: () => void
  onSaved: () => Promise<void>
}) {
  const { t } = useTranslation('community')
  const [draft, setDraft] = useState(reply.content)
  const [saving, setSaving] = useState(false)

  const handleSave = async () => {
    const trimmed = draft.trim()
    if (!trimmed || saving) return
    setSaving(true)
    try {
      await updateCommunityMyReply(reply.replyId, trimmed)
      toast.success(t('my.replyUpdated'))
      await onSaved()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="mt-1.5 space-y-1.5">
      <Textarea
        value={draft}
        maxLength={1000}
        placeholder={t('post.contentPlaceholder')}
        onChange={(e) => setDraft(e.target.value)}
        className="min-h-16 text-xs leading-relaxed"
      />
      <div className="flex items-center justify-end gap-1">
        <span className="text-muted-foreground mr-auto text-[10px] tabular-nums">{draft.length}/1000</span>
        <Button
          variant="ghost"
          size="sm"
          className="text-muted-foreground h-6 px-2 text-[11px]"
          onClick={onCancel}
        >
          {t('post.cancel')}
        </Button>
        <Button
          size="sm"
          className="h-6 px-2 text-[11px]"
          disabled={saving || !draft.trim()}
          onClick={() => void handleSave()}
        >
          {saving && <i className="fa-solid fa-spinner fa-spin mr-1 size-2.5" />}
          {t('my.save')}
        </Button>
      </div>
    </div>
  )
}

/** 编辑我的帖子弹窗（打开时异步拉取详情填充，保存走更新接口） */
function EditMyPostDialog({
  postId,
  onOpenChange,
  onSaved,
}: {
  postId: number | null
  onOpenChange: (open: boolean) => void
  onSaved: () => void
}) {
  const { t } = useTranslation('community')
  const isOpen = postId != null
  const [title, setTitle] = useState('')
  const [content, setContent] = useState('')
  const [section, setSection] = useState<CommunitySection>('chat')
  const [loading, setLoading] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  // 系统全屏（参考新建帖子弹窗：Fullscreen API + CSS 回退）
  const { expanded, toggleFullscreen } = useDialogFullscreen(isOpen)

  // 打开时拉取详情填充（列表项仅含截断摘要，需全文）
  useEffect(() => {
    if (!isOpen || postId == null) return
    let cancelled = false
    setLoading(true)
    getCommunityPost(postId)
      .then((data) => {
        if (cancelled) return
        setTitle(data.post.title)
        setContent(data.post.content)
        setSection(data.post.section)
      })
      .catch((e) => {
        toast.error(e instanceof Error ? e.message : String(e))
        onOpenChange(false)
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, postId])

  const titleValid = title.trim().length > 0 && title.trim().length <= 80
  const contentValid = content.trim().length > 0 && content.length <= 10000

  const handleSubmit = async () => {
    if (postId == null || !titleValid || !contentValid || submitting) return
    setSubmitting(true)
    try {
      await updateCommunityMyPost(postId, {
        title: title.trim(),
        content: content.trim(),
        section,
      })
      toast.success(t('my.postUpdated'))
      onOpenChange(false)
      onSaved()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={isOpen} onOpenChange={onOpenChange}>
      <DialogContent
        className={cn(
          'flex max-w-lg flex-col h-[98vh]',
          expanded &&
            '!fixed !inset-0 !left-0 !top-0 !h-screen !w-screen !max-h-none !max-w-none !translate-x-0 !translate-y-0 !rounded-none border-0'
        )}
      >
        {/* 整窗全屏放大 / 还原（置于 close 按钮左侧，与新建帖子弹窗一致） */}
        <button
          type="button"
          title={expanded ? t('editor.restore') : t('editor.expand')}
          aria-label={expanded ? t('editor.restore') : t('editor.expand')}
          onClick={() => void toggleFullscreen()}
          className="absolute right-11 top-4 rounded-sm opacity-70 ring-offset-background transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
        >
          <i className={cn('fa-solid h-4 w-4', expanded ? 'fa-compress' : 'fa-expand')} />
        </button>

        <DialogHeader className="shrink-0 pr-14">
          <DialogTitle className="text-sm">{t('my.editTitle')}</DialogTitle>
          <DialogDescription className="text-xs">{t('post.createDesc')}</DialogDescription>
        </DialogHeader>

        {loading ? (
          <div className="text-muted-foreground flex h-40 items-center justify-center gap-2 text-xs">
            <i className="fa-solid fa-spinner fa-spin size-3.5" />
            {t('loadError.loading')}
          </div>
        ) : (
          <div className={cn('min-h-0', expanded ? 'flex flex-1 flex-col gap-1.5 overflow-hidden' : 'space-y-1.5 py-1')}>
            <div className="space-y-1">
              <Label className="text-xs">{t('post.sectionLabel')}</Label>
              <div className="flex gap-1">
                {COMMUNITY_SECTIONS.map((s) => (
                  <button
                    key={s}
                    type="button"
                    title={t(`section.${s}`)}
                    className={cn(
                      'flex h-7 flex-1 items-center justify-center gap-1 rounded-md border px-2 text-xs transition-colors',
                      section === s
                        ? 'border-primary bg-primary text-primary-foreground'
                        : 'bg-background text-muted-foreground hover:bg-muted hover:text-foreground'
                    )}
                    onClick={() => setSection(s)}
                  >
                    {t(`section.${s}`)}
                  </button>
                ))}
              </div>
            </div>

            <div className="space-y-1">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-0.5">
                  <Label htmlFor="my-post-title" className="text-xs">
                    {t('post.titleLabel')}
                  </Label>
                  {/* 字数限制提示：helpicon 形式置于标题右侧，与新建帖子弹窗一致 */}
                  <HelpIcon size="sm" type="popover" trigger="click" side="bottom" align="start" contentClassName="max-w-xs text-xs leading-relaxed">
                    <p>{t('post.createDesc')}</p>
                  </HelpIcon>
                </div>
                <span className="text-muted-foreground text-[10px] tabular-nums">
                  {title.trim().length}/80
                </span>
              </div>
              <Input
                id="my-post-title"
                value={title}
                maxLength={80}
                onChange={(e) => setTitle(e.target.value)}
                className="h-8 text-xs"
              />
            </div>

            <div className="space-y-1">
              <div className="flex items-center justify-between">
                <Label htmlFor="my-post-content" className="text-xs">
                  {t('post.contentLabel')}
                </Label>
                <span className="text-muted-foreground text-[10px] tabular-nums">
                  {content.length}/10000
                </span>
              </div>
              {/* 新 Markdown 编辑器（工具栏 / 预览，全屏时铺满） */}
              <MarkdownEditor
                id="my-post-content"
                value={content}
                onChange={setContent}
                maxLength={10000}
                placeholder={t('post.contentPlaceholder')}
                heightClass="h-[50vh]"
                fill={expanded}
              />
            </div>
          </div>
        )}

        <DialogFooter className="shrink-0">
          <Button variant="outline" size="sm" className="h-8 text-xs" onClick={() => onOpenChange(false)}>
            {t('post.cancel')}
          </Button>
          <Button
            size="sm"
            className="h-8 text-xs"
            disabled={loading || !titleValid || !contentValid || submitting}
            onClick={handleSubmit}
          >
            {submitting && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
            {t('my.save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
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