//! # 社区模块类型定义（DTO / 领域类型）
//!
//! 与 Worker REST API（i-code-community-worker）的 JSON 字段对齐，字段统一 camelCase，
//! 便于前端 `src/modules/community/types.ts` 直接使用（由后续前端开发维护）。
//!
//! 设计见 `docs/proposals/community.md`。

use serde::{Deserialize, Serialize};

// ===== 本地状态（存 app_settings.community_json，见 §7.3）=====

/// 社区默认基础地址（§5.1：自定义域名，/api/v1 为 REST 前缀）
pub fn default_base_url() -> String {
    "https://community-beta.tenma.work/api/v1".to_string()
}

/// 社区本地状态
///
/// 只保存在本机（`app_settings.community_json`），不随社区请求上传：
/// - `user_id`：机器标识加盐哈希（64 hex），null = 未生成
/// - `nickname` / `avatar_index`：本地缓存，启动/拉取资料时以 /users/me 为准
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityLocalState {
    /// 门禁开关：false = 未开启（前端展示模糊门禁页）
    #[serde(default)]
    pub enabled: bool,
    /// Worker 基础地址（默认 community-beta.tenma.work/api/v1，可切换备用域名）
    #[serde(default = "default_base_url")]
    pub base_url: String,
    /// 64 hex 设备身份；null = 未生成
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// 本地缓存的昵称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    /// 本地缓存的头像索引（0~29 预设 emoji）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_index: Option<i64>,
}

impl Default for CommunityLocalState {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: default_base_url(),
            user_id: None,
            nickname: None,
            avatar_index: None,
        }
    }
}

// ===== 帖子 =====

/// 固定板块枚举值（与 Worker 侧 `SECTIONS` / D1 `posts.section` 一致）
///
/// `chat` = 闲聊 / `eggs` = 领鸡蛋 / `tech` = 技术；
/// 前端「最近」Tab = 不带板块过滤（全部帖子按时间倒序）。
pub const SECTIONS: &[&str] = &["chat", "eggs", "tech"];

/// 校验板块值是否合法（None 视为合法 = 最近 / 缺省闲聊，由调用方决定语义）
pub fn is_valid_section(section: &str) -> bool {
    SECTIONS.contains(&section)
}

/// serde 缺省板块（兼容旧 Worker 响应：无 section 字段时按闲聊处理）
fn default_section() -> String {
    "chat".to_string()
}

/// 作者摘要（帖子/回复通用）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserBrief {
    pub user_id: String,
    pub nickname: String,
    pub avatar_index: i64,
    /// 当前是否被禁言（D12：懒删除，到期自动视为未禁言）
    #[serde(default)]
    pub muted: bool,
}

/// 帖子列表项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostSummary {
    pub post_id: i64,
    pub title: String,
    /// 所属板块（chat / eggs / tech）
    #[serde(default = "default_section")]
    pub section: String,
    /// 正文截断摘要（Worker 侧取前 200 字）
    pub excerpt: String,
    pub reply_count: i64,
    /// 帖子级锁定（D11：locked=1 时禁止新增评论回复）
    #[serde(default)]
    pub locked: bool,
    /// 是否置顶（管理员置顶后列表排序置顶，默认 false）
    #[serde(default)]
    pub pinned: bool,
    pub created_at: String,
    pub author: UserBrief,
}

/// 帖子列表响应（游标分页）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostListData {
    pub posts: Vec<PostSummary>,
    pub next_cursor: Option<String>,
}

/// 帖子详情
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostDetail {
    pub post_id: i64,
    pub title: String,
    pub content: String,
    /// 所属板块（chat / eggs / tech）
    #[serde(default = "default_section")]
    pub section: String,
    pub reply_count: i64,
    /// 帖子级锁定（D11：locked=1 时禁止新增评论回复）
    #[serde(default)]
    pub locked: bool,
    /// 是否置顶（管理员置顶后列表排序置顶，默认 false）
    #[serde(default)]
    pub pinned: bool,
    pub created_at: String,
    pub author: UserBrief,
}

