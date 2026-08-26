/**
 * 社区模块类型定义
 *
 * 与 Rust 侧 `src-tauri/src/modules/community/types.rs` 的 DTO 一一对应
 * （serde camelCase），字段由 Rust 端透传 Worker REST API 响应。
 * 设计见 `docs/proposals/community.md`。
 */

// ===== 本地状态（门禁，不进 Worker）=====

/** 社区本地状态（存 app_settings，含门禁开关与身份缓存） */
export interface CommunityLocalState {
  /** 门禁开关：false = 未开启（展示模糊门禁页） */
  enabled: boolean
  /** Worker 基础地址（可切换备用域名） */
  baseUrl: string
  /** 64 hex 设备身份；null = 未生成 */
  userId: string | null
  /** 本地缓存的昵称（以 /users/me 为准） */
  nickname: string | null
  /** 本地缓存的头像索引（emoji 预设下标，见 avatars.ts，0 ~ COUNT-1） */
  avatarIndex: number | null
}

// ===== 帖子 =====

/** 帖子板块（固定三板块，与 Worker `SECTIONS` / D1 `posts.section` 一致） */
export type CommunitySection = 'chat' | 'eggs' | 'tech'

/** 板块顺序（顶部 Tab 展示顺序：闲聊 / 领鸡蛋 / 技术） */
export const COMMUNITY_SECTIONS: readonly CommunitySection[] = ['chat', 'eggs', 'tech'] as const

/** 左列视图：最近（全部）/ 板块 / 我的帖子 / 我的回复 / 积分排行 / 签到排行 */
export type CommunityView = 'latest' | CommunitySection | 'myPosts' | 'myReplies' | 'leaderboard' | 'checkInLeaderboard'

/** 作者摘要（帖子/回复通用） */
export interface UserBrief {
  userId: string
  nickname: string
  avatarIndex: number
  /** 当前是否被禁言（D12：懒删除，到期自动视为未禁言） */
  muted: boolean
}

/** 帖子列表项 */
export interface PostSummary {
  postId: number
  title: string
  /** 所属板块（chat / eggs / tech） */
  section: CommunitySection
  /** 正文截断摘要（Worker 侧取前 200 字） */
  excerpt: string
  replyCount: number
  /** 帖子级锁定（D11：locked=1 时禁止新增评论回复，存量保留展示） */
  locked: boolean
  /** 是否置顶（管理员置顶后列表排序置顶） */
  pinned: boolean
  createdAt: string
  author: UserBrief
}

/** 帖子列表响应（游标分页） */
export interface PostListData {
  posts: PostSummary[]
  nextCursor: string | null
}

/** 帖子详情 */
export interface PostDetail {
  postId: number
  title: string
  content: string
  /** 所属板块（chat / eggs / tech） */
  section: CommunitySection
  replyCount: number
  /** 帖子级锁定（D11：locked=1 时禁止新增评论回复） */
  locked: boolean
  /** 是否置顶（管理员置顶后列表排序置顶） */
  pinned: boolean
  createdAt: string
  author: UserBrief
}

/** 楼中楼回复项（第 2 层） */
export interface ReplyItem {
  replyId: number
  content: string
  createdAt: string
  author: UserBrief
  /** 实际回复目标作者昵称（仅二级回复：回复的是另一条二级评论时为 @昵称，否则为 null） */
  replyToNickname?: string | null
}

/** 顶层评论项（含楼中楼，深度限 2 层） */
export interface CommentItem extends ReplyItem {
  replies: ReplyItem[]
  /** 是否还有更多楼中楼（本页最多 50 条） */
  hasMoreReplies: boolean
}

/** 评论区（顶层评论分页） */
export interface CommentsData {
  items: CommentItem[]
  nextCursor: string | null
}

/** 帖子详情响应 */
export interface PostDetailData {
  post: PostDetail
  comments: CommentsData
}

// ===== 发帖 / 回复输入 =====

/** 发帖输入（标题 ≤ 80 字，正文 ≤ 5000 字） */
export interface CreatePostInput {
  title: string
  content: string
  /** 所属板块（chat / eggs / tech）；缺省由后端归一化为闲聊 */
  section?: CommunitySection
}

/** 回复 / 楼中楼输入（≤ 1000 字） */
export interface CreateReplyInput {
  content: string
  /** 父回复 ID；缺省 = 顶层评论 */
  parentReplyId?: number
}

// ===== 用户中心 =====

/** 资料用户 */
export interface ProfileUser {
  userId: string
  nickname: string
  avatarIndex: number
  banned: boolean
  banReason?: string | null
  /** 当前是否被禁言（D12） */
  muted: boolean
  /** 禁言到期时间（UTC ISO）；null = 未禁言或永久禁言 */
  muteUntil?: string | null
  /** 禁言原因 */
  muteReason?: string | null
}

