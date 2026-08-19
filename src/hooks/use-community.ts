/**
 * 社区模块数据访问 hooks
 *
 * 统一封装 `community_*` Tauri Command 调用（内部 invokeCommand + toast），
 * 业务组件禁止散落 invoke。设计见 `docs/proposals/community.md` §8。
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { invokeCommand } from '@/hooks/use-command'
import type {
  AdminLoginData,
  AdminMuteInput,
  AdminPostListData,
  AdminReportItem,
  AdminUpdateGovernanceInput,
  AdminUpdatePostInput,
  AdminUserItem,
  CheckInResult,
  CommunityLocalState,
  CommunitySection,
  CreatePostInput,
  CreateReplyInput,
  MyPostsData,
  MyRepliesData,
  PointsLeaderboardData,
  PostDetailData,
  PostListData,
  ProfileData,
  ProfileUser,
  ReportInput,
  SiteGovernance,
  UpdateMyPostInput,
  UpdateProfileInput,
} from '@/modules/community/types'

// ===== API 封装（纯函数，详情页等一次性场景直接使用）=====

/** 读取社区本地状态（门禁开关 / 身份缓存） */
export async function getCommunityState(): Promise<CommunityLocalState> {
  return invokeCommand<CommunityLocalState>('community_get_local_state')
}

/** 设置门禁开关（开启时后端自动生成设备身份） */
export async function setCommunityEnabled(enabled: boolean): Promise<CommunityLocalState> {
  return invokeCommand<CommunityLocalState>('community_set_enabled', { enabled })
}

/** 帖子列表（游标分页；section 缺省 = 最近/全部板块） */
export async function getCommunityPosts(
  cursor?: string,
  limit?: number,
  section?: CommunitySection,
): Promise<PostListData> {
  return invokeCommand<PostListData>('community_get_posts', {
    cursor: cursor ?? null,
    limit: limit ?? null,
    section: section ?? null,
  })
}

/** 帖子详情 + 评论区（含楼中楼） */
export async function getCommunityPost(postId: number): Promise<PostDetailData> {
  return invokeCommand<PostDetailData>('community_get_post', { postId })
}

/** 发帖，返回 post_id */
export async function createCommunityPost(input: CreatePostInput): Promise<number> {
  return invokeCommand<number>('community_create_post', { input })
}

/** 回复 / 楼中楼，返回 reply_id */
export async function createCommunityReply(postId: number, input: CreateReplyInput): Promise<number> {
  return invokeCommand<number>('community_create_reply', { postId, input })
}

/** 我的资料 + 签到统计 */
export async function getCommunityProfile(): Promise<ProfileData> {
  return invokeCommand<ProfileData>('community_get_profile')
}

/** 改昵称 / 头像 */
export async function updateCommunityProfile(input: UpdateProfileInput): Promise<ProfileUser> {
  return invokeCommand<ProfileUser>('community_update_profile', { input })
}

/** 签到（同 UTC 日重复 → Worker 409）；返回统计 + 本次获得积分 */
export async function communityCheckIn(): Promise<CheckInResult> {
  return invokeCommand<CheckInResult>('community_check_in')
}

/** 我的帖子 */
export async function getCommunityMyPosts(cursor?: string, limit?: number): Promise<MyPostsData> {
  return invokeCommand<MyPostsData>('community_get_my_posts', { cursor: cursor ?? null, limit: limit ?? null })
}

/** 我的回复 */
export async function getCommunityMyReplies(cursor?: string, limit?: number): Promise<MyRepliesData> {
  return invokeCommand<MyRepliesData>('community_get_my_replies', { cursor: cursor ?? null, limit: limit ?? null })
}

/** 编辑自己的帖子（title / content / section 至少一项） */
export async function updateCommunityMyPost(postId: number, input: UpdateMyPostInput): Promise<void> {
  return invokeCommand<void>('community_update_my_post', { postId, input })
}

/** 删除自己的帖子（Worker 级联删除其全部回复与相关举报） */
export async function deleteCommunityMyPost(postId: number): Promise<void> {
  return invokeCommand<void>('community_delete_my_post', { postId })
}

/** 编辑自己的回复 */
export async function updateCommunityMyReply(replyId: number, content: string): Promise<void> {
  return invokeCommand<void>('community_update_my_reply', { replyId, content })
}

/** 删除自己的回复（顶层评论级联楼中楼） */
export async function deleteCommunityMyReply(replyId: number): Promise<void> {
  return invokeCommand<void>('community_delete_my_reply', { replyId })
}

/** 举报帖子 / 回复 */
export async function reportCommunityContent(input: ReportInput): Promise<number> {
  return invokeCommand<number>('community_report', { input })
}

