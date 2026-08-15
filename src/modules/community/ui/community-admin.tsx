/**
 * 社区管理员页面（§5.3 / D3 + D10）
 *
 * 登录 Worker 固定凭据 → 持有短期 adminToken（仅内存，不持久化，每次使用需重新登录）：
 * - 用户列表：封禁 / 解封
 * - 举报列表：查看目标预览并标记已处理
 * - 帖子管理（D10）：所有用户帖子分页列表，编辑 / 删除帖子；进入详情可编辑 / 删除任意回复
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Badge } from '@/components/ui/badge'
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
import { ScrollPage } from '@/components/ui/scroll-page'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Textarea } from '@/components/ui/textarea'
import { MarkdownContent } from '@/components/ui/markdown-content'
import { cn } from '@/lib/utils'
import { useAvailableHeight } from '@/hooks/use-available-height'
import {
  communityAdminBanUser,
  communityAdminDeletePost,
  communityAdminDeleteReply,
  communityAdminGetPost,
  communityAdminGetPosts,
  communityAdminGetReports,
  communityAdminGetUsers,
  communityAdminLogin,
  communityAdminResolveReport,
  communityAdminUnbanUser,
  communityAdminUpdatePost,
  communityAdminUpdateReply,
  formatCommunityTime,
} from '@/hooks/use-community'
import { getCommunityAvatar } from '@/modules/community/avatars'
import {
  COMMUNITY_SECTIONS,
  type AdminPostItem,
  type AdminReportItem,
  type AdminUserItem,
  type CommentItem,
  type CommunitySection,
  type PostDetailData,
  type ReplyItem,
} from '@/modules/community/types'
import { SectionBadge } from '@/modules/community/ui/section-badge'

export function CommunityAdmin() {
  const { t } = useTranslation('community')
  const [token, setToken] = useState<string | null>(null)
  const [loggingIn, setLoggingIn] = useState(false)

  // 登录表单
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')

  // 登录态重置（退出登录）
  const logout = useCallback(() => {
    setToken(null)
    setPassword('')
  }, [])

  const handleLogin = async () => {
    if (!username.trim() || !password || loggingIn) return
    setLoggingIn(true)
    try {
      const data = await communityAdminLogin(username.trim(), password)
      setToken(data.adminToken)
      toast.success(t('admin.loginSuccess'))
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setLoggingIn(false)
    }
  }

  // ===== 布局高度（Tabs 上为页头，Tabs 下为滚动列表）=====
  const [pageHeight, pageRef] = useAvailableHeight()
  const [headerHeight, headerRef] = useAvailableHeight()
  const listHeight = useMemo(
    () => Math.max(0, pageHeight - headerHeight - 32),
    [pageHeight, headerHeight]
  )

  return (
    <div ref={pageRef} className="flex h-full flex-col p-4">
      {/* 页头 */}
      <div ref={headerRef} className="mb-3 flex items-center gap-2">
        <Link
          to="/community"
          className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs transition-colors"
        >
          <i className="fa-solid fa-arrow-left size-3" />
          {t('action.backToList')}
        </Link>
        <h1 className="text-sm font-semibold">{t('admin.title')}</h1>
        {token && (
          <Button variant="ghost" size="sm" className="text-muted-foreground ml-auto h-7 text-xs" onClick={logout}>
            <i className="fa-solid fa-right-from-bracket mr-1.5 size-3" />
            {t('admin.logout')}
          </Button>
        )}
      </div>

      {token ? (
        <AdminPanels token={token} listHeight={listHeight} />
      ) : (
        // 登录卡片
        <div className="flex flex-1 items-center justify-center">
          <div className="w-72 space-y-3 rounded-lg border bg-card p-4">
            <div className="text-center">
              <i className="fa-solid fa-user-shield text-primary mb-1 size-5" />
              <p className="text-xs font-medium">{t('admin.title')}</p>
            </div>
            <div className="space-y-1">
              <Label htmlFor="admin-username" className="text-xs">
                {t('admin.username')}
              </Label>
              <Input
                id="admin-username"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                className="h-8 text-xs"
                autoComplete="off"
              />
            </div>
            <div className="space-y-1">
              <Label htmlFor="admin-password" className="text-xs">
                {t('admin.password')}
              </Label>
              <Input
                id="admin-password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && void handleLogin()}
                className="h-8 text-xs"
              />
            </div>
            <Button
              size="sm"
              className="h-8 w-full text-xs"
              disabled={!username.trim() || !password || loggingIn}
              onClick={handleLogin}
            >
              {loggingIn && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
              {t('admin.login')}
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}

/** 已登录面板：用户 / 举报 / 帖子管理 三个 Tab */
function AdminPanels({ token, listHeight }: { token: string; listHeight: number }) {
  const { t } = useTranslation('community')
  const [tab, setTab] = useState<'users' | 'reports' | 'posts'>('users')

  return (
    <Tabs
      value={tab}
      onValueChange={(v) => setTab(v as 'users' | 'reports' | 'posts')}
      className="flex min-h-0 flex-1 flex-col"
    >
      <TabsList className="mb-2 self-start">
        <TabsTrigger value="users" className="text-xs">
          {t('admin.users')}
        </TabsTrigger>
        <TabsTrigger value="reports" className="text-xs">
          {t('admin.reports')}
        </TabsTrigger>
        <TabsTrigger value="posts" className="text-xs">
          {t('admin.posts')}
        </TabsTrigger>
      </TabsList>

      <TabsContent value="users" className="min-h-0 flex-1">
        <AdminUsersTab token={token} height={listHeight} />
      </TabsContent>
      <TabsContent value="reports" className="min-h-0 flex-1">
        <AdminReportsTab token={token} height={listHeight} />
      </TabsContent>
      <TabsContent value="posts" className="min-h-0 flex-1">
        <AdminPostsTab token={token} height={listHeight} />
      </TabsContent>
    </Tabs>
  )
}

/** 用户列表 Tab：封禁 / 解封 */
function AdminUsersTab({ token, height }: { token: string; height: number }) {
  const { t } = useTranslation('community')
  const [users, setUsers] = useState<AdminUserItem[]>([])
  const [loading, setLoading] = useState(true)
  // 待确认封禁的用户（展开原因输入）
  const [banningId, setBanningId] = useState<string | null>(null)
  const [banReason, setBanReason] = useState('')
  const [acting, setActing] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      setUsers(await communityAdminGetUsers(token))
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [token])

  useEffect(() => {
    void load()
  }, [load])

  const handleBan = async (userId: string) => {
    if (acting) return
    setActing(true)
    try {
      await communityAdminBanUser(token, userId, banReason.trim() || undefined)
      toast.success(t('admin.banDone'))
      setBanningId(null)
      setBanReason('')
      await load()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setActing(false)
    }
  }

  const handleUnban = async (userId: string) => {
    if (acting) return
    setActing(true)
    try {
      await communityAdminUnbanUser(token, userId)
      toast.success(t('admin.unbanDone'))
      await load()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setActing(false)
    }
  }

  if (loading) {
    return (
      <div className="text-muted-foreground flex h-20 items-center justify-center gap-2 text-xs">
        <i className="fa-solid fa-spinner fa-spin size-3.5" />
        {t('loadError.loading')}
      </div>
    )
  }

  if (users.length === 0) {
    return <AdminEmpty text={t('admin.emptyUsers')} />
  }

  return (
    <ScrollPage style={{ height: height || undefined }} variant="borderless">
      <div className="space-y-2 pr-2">
        {users.map((user) => (
          <div key={user.userId} className="rounded-lg border bg-card p-3">
            <div className="flex items-center gap-2">
              <span className="text-lg leading-none">{getCommunityAvatar(user.avatarIndex)}</span>
              <span className="max-w-40 truncate text-xs font-medium">{user.nickname}</span>
              {user.banned ? (
                <Badge variant="destructive" className="h-4 px-1 text-[10px]">
                  {t('admin.banned')}
                </Badge>
              ) : (
                <Badge variant="secondary" className="h-4 px-1 text-[10px]">
                  {t('admin.normal')}
                </Badge>
              )}
              <span className="text-muted-foreground ml-auto text-[11px] tabular-nums">
                <i className="fa-solid fa-file-lines mr-0.5 size-2.5" />
                {user.postCount}
                <i className="fa-solid fa-comment ml-1.5 mr-0.5 size-2.5" />
                {user.replyCount}
              </span>
              <span className="text-muted-foreground shrink-0 text-[11px] tabular-nums">
                {formatCommunityTime(user.createdAt, t)}
              </span>
              {user.banned ? (
                <Button variant="outline" size="sm" className="h-6 shrink-0 px-2 text-[11px]" disabled={acting} onClick={() => void handleUnban(user.userId)}>
                  {t('admin.unban')}
                </Button>
              ) : banningId === user.userId ? (
                <Button variant="destructive" size="sm" className="h-6 shrink-0 px-2 text-[11px]" disabled={acting} onClick={() => void handleBan(user.userId)}>
                  {t('admin.banConfirm')}
                </Button>
              ) : (
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-muted-foreground hover:text-destructive h-6 shrink-0 px-2 text-[11px]"
                  onClick={() => setBanningId(user.userId)}
                >
                  {t('admin.ban')}
                </Button>
              )}
            </div>
            {user.banned && user.banReason && (
              <p className="text-destructive mt-1.5 text-[11px]">{t('profile.banReason', { reason: user.banReason })}</p>
            )}
            {banningId === user.userId && !user.banned && (
              <div className="mt-2 flex items-center gap-1.5">
                <Input
                  value={banReason}
                  maxLength={100}
                  placeholder={t('admin.banReasonPlaceholder')}
                  onChange={(e) => setBanReason(e.target.value)}
                  className="h-7 flex-1 text-[11px]"
                />
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-muted-foreground h-6 px-2 text-[11px]"
                  onClick={() => {
                    setBanningId(null)
                    setBanReason('')
                  }}
                >
                  {t('post.cancel')}
                </Button>
              </div>
            )}
          </div>
        ))}
      </div>
    </ScrollPage>
  )
}

/** 举报列表 Tab：查看 + 标记已处理 */
function AdminReportsTab({ token, height }: { token: string; height: number }) {
  const { t } = useTranslation('community')
  const [reports, setReports] = useState<AdminReportItem[]>([])
  const [loading, setLoading] = useState(true)
  const [acting, setActing] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      setReports(await communityAdminGetReports(token))
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [token])

  useEffect(() => {
    void load()
  }, [load])

  const handleResolve = async (reportId: number) => {
    if (acting) return
    setActing(true)
    try {
      await communityAdminResolveReport(token, reportId)
      toast.success(t('admin.resolveDone'))
      await load()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setActing(false)
    }
  }

  if (loading) {
    return (
      <div className="text-muted-foreground flex h-20 items-center justify-center gap-2 text-xs">
        <i className="fa-solid fa-spinner fa-spin size-3.5" />
        {t('loadError.loading')}
      </div>
    )
  }

  if (reports.length === 0) {
    return <AdminEmpty text={t('admin.emptyReports')} />
  }

  return (
    <ScrollPage style={{ height: height || undefined }} variant="borderless">
      <div className="space-y-2 pr-2">
        {reports.map((report) => (
          <div key={report.reportId} className="rounded-lg border bg-card p-3">
            <div className="flex items-center gap-2 text-[11px]">
              <Badge variant="outline" className="h-4 px-1 text-[10px]">
                {report.targetType === 'post' ? t('report.post') : t('report.reply')}
              </Badge>
              <span className="text-muted-foreground">
                {t('admin.reporter', { name: report.reporter.nickname })}
              </span>
              <span className="text-muted-foreground ml-auto tabular-nums">
                {formatCommunityTime(report.createdAt, t)}
              </span>
              <Button
                variant="outline"
                size="sm"
                className="text-muted-foreground h-6 shrink-0 px-2 text-[11px]"
                disabled={acting}
                onClick={() => void handleResolve(report.reportId)}
              >
                <i className="fa-solid fa-check mr-1 size-2.5" />
                {t('admin.resolve')}
              </Button>
            </div>
            {/* 目标预览 + 举报原因 */}
            {report.targetPreview && (
              <p className="bg-muted/50 mt-2 line-clamp-3 rounded-md border p-2 text-xs leading-relaxed">
                {report.targetPreview}
              </p>
            )}
            {report.reason && (
              <p className="text-destructive mt-1.5 text-[11px]">{t('report.reason')}: {report.reason}</p>
            )}
          </div>
        ))}
      </div>
    </ScrollPage>
  )
}

