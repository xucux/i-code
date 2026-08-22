/**
 * 社区管理员页面（§5.3 / D3 + D10 + D11）
 *
 * 登录 Worker 固定凭据 → 持有短期 adminToken（仅内存，不持久化，每次使用需重新登录）：
 * - 用户列表：封禁 / 解封
 * - 举报列表：查看目标预览并标记已处理
 * - 帖子管理（D10）：所有用户帖子分页列表，编辑 / 删除帖子；进入详情可编辑 / 删除任意回复
 * - 站点治理（D11）：全站禁言 / 禁发帖 / 禁回复开关；帖子级锁定（禁止新增评论回复）
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
import { Switch } from '@/components/ui/switch'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Textarea } from '@/components/ui/textarea'
import { MarkdownContent } from '@/components/ui/markdown-content'
import { cn } from '@/lib/utils'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { formatDateTime } from '@/core/utils'
import {
  communityAdminBanUser,
  communityAdminDeletePost,
  communityAdminDeleteReply,
  communityAdminGetGovernance,
  communityAdminGetPost,
  communityAdminGetPosts,
  communityAdminGetReports,
  communityAdminGetUsers,
  communityAdminLogin,
  communityAdminMuteUser,
  communityAdminResolveReport,
  communityAdminSetPostLocked,
  communityAdminSetPostPin,
  communityAdminUnbanUser,
  communityAdminUnmuteUser,
  communityAdminUpdateGovernance,
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
  type SiteGovernance,
} from '@/modules/community/types'
import { MuteBadge } from '@/modules/community/ui/mute-badge'
import { SectionBadge } from '@/modules/community/ui/section-badge'

export function CommunityAdmin() {
  const { t } = useTranslation('community')
  const [token, setToken] = useState<string | null>(null)
  const [loggingIn, setLoggingIn] = useState(false)

  // 登录表单
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')

  // ===== adminToken 6h 持久化（D12：避免频繁重复登录）=====
  // 存 localStorage + 过期时间戳；读取时校验，过期则清除。
  const TOKEN_KEY = 'community.adminToken'
  const TOKEN_EXPIRY_KEY = 'community.adminTokenExpiry'
  /** 前端缓存时长 6 小时（服务端 TTL 7h，留有缓冲，避免时钟偏差提前 401） */
  const TOKEN_CACHE_MS = 6 * 60 * 60 * 1000

  // 初始化：尝试从 localStorage 恢复会话
  useEffect(() => {
    try {
      const cached = localStorage.getItem(TOKEN_KEY)
      const expiry = Number(localStorage.getItem(TOKEN_EXPIRY_KEY) ?? 0)
      if (cached && expiry > Date.now()) {
        setToken(cached)
      } else {
        localStorage.removeItem(TOKEN_KEY)
        localStorage.removeItem(TOKEN_EXPIRY_KEY)
      }
    } catch {
      // localStorage 不可用（隐私模式等）时按未登录处理，不影响功能
    }
  }, [])

  // 登录态重置（退出登录）：清内存 + 清 localStorage
  const logout = useCallback(() => {
    setToken(null)
    setPassword('')
    try {
      localStorage.removeItem(TOKEN_KEY)
      localStorage.removeItem(TOKEN_EXPIRY_KEY)
    } catch {
      // ignore
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const handleLogin = async () => {
    if (!username.trim() || !password || loggingIn) return
    setLoggingIn(true)
    try {
      const data = await communityAdminLogin(username.trim(), password)
      setToken(data.adminToken)
      try {
        localStorage.setItem(TOKEN_KEY, data.adminToken)
        localStorage.setItem(TOKEN_EXPIRY_KEY, String(Date.now() + TOKEN_CACHE_MS))
      } catch {
        // ignore
      }
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

/** 已登录面板：用户 / 举报 / 帖子管理 / 站点治理 四个 Tab */
function AdminPanels({ token, listHeight }: { token: string; listHeight: number }) {
  const { t } = useTranslation('community')
  const [tab, setTab] = useState<'users' | 'reports' | 'posts' | 'governance'>('users')

  return (
    <Tabs
      value={tab}
      onValueChange={(v) => setTab(v as 'users' | 'reports' | 'posts' | 'governance')}
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
        <TabsTrigger value="governance" className="text-xs">
          {t('admin.governance')}
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
      <TabsContent value="governance" className="min-h-0 flex-1">
        <AdminGovernanceTab token={token} />
      </TabsContent>
    </Tabs>
  )
}

/**
 * 站点治理 Tab（D11）：全站禁言 / 禁发帖 / 禁回复 三个开关
 *
 * - muteAll 开启时语义上已覆盖发帖与回复（Worker 侧最高优先级拦截），UI 提示但不联动禁用子开关；
 * - 每个开关独立提交（部分更新），成功后用返回值回填，避免并发下显示漂移。
 */
function AdminGovernanceTab({ token }: { token: string }) {
  const { t } = useTranslation('community')
  const [gov, setGov] = useState<SiteGovernance | null>(null)
  const [loading, setLoading] = useState(true)
  // 各开关独立 pending，防止切换期间重复提交
  const [pendingKey, setPendingKey] = useState<string | null>(null)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      setGov(await communityAdminGetGovernance(token))
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [token])

  useEffect(() => {
    void load()
  }, [load])

  /** 切换单个开关（提交后用服务端返回值回填） */
  const handleToggle = async (key: 'muteAll' | 'postLocked' | 'replyLocked', value: boolean) => {
    if (pendingKey) return
    setPendingKey(key)
    // 乐观更新，失败回滚由 load 保证（此处直接以返回值覆盖）
    try {
      const next = await communityAdminUpdateGovernance(token, { [key]: value })
      setGov(next)
      toast.success(t('admin.governanceUpdated'))
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setPendingKey(null)
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

  if (!gov) {
    return <AdminEmpty text={t('admin.governanceLoadFailed')} />
  }

  /** 开关行配置（key 与 SiteGovernance 字段一一对应） */
  const rows: { key: 'muteAll' | 'postLocked' | 'replyLocked'; icon: string }[] = [
    { key: 'muteAll', icon: 'fa-volume-xmark' },
    { key: 'postLocked', icon: 'fa-file-circle-xmark' },
    { key: 'replyLocked', icon: 'fa-comments' },
  ]

  return (
    <ScrollPage variant="borderless">
      <div className="space-y-2 pr-2">
        {/* 生效优先级说明 */}
        <p className="text-muted-foreground text-[11px] leading-relaxed">
          <i className="fa-solid fa-circle-info mr-1 size-2.5" />
          {t('admin.governanceHint')}
        </p>
        {rows.map(({ key, icon }) => (
          <div key={key} className="flex items-center gap-3 rounded-lg border bg-card p-3">
            <i className={cn('text-muted-foreground size-4', `fa-solid ${icon}`)} />
            <div className="min-w-0 flex-1">
              <p className="text-xs font-medium">{t(`admin.${key}`)}</p>
              <p className="text-muted-foreground mt-0.5 text-[11px]">{t(`admin.${key}Desc`)}</p>
            </div>
            <Switch
              checked={gov[key]}
              disabled={pendingKey === key}
              onCheckedChange={(v) => void handleToggle(key, v)}
            />
          </div>
        ))}
      </div>
    </ScrollPage>
  )
}

/** 用户列表 Tab：封禁 / 解封 + 禁言 / 解禁 */
function AdminUsersTab({ token, height }: { token: string; height: number }) {
  const { t } = useTranslation('community')
  const [users, setUsers] = useState<AdminUserItem[]>([])
  const [loading, setLoading] = useState(true)
  // 待确认封禁的用户（展开原因输入）
  const [banningId, setBanningId] = useState<string | null>(null)
  const [banReason, setBanReason] = useState('')
  // 禁言弹窗目标用户
  const [muteTarget, setMuteTarget] = useState<AdminUserItem | null>(null)
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

  /** 解除禁言 */
  const handleUnmute = async (userId: string) => {
    if (acting) return
    setActing(true)
    try {
      await communityAdminUnmuteUser(token, userId)
      toast.success(t('admin.unmuteDone'))
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
      <div className="space-y-2 pr-2 pb-20">
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
              <MuteBadge muted={user.muted} until={user.muteUntil} />
              <span className="text-muted-foreground ml-auto text-[11px] tabular-nums">
                <i className="fa-solid fa-file-lines mr-0.5 size-2.5" />
                {user.postCount}
                <i className="fa-solid fa-comment ml-1.5 mr-0.5 size-2.5" />
                {user.replyCount}
              </span>
              {user.muted ? (
                <Button variant="outline" size="sm" className="h-6 shrink-0 px-2 text-[11px]" disabled={acting} onClick={() => void handleUnmute(user.userId)}>
                  <i className="fa-solid fa-volume-high mr-1 size-2.5" />
                  {t('admin.unmute')}
                </Button>
              ) : (
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-muted-foreground hover:text-amber-500 h-6 shrink-0 px-2 text-[11px]"
                  disabled={user.banned}
                  title={user.banned ? t('admin.muteDisabledWhenBanned') : undefined}
                  onClick={() => setMuteTarget(user)}
                >
                  <i className="fa-solid fa-volume-xmark mr-1 size-2.5" />
                  {t('admin.mute')}
                </Button>
              )}
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
            {/* 注册时间 / 最近登录时间 / 最近登录 IP */}
            <div className="text-muted-foreground mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] tabular-nums">
              <span>
                <i className="fa-regular fa-calendar mr-1 size-2.5" />
                {t('admin.registeredAt', { time: formatDateTime(user.createdAt).slice(0, 10) })}
              </span>
              {user.lastLoginAt && (
                <span>
                  <i className="fa-solid fa-right-to-bracket mr-1 size-2.5" />
                  {t('admin.lastLoginAt', { time: formatCommunityTime(user.lastLoginAt, t) })}
                </span>
              )}
              {user.lastLoginIp && (
                <span>
                  <i className="fa-solid fa-location-dot mr-1 size-2.5" />
                  {t('admin.lastLoginIp', { ip: user.lastLoginIp })}
                </span>
              )}
            </div>
            {user.banned && user.banReason && (
              <p className="text-destructive mt-1.5 text-[11px]">{t('profile.banReason', { reason: user.banReason })}</p>
            )}
            {user.muted && user.muteReason && (
              <p className="text-amber-600 mt-1.5 text-[11px]">{t('profile.muteReason', { reason: user.muteReason })}</p>
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
      {/* 禁言弹窗（设置时长 / 永久 + 原因） */}
      <MuteUserDialog
        token={token}
        user={muteTarget}
        onOpenChange={(open) => !open && setMuteTarget(null)}
        onSaved={() => void load()}
      />
    </ScrollPage>
  )
}

/**
 * 禁言弹窗（D12）：选择时长（预设小时 / 永久）+ 输入原因
 *
 * 提交后调用禁言接口（until 为 UTC ISO 或 null=永久），成功回调 onSaved 刷新列表。
 */
function MuteUserDialog({
  token,
  user,
  onOpenChange,
  onSaved,
}: {
  token: string
  user: AdminUserItem | null
  onOpenChange: (open: boolean) => void
  onSaved: () => void
}) {
  const { t } = useTranslation('community')
  const [hours, setHours] = useState<number>(24)
  const [permanent, setPermanent] = useState(false)
  const [reason, setReason] = useState('')
  const [submitting, setSubmitting] = useState(false)

  const isOpen = user != null
  // 打开时重置表单
  useEffect(() => {
    if (isOpen) {
      setHours(24)
      setPermanent(false)
      setReason('')
    }
  }, [isOpen])

  /** 预设时长选项（小时）；0 表示切换到「永久」 */
  const presets = [1, 6, 24, 72, 168, 720]

  const handleSubmit = async () => {
    if (!user || submitting) return
    setSubmitting(true)
    try {
      // until：永久 → null；否则 now + hours 的 RFC3339 字符串（与 Worker 校验一致）
      const until = permanent
        ? null
        : new Date(Date.now() + hours * 60 * 60 * 1000).toISOString()
      await communityAdminMuteUser(token, user.userId, {
        until: until ?? null,
        reason: reason.trim() || undefined,
      })
      toast.success(t('admin.muteDone'))
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
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="text-sm">{t('admin.muteTitle')}</DialogTitle>
          <DialogDescription className="text-xs">
            {user ? t('admin.muteDesc', { name: user.nickname }) : ''}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-1">
          {/* 时长选择：预设按钮组 + 永久开关 */}
          <div className="space-y-1">
            <Label className="text-xs">{t('admin.muteDuration')}</Label>
            <div className="flex flex-wrap gap-1.5">
              {presets.map((h) => (
                <button
                  key={h}
                  type="button"
                  disabled={permanent}
                  className={cn(
                    'h-7 rounded-md border px-2 text-xs transition-colors',
                    !permanent && hours === h
                      ? 'border-primary bg-primary text-primary-foreground'
                      : 'bg-background text-muted-foreground hover:bg-muted hover:text-foreground'
                  )}
                  onClick={() => {
                    setHours(h)
                    setPermanent(false)
                  }}
                >
                  {t('admin.muteHours', { hours: h })}
                </button>
              ))}
            </div>
            <div className="flex items-center gap-2 pt-1">
              <Switch checked={permanent} onCheckedChange={(v) => setPermanent(v)} />
              <span className="text-muted-foreground text-xs">{t('admin.mutePermanent')}</span>
            </div>
          </div>

          {/* 原因 */}
          <div className="space-y-1">
            <Label htmlFor="mute-reason" className="text-xs">
              {t('admin.muteReason')}
            </Label>
            <Input
              id="mute-reason"
              value={reason}
              maxLength={100}
              placeholder={t('admin.muteReasonPlaceholder')}
              onChange={(e) => setReason(e.target.value)}
              className="h-8 text-xs"
            />
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" size="sm" className="h-8 text-xs" onClick={() => onOpenChange(false)}>
            {t('post.cancel')}
          </Button>
          <Button size="sm" className="h-8 text-xs" disabled={submitting} onClick={handleSubmit}>
            {submitting && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
            {t('admin.save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
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

  /** 锁定 / 解锁帖子（D11：锁定后禁止新增评论回复，存量保留） */
  const handleToggleLock = async (postId: number, locked: boolean) => {
    if (acting) return
    setActing(true)
    try {
      await communityAdminSetPostLocked(token, postId, locked)
      toast.success(locked ? t('admin.postLockedDone') : t('admin.postUnlockedDone'))
      // 就地更新列表项，避免整页刷新闪烁
      setPosts((prev) => prev.map((p) => (p.postId === postId ? { ...p, locked } : p)))
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setActing(false)
    }
  }

  /** 置顶 / 取消置顶帖子（置顶帖在列表排序时排在最前） */
  const handleTogglePin = async (postId: number, pinned: boolean) => {
    if (acting) return
    setActing(true)
    try {
      await communityAdminSetPostPin(token, postId, pinned)
      toast.success(pinned ? t('admin.postPinnedDone') : t('admin.postUnpinnedDone'))
      // 就地更新列表项的置顶状态，避免整页刷新闪烁
      setPosts((prev) => prev.map((p) => (p.postId === postId ? { ...p, pinned } : p)))
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
        <div className="space-y-2 pr-2 pb-20">
          {posts.map((post) => (
            <div key={post.postId} className="rounded-lg border bg-card p-3">
              <div className="flex items-center gap-2">
                <span className="text-lg leading-none">{getCommunityAvatar(post.author.avatarIndex)}</span>
                <span className="max-w-40 truncate text-xs font-medium">{post.author.nickname}</span>
                <MuteBadge muted={post.author.muted} />
                <SectionBadge section={post.section} />
                {post.pinned && (
                  <Badge variant="secondary" className="h-4 gap-0.5 px-1 text-[10px]">
                    <i className="fa-solid fa-thumbtack size-2" />
                    {t('admin.pinned')}
                  </Badge>
                )}
                {post.locked && (
                  <Badge variant="outline" className="h-4 px-1 text-[10px]">
                    <i className="fa-solid fa-lock mr-0.5 size-2" />
                    {t('admin.locked')}
                  </Badge>
                )}
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
                <Button
                  variant="ghost"
                  size="sm"
                  className={cn(
                    'h-6 px-2 text-[11px]',
                    post.pinned
                      ? 'text-primary hover:text-foreground'
                      : 'text-muted-foreground hover:text-primary'
                  )}
                  disabled={acting}
                  title={post.pinned ? t('admin.unpinPost') : t('admin.pinPost')}
                  onClick={() => void handleTogglePin(post.postId, !post.pinned)}
                >
                  <i className="fa-solid fa-thumbtack mr-1 size-2.5" />
                  {post.pinned ? t('admin.unpinPost') : t('admin.pinPost')}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  className={cn(
                    'h-6 px-2 text-[11px]',
                    post.locked
                      ? 'text-muted-foreground hover:text-foreground'
                      : 'text-muted-foreground hover:text-amber-500'
                  )}
                  disabled={acting}
                  title={post.locked ? t('admin.unlockPost') : t('admin.lockPost')}
                  onClick={() => void handleToggleLock(post.postId, !post.locked)}
                >
                  <i className={cn('mr-1 size-2.5', post.locked ? 'fa-solid fa-lock-open' : 'fa-solid fa-lock')} />
                  {post.locked ? t('admin.unlockPost') : t('admin.lockPost')}
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

  /** 锁定 / 解锁帖子（D11）：就地更新详情，避免整页刷新 */
  const handleToggleLock = async () => {
    if (!detail || acting) return
    setActing(true)
    try {
      const next = !detail.post.locked
      await communityAdminSetPostLocked(token, postId, next)
      toast.success(next ? t('admin.postLockedDone') : t('admin.postUnlockedDone'))
      setDetail({ ...detail, post: { ...detail.post, locked: next } })
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
                className={cn(
                  'h-7 px-2 text-[11px]',
                  detail.post.locked
                    ? 'text-amber-500 hover:text-foreground'
                    : 'text-muted-foreground hover:text-amber-500'
                )}
                disabled={acting}
                onClick={() => void handleToggleLock()}
              >
                <i className={cn('mr-1 size-2.5', detail.post.locked ? 'fa-solid fa-lock' : 'fa-solid fa-lock-open')} />
                {detail.post.locked ? t('admin.unlockPost') : t('admin.lockPost')}
              </Button>
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
              <MuteBadge muted={post.author.muted} />
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
        <MuteBadge muted={reply.author.muted} />
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