/** 全站治理开关（D11：用户端只读，用于禁用发帖 / 回复入口） */
export async function getCommunitySiteGovernance(): Promise<SiteGovernance> {
  return invokeCommand<SiteGovernance>('community_get_site_governance')
}

/** 积分排行（offset 分页；Worker 侧过滤封禁用户，禁言用户仍展示） */
export async function getCommunityPointsLeaderboard(
  offset?: number,
  limit?: number,
): Promise<PointsLeaderboardData> {
  return invokeCommand<PointsLeaderboardData>('community_get_points_leaderboard', {
    offset: offset ?? null,
    limit: limit ?? null,
  })
}

// ===== 管理员 =====

/** 管理员登录（固定凭据，Worker 校验后返回短期 adminToken） */
export async function communityAdminLogin(username: string, password: string): Promise<AdminLoginData> {
  return invokeCommand<AdminLoginData>('community_admin_login', {
    input: { username, password },
  })
}

/** 管理员：用户列表 */
export async function communityAdminGetUsers(adminToken: string): Promise<AdminUserItem[]> {
  return invokeCommand<AdminUserItem[]>('community_admin_get_users', { adminToken })
}

/** 管理员：封禁用户 */
export async function communityAdminBanUser(adminToken: string, userId: string, reason?: string): Promise<void> {
  return invokeCommand<void>('community_admin_ban', { adminToken, userId, reason: reason ?? null })
}

/** 管理员：解封用户 */
export async function communityAdminUnbanUser(adminToken: string, userId: string): Promise<void> {
  return invokeCommand<void>('community_admin_unban', { adminToken, userId })
}

/** 管理员：禁言用户（D12：设置时长 / 永久 + 原因） */
export async function communityAdminMuteUser(
  adminToken: string,
  userId: string,
  input: AdminMuteInput
): Promise<void> {
  return invokeCommand<void>('community_admin_mute_user', { adminToken, userId, input })
}

/** 管理员：解除用户禁言 */
export async function communityAdminUnmuteUser(adminToken: string, userId: string): Promise<void> {
  return invokeCommand<void>('community_admin_unmute_user', { adminToken, userId })
}

/** 管理员：举报列表 */
export async function communityAdminGetReports(adminToken: string): Promise<AdminReportItem[]> {
  return invokeCommand<AdminReportItem[]>('community_admin_get_reports', { adminToken })
}

/** 管理员：处理举报 */
export async function communityAdminResolveReport(adminToken: string, reportId: number): Promise<void> {
  return invokeCommand<void>('community_admin_resolve_report', { adminToken, reportId })
}

// ===== 管理员：帖子管理（D10）=====

/** 管理员：所有用户帖子列表（游标分页；section 可选过滤） */
export async function communityAdminGetPosts(
  adminToken: string,
  cursor?: string,
  limit?: number,
  section?: CommunitySection,
): Promise<AdminPostListData> {
  return invokeCommand<AdminPostListData>('community_admin_get_posts', {
    adminToken,
    cursor: cursor ?? null,
    limit: limit ?? null,
    section: section ?? null,
  })
}

/** 管理员：帖子详情 + 评论区（定位待处置回复） */
export async function communityAdminGetPost(adminToken: string, postId: number): Promise<PostDetailData> {
  return invokeCommand<PostDetailData>('community_admin_get_post', { adminToken, postId })
}

/** 管理员：编辑帖子（title / content / section 至少一项） */
export async function communityAdminUpdatePost(
  adminToken: string,
  postId: number,
  input: AdminUpdatePostInput,
): Promise<void> {
  return invokeCommand<void>('community_admin_update_post', { adminToken, postId, input })
}

/** 管理员：删除帖子（级联删除其全部回复与相关举报） */
export async function communityAdminDeletePost(adminToken: string, postId: number): Promise<void> {
  return invokeCommand<void>('community_admin_delete_post', { adminToken, postId })
}

/** 管理员：编辑回复 */
export async function communityAdminUpdateReply(adminToken: string, replyId: number, content: string): Promise<void> {
  return invokeCommand<void>('community_admin_update_reply', { adminToken, replyId, content })
}

/** 管理员：删除回复（顶层评论级联楼中楼） */
export async function communityAdminDeleteReply(adminToken: string, replyId: number): Promise<void> {
  return invokeCommand<void>('community_admin_delete_reply', { adminToken, replyId })
}

// ===== 管理员：站点治理（D11）=====

/** 管理员：读取全站治理开关 */
export async function communityAdminGetGovernance(adminToken: string): Promise<SiteGovernance> {
  return invokeCommand<SiteGovernance>('community_admin_get_governance', { adminToken })
}

