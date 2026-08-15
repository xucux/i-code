/**
 * 帖子详情 + 评论区（楼中楼，§8.4）
 *
 * 结构：帖子头（标题/作者/正文）→ 顶层评论列表（每条含楼中楼，深度限 2 层）
 *      → 底部固定回复输入框（目标可为帖子或某条回复，发送后整页刷新）。
 * 「楼主」Badge：评论作者 == 发帖人。
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { ScrollPage } from '@/components/ui/scroll-page'
import { MarkdownContent } from '@/components/ui/markdown-content'
import { SectionBadge } from '@/modules/community/ui/section-badge'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { createCommunityReply, formatCommunityTime, getCommunityPost } from '@/hooks/use-community'
import { getCommunityAvatar } from '@/modules/community/avatars'
import type { PostDetailData, ReplyItem } from '@/modules/community/types'
import { ReportDialog } from './report-dialog'

/** 回复字数上限（与 Worker / Rust 侧一致） */
const REPLY_MAX = 1000

export interface PostDetailProps {
  postId: number
  /** 当前用户 ID（用于隐藏对自己内容的举报入口） */
  currentUserId: string | null
}

export function PostDetail({ postId, currentUserId }: PostDetailProps) {
  const { t } = useTranslation('community')
  const [data, setData] = useState<PostDetailData | null>(null)
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [notFound, setNotFound] = useState(false)

  // 回复目标：null = 顶层评论；否则为楼中楼（指向某条回复）
  const [replyTarget, setReplyTarget] = useState<{ replyId: number; nickname: string } | null>(null)
  const [draft, setDraft] = useState('')
  const [sending, setSending] = useState(false)

  // 举报弹层状态
  const [reportTarget, setReportTarget] = useState<{ type: 'post' | 'reply'; id: number; preview: string } | null>(null)

  /** 实测页面总高度（含固定输入区，用于推导滚动区高度） */
  const [pageHeight, pageRef] = useAvailableHeight()
  /** 底部输入区实际高度 */
  const [inputHeight, inputRef] = useAvailableHeight()

  const load = useCallback(async (silent = false) => {
    if (silent) setRefreshing(true)
    else setLoading(true)
    setNotFound(false)
    try {
      const result = await getCommunityPost(postId)
      setData(result)
    } catch {
      setNotFound(true)
      setData(null)
    } finally {
      setLoading(false)
      setRefreshing(false)
    }
  }, [postId])

  useEffect(() => {
    setData(null)
    setReplyTarget(null)
    setDraft('')
    void load()
  }, [load])

  /** 评论滚动区高度 = 页面高度 - 输入区 - 上下内边距（p-4 = 32px） */
  const scrollHeight = useMemo(
    () => Math.max(0, pageHeight - inputHeight - 32),
    [pageHeight, inputHeight]
  )

  /** 发送回复（顶层或楼中楼），成功后整页刷新保证 reply_count / 楼层一致 */
  const handleSend = async () => {
    const content = draft.trim()
    if (!content || sending) return
    setSending(true)
    try {
      await createCommunityReply(postId, {
        content,
        parentReplyId: replyTarget?.replyId,
      })
      toast.success(t('success.reply'))
      setDraft('')
      setReplyTarget(null)
      await load(true)
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setSending(false)
    }
  }

  if (loading) {
    return (
      <div ref={pageRef} className="text-muted-foreground flex h-full items-center justify-center gap-2 text-xs">
        <i className="fa-solid fa-spinner fa-spin size-3.5" />
        {t('loadError.loading')}
      </div>
    )
  }

  if (notFound || !data) {
    return (
      <div ref={pageRef} className="flex h-full flex-col items-center justify-center gap-3 text-xs">
        <i className="fa-solid fa-file-circle-xmark text-muted-foreground size-6" />
        <p className="text-muted-foreground">{t('post.notFound')}</p>
        <Link to="/community" className="text-primary text-xs hover:underline">
          {t('action.backToList')}
        </Link>
      </div>
    )
  }

  const { post, comments } = data
  const canSend = draft.trim().length > 0 && draft.length <= REPLY_MAX && !sending

  return (
    <div ref={pageRef} className="flex h-full flex-col p-4">
      {/* 帖子 + 评论区（整体滚动） */}
      <ScrollPage style={{ height: scrollHeight || undefined }} variant="borderless">
        <div className="space-y-4 pr-2">
          {/* 帖子头 */}
          <div className="rounded-lg border bg-card p-3">
            <div className="flex items-center justify-between gap-2">
              <Link
                to="/community"
                className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs transition-colors"
              >
                <i className="fa-solid fa-arrow-left size-3" />
                {t('action.backToList')}
              </Link>
              <div className="flex items-center gap-1">
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-7"
                  title={t('action.refresh')}
                  disabled={refreshing}
                  onClick={() => void load(true)}
                >
                  <i className={refreshing ? 'fa-solid fa-spinner fa-spin size-3' : 'fa-solid fa-rotate size-3'} />
                </Button>
                {post.author.userId !== currentUserId && (
                  <Button
                    variant="ghost"
                    size="icon"
                    className="text-muted-foreground hover:text-destructive size-7"
                    title={t('action.report')}
                    onClick={() =>
                      setReportTarget({ type: 'post', id: post.postId, preview: post.title })
                    }
                  >
                    <i className="fa-solid fa-flag size-3" />
                  </Button>
                )}
              </div>
            </div>

            <div className="mt-2 flex items-start gap-2">
              <h1 className="min-w-0 flex-1 text-base font-semibold leading-snug">{post.title}</h1>
              <SectionBadge section={post.section} className="mt-0.5 shrink-0" />
            </div>
            <div className="text-muted-foreground mt-1.5 flex items-center gap-1.5 text-[11px]">
              <span className="text-sm leading-none">{getCommunityAvatar(post.author.avatarIndex)}</span>
              <span className="max-w-32 truncate">{post.author.nickname}</span>
              <span>{formatCommunityTime(post.createdAt, t)}</span>
              <span className="ml-auto flex items-center gap-1 tabular-nums">
                <i className="fa-solid fa-comment size-2.5" />
                {post.replyCount}
              </span>
            </div>
            {/* 正文：Markdown 渲染（GFM + GitHub Alert） */}
            <div className="mt-2.5">
              <MarkdownContent content={post.content} />
            </div>
          </div>

          {/* 评论区 */}
          <div className="space-y-2">
            <div className="text-muted-foreground px-1 text-xs">
              {t('comment.sectionTitle', { count: comments.items.length })}
            </div>
            {comments.items.length === 0 && (
              <div className="text-muted-foreground flex h-20 items-center justify-center text-xs">
                {t('comment.empty')}
              </div>
            )}
            {comments.items.map((comment) => (
              <CommentBlock
                key={comment.replyId}
                comment={comment}
                postAuthorId={post.author.userId}
                currentUserId={currentUserId}
                onReply={(replyId, nickname) => setReplyTarget({ replyId, nickname })}
                onReport={(type, id, preview) => setReportTarget({ type, id, preview })}
              />
            ))}
          </div>
        </div>
      </ScrollPage>

      {/* 底部回复输入区（固定） */}
      <div ref={inputRef} className="mt-3 shrink-0 space-y-1.5">
        {replyTarget && (
          <div className="flex items-center justify-between text-[11px]">
            <span className="text-primary truncate">
              <i className="fa-solid fa-reply mr-1 size-2.5" />
              {t('comment.replyTo', { name: replyTarget.nickname })}
            </span>
            <button
              type="button"
              className="text-muted-foreground hover:text-foreground shrink-0"
              onClick={() => setReplyTarget(null)}
            >
              {t('comment.cancelReply')}
            </button>
          </div>
        )}
        <div className="flex items-end gap-2">
          <Textarea
            value={draft}
            maxLength={REPLY_MAX}
            placeholder={
              replyTarget
                ? t('comment.placeholderReply', { name: replyTarget.nickname })
                : t('comment.placeholder')
            }
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              // Ctrl/Cmd + Enter 快捷发送
              if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') void handleSend()
            }}
            className="min-h-10 resize-none text-xs leading-relaxed"
            rows={2}
          />
          <Button size="sm" className="h-9 shrink-0 text-xs" disabled={!canSend} onClick={handleSend}>
            {sending && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
            {t('action.reply')}
          </Button>
        </div>
      </div>

      {/* 举报弹层 */}
      <ReportDialog
        open={reportTarget != null}
        onOpenChange={(open) => !open && setReportTarget(null)}
        targetType={reportTarget?.type ?? 'post'}
        targetId={reportTarget?.id ?? null}
        preview={reportTarget?.preview}
      />
    </div>
  )
}