/** 帖子管理 Tab（D10）：所有用户帖子分页列表 + 编辑 / 删除 / 详情（回复治理） */
function AdminPostsTab({ token, height }: { token: string; height: number }) {
  const { t } = useTranslation('community')
  const [posts, setPosts] = useState<AdminPostItem[]>([])
  const [cursor, setCursor] = useState<string | null>(null)
  const [hasMore, setHasMore] = useState(false)
  const [loading, setLoading] = useState(true)
  const [loadingMore, setLoadingMore] = useState(false)
  const [acting, setActing] = useState(false)
  // 待确认删除的帖子（内联二次确认，与封禁交互一致）
  const [deletingId, setDeletingId] = useState<number | null>(null)
  // 编辑弹窗目标帖子（打开时异步拉取详情填充表单）
  const [editingPostId, setEditingPostId] = useState<number | null>(null)
  // 进入详情视图的帖子 ID（null = 列表视图）
  const [detailPostId, setDetailPostId] = useState<number | null>(null)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const data = await communityAdminGetPosts(token)
      setPosts(data.posts)
      setCursor(data.nextCursor)
      setHasMore(data.nextCursor != null)
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [token])

  useEffect(() => {
    void load()
  }, [load])

  const loadMore = async () => {
    if (!cursor || loadingMore) return
    setLoadingMore(true)
    try {
      const data = await communityAdminGetPosts(token, cursor)
      setPosts((prev) => [...prev, ...data.posts])
      setCursor(data.nextCursor)
      setHasMore(data.nextCursor != null)
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setLoadingMore(false)
    }
  }

  const handleDelete = async (postId: number) => {
    if (acting) return
    setActing(true)
    try {
      await communityAdminDeletePost(token, postId)
      toast.success(t('admin.postDeleted'))
      setDeletingId(null)
      // 命中游标分页末页时回退游标，简单起见直接重载第一页
      await load()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setActing(false)
    }
  }

  // ===== 详情视图（帖子全文 + 回复治理）=====
  if (detailPostId != null) {
    return (
      <AdminPostDetail
        token={token}
        postId={detailPostId}
        height={height}
        onBack={() => setDetailPostId(null)}
        onDeleted={() => {
          setDetailPostId(null)
          void load()
        }}
      />
    )
  }

  if (loading) {
    return (
      <div className="text-muted-foreground flex h-20 items-center justify-center gap-2 text-xs">
        <i className="fa-solid fa-spinner fa-spin size-3.5" />
        {t('loadError.loading')}
      </div>
    )
  }

  if (posts.length === 0) {
    return <AdminEmpty text={t('admin.emptyPosts')} />
  }

  return (
    <>
      <ScrollPage style={{ height: height || undefined }} variant="borderless">
        <div className="space-y-2 pr-2">
          {posts.map((post) => (
            <div key={post.postId} className="rounded-lg border bg-card p-3">
              <div className="flex items-center gap-2">
                <span className="text-lg leading-none">{getCommunityAvatar(post.author.avatarIndex)}</span>
                <span className="max-w-40 truncate text-xs font-medium">{post.author.nickname}</span>
                <SectionBadge section={post.section} />
                <span className="text-muted-foreground ml-auto shrink-0 text-[11px] tabular-nums">
                  <i className="fa-solid fa-comment mr-0.5 size-2.5" />
                  {post.replyCount}
                </span>
                <span className="text-muted-foreground shrink-0 text-[11px] tabular-nums">
                  {formatCommunityTime(post.createdAt, t)}
                </span>
              </div>
              <p className="mt-1.5 line-clamp-1 text-xs font-medium">{post.title}</p>
              <p className="text-muted-foreground mt-0.5 line-clamp-2 text-[11px] leading-relaxed">
                {post.excerpt}
              </p>
              <div className="mt-1.5 flex items-center gap-1">
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-muted-foreground hover:text-foreground h-6 px-2 text-[11px]"
                  onClick={() => setDetailPostId(post.postId)}
                >
                  <i className="fa-solid fa-eye mr-1 size-2.5" />
                  {t('admin.viewDetail')}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-muted-foreground hover:text-foreground h-6 px-2 text-[11px]"
                  onClick={() => setEditingPostId(post.postId)}
                >
                  <i className="fa-solid fa-pen mr-1 size-2.5" />
                  {t('admin.edit')}
                </Button>
                {deletingId === post.postId ? (
                  <Button
                    variant="destructive"
                    size="sm"
                    className="h-6 px-2 text-[11px]"
                    disabled={acting}
                    onClick={() => void handleDelete(post.postId)}
                  >
                    {t('admin.deleteConfirm')}
                  </Button>
                ) : (
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-muted-foreground hover:text-destructive h-6 px-2 text-[11px]"
                    onClick={() => setDeletingId(post.postId)}
                  >
                    <i className="fa-solid fa-trash mr-1 size-2.5" />
                    {t('admin.delete')}
                  </Button>
                )}
                {deletingId === post.postId && (
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-muted-foreground h-6 px-2 text-[11px]"
                    onClick={() => setDeletingId(null)}
                  >
                    {t('post.cancel')}
                  </Button>
                )}
              </div>
            </div>
          ))}
          {hasMore && (
            <div className="flex justify-center py-1">
              <Button
                variant="outline"
                size="sm"
                className="text-muted-foreground h-7 text-[11px]"
                disabled={loadingMore}
                onClick={() => void loadMore()}
              >
                {loadingMore && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
                {t('admin.loadMore')}
              </Button>
            </div>
          )}
        </div>
      </ScrollPage>
      {/* 编辑帖子弹窗（打开时拉取最新详情填充） */}
      <AdminPostEditDialog
        token={token}
        postId={editingPostId}
        onOpenChange={(open) => {
          if (!open) setEditingPostId(null)
        }}
        onSaved={() => void load()}
      />
    </>
  )
}