/** 管理员：更新全站治理开关（muteAll / postLocked / replyLocked 至少一项） */
export async function communityAdminUpdateGovernance(
  adminToken: string,
  input: AdminUpdateGovernanceInput,
): Promise<SiteGovernance> {
  return invokeCommand<SiteGovernance>('community_admin_update_governance', { adminToken, input })
}

/** 管理员：锁定 / 解锁帖子（locked=1 时该帖禁止新增评论回复） */
export async function communityAdminSetPostLocked(
  adminToken: string,
  postId: number,
  locked: boolean,
): Promise<void> {
  return invokeCommand<void>('community_admin_set_post_locked', { adminToken, postId, locked })
}

/** 管理员：置顶 / 取消置顶帖子（置顶帖在列表排序时排在最前） */
export async function communityAdminSetPostPin(
  adminToken: string,
  postId: number,
  pinned: boolean,
): Promise<void> {
  return invokeCommand<void>('community_admin_set_post_pin', { adminToken, postId, pinned })
}

// ===== 通用 hooks =====

/**
 * 社区门禁本地状态
 *
 * - `state` 为 null 表示尚未加载完成
 * - `enable()` / `disable()` 切换门禁并刷新状态
 */
export function useCommunityState(): {
  state: CommunityLocalState | null
  pending: boolean
  reload: () => Promise<void>
  setEnabled: (enabled: boolean) => Promise<CommunityLocalState | null>
} {
  const [state, setState] = useState<CommunityLocalState | null>(null)
  const [pending, setPending] = useState(false)

  const reload = useCallback(async () => {
    try {
      setState(await getCommunityState())
    } catch {
      setState(null)
    }
  }, [])

  useEffect(() => {
    void reload()
  }, [reload])

  const setEnabled = useCallback(async (enabled: boolean) => {
    setPending(true)
    try {
      const next = await setCommunityEnabled(enabled)
      setState(next)
      return next
    } catch {
      return null
    } finally {
      setPending(false)
    }
  }, [])

  return { state, pending, reload, setEnabled }
}

/**
 * 帖子列表第一页缓存条目
 */
interface FirstPageCacheEntry {
  posts: PostListData['posts']
  nextCursor: string | null
  cachedAt: number
}

/**
 * 模块级第一页缓存：key = section（null → 'latest'）
 *
 * 板块 Tab 切换复用新鲜缓存、不发请求，过期才后台重拉，
 * 大幅降低列表请求频率（避免触达 Worker 读限流 60 次/60 秒）。
 */
const firstPageCache = new Map<string, FirstPageCacheEntry>()

/** 缓存有效期（ms）：30 秒内切回同一板块直接使用缓存 */
const FIRST_PAGE_TTL = 30_000

/**
 * 帖子列表（游标分页，支持刷新与加载更多）
 *
 * `section` 变化时自动加载对应板块（null = 最近 / 全部）：
 * - 命中新鲜缓存（< 30s）→ 直接展示，不发请求；
 * - 无缓存 / 已过期 → 请求第一页并写入缓存。
 * `refresh` 总是强制请求并更新缓存；`invalidate` 清空全部缓存（发帖后调用）。
 */
export function useCommunityPosts(
  section: CommunitySection | null = null,
  pageSize = 20,
): {
  posts: PostListData['posts']
  loading: boolean
  loadingMore: boolean
  hasMore: boolean
  error: string | null
  refresh: () => Promise<void>
  loadMore: () => Promise<void>
  /** 清空全部板块缓存（下次切换/刷新强制重拉），发帖后调用保证新帖可见 */
  invalidate: () => void
} {
  const [posts, setPosts] = useState<PostListData['posts']>([])
  const [cursor, setCursor] = useState<string | null>(null)
  const [hasMore, setHasMore] = useState(false)
  const [loading, setLoading] = useState(false)
  const [loadingMore, setLoadingMore] = useState(false)
  const [error, setError] = useState<string | null>(null)
  // 记录已发起的首次加载与已加载板块，避免 StrictMode 双执行重复请求，
  // 同时允许板块切换时重新加载
  const initialized = useRef(false)
  const loadedSection = useRef<CommunitySection | null>(section)

  /** 当前板块缓存 key（null → 'latest'） */
  const cacheKey = section ?? 'latest'

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const data = await getCommunityPosts(undefined, pageSize, section ?? undefined)
      setPosts(data.posts)
      setCursor(data.nextCursor)
      setHasMore(data.nextCursor != null)
      // 写入第一页缓存（Tab 切回时复用）
      firstPageCache.set(cacheKey, {
        posts: data.posts,
        nextCursor: data.nextCursor,
        cachedAt: Date.now(),
      })
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [pageSize, section, cacheKey])

  useEffect(() => {
    // 已初始化且板块未变化时跳过（防 StrictMode 双执行）；板块变化时重新加载
    if (initialized.current && loadedSection.current === section) return
    initialized.current = true
    loadedSection.current = section
    // 命中新鲜缓存：直接展示，不发请求（避免频繁切 Tab 触发限流）
    const cached = firstPageCache.get(cacheKey)
    if (cached && Date.now() - cached.cachedAt < FIRST_PAGE_TTL) {
      setPosts(cached.posts)
      setCursor(cached.nextCursor)
      setHasMore(cached.nextCursor != null)
      setError(null)
      return
    }
    void refresh()
  }, [refresh, section, cacheKey])

  const loadMore = useCallback(async () => {
    if (!cursor || loadingMore) return
    setLoadingMore(true)
    try {
      const data = await getCommunityPosts(cursor, pageSize, section ?? undefined)
      setPosts((prev) => [...prev, ...data.posts])
      setCursor(data.nextCursor)
      setHasMore(data.nextCursor != null)
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setLoadingMore(false)
    }
  }, [cursor, loadingMore, pageSize, section])

  /** 清空全部板块缓存（模块级 Map），发帖后调用保证新帖可见 */
  const invalidate = useCallback(() => {
    firstPageCache.clear()
  }, [])

  return { posts, loading, loadingMore, hasMore, error, refresh, loadMore, invalidate }
}