/** 签到 / 数据统计（纯计数 + 连续天数 + 累计积分） */
export interface CheckInStats {
  totalCheckIns: number
  streakDays: number
  postCount: number
  replyCount: number
  todayCheckedIn: boolean
  /** 累计积分（Points，SUM(points_ledger.change)） */
  points: number
}

/** 签到结果：统计 + 本次获得积分（Rust 侧 `CheckInResult` 以 flatten 展开统计字段） */
export interface CheckInResult extends CheckInStats {
  /** 本次签到获得积分（基础分 + 连续奖励） */
  pointsEarned: number
  /** 连续满 5 天奖励积分（0 = 未触发） */
  streakBonus: number
}

/** 我的资料 + 签到统计响应 */
export interface ProfileData {
  user: ProfileUser
  stats: CheckInStats
}

/** 改资料输入 */
export interface UpdateProfileInput {
  nickname?: string
  avatarIndex?: number
}

/** 我的帖子项 */
export interface MyPostItem {
  postId: number
  title: string
  /** 所属板块（chat / eggs / tech） */
  section: CommunitySection
  excerpt: string
  replyCount: number
  /** 是否置顶（默认 false） */
  pinned: boolean
  createdAt: string
}

/** 我的帖子列表响应 */
export interface MyPostsData {
  posts: MyPostItem[]
  nextCursor: string | null
}

/** 我的回复所在帖子 */
export interface MyReplyPost {
  postId: number
  title: string
  /** 所属板块（chat / eggs / tech） */
  section: CommunitySection
}

/** 我的回复项（含所在帖子标题） */
export interface MyReplyItem {
  replyId: number
  content: string
  createdAt: string
  post: MyReplyPost
}

/** 我的回复列表响应 */
export interface MyRepliesData {
  replies: MyReplyItem[]
  nextCursor: string | null
}

/** 积分排行项（点进排行视图展示；Worker 聚合 points_ledger，过滤封禁用户，禁言用户仍展示） */
export interface PointsLeaderboardItem {
  /** 名次（顺延式：1/2/3…） */
  rank: number
  userId: string
  nickname: string
  avatarIndex: number
  /** 累计积分 */
  points: number
}

/** 积分排行响应（offset 分页） */
export interface PointsLeaderboardData {
  items: PointsLeaderboardItem[]
  /** 下一页 offset；null = 无更多 */
  nextOffset: number | null
}

/** 签到排行项（累计 / 连续两列表共用；未涉及维度为空） */
export interface CheckInLeaderboardItem {
  /** 名次（顺延式：1/2/3…） */
  rank: number
  userId: string
  nickname: string
  avatarIndex: number
  /** 累计签到天数（累计签到排行维度） */
  totalCheckIns?: number
  /** 当前连续签到天数（连续签到排行维度） */
  streakDays?: number
}

/** 签到排行响应（offset 分页；`total` / `streak` 两列表共用同一分页） */
export interface CheckInLeaderboardData {
  /** 累计签到排行（含 totalCheckIns） */
  total: CheckInLeaderboardItem[]
  /** 连续签到排行（含 streakDays） */
  streak: CheckInLeaderboardItem[]
  /** 下一页 offset；null = 无更多 */
  nextOffset: number | null
}

// ===== 举报 =====

/** 举报输入 */
export interface ReportInput {
  /** 'post' | 'reply' */
  targetType: 'post' | 'reply'
  targetId: number
  reason?: string
}

// ===== 管理员 =====

/** 管理员登录响应（短期 adminToken） */
export interface AdminLoginData {
  adminToken: string
  expiresIn: number
}

/** 管理员用户列表项 */
export interface AdminUserItem {
  userId: string
  nickname: string
  avatarIndex: number
  banned: boolean
  banReason?: string | null
  /** 是否处于禁言状态（D12） */
  muted: boolean
  /** 禁言到期时间（UTC ISO）；null = 未禁言或永久禁言 */
  muteUntil?: string | null
  /** 禁言原因 */
  muteReason?: string | null
  postCount: number
  replyCount: number
  createdAt: string
  /** 最近登录时间（UTC ISO）；null = 尚未记录登录 */
  lastLoginAt?: string | null
  /** 最近登录 IP；null = 尚未记录 */
  lastLoginIp?: string | null
}

/** 管理员禁言输入（D12：设置时长 / 永久 + 原因） */
export interface AdminMuteInput {
  /** 到期时间（UTC ISO 字符串）；省略 = 永久禁言 */
  until?: string | null
  /** 禁言原因（可选） */
  reason?: string
}

/** 管理员举报列表项 */
export interface AdminReportItem {
  reportId: number
  targetType: 'post' | 'reply'
  targetId: number
  reason?: string | null
  createdAt: string
  reporter: {
    nickname: string
    avatarIndex: number
  }
  /** 目标预览（帖子标题 / 回复内容） */
  targetPreview?: string | null
  /** 目标帖子 ID（帖子举报 = targetId；回复举报 = 所属帖子；帖子已删除时为 null） */
  postId?: number | null
  /** 目标帖子标题（跳转 / 展示用） */
  postTitle?: string | null
  /** 被举报目标作者（封禁 / 禁言用） */
  targetAuthor?: { userId: string; nickname: string; avatarIndex: number } | null
}

