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

/** 左列视图：最近（全部）/ 板块 / 我的帖子 / 我的回复 */
export type CommunityView = 'latest' | CommunitySection | 'myPosts' | 'myReplies'

/** 作者摘要（帖子/回复通用） */
export interface UserBrief {
  userId: string
  nickname: string
  avatarIndex: number
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
  createdAt: string
  author: UserBrief
}

/** 楼中楼回复项（第 2 层） */
export interface ReplyItem {
  replyId: number
  content: string
  createdAt: string
  author: UserBrief
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
}

/** 签到 / 数据统计（纯计数 + 连续天数） */
export interface CheckInStats {
  totalCheckIns: number
  streakDays: number
  postCount: number
  replyCount: number
  todayCheckedIn: boolean
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
  postCount: number
  replyCount: number
  createdAt: string
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