/// 楼中楼回复项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplyItem {
    pub reply_id: i64,
    pub content: String,
    pub created_at: String,
    pub author: UserBrief,
}

/// 顶层评论项（含楼中楼，深度限 2 层）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentItem {
    pub reply_id: i64,
    pub content: String,
    pub created_at: String,
    pub author: UserBrief,
    /// 楼中楼子回复（每顶层最多 50 条）
    #[serde(default)]
    pub replies: Vec<ReplyItem>,
    /// 是否还有更多楼中楼（前端展示「加载更多」占位）
    #[serde(default)]
    pub has_more_replies: bool,
}

/// 评论区（顶层评论分页）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentsData {
    pub items: Vec<CommentItem>,
    pub next_cursor: Option<String>,
}

/// 帖子详情响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostDetailData {
    pub post: PostDetail,
    pub comments: CommentsData,
}

// ===== 发帖 / 回复输入 =====

/// 发帖输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePostInput {
    pub title: String,
    pub content: String,
    /// 所属板块（chat / eggs / tech）；None = 缺省闲聊（Service 层归一化）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub section: Option<String>,
}

/// 回复 / 楼中楼输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReplyInput {
    pub content: String,
    /// 父回复 ID；None = 顶层评论，Some = 楼中楼
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_reply_id: Option<i64>,
}

// ===== 用户中心 =====

/// 资料用户
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUser {
    pub user_id: String,
    pub nickname: String,
    pub avatar_index: i64,
    pub banned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ban_reason: Option<String>,
    /// 当前是否被禁言（D12）
    #[serde(default)]
    pub muted: bool,
    /// 禁言到期时间（UTC ISO）；None = 未禁言或永久禁言
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mute_until: Option<String>,
    /// 禁言原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mute_reason: Option<String>,
}

/// 签到 / 数据统计（§8.3：纯计数 + 连续天数 + 累计积分）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckInStats {
    pub total_check_ins: i64,
    pub streak_days: i64,
    pub post_count: i64,
    pub reply_count: i64,
    pub today_checked_in: bool,
    /// 累计积分（Points，`SUM(points_ledger.change)`）
    #[serde(default)]
    pub points: i64,
}

/// 签到结果：统计 + 本次获得积分
///
/// Worker `/users/me/check-in` 返回 `{ ...stats, pointsEarned, streakBonus }`，
/// 此处用 `#[serde(flatten)]` 展开统计字段，前端可直接取 `stats` 与 `pointsEarned`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckInResult {
    #[serde(flatten)]
    pub stats: CheckInStats,
    /// 本次签到获得积分（基础分 + 连续奖励）
    pub points_earned: i64,
    /// 连续满 5 天奖励积分（0 = 未触发）
    #[serde(default)]
    pub streak_bonus: i64,
}

/// 我的资料 + 签到统计响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileData {
    pub user: ProfileUser,
    pub stats: CheckInStats,
}

/// 改资料输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_index: Option<i64>,
}

/// 我的帖子项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyPostItem {
    pub post_id: i64,
    pub title: String,
    /// 所属板块（chat / eggs / tech）
    #[serde(default = "default_section")]
    pub section: String,
    pub excerpt: String,
    pub reply_count: i64,
    /// 是否置顶（默认 false）
    #[serde(default)]
    pub pinned: bool,
    pub created_at: String,
}

/// 我的帖子列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyPostsData {
    pub posts: Vec<MyPostItem>,
    pub next_cursor: Option<String>,
}

/// 我的回复项（含所在帖子标题）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyReplyItem {
    pub reply_id: i64,
    pub content: String,
    pub created_at: String,
    pub post: MyReplyPost,
}

/// 我的回复所在帖子
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyReplyPost {
    pub post_id: i64,
    pub title: String,
    /// 所属板块（chat / eggs / tech）
    #[serde(default = "default_section")]
    pub section: String,
}

/// 我的回复列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyRepliesData {
    pub replies: Vec<MyReplyItem>,
    pub next_cursor: Option<String>,
}

/// 积分排行项（Points leaderboard）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointsLeaderboardItem {
    /// 名次（顺延式：1/2/3…）
    pub rank: i64,
    pub user_id: String,
    pub nickname: String,
    pub avatar_index: i64,
    /// 累计积分
    pub points: i64,
}