/** 帖子详情视图（管理员）：帖子全文 + 编辑 / 删除 + 评论区回复治理 */
function AdminPostDetail({
  token,
  postId,
  height,
  onBack,
  onDeleted,
}: {
  token: string
  postId: number
  height: number
  onBack: () => void
  onDeleted: () => void
}) {
  const { t } = useTranslation('community')
  const [detail, setDetail] = useState<PostDetailData | null>(null)
  const [loading, setLoading] = useState(true)
  const [acting, setActing] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [editingPost, setEditingPost] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      setDetail(await communityAdminGetPost(token, postId))
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [token, postId])

  useEffect(() => {
    void load()
  }, [load])

  const handleDeletePost = async () => {
    if (acting) return
    setActing(true)
    try {
      await communityAdminDeletePost(token, postId)
      toast.success(t('admin.postDeleted'))
      onDeleted()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setActing(false)
    }
  }

  /** 回复编辑 / 删除后局部刷新评论区（帖子本身不变，避免整页闪烁） */
  const refreshComments = async () => {
    try {
      setDetail(await communityAdminGetPost(token, postId))
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    }
  }

  if (loading || !detail) {
    return (
      <div className="text-muted-foreground flex h-20 items-center justify-center gap-2 text-xs">
        <i className="fa-solid fa-spinner fa-spin size-3.5" />
        {t('loadError.loading')}
      </div>
    )
  }

  const { post, comments } = detail

  return (
    <>
      <ScrollPage style={{ height: height || undefined }} variant="borderless">
        <div className="space-y-2 pr-2">
          {/* 返回 + 帖子操作 */}
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="sm"
              className="text-muted-foreground h-7 px-2 text-[11px]"
              onClick={onBack}
            >
              <i className="fa-solid fa-arrow-left mr-1 size-3" />
              {t('admin.backToPosts')}
            </Button>
            <div className="ml-auto flex items-center gap-1">
              <Button
                variant="ghost"
                size="sm"
                className="text-muted-foreground hover:text-foreground h-7 px-2 text-[11px]"
                onClick={() => setEditingPost(true)}
              >
                <i className="fa-solid fa-pen mr-1 size-2.5" />
                {t('admin.edit')}
              </Button>
              {deleting ? (
                <>
                  <Button
                    variant="destructive"
                    size="sm"
                    className="h-7 px-2 text-[11px]"
                    disabled={acting}
                    onClick={() => void handleDeletePost()}
                  >
                    {t('admin.deleteConfirm')}
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-muted-foreground h-7 px-2 text-[11px]"
                    onClick={() => setDeleting(false)}
                  >
                    {t('post.cancel')}
                  </Button>
                </>
              ) : (
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-muted-foreground hover:text-destructive h-7 px-2 text-[11px]"
                  onClick={() => setDeleting(true)}
                >
                  <i className="fa-solid fa-trash mr-1 size-2.5" />
                  {t('admin.delete')}
                </Button>
              )}
            </div>
          </div>

          {/* 帖子头 */}
          <div className="rounded-lg border bg-card p-3">
            <div className="flex items-center gap-2">
              <span className="text-lg leading-none">{getCommunityAvatar(post.author.avatarIndex)}</span>
              <span className="max-w-40 truncate text-xs font-medium">{post.author.nickname}</span>
              <SectionBadge section={post.section} />
              <span className="text-muted-foreground ml-auto text-[11px] tabular-nums">
                {formatCommunityTime(post.createdAt, t)}
              </span>
            </div>
            <h2 className="mt-1.5 text-sm font-semibold">{post.title}</h2>
            <div className="mt-1.5 text-xs leading-relaxed">
              <MarkdownContent content={post.content} />
            </div>
          </div>

          {/* 评论区（顶层 + 楼中楼，均可编辑 / 删除） */}
          {comments.items.length === 0 ? (
            <AdminEmpty text={t('post.noComments')} />
          ) : (
            comments.items.map((comment) => (
              <AdminReplyCard
                key={comment.replyId}
                token={token}
                reply={comment}
                isTop
                onChanged={() => void refreshComments()}
              />
            ))
          )}
        </div>
      </ScrollPage>
      {/* 详情内编辑帖子（已有全文，直接带初始值） */}
      <AdminPostEditDialog
        token={token}
        postId={postId}
        open={editingPost}
        initial={{ title: post.title, content: post.content, section: post.section }}
        onOpenChange={setEditingPost}
        onSaved={() => void load()}
      />
    </>
  )
}