/**
 * 顶层评论块（含楼中楼，深度限 2 层：顶层 + 一层子回复）
 */
function CommentBlock({
  comment,
  postAuthorId,
  currentUserId,
  onReply,
  onReport,
}: {
  comment: PostDetailData['comments']['items'][number]
  postAuthorId: string
  currentUserId: string | null
  onReply: (replyId: number, nickname: string) => void
  onReport: (type: 'post' | 'reply', id: number, preview: string) => void
}) {
  const { t } = useTranslation('community')

  return (
    <div className="rounded-lg border bg-card p-3">
      <ReplyMeta
        reply={comment}
        isOp={comment.author.userId === postAuthorId}
        currentUserId={currentUserId}
        onReply={() => onReply(comment.replyId, comment.author.nickname)}
        onReport={() => onReport('reply', comment.replyId, comment.content)}
      />
      <p className="mt-1 whitespace-pre-wrap text-xs leading-relaxed">{comment.content}</p>

      {/* 楼中楼（第 2 层，缩进 + 左竖线） */}
      {comment.replies.length > 0 && (
        <div className="mt-2.5 space-y-2 border-l-2 border-muted pl-3">
          {comment.replies.map((sub) => (
            <div key={sub.replyId}>
              <ReplyMeta
                reply={sub}
                isOp={sub.author.userId === postAuthorId}
                currentUserId={currentUserId}
                compact
                onReply={() => onReply(sub.replyId, sub.author.nickname)}
                onReport={() => onReport('reply', sub.replyId, sub.content)}
              />
              <p className="mt-0.5 whitespace-pre-wrap text-xs leading-relaxed">{sub.content}</p>
            </div>
          ))}
          {/* Worker 每顶层最多返回 50 条子回复，超出仅提示（暂无分页接口） */}
          {comment.hasMoreReplies && (
            <p className="text-muted-foreground text-[11px]">
              <i className="fa-solid fa-ellipsis mr-1 size-2.5" />
              {t('comment.more')}
            </p>
          )}
        </div>
      )}
    </div>
  )
}