/// 积分排行响应（offset 分页）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointsLeaderboardData {
    pub items: Vec<PointsLeaderboardItem>,
    /// 下一页 offset；None = 无更多
    pub next_offset: Option<i64>,
}

// ===== 举报 =====

/// 举报输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportInput {
    /// 'post' | 'reply'
    pub target_type: String,
    pub target_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// ===== 管理员（§5.3）=====

/// 管理员登录输入（用户手动输入固定凭据）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminLoginInput {
    pub username: String,
    pub password: String,
}

/// 管理员登录响应（短期 adminToken，客户端持有）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminLoginData {
    pub admin_token: String,
    pub expires_in_seconds: i64,
}

/// 管理员用户列表项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserItem {
    pub user_id: String,
    pub nickname: String,
    pub avatar_index: i64,
    pub banned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ban_reason: Option<String>,
    /// 是否处于禁言状态（D12）
    #[serde(default)]
    pub muted: bool,
    /// 禁言到期时间（UTC ISO）；None = 未禁言或永久禁言
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mute_until: Option<String>,
    /// 禁言原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mute_reason: Option<String>,
    pub post_count: i64,
    pub reply_count: i64,
    pub created_at: String,
    /// 最近登录时间（UTC ISO）；null = 尚未记录登录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_login_at: Option<String>,
    /// 最近登录 IP；null = 尚未记录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_login_ip: Option<String>,
}

/// 管理员禁言输入（D12：设置时长 / 永久 + 原因）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminMuteInput {
    /// 到期时间（UTC ISO 字符串）；None = 永久禁言
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    /// 禁言原因（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// 举报人摘要（管理员视角）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportReporter {
    pub nickname: String,
    pub avatar_index: i64,
}

/// 管理员举报列表项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminReportItem {
    pub report_id: i64,
    pub target_type: String,
    pub target_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub created_at: String,
    pub reporter: ReportReporter,
    /// 目标预览（帖子标题 / 回复内容）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_preview: Option<String>,
}

// ===== 管理员帖子管理（D10，2026-08-15）=====

/// 管理员帖子列表项（所有用户，含作者摘要与 updatedAt）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPostItem {
    pub post_id: i64,
    pub title: String,
    /// 所属板块（chat / eggs / tech）
    #[serde(default = "default_section")]
    pub section: String,
    /// 正文截断摘要（Worker 侧取前 200 字）
    pub excerpt: String,
    pub reply_count: i64,
    /// 帖子级锁定（D11：locked=1 时禁止新增评论回复）
    #[serde(default)]
    pub locked: bool,
    /// 是否置顶（管理员置顶后列表排序置顶，默认 false）
    #[serde(default)]
    pub pinned: bool,
    pub created_at: String,
    /// 最后编辑时间（管理员识别被编辑过的帖子）
    pub updated_at: String,
    pub author: UserBrief,
}

/// 管理员帖子列表响应（游标分页）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPostListData {
    pub posts: Vec<AdminPostItem>,
    pub next_cursor: Option<String>,
}

/// 管理员编辑帖子输入（部分更新：title / content / section 至少一项）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUpdatePostInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// 所属板块（chat / eggs / tech）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
}

/// 编辑「我的帖子」输入（字段与管理员编辑帖子一致：title / content / section 至少一项）
pub type UpdateMyPostInput = AdminUpdatePostInput;

// ===== 站点治理（D11，2026-08-15：全站禁言 / 禁发帖 / 禁回复 + 帖子级锁定）=====

/// 全站治理开关（存 Worker D1 `site_settings` 表，缺省全关）
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteGovernance {
    /// 全站禁言：发帖 + 评论回复全部禁止（最高优先级）
    pub mute_all: bool,
    /// 全站禁止发帖
    pub post_locked: bool,
    /// 全站禁止评论回复
    pub reply_locked: bool,
}

/// 管理员更新全站治理开关输入（部分更新：至少一项）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUpdateGovernanceInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mute_all: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_locked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_locked: Option<bool>,
}