/**
 * 管理员回复卡片（顶层评论或楼中楼通用）
 *
 * - 顶层评论渲染其楼中楼子回复（缩进 + 左竖线，与用户视角一致）；
 * - 编辑：行内展开 textarea；删除：内联二次确认（顶层评论提示级联楼中楼）。
 */
function AdminReplyCard({
  token,
  reply,
  isTop,
  onChanged,
}: {
  token: string
  reply: ReplyItem | CommentItem
  isTop: boolean
  onChanged: () => void
}) {
  const { t } = useTranslation('community')
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(reply.content)
  const [deleting, setDeleting] = useState(false)
  const [acting, setActing] = useState(false)

  const handleSave = async () => {
    if (acting) return
    const trimmed = draft.trim()
    if (!trimmed) return
    setActing(true)
    try {
      await communityAdminUpdateReply(token, reply.replyId, trimmed)
      toast.success(t('admin.replyUpdated'))
      setEditing(false)
      onChanged()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setActing(false)
    }
  }

  const handleDelete = async () => {
    if (acting) return
    setActing(true)
    try {
      await communityAdminDeleteReply(token, reply.replyId)
      toast.success(t('admin.replyDeleted'))
      onChanged()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setActing(false)
    }
  }

  const children = isTop ? (reply as CommentItem).replies : undefined

  return (
    <div className={cn('rounded-lg border bg-card p-3', !isTop && 'border-l-2 border-l-muted')}>
      <div className="flex items-center gap-2">
        <span className="text-base leading-none">{getCommunityAvatar(reply.author.avatarIndex)}</span>
        <span className="max-w-40 truncate text-xs font-medium">{reply.author.nickname}</span>
        <span className="text-muted-foreground ml-auto text-[11px] tabular-nums">
          {formatCommunityTime(reply.createdAt, t)}
        </span>
        {!editing && (
          <div className="flex shrink-0 items-center gap-0.5">
            <Button
              variant="ghost"
              size="sm"
              className="text-muted-foreground hover:text-foreground h-6 px-1.5 text-[11px]"
              onClick={() => {
                setDraft(reply.content)
                setEditing(true)
                setDeleting(false)
              }}
            >
              <i className="fa-solid fa-pen size-2.5" />
            </Button>
            {deleting ? (
              <>
                <Button
                  variant="destructive"
                  size="sm"
                  className="h-6 px-1.5 text-[11px]"
                  disabled={acting}
                  onClick={() => void handleDelete()}
                >
                  {t('admin.deleteConfirm')}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-muted-foreground h-6 px-1.5 text-[11px]"
                  onClick={() => setDeleting(false)}
                >
                  {t('post.cancel')}
                </Button>
              </>
            ) : (
              <Button
                variant="ghost"
                size="sm"
                className="text-muted-foreground hover:text-destructive h-6 px-1.5 text-[11px]"
                title={isTop ? t('admin.deleteCascadeHint') : undefined}
                onClick={() => setDeleting(true)}
              >
                <i className="fa-solid fa-trash size-2.5" />
              </Button>
            )}
          </div>
        )}
      </div>
      {editing ? (
        <div className="mt-1.5 space-y-1.5">
          <Textarea
            value={draft}
            maxLength={1000}
            placeholder={t('admin.editReplyPlaceholder')}
            onChange={(e) => setDraft(e.target.value)}
            className="min-h-16 text-xs leading-relaxed"
          />
          <div className="flex items-center justify-end gap-1">
            <span className="text-muted-foreground mr-auto text-[10px] tabular-nums">
              {draft.length}/1000
            </span>
            <Button
              variant="ghost"
              size="sm"
              className="text-muted-foreground h-6 px-2 text-[11px]"
              onClick={() => setEditing(false)}
            >
              {t('post.cancel')}
            </Button>
            <Button
              size="sm"
              className="h-6 px-2 text-[11px]"
              disabled={acting || !draft.trim()}
              onClick={() => void handleSave()}
            >
              {acting && <i className="fa-solid fa-spinner fa-spin mr-1 size-2.5" />}
              {t('admin.save')}
            </Button>
          </div>
        </div>
      ) : (
        <p className="text-muted-foreground mt-1 whitespace-pre-wrap text-xs leading-relaxed">
          {reply.content}
        </p>
      )}
      {/* 楼中楼子回复（仅顶层渲染） */}
      {isTop && children && children.length > 0 && (
        <div className="mt-2 space-y-2 border-l-2 pl-3">
          {children.map((sub) => (
            <AdminReplyCard key={sub.replyId} token={token} reply={sub} isTop={false} onChanged={onChanged} />
          ))}
        </div>
      )}
    </div>
  )
}