// ===== 管理员帖子管理（D10）=====

/** 管理员帖子列表项（所有用户，含作者摘要与 updatedAt） */
export interface AdminPostItem {
  postId: number
  title: string
  section: CommunitySection
  /** 正文截断摘要（Worker 侧取前 200 字） */
  excerpt: string
  replyCount: number
  /** 帖子级锁定（D11：locked=1 时禁止新增评论回复） */
  locked: boolean
  /** 是否置顶（管理员置顶后列表排序置顶） */
  pinned: boolean
  createdAt: string
  /** 最后编辑时间（管理员识别被编辑过的帖子） */
  updatedAt: string
  author: UserBrief
}

/** 管理员帖子列表响应（游标分页） */
export interface AdminPostListData {
  posts: AdminPostItem[]
  nextCursor: string | null
}

/** 管理员编辑帖子输入（部分更新：title / content / section 至少一项） */
export interface AdminUpdatePostInput {
  title?: string
  content?: string
  section?: CommunitySection
}

/** 编辑「我的帖子」输入（字段与管理端编辑一致：title / content / section 至少一项） */
export type UpdateMyPostInput = AdminUpdatePostInput

// ===== 站点治理（D11：全站禁言 / 禁发帖 / 禁回复 + 帖子级锁定）=====

/** 全站治理开关（存 Worker D1 `site_settings` 表，缺省全关） */
export interface SiteGovernance {
  /** 全站禁言：发帖 + 评论回复全部禁止（最高优先级） */
  muteAll: boolean
  /** 全站禁止发帖 */
  postLocked: boolean
  /** 全站禁止评论回复 */
  replyLocked: boolean
}

/** 管理员更新全站治理开关输入（部分更新：至少一项） */
export interface AdminUpdateGovernanceInput {
  muteAll?: boolean
  postLocked?: boolean
  replyLocked?: boolean
}

// ===== 消息通知（2026-08-23 通知迭代）=====

/** 消息通知项 */
export interface NotificationItem {
  notificationId: number
  /** 'reply'（内容被回复）| 'ban'（被封禁）| 'mute'（被禁言） */
  type: 'reply' | 'ban' | 'mute'
  /** 触发者昵称（reply 类 = 回复人；ban / mute 类为 null） */
  actorNickname?: string | null
  /** 通知正文（reply 类 = 回复预览；ban / mute 类 = 原因，可为空） */
  content?: string | null
  /** 关联帖子 ID（reply 类可点击跳转；null = 不可跳转） */
  postId?: number | null
  /** 关联帖子标题（帖子已删除时为 null） */
  postTitle?: string | null
  /** 禁言到期时间（UTC ISO）；mute 类 null = 永久禁言 */
  until?: string | null
  isRead: boolean
  createdAt: string
}

/** 通知列表响应（游标分页 + 未读数） */
export interface NotificationListData {
  items: NotificationItem[]
  nextCursor: string | null
  /** 未读数（供小红点；列表接口顺带返回） */
  unreadCount: number
}

/** 未读通知数响应 */
export interface UnreadCountData {
  unreadCount: number
}

/** 全部标记已读响应 */
export interface ReadAllNotificationsData {
  updated: number
}

// ===== 帖子外链分享（2026-08-26 分享迭代，见 docs/proposals/community-post-share.md）=====

/** 发起分享输入（maxViews 缺省 1000，范围 1~10000；发起扣 100 积分，Worker 校验） */
export interface ShareLinkInput {
  /** 访问配额上限（1~10000）；缺省 1000 */
  maxViews?: number
}

/** 分享链接（用户视角；url 由 Worker 按请求域根拼装直链） */
export interface ShareLink {
  /** 8 位 base62 随机短码（/s/{pid} 直链） */
  pid: string
  /** 被分享的帖子 ID */
  postId: number
  /** 访问配额上限 */
  maxViews: number
  /** 已访问次数（阈值自动封顶，原子自增） */
  views: number
  createdAt: string
  /** 组装好的直链（如 https://community-beta.tenma.work/s/{pid}） */
  url: string
}

/** 帖内分享列表响应（游标分页） */
export interface ShareLinkListData {
  items: ShareLink[]
  nextCursor: string | null
}

/** 管理员分享项（含帖子标题与创建者摘要） */
export interface AdminShareItem extends ShareLink {
  /** 分享所在帖子标题 */
  postTitle: string
  /** 创建者（= 帖子作者）摘要 */
  author: UserBrief
}

/** 管理员分享列表响应（游标分页，可按帖子过滤） */
export interface AdminShareListData {
  items: AdminShareItem[]
  nextCursor: string | null
}
