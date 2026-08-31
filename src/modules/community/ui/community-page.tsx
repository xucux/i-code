/**
 * 社区主界面页面组件（/community，由 routes/community/index.tsx 引用）
 *
 * - 未开启门禁：整页模糊 + 开关确认（§8.2）
 * - 已开启：左列帖子列表（最近 / 闲聊 / 领鸡蛋 / 技术）
 *   + 右侧个人栏（§8.3，「我的帖子 / 我的回复」由个人栏入口进入）
 * - 首次开启且无资料 → 自动弹层引导设置昵称与头像
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { cn } from '@/lib/utils'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { useAutoHideScrollbar } from '@/hooks/use-auto-hide-scrollbar'
import {
  communityAuthAnonymous,
  communityAuthBind,
  communityAuthLogin,
  communityAuthLogout,
  communityAuthRegister,
  communityCheckIn,
  createCommunityPost,
  getCommunitySiteGovernance,
  getCommunityUnreadCount,
  updateCommunityProfile,
  useCommunityPosts,
  useCommunityProfile,
  useCommunityState,
} from '@/hooks/use-community'
import { CommunityGate } from '@/modules/community/ui/community-gate'
import { BindAccountDialog } from '@/modules/community/ui/bind-account-dialog'
import { CheckInLeaderboardList } from '@/modules/community/ui/checkin-leaderboard-list'
import { CommunityProfilePanel } from '@/modules/community/ui/community-profile-panel'
import { CreatePostDialog } from '@/modules/community/ui/create-post-dialog'
import { MyContentList } from '@/modules/community/ui/my-content-list'
import { LeaderboardList } from '@/modules/community/ui/leaderboard-list'
import { NotificationListDialog } from '@/modules/community/ui/notification-list-dialog'
import { PostList } from '@/modules/community/ui/post-list'
import { ProfileSetupDialog } from '@/modules/community/ui/profile-setup-dialog'
import { getCommunityAvatar } from '@/modules/community/avatars'
import {
  COMMUNITY_SECTIONS,
  type CommunitySection,
  type CommunityView,
  type SiteGovernance,
} from '@/modules/community/types'

/** 判断视图是否为板块视图 */
function isSectionView(view: CommunityView): view is CommunitySection {
  return view === 'chat' || view === 'eggs' || view === 'tech'
}