/**
 * 编辑帖子弹窗（D10）
 *
 * 打开时拉取最新详情填充（列表仅含 excerpt）；`initial` 提供时直接使用（详情视图复用）。
 * 保存提交 title / content / section 三字段（Worker 侧部分更新语义）。
 */
function AdminPostEditDialog({
  token,
  postId,
  open,
  initial,
  onOpenChange,
  onSaved,
}: {
  token: string
  /** null（由列表打开，弹窗自行拉详情）或显式 true/false（详情视图控制） */
  postId: number | null
  open?: boolean
  /** 初始表单值（详情视图已有全文时传入，避免重复请求） */
  initial?: { title: string; content: string; section: CommunitySection }
  onOpenChange: (open: boolean) => void
  onSaved: () => void
}) {
  const { t } = useTranslation('community')
  // 列表模式：postId 非 null 即视为打开（由父组件切换 postId 控制）
  const isOpen = open ?? postId != null
  const [title, setTitle] = useState('')
  const [content, setContent] = useState('')
  const [section, setSection] = useState<CommunitySection>('chat')
  const [loading, setLoading] = useState(false)
  const [submitting, setSubmitting] = useState(false)

  // 打开时填充表单：优先 initial，否则拉取详情
  useEffect(() => {
    if (!isOpen || postId == null) return
    if (initial) {
      setTitle(initial.title)
      setContent(initial.content)
      setSection(initial.section)
      return
    }
    let cancelled = false
    setLoading(true)
    communityAdminGetPost(token, postId)
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
  const contentValid = content.trim().length > 0 && content.length <= 5000

  const handleSubmit = async () => {
    if (postId == null || !titleValid || !contentValid || submitting) return
    setSubmitting(true)
    try {
      await communityAdminUpdatePost(token, postId, {
        title: title.trim(),
        content: content.trim(),
        section,
      })
      toast.success(t('admin.postUpdated'))
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
      <DialogContent className="h-[98vh] max-w-lg">
        <DialogHeader>
          <DialogTitle className="text-sm">{t('admin.editPostTitle')}</DialogTitle>
          <DialogDescription className="text-xs">{t('admin.editPostDesc')}</DialogDescription>
        </DialogHeader>

        {loading ? (
          <div className="text-muted-foreground flex h-40 items-center justify-center gap-2 text-xs">
            <i className="fa-solid fa-spinner fa-spin size-3.5" />
            {t('loadError.loading')}
          </div>
        ) : (
          <div className="space-y-3 py-2">
            {/* 板块选择（与发帖弹窗一致的三按钮组） */}
            <div className="space-y-1">
              <Label className="text-xs">{t('post.sectionLabel')}</Label>
              <div className="flex gap-1.5">
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
                <Label htmlFor="admin-post-title" className="text-xs">
                  {t('post.titleLabel')}
                </Label>
                <span className="text-muted-foreground text-[10px] tabular-nums">
                  {title.trim().length}/80
                </span>
              </div>
              <Input
                id="admin-post-title"
                value={title}
                maxLength={80}
                onChange={(e) => setTitle(e.target.value)}
                className="h-8 text-xs"
              />
            </div>

            <div className="space-y-1">
              <div className="flex items-center justify-between">
                <Label htmlFor="admin-post-content" className="text-xs">
                  {t('post.contentLabel')}
                </Label>
                <span className="text-muted-foreground text-[10px] tabular-nums">
                  {content.length}/5000
                </span>
              </div>
              <Textarea
                id="admin-post-content"
                value={content}
                maxLength={5000}
                onChange={(e) => setContent(e.target.value)}
                className="h-[50vh] font-mono text-xs leading-relaxed"
              />
            </div>
          </div>
        )}

        <DialogFooter>
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
            {t('admin.save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

/** 管理页空状态 */
function AdminEmpty({ text }: { text: string }) {
  return (
    <div className="text-muted-foreground flex h-24 flex-col items-center justify-center gap-1.5 text-xs">
      <i className="fa-solid fa-inbox size-5 opacity-50" />
      {text}
    </div>
  )
}
