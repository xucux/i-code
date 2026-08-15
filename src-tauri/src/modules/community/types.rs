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
}

/// 签到 / 数据统计（§8.3：纯计数 + 连续天数）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckInStats {
    pub total_check_ins: i64,
    pub streak_days: i64,
    pub post_count: i64,
    pub reply_count: i64,
    pub today_checked_in: bool,
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
    pub post_count: i64,
    pub reply_count: i64,
    pub created_at: String,
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
