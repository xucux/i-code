/**
 * 社区主界面页面组件（/community，由 routes/community/index.tsx 引用）
 *
 * - 未开启门禁：整页模糊 + 开关确认（§8.2）
 * - 已开启：左列帖子列表（最近 / 闲聊 / 领鸡蛋 / 技术）
 *   + 右侧个人栏（§8.3，「我的帖子 / 我的回复」由个人栏入口进入）
 * - 首次开启且无资料 → 自动弹层引导设置昵称与头像
 */

import { useEffect, useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useAvailableHeight } from '@/hooks/use-available-height'
import {
  communityCheckIn,
  createCommunityPost,
  getCommunitySiteGovernance,
  updateCommunityProfile,
  useCommunityPosts,
  useCommunityProfile,
  useCommunityState,
} from '@/hooks/use-community'
import { CommunityGate } from '@/modules/community/ui/community-gate'
import { CheckInLeaderboardList } from '@/modules/community/ui/checkin-leaderboard-list'
import { CommunityProfilePanel } from '@/modules/community/ui/community-profile-panel'
import { CreatePostDialog } from '@/modules/community/ui/create-post-dialog'
import { MyContentList } from '@/modules/community/ui/my-content-list'
import { LeaderboardList } from '@/modules/community/ui/leaderboard-list'
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
  const { state, pending: statePending, setEnabled } = useCommunityState()
  const enabled = state?.enabled ?? false

  // 资料 / 帖子 hooks 须无条件调用（内部按 enabled 门控）
  const { profile, loading: profileLoading, notFound, refresh: refreshProfile } = useCommunityProfile(enabled)

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

  // 站点治理开关（D11）：开启社区后拉取，null = 加载中/失败（宽松处理，不阻塞浏览）
  const [governance, setGovernance] = useState<SiteGovernance | null>(null)
  // 禁言 / 禁发帖时禁用发帖入口（Worker 侧仍兜底拦截）
  const postDisabled = governance != null && (governance.muteAll || governance.postLocked)

  useEffect(() => {
    if (!enabled) return
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
  }, [enabled])

  // 布局高度：页头（标题 + Tabs）固定，左列滚动
  const [pageHeight, pageRef] = useAvailableHeight()
  const [headerHeight, headerRef] = useAvailableHeight()
  const listHeight = useMemo(
    () => Math.max(0, pageHeight - headerHeight - 32),
    [pageHeight, headerHeight]
  )

  // 开启后无资料（Worker 404 或昵称为空）→ 引导设置（仅自动弹一次）
  useEffect(() => {
    if (!enabled || profileLoading || profileDialogOpen) return
    if (notFound || (profile && !profile.user.nickname)) {
      setProfileDialogMode('setup')
      setProfileDialogOpen(true)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, profileLoading, notFound, profile])

  /** 开启社区（后端生成设备身份，随后自动拉取资料并按需引导设置） */
  const handleEnable = () => {
    void setEnabled(true)
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

  if (!enabled) {
    return <CommunityGate pending={statePending} onEnable={handleEnable} />
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
          <div className="overflow-y-auto pr-2 custom-scrollbar" style={{ height: listHeight || undefined }}>
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
    </div>
  )
}