export function CommunityPage() {
  const { t } = useTranslation('community')
  const { state, pending: statePending, setEnabled, reload } = useCommunityState()
  const enabled = state?.enabled ?? false
  // 2026-08-31 鉴权迭代：已登录 = 门禁开启且已持有会话 token（60 天有效）
  const signedIn = enabled && !!state?.authToken

  // 资料 hook 须无条件调用（内部按 signedIn 门控；会话失效时 onUnauthorized 拉回登录卡）
  const { profile, loading: profileLoading, notFound, refresh: refreshProfile } = useCommunityProfile(
    signedIn,
    () => void reload()
  )

  const [view, setView] = useState<CommunityView>('latest')
  // 当前板块 Tab（最近 / 我的内容 = null）；发帖弹窗默认板块取自当前 Tab，缺省闲聊
  const activeSection = isSectionView(view) ? view : null

  const {
    posts,
    loading,
    loadingMore,
    hasMore,
    refresh: refreshPosts,
    loadMore,
    invalidate: invalidatePostsCache,
  } = useCommunityPosts(activeSection)

  const [profileDialogOpen, setProfileDialogOpen] = useState(false)
  const [profileDialogMode, setProfileDialogMode] = useState<'setup' | 'edit'>('setup')
  const [createOpen, setCreateOpen] = useState(false)
  const [checkInPending, setCheckInPending] = useState(false)
  // 2026-08-31 鉴权迭代：登录 / 绑定执行中（禁用按钮防重复提交）与绑定弹窗开关
  const [authBusy, setAuthBusy] = useState(false)
  const [bindOpen, setBindOpen] = useState(false)

  // 消息通知（通知迭代）：未读数小红点 + 通知列表弹层
  const [unreadCount, setUnreadCount] = useState(0)
  const [notifOpen, setNotifOpen] = useState(false)

  /** 拉取未读通知数（进入社区 / 重新启用时刷新小红点） */
  const refreshUnread = useCallback(async () => {
    if (!signedIn) return
    try {
      setUnreadCount(await getCommunityUnreadCount())
    } catch {
      // 未读数拉取失败按 0 处理（不阻塞社区浏览）
    }
  }, [signedIn])

  useEffect(() => {
    void refreshUnread()
  }, [refreshUnread])

  // 站点治理开关（D11）：开启社区后拉取，null = 加载中/失败（宽松处理，不阻塞浏览）
  const [governance, setGovernance] = useState<SiteGovernance | null>(null)
  // 禁言 / 禁发帖时禁用发帖入口（Worker 侧仍兜底拦截）
  const postDisabled = governance != null && (governance.muteAll || governance.postLocked)

  useEffect(() => {
    if (!signedIn) return
    let cancelled = false
    getCommunitySiteGovernance()
      .then((gov) => {
        if (!cancelled) setGovernance(gov)
      })
      .catch(() => {
        // 治理开关拉取失败按全关处理，不阻塞社区浏览
      })
    return () => {
      cancelled = true
    }
  }, [signedIn])

  // 布局高度：页头（标题 + Tabs）固定，左列滚动
  const [pageHeight, pageRef] = useAvailableHeight()
  const [headerHeight, headerRef] = useAvailableHeight()
  const [scrollRef, scrolling] = useAutoHideScrollbar()
  const listHeight = useMemo(
    () => Math.max(0, pageHeight - headerHeight - 32),
    [pageHeight, headerHeight]
  )

  // 登录后无资料（Worker 404 或昵称为空）→ 引导设置（仅自动弹一次）
  useEffect(() => {
    if (!signedIn || profileLoading || profileDialogOpen) return
    if (notFound || (profile && !profile.user.nickname)) {
      setProfileDialogMode('setup')
      setProfileDialogOpen(true)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [signedIn, profileLoading, notFound, profile])

  // ===== 登录态管理（2026-08-31 鉴权迭代，docs/proposals/community-auth-accounts.md）=====

  /** 登录成功统一收尾：重新拉取本地状态（含新 token / 用户名） */
  const finalizeAuth = async () => {
    await reload()
  }

  /** 匿名进入：未开启时先开启门禁（生成机器身份），再换取匿名 token */
  const handleAnonymous = async () => {
    setAuthBusy(true)
    try {
      if (!enabled) {
        await setEnabled(true)
      }
      await communityAuthAnonymous()
      await finalizeAuth()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setAuthBusy(false)
    }
  }

  /** 账号登录 / 注册（成功即进入社区；注册=新建独立身份，Worker 校验规则） */
  const handleAccountAuth = async (
    mode: 'login' | 'register',
    username: string,
    password: string
  ) => {
    setAuthBusy(true)
    try {
      if (!enabled) {
        await setEnabled(true)
      }
      const input = { username, password }
      if (mode === 'login') {
        await communityAuthLogin(input)
      } else {
        await communityAuthRegister(input)
      }
      await finalizeAuth()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setAuthBusy(false)
    }
  }

  /** 匿名绑定账号（D3）：成功后 Worker 吊销匿名 token 并签发 account token */
  const handleBind = async (username: string, password: string) => {
    setAuthBusy(true)
    try {
      await communityAuthBind({ username, password })
      setBindOpen(false)
      await finalizeAuth()
      toast.success(t('auth.bindSuccess'))
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setAuthBusy(false)
    }
  }

  /** 退出登录：吊销会话并回到登录卡 */
  const handleLogout = async () => {
    try {
      await communityAuthLogout()
      await finalizeAuth()
      toast.info(t('auth.logoutSuccess'))
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    }
  }

  /** 关闭社区：回到模糊门禁态（D8：可再次开启） */
  const handleCloseCommunity = async () => {
    await setEnabled(false)
    toast.info(t('gate.closedTip'))
  }

  /** 提交资料（setup 首次 / edit 编辑共用） */
  const handleProfileSubmit = async (nickname: string, avatarIndex: number) => {
    await updateCommunityProfile({ nickname, avatarIndex })
    toast.success(t('success.profile'))
    setProfileDialogOpen(false)
    await refreshProfile()
  }

  /** 签到：成功刷新资料；同日重复（409）按提示展示 */
  const handleCheckIn = async () => {
    setCheckInPending(true)
    try {
      const result = await communityCheckIn()
      // 签到成功 toast：本次获得积分 + 连续 5 天奖励
      if (result.streakBonus > 0) {
        toast.success(t('success.checkInBonus', { earned: result.pointsEarned, bonus: result.streakBonus }))
      } else {
        toast.success(t('success.checkInPoints', { earned: result.pointsEarned }))
      }
      await refreshProfile()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setCheckInPending(false)
    }
  }

  /** 发帖成功：清空列表缓存并跳到对应板块（保证新帖可见），同板块则直接刷新 */
  const handleCreatePost = async (title: string, content: string, section: CommunitySection) => {
    await createCommunityPost({ title, content, section })
    toast.success(t('success.post'))
    setCreateOpen(false)
    // 清空缓存：目标板块（及 latest）下次加载强制重拉，新帖立即可见
    invalidatePostsCache()
    if (view !== section) {
      // 切到发帖板块：useCommunityPosts 检测到 section 变化自动重拉
      setView(section)
    } else {
      await refreshPosts()
    }
    // 发帖数变化，同步刷新个人栏统计
    await refreshProfile()
  }

  // ===== 渲染 =====

  if (!state) {
    return (
      <div className="text-muted-foreground flex h-full items-center justify-center gap-2 text-xs">
        <i className="fa-solid fa-spinner fa-spin size-3.5" />
        {t('loadError.loading')}
      </div>
    )
  }

  // 未开启 或 已开启但未登录（登出 / token 失效）→ 登录 / 进入卡（2026-08-31 鉴权迭代）
  if (!signedIn) {
    return (
      <CommunityGate
        pending={statePending || authBusy}
        onAnonymous={() => void handleAnonymous()}
        onLogin={(username, password) => void handleAccountAuth('login', username, password)}
        onRegister={(username, password) => void handleAccountAuth('register', username, password)}
      />
    )
  }

  return (
    <div ref={pageRef} className="flex h-full flex-col p-4">
      {/* 页头：标题 + 工具 + 视图 Tabs（最近 / 闲聊 / 领鸡蛋 / 技术） */}
      <div ref={headerRef} className="mb-3 space-y-2">
        <div className="flex items-center gap-2">
          <h1 className="text-sm font-semibold">{t('title')}</h1>
          <Link
            to="/community/admin"
            className="text-muted-foreground hover:text-foreground"
            title={t('admin.title')}
          >
            <i className="fa-solid fa-user-shield size-3.5" />
          </Link>
          <div className="ml-auto flex items-center gap-1.5">
            <Button
              variant="ghost"
              size="icon"
              className="text-muted-foreground size-7"
              title={t('action.refresh')}
              disabled={loading}
              onClick={() =>
                view === 'myPosts' || view === 'myReplies' || view === 'leaderboard' || view === 'checkInLeaderboard'
                  ? setView('latest')
                  : void refreshPosts()
              }
            >
              <i className={loading ? 'fa-solid fa-spinner fa-spin size-3' : 'fa-solid fa-rotate size-3'} />
            </Button>
            <Button
              size="sm"
              className="h-7 text-xs"
              disabled={postDisabled}
              title={postDisabled ? t('governance.postDisabledTip') : undefined}
              onClick={() => setCreateOpen(true)}
            >
              <i className="fa-solid fa-pen-to-square mr-1.5 size-3" />
              {t('action.newPost')}
            </Button>
            {/* 消息通知：仅图标按钮，未读时展示小红点（通知迭代） */}
            <Button
              variant="ghost"
              size="icon"
              className="text-muted-foreground relative size-7"
              title={t('notifications.title')}
              onClick={() => setNotifOpen(true)}
            >
              <i className="fa-solid fa-bell size-3.5" />
              {unreadCount > 0 && (
                <span className="bg-destructive absolute right-0.5 top-0.5 size-2 rounded-full" />
              )}
            </Button>
          </div>
        </div>

        <Tabs value={view} onValueChange={(v) => setView(v as CommunityView)}>
          <TabsList className="h-7">
            <TabsTrigger value="latest" className="text-muted-foreground h-5 px-2 text-xs data-[state=active]:text-foreground">
              <i className="fa-solid fa-clock mr-1 size-2.5" />
              {t('tab.latest')}
            </TabsTrigger>
            {COMMUNITY_SECTIONS.map((s) => (
              <TabsTrigger
                key={s}
                value={s}
                className="text-muted-foreground h-5 px-2 text-xs data-[state=active]:text-foreground"
              >
                {t(`section.${s}`)}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
      </div>

      {/* 主体：左列列表（原生滚动）+ 右侧个人栏 */}
      <div className="flex h-[80vh] gap-3">
        <div className="min-w-0 flex-1">
          <div
            ref={scrollRef}
            className={cn(
              'overflow-y-auto pr-2 custom-scrollbar custom-scrollbar-auto-hide',
              scrolling && 'scrollbar-visible'
            )}
            style={{ height: listHeight || undefined }}
          >
            {view === 'myPosts' || view === 'myReplies' ? (
              <MyContentList kind={view === 'myPosts' ? 'posts' : 'replies'} />
            ) : view === 'leaderboard' ? (
              <LeaderboardList />
            ) : view === 'checkInLeaderboard' ? (
              <CheckInLeaderboardList />
            ) : (
              <PostList
                posts={posts}
                loading={loading}
                loadingMore={loadingMore}
                hasMore={hasMore}
                onRefresh={() => void refreshPosts()}
                onLoadMore={() => void loadMore()}
              />
            )}
          </div>
        </div>

        <div className="w-52 shrink-0">
          {profile ? (
            <CommunityProfilePanel
              profile={profile}
              activeView={view}
              onViewChange={(v) => setView(view === v ? 'latest' : v)}
              onEditProfile={() => {
                setProfileDialogMode('edit')
                setProfileDialogOpen(true)
              }}
              onCheckIn={() => void handleCheckIn()}
              checkInPending={checkInPending}
              onCloseCommunity={() => void handleCloseCommunity()}
              authMode={state.authMode}
              username={state.username}
              onBindAccount={() => setBindOpen(true)}
              onLogout={() => void handleLogout()}
            />
          ) : (
            // 无资料（未在 Worker 注册）：简化卡片引导设置
            <div className="rounded-lg border bg-card p-3 text-center">
              <div className="bg-muted mx-auto flex size-10 items-center justify-center rounded-full border text-xl">
                {getCommunityAvatar(state.avatarIndex)}
              </div>
              <p className="text-muted-foreground mt-2 text-xs">{t('profile.notFoundTip')}</p>
              <Button
                size="sm"
                variant="outline"
                className="mt-2 h-7 text-xs"
                onClick={() => {
                  setProfileDialogMode('setup')
                  setProfileDialogOpen(true)
                }}
              >
                {t('profile.setupTitle')}
              </Button>
            </div>
          )}
        </div>
      </div>

      {/* 弹层：资料设置 / 发帖 */}
      <ProfileSetupDialog
        open={profileDialogOpen}
        onOpenChange={setProfileDialogOpen}
        mode={profileDialogMode}
        defaultNickname={profile?.user.nickname ?? state.nickname ?? undefined}
        defaultAvatarIndex={profile?.user.avatarIndex ?? state.avatarIndex}
        onSubmit={handleProfileSubmit}
      />
      <CreatePostDialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        defaultSection={activeSection ?? 'chat'}
        onSubmit={handleCreatePost}
      />
      {/* 消息通知列表（打开时自动全部已读，成功后清空小红点） */}
      <NotificationListDialog
        open={notifOpen}
        onOpenChange={setNotifOpen}
        onUnreadCleared={() => setUnreadCount(0)}
      />
      {/* 匿名绑定账号（2026-08-31 鉴权迭代 D3） */}
      <BindAccountDialog
        open={bindOpen}
        onOpenChange={setBindOpen}
        onSubmit={(username, password) => void handleBind(username, password)}
        pending={authBusy}
      />
    </div>
  )
}
