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
  AdminReportItem,
  AdminUserItem,
  CheckInStats,
  CommunityLocalState,
  CommunitySection,
  CreatePostInput,
  CreateReplyInput,
  MyPostsData,
  MyRepliesData,
  PostDetailData,
  PostListData,
  ProfileData,
  ProfileUser,
  ReportInput,
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

/** 签到（同 UTC 日重复 → Worker 409） */
export async function communityCheckIn(): Promise<CheckInStats> {
  return invokeCommand<CheckInStats>('community_check_in')
}

/** 我的帖子 */
export async function getCommunityMyPosts(cursor?: string, limit?: number): Promise<MyPostsData> {
  return invokeCommand<MyPostsData>('community_get_my_posts', { cursor: cursor ?? null, limit: limit ?? null })
}

/** 我的回复 */
export async function getCommunityMyReplies(cursor?: string, limit?: number): Promise<MyRepliesData> {
  return invokeCommand<MyRepliesData>('community_get_my_replies', { cursor: cursor ?? null, limit: limit ?? null })
}

/** 举报帖子 / 回复 */
export async function reportCommunityContent(input: ReportInput): Promise<number> {
  return invokeCommand<number>('community_report', { input })
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

/** 管理员：举报列表 */
export async function communityAdminGetReports(adminToken: string): Promise<AdminReportItem[]> {
  return invokeCommand<AdminReportItem[]>('community_admin_get_reports', { adminToken })
}

/** 管理员：处理举报 */
export async function communityAdminResolveReport(adminToken: string, reportId: number): Promise<void> {
  return invokeCommand<void>('community_admin_resolve_report', { adminToken, reportId })
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
 * 帖子列表（游标分页，支持刷新与加载更多）
 *
 * `section` 变化时自动重新加载（null = 最近 / 全部板块）；
 * 切回已加载过的板块会重新拉取第一页（实现简单，列表规模小可接受）。
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

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const data = await getCommunityPosts(undefined, pageSize, section ?? undefined)
      setPosts(data.posts)
      setCursor(data.nextCursor)
      setHasMore(data.nextCursor != null)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [pageSize, section])

  useEffect(() => {
    // 已初始化且板块未变化时跳过（防 StrictMode 双执行）；板块变化时重新加载
    if (initialized.current && loadedSection.current === section) return
    initialized.current = true
    loadedSection.current = section
    void refresh()
  }, [refresh, section])

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

  return { posts, loading, loadingMore, hasMore, error, refresh, loadMore }
}

/**
 * 我的资料 + 签到统计
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
    if (enabled) void refresh()
  }, [enabled, refresh])

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
