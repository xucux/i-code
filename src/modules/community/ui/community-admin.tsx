/**
 * 社区管理员页面（§5.3 / D3）
 *
 * 登录 Worker 固定凭据 → 持有短期 adminToken（仅内存，不持久化，每次使用需重新登录）：
 * - 用户列表：封禁 / 解封
 * - 举报列表：查看目标预览并标记已处理
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ScrollPage } from '@/components/ui/scroll-page'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useAvailableHeight } from '@/hooks/use-available-height'
import {
  communityAdminBanUser,
  communityAdminGetReports,
  communityAdminGetUsers,
  communityAdminLogin,
  communityAdminResolveReport,
  communityAdminUnbanUser,
  formatCommunityTime,
} from '@/hooks/use-community'
import { getCommunityAvatar } from '@/modules/community/avatars'
import type { AdminReportItem, AdminUserItem } from '@/modules/community/types'

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

/** 已登录面板：用户 / 举报两个 Tab */
function AdminPanels({ token, listHeight }: { token: string; listHeight: number }) {
  const { t } = useTranslation('community')
  const [tab, setTab] = useState<'users' | 'reports'>('users')

  return (
    <Tabs
      value={tab}
      onValueChange={(v) => setTab(v as 'users' | 'reports')}
      className="flex min-h-0 flex-1 flex-col"
    >
      <TabsList className="mb-2 self-start">
        <TabsTrigger value="users" className="text-xs">
          {t('admin.users')}
        </TabsTrigger>
        <TabsTrigger value="reports" className="text-xs">
          {t('admin.reports')}
        </TabsTrigger>
      </TabsList>

      <TabsContent value="users" className="min-h-0 flex-1">
        <AdminUsersTab token={token} height={listHeight} />
      </TabsContent>
      <TabsContent value="reports" className="min-h-0 flex-1">
        <AdminReportsTab token={token} height={listHeight} />
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
              <Button variant="outline" size="sm" className="h-6 shrink-0 px-2 text-[11px]" disabled={acting} onClick={() => void handleResolve(report.reportId)}>
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

/** 管理页空状态 */
function AdminEmpty({ text }: { text: string }) {
  return (
    <div className="text-muted-foreground flex h-24 flex-col items-center justify-center gap-1.5 text-xs">
      <i className="fa-solid fa-inbox size-5 opacity-50" />
      {text}
    </div>
  )
}