/**
 * 我的资料缓存入口（模块级单例）
 *
 * 社区的「我的信息」通常只有本机一个身份，故用单入口缓存即可（无需按 user_id 分键）。
 * 与帖子列表缓存（`firstPageCache`）同理，用于减少重复进入社区时的 Worker 请求次数。
 */
interface ProfileCacheEntry {
  profile: ProfileData
  cachedAt: number
}

/** 我的资料缓存有效期（ms）：6h 内再次进入社区直接展示缓存，不发请求（降低 Worker 调用频率） */
const PROFILE_TTL = 6 * 60 * 60 * 1000

/** 模块级我的资料缓存：成功拉取后写入，供下次进入社区兜底展示 */
let profileCache: ProfileCacheEntry | null = null

/**
 * 我的资料 + 签到统计
 *
 * - 初始加载（进入社区）优先命中新鲜缓存（< 6h）直接展示；
 * - `refresh` 总是强制请求并更新缓存，供签到 / 改资料 / 发帖后保持最新。
 */
export function useCommunityProfile(enabled: boolean): {
  profile: ProfileData | null
  loading: boolean
  notFound: boolean
  refresh: () => Promise<ProfileData | null>
} {
  const [profile, setProfile] = useState<ProfileData | null>(null)
  const [loading, setLoading] = useState(false)
  const [notFound, setNotFound] = useState(false)

  const refresh = useCallback(async () => {
    if (!enabled) return null
    setLoading(true)
    setNotFound(false)
    try {
      const data = await getCommunityProfile()
      setProfile(data)
      // 写入模块级缓存：下次进入社区新鲜期内直接复用
      profileCache = { profile: data, cachedAt: Date.now() }
      return data
    } catch {
      // 新设备尚未在 Worker 注册（404）→ 引导设置资料
      setProfile(null)
      setNotFound(true)
      return null
    } finally {
      setLoading(false)
    }
  }, [enabled])

  useEffect(() => {
    if (!enabled) return
    // 命中新鲜缓存：直接展示，不发请求（避免频繁进出社区触发 Worker 限流）
    if (profileCache && Date.now() - profileCache.cachedAt < PROFILE_TTL) {
      setProfile(profileCache.profile)
      setNotFound(false)
      return
    }
    void refresh()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled])

  return { profile, loading, notFound, refresh }
}

// ===== 工具 =====

/**
 * 社区时间展示：相对时间（X 分钟前 / X 小时前 / X 天前），超过 7 天落为本地日期
 *
 * 服务器时间为 UTC ISO 字符串，此处转本地时区展示；
 * 文案经 `t`（community.time.*）本地化，由调用方传入。
 */
export function formatCommunityTime(
  iso: string,
  t: (key: string, options?: Record<string, unknown>) => string
): string {
  const date = new Date(iso.endsWith('Z') || iso.includes('+') ? iso : `${iso}Z`)
  const time = date.getTime()
  if (Number.isNaN(time)) return iso

  const diff = Date.now() - time
  const minute = 60_000
  const hour = 60 * minute
  const day = 24 * hour

  if (diff < minute) return t('time.justNow')
  if (diff < hour) return t('time.minutesAgo', { count: Math.floor(diff / minute) })
  if (diff < day) return t('time.hoursAgo', { count: Math.floor(diff / hour) })
  if (diff < 7 * day) return t('time.daysAgo', { count: Math.floor(diff / day) })
  // 超过 7 天：YYYY-MM-DD 本地日期
  const y = date.getFullYear()
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${y}-${m}-${d}`
}