/**
 * 回复元信息行：头像 / 昵称 / 楼主 Badge / 时间 / 回复与举报操作
 */
function ReplyMeta({
  reply,
  isOp,
  currentUserId,
  compact,
  onReply,
  onReport,
}: {
  reply: ReplyItem
  isOp: boolean
  currentUserId: string | null
  compact?: boolean
  onReply: () => void
  onReport: () => void
}) {
  const { t } = useTranslation('community')

  return (
    <div className="flex items-center gap-1.5">
      <span className={compact ? 'text-sm leading-none' : 'text-base leading-none'}>
        {getCommunityAvatar(reply.author.avatarIndex)}
      </span>
      <span className="max-w-32 truncate text-xs font-medium">{reply.author.nickname}</span>
      {isOp && (
        <Badge variant="secondary" className="h-4 px-1 text-[10px]">
          {t('comment.op')}
        </Badge>
      )}
      <span className="text-muted-foreground text-[11px] tabular-nums">
        {formatCommunityTime(reply.createdAt, t)}
      </span>
      {/* 操作：回复 / 举报（自己的内容不显示举报） */}
      <div className="ml-auto flex items-center gap-0.5">
        <button
          type="button"
          className="text-muted-foreground hover:text-foreground px-1 text-[11px] transition-colors"
          title={t('action.reply')}
          onClick={onReply}
        >
          <i className="fa-solid fa-reply size-3" />
        </button>
        {reply.author.userId !== currentUserId && (
          <button
            type="button"
            className="text-muted-foreground hover:text-destructive px-1 text-[11px] transition-colors"
            title={t('action.report')}
            onClick={onReport}
          >
            <i className="fa-solid fa-flag size-3" />
          </button>
        )}
      </div>
    </div>
  )
}
