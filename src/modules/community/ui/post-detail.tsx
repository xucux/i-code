/**
 * 帖子详情 + 评论区（楼中楼，§8.4）
 *
 * 结构：帖子头（标题/作者/正文）→ 顶层评论列表（每条含楼中楼，深度限 2 层）
 *      → 底部固定「回复」按钮，点击弹窗输入回复（目标可为帖子或某条回复）。
 * 「楼主」Badge：评论作者 == 发帖人。
 * 滚动：帖子 + 评论区整体使用原生滚动（overflow-y-auto），底部回复栏固定。
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { cn } from '@/lib/utils'
import { MarkdownContent } from '@/components/ui/markdown-content'
import { SectionBadge } from '@/modules/community/ui/section-badge'
import { MuteBadge } from '@/modules/community/ui/mute-badge'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { useAutoHideScrollbar } from '@/hooks/use-auto-hide-scrollbar'
import { createCommunityReply, formatCommunityTime, getCommunityPost, getCommunitySiteGovernance } from '@/hooks/use-community'
import { getCommunityAvatar } from '@/modules/community/avatars'
import type { PostDetailData, ReplyItem, SiteGovernance } from '@/modules/community/types'
import { MarkdownReplyDialog } from './markdown-reply-dialog'
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

  // 一级评论回复（Markdown 弹窗）：目标 null = 发表新顶层评论；非 null = 回复某顶层评论
  const [mdReplyOpen, setMdReplyOpen] = useState(false)
  const [mdReplyTarget, setMdReplyTarget] = useState<{ replyId: number; nickname: string } | null>(null)

  // 二级（楼中楼）回复：原纯文本弹窗
  const [replyDialogOpen, setReplyDialogOpen] = useState(false)
  const [replyTarget, setReplyTarget] = useState<{ replyId: number; nickname: string } | null>(null)
  const [draft, setDraft] = useState('')
  const [sending, setSending] = useState(false)

  // 举报弹层状态
  const [reportTarget, setReportTarget] = useState<{ type: 'post' | 'reply'; id: number; preview: string } | null>(null)

  // 站点治理开关（D11）：null = 加载中/失败按全关处理，不阻塞浏览
  const [governance, setGovernance] = useState<SiteGovernance | null>(null)

  useEffect(() => {
    let cancelled = false
    getCommunitySiteGovernance()
      .then((gov) => {
        if (!cancelled) setGovernance(gov)
      })
      .catch(() => {
        // 治理开关拉取失败按全关处理（Worker 侧仍兜底拦截）
      })
    return () => {
      cancelled = true
    }
  }, [])

  /** 实测页面总高度（用于推导滚动区高度） */
  const [pageHeight, pageRef] = useAvailableHeight()
  const [scrollRef, scrolling] = useAutoHideScrollbar()

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
    setMdReplyTarget(null)
    setMdReplyOpen(false)
    setReplyTarget(null)
    setDraft('')
    setReplyDialogOpen(false)
    void load()
  }, [load])

  /** 滚动区高度 = 页面高度 - 上下内边距（p-4 = 32px），整页原生滚动 */
  const scrollHeight = useMemo(() => Math.max(0, pageHeight - 32), [pageHeight])

  /** 打开一级评论 Markdown 回复弹窗（target 缺省 = 发表新顶层评论） */
  const openMdReply = (target?: { replyId: number; nickname: string }) => {
    setMdReplyTarget(target ?? null)
    setMdReplyOpen(true)
  }

  /** 打开二级（楼中楼）回复弹窗（原纯文本） */
  const openReplyDialog = (target: { replyId: number; nickname: string }) => {
    setReplyTarget(target)
    setReplyDialogOpen(true)
  }

  /** 发送一级回复（Markdown 弹窗提交；顶层评论或回复顶层评论），成功后关弹窗并整页刷新 */
  const handleMdReplySubmit = async (content: string) => {
    await createCommunityReply(postId, {
      content,
      parentReplyId: mdReplyTarget?.replyId,
    })
    toast.success(t('success.reply'))
    setMdReplyTarget(null)
    setMdReplyOpen(false)
    await load(true)
  }

  /** 发送二级回复（楼中楼，原弹窗），成功后关弹窗并整页刷新 */
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
      setReplyDialogOpen(false)
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
  // 回复禁用（D11）：帖子锁定 > 全站禁言 > 全站禁回复，任一命中即禁止
  const replyDisabled =
    post.locked || (governance != null && (governance.muteAll || governance.replyLocked))
  const replyDisabledTip = post.locked
    ? t('governance.postLockedTip')
    : governance?.muteAll
      ? t('governance.muteAllTip')
      : t('governance.replyLockedTip')

  return (
    <div ref={pageRef} className="flex h-full flex-col p-4">
      {/* 帖子 + 评论区（原生滚动 + 滚动条自动隐藏） */}
      <div
        ref={scrollRef}
        className={cn(
          'min-h-0 overflow-y-auto pr-2 custom-scrollbar custom-scrollbar-auto-hide',
          scrolling && 'scrollbar-visible'
        )}
        style={{ height: scrollHeight || undefined }}
      >
        <div className="space-y-4">
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
                  className="text-muted-foreground size-7"
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
              {post.pinned && (
                <Badge variant="secondary" className="mt-0.5 h-5 shrink-0 gap-1 px-1.5 text-[10px]">
                  <i className="fa-solid fa-thumbtack size-2" />
                  {t('pinned')}
                </Badge>
              )}
              {post.locked && (
                <Badge variant="outline" className="mt-0.5 h-5 shrink-0 gap-1 px-1.5 text-[10px]">
                  <i className="fa-solid fa-lock size-2" />
                  {t('governance.lockedBadge')}
                </Badge>
              )}
              <SectionBadge section={post.section} className="mt-0.5 shrink-0" />
            </div>
            <div className="text-muted-foreground mt-1.5 flex items-center gap-1.5 text-[11px]">
              <span className="text-sm leading-none">{getCommunityAvatar(post.author.avatarIndex)}</span>
              <span className="max-w-32 truncate">{post.author.nickname}</span>
              <MuteBadge muted={post.author.muted} />
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
            {/* 标题行：「评论（N）」+ 右侧「回复」入口（弹窗输入；D11 锁定时替换为提示） */}
            <div className="flex items-center justify-between px-1">
              <span className="text-muted-foreground text-xs">
                {t('comment.sectionTitle', { count: comments.items.length })}
              </span>
              {replyDisabled ? (
                <span className="text-muted-foreground flex items-center gap-1 text-xs">
                  <i className="fa-solid fa-lock size-2.5" />
                  {replyDisabledTip}
                </span>
              ) : (
                <button
                  type="button"
                  className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs transition-colors"
                  onClick={() => openMdReply()}
                >
                  <i className="fa-solid fa-reply size-2.5" />
                  {t('action.reply')}
                </button>
              )}
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
                replyDisabled={replyDisabled}
                onReply={(replyId, nickname) => openMdReply({ replyId, nickname })}
                onNestedReply={(replyId, nickname) => openReplyDialog({ replyId, nickname })}
                onReport={(type, id, preview) => setReportTarget({ type, id, preview })}
              />
            ))}
          </div>
        </div>
      </div>

      {/* 一级评论回复弹层（Markdown 编辑 / 预览；通知迭代） */}
      <MarkdownReplyDialog
        open={mdReplyOpen}
        onOpenChange={setMdReplyOpen}
        target={mdReplyTarget}
        onSubmit={handleMdReplySubmit}
      />

      {/* 二级（楼中楼）回复弹层（原纯文本） */}
      <Dialog open={replyDialogOpen} onOpenChange={setReplyDialogOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle className="text-sm">{t('comment.replyTitle')}</DialogTitle>
          </DialogHeader>

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

          <Textarea
            value={draft}
            maxLength={REPLY_MAX}
            autoFocus
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
            className="min-h-32 resize-none text-xs leading-relaxed"
            rows={5}
          />

          <DialogFooter>
            <span className="text-muted-foreground mr-auto text-[10px] tabular-nums">
              {draft.length}/{REPLY_MAX}
            </span>
            <Button variant="outline" size="sm" className="h-8 text-xs" onClick={() => setReplyDialogOpen(false)}>
              {t('post.cancel')}
            </Button>
            <Button size="sm" className="h-8 text-xs" disabled={!canSend} onClick={handleSend}>
              {sending && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
              {t('action.reply')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

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
  replyDisabled,
  onReply,
  onNestedReply,
  onReport,
}: {
  comment: PostDetailData['comments']['items'][number]
  postAuthorId: string
  currentUserId: string | null
  /** D11 治理：帖子锁定 / 全站禁言 / 禁回复时隐藏回复入口 */
  replyDisabled: boolean
  /** 回复顶层评论（一级，Markdown 弹窗） */
  onReply: (replyId: number, nickname: string) => void
  /** 回复楼中楼（二级，原纯文本弹窗） */
  onNestedReply: (replyId: number, nickname: string) => void
  onReport: (type: 'post' | 'reply', id: number, preview: string) => void
}) {
  const { t } = useTranslation('community')

  return (
    <div className="rounded-lg border bg-card p-3">
      <ReplyMeta
        reply={comment}
        isOp={comment.author.userId === postAuthorId}
        currentUserId={currentUserId}
        replyDisabled={replyDisabled}
        onReply={() => onReply(comment.replyId, comment.author.nickname)}
        onReport={() => onReport('reply', comment.replyId, comment.content)}
      />
      {/* 一级评论正文：Markdown 渲染（通知迭代） */}
      <div className="mt-1">
        <MarkdownContent content={comment.content} />
      </div>

      {/* 楼中楼（第 2 层，缩进 + 左竖线）；二级正文保持纯文本，@目标昵称见 ReplyMeta */}
      {comment.replies.length > 0 && (
        <div className="mt-2.5 space-y-2 border-l-2 border-muted pl-3">
          {comment.replies.map((sub) => (
            <div key={sub.replyId}>
              <ReplyMeta
                reply={sub}
                isOp={sub.author.userId === postAuthorId}
                currentUserId={currentUserId}
                replyDisabled={replyDisabled}
                compact
                replyToNickname={sub.replyToNickname}
                onReply={() => onNestedReply(sub.replyId, sub.author.nickname)}
                onReport={() => onReport('reply', sub.replyId, sub.content)}
              />
              <p className="mt-0.5 break-words whitespace-pre-wrap text-xs leading-relaxed">{sub.content}</p>
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
  replyDisabled,
  compact,
  replyToNickname,
  onReply,
  onReport,
}: {
  reply: ReplyItem
  isOp: boolean
  currentUserId: string | null
  /** D11 治理：禁用时隐藏回复按钮（评论区标题行已有统一锁定提示） */
  replyDisabled?: boolean
  compact?: boolean
  /** 楼中楼 @目标昵称：仅二级回复「回复的是另一条二级评论」时展示（通知迭代） */
  replyToNickname?: string | null
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
      {/* 楼中楼：回复的是另一条二级评论时，昵称后追加 @目标昵称（通知迭代） */}
      {replyToNickname && (
        <span className="text-muted-foreground max-w-24 truncate text-[11px]">@{replyToNickname}</span>
      )}
      <MuteBadge muted={reply.author.muted} />
      {isOp && (
        <Badge variant="secondary" className="h-4 px-1 text-[10px]">
          {t('comment.op')}
        </Badge>
      )}
      <span className="text-muted-foreground text-[11px] tabular-nums">
        {formatCommunityTime(reply.createdAt, t)}
      </span>
      {/* 操作：回复 / 举报（自己的内容不显示举报；治理锁定时不显示回复） */}
      <div className="ml-auto flex items-center gap-0.5">
        {!replyDisabled && (
          <button
            type="button"
            className="text-muted-foreground hover:text-foreground px-1 text-[11px] transition-colors"
            title={t('action.reply')}
            onClick={onReply}
          >
            <i className="fa-solid fa-reply size-3" />
          </button>
        )}
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
