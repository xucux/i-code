//! # 社区 Worker REST API 客户端（§7.2）
//!
//! - `base_url` 由调用方传入（读取自 `app_settings.community.base_url`）
//! - 复用 `shared::apply_global_proxy` 应用全局代理
//! - `X-User-Id` 从本地状态注入；`X-App-Token` 为编译期常量
//! - 请求头校验（§5.1 / §6）：`User-Agent` 为 `i-code/X.Y.Z`、`Referer` 为合法主域，
//!   不满足会被 Worker 将来源 IP 记入 `ip_blocklist` 并阻拦 48 小时
//! - 响应先验 `code`：非 0 转业务错误；429 → 限流文案；错误不暴露 SQL / 堆栈

use std::time::Duration;

use reqwest::header::{HeaderValue, REFERER, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Method;
use serde::de::DeserializeOwned;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::shared;

use super::types::{
    AdminLoginData, AdminLoginInput, AdminMuteInput, AdminPostListData, AdminReportItem,
    AdminUpdateGovernanceInput, AdminUpdatePostInput, AdminUserItem, CheckInLeaderboardData,
    CheckInResult, CreatePostInput, CreateReplyInput, MyPostsData, MyRepliesData,
    NotificationListData, PointsLeaderboardData, PostDetailData, PostListData, ProfileData,
    ProfileUser, ReadAllNotificationsData, ReportInput, SiteGovernance, UnreadCountData,
    UpdateMyPostInput, UpdateProfileInput,
};

/// App Token：与 Worker 侧 `APP_TOKEN`（wrangler.toml `[vars]`）保持一致，
/// 防止外部脚本直接刷接口（§5.1）
pub const APP_TOKEN: &str = "i-code-community-app-token-prod";

/// 请求 User-Agent：须匹配 Worker 侧 `i-code/\d+\.\d+\.\d+`（§5.1 请求头检查）。
/// 用 `CARGO_PKG_VERSION`（如 `0.2.2`）拼装，随版本自动变化。
const USER_AGENT: &str = concat!("i-code/", env!("CARGO_PKG_VERSION"));

/// 请求 Referer：须为 Worker 白名单内的主域（§5.1 / §6）。
/// 即使 `base_url` 切换到备用域名，Referer 仍需保持主域，故用固定常量。
/// 命名 `REFERER_VALUE` 避免与 `reqwest::header::REFERER`（头名常量）冲突。
const REFERER_VALUE: &str = "https://community-beta.tenma.work/";

/// 读接口超时（秒）
const READ_TIMEOUT_SECS: u64 = 15;
/// 写接口超时（秒）
const WRITE_TIMEOUT_SECS: u64 = 10;

/// Worker 统一响应包裹
#[derive(Debug, serde::Deserialize)]
struct ApiEnvelope<T> {
    /// 0 = 成功；非 0 = 业务错误码（通常等于 HTTP 状态码）
    code: i64,
    message: String,
    // Option<T> 缺失时自动为 None（无需 #[serde(default)]，避免引入 T: Default 约束）
    data: Option<T>,
}

/// 发帖返回（data.postId）
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostCreated {
    post_id: i64,
}

/// 回复返回（data.replyId）
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplyCreated {
    reply_id: i64,
}

/// 举报返回（data.reportId）
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportCreated {
    report_id: i64,
}

/// 改资料返回（data.user）
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileUserData {
    user: ProfileUser,
}

/// 管理员用户列表返回（data.users）
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminUsersData {
    users: Vec<AdminUserItem>,
}

/// 管理员举报列表返回（data.reports）
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminReportsData {
    reports: Vec<AdminReportItem>,
}

/// 构建 HTTP 客户端（复用全局代理）
fn build_client(write: bool) -> IcodeResult<reqwest::Client> {
    let timeout = if write {
        Duration::from_secs(WRITE_TIMEOUT_SECS)
    } else {
        Duration::from_secs(READ_TIMEOUT_SECS)
    };
    let builder = reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(USER_AGENT);
    shared::apply_global_proxy(builder)
        .build()
        .map_err(|e| IcodeError::internal(format!("创建社区 HTTP 客户端失败：{e}")))
}

/// 将 HTTP 状态码 + Worker 消息映射为 `IcodeError`（§5.1）
fn map_status(status: u16, message: String) -> IcodeError {
    match status {
        400 => IcodeError::validation(message),
        401 => IcodeError::unauthorized(message),
        403 => IcodeError::forbidden(message),
        404 => IcodeError::new("NOT_FOUND", message),
        409 => IcodeError::conflict(message),
        // 429 → 统一转「操作过于频繁」文案
        429 => IcodeError::new("RATE_LIMITED", "操作过于频繁，请稍后再试"),
        _ => IcodeError::gateway(message),
    }
}

/// 从错误响应体提取可读 message；失败时回退到 HTTP 状态描述
fn extract_message(text: &str, status: reqwest::StatusCode) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(m) = v.get("message").and_then(|m| m.as_str()) {
            if !m.is_empty() {
                return m.to_string();
            }
        }
    }
    format!("社区服务错误（HTTP {}）", status.as_u16())
}

/// 统一请求发送核心
///
/// - `path`：相对路径（如 `"posts"`、`"users/me/check-in"`），自动拼接到 base_url
/// - `query`：可选的查询参数（游标分页）
/// - `user_id`：`X-User-Id` 头（用户接口必带，管理员接口可空）
/// - `admin_token`：`Authorization: Bearer`（管理员接口）
/// - `body`：JSON 请求体（写接口）
/// - `write`：是否走写超时
async fn send<T: DeserializeOwned>(
    base_url: &str,
    method: Method,
    path: &str,
    query: Option<Vec<(String, String)>>,
    user_id: Option<&str>,
    admin_token: Option<&str>,
    body: Option<serde_json::Value>,
    write: bool,
) -> IcodeResult<T> {
    let client = build_client(write)?;
    let url = format!("{}/{}", base_url.trim_end_matches('/'), path.trim_start_matches('/'));
    tracing::info!("[community] {} {}", method.as_str(), url);

    let mut req = client.request(method, &url);
    req = req.header("X-App-Token", APP_TOKEN);
    // 请求头校验（§5.1）：Referer 必须命中 Worker 白名单，否则来源 IP 会被阻拦 48 小时
    req = req.header(REFERER, REFERER_VALUE);
    if let Some(uid) = user_id {
        req = req.header("X-User-Id", uid);
    }
    if let Some(tok) = admin_token {
        let value = format!("Bearer {tok}");
        req = req.header(
            AUTHORIZATION,
            HeaderValue::from_str(&value)
                .map_err(|_| IcodeError::validation("管理员会话令牌格式非法"))?,
        );
    }
    if let Some(q) = query {
        req = req.query(&q);
    }
    if let Some(b) = body {
        req = req
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(&b)?);
    }

    let resp = req.send().await.map_err(|e| {
        if e.is_timeout() {
            IcodeError::gateway(format!("社区请求超时：{e}"))
        } else if e.is_connect() {
            IcodeError::gateway(format!("无法连接社区服务：{e}"))
        } else {
            IcodeError::gateway(format!("社区请求失败：{e}"))
        }
    })?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| {
        IcodeError::gateway(format!("读取社区响应失败：{e}"))
    })?;

    if !status.is_success() {
        let message = extract_message(&text, status);
        return Err(map_status(status.as_u16(), message));
    }

    // 成功响应：解析 { code, message, data }
    let env: ApiEnvelope<T> = serde_json::from_str(&text)
        .map_err(|e| IcodeError::gateway(format!("社区响应解析失败：{e}")))?;
    if env.code != 0 {
        return Err(IcodeError::gateway(env.message));
    }
    env.data
        .ok_or_else(|| IcodeError::gateway("社区响应缺少 data"))
}

// ===== 用户接口 =====

/// 帖子列表（游标分页；`section` = Some 时按板块过滤，None = 最近/全部）
pub async fn list_posts(
    base_url: &str,
    user_id: &str,
    cursor: Option<String>,
    limit: Option<u32>,
    section: Option<&str>,
) -> IcodeResult<PostListData> {
    let mut query = Vec::new();
    if let Some(c) = cursor {
        query.push(("cursor".to_string(), c));
    }
    if let Some(l) = limit {
        query.push(("limit".to_string(), l.to_string()));
    }
    if let Some(s) = section {
        query.push(("section".to_string(), s.to_string()));
    }
    send(
        base_url,
        Method::GET,
        "posts",
        if query.is_empty() { None } else { Some(query) },
        Some(user_id),
        None,
        None,
        false,
    )
    .await
}

/// 帖子详情 + 评论区（含楼中楼）
pub async fn get_post(
    base_url: &str,
    user_id: &str,
    post_id: i64,
) -> IcodeResult<PostDetailData> {
    send(
        base_url,
        Method::GET,
        &format!("posts/{post_id}"),
        None,
        Some(user_id),
        None,
        None,
        false,
    )
    .await
}

/// 发帖，返回 post_id
pub async fn create_post(
    base_url: &str,
    user_id: &str,
    input: &CreatePostInput,
) -> IcodeResult<i64> {
    let data: PostCreated = send(
        base_url,
        Method::POST,
        "posts",
        None,
        Some(user_id),
        None,
        Some(serde_json::to_value(input)?),
        true,
    )
    .await?;
    Ok(data.post_id)
}

/// 回复 / 楼中楼，返回 reply_id
pub async fn create_reply(
    base_url: &str,
    user_id: &str,
    post_id: i64,
    input: &CreateReplyInput,
) -> IcodeResult<i64> {
    let data: ReplyCreated = send(
        base_url,
        Method::POST,
        &format!("posts/{post_id}/replies"),
        None,
        Some(user_id),
        None,
        Some(serde_json::to_value(input)?),
        true,
    )
    .await?;
    Ok(data.reply_id)
}

/// 我的资料 + 签到统计
pub async fn get_profile(base_url: &str, user_id: &str) -> IcodeResult<ProfileData> {
    send(
        base_url,
        Method::GET,
        "users/me",
        None,
        Some(user_id),
        None,
        None,
        false,
    )
    .await
}

/// 改昵称 / 头像，返回最新用户资料
pub async fn update_profile(
    base_url: &str,
    user_id: &str,
    input: &UpdateProfileInput,
) -> IcodeResult<ProfileUser> {
    let data: ProfileUserData = send(
        base_url,
        Method::PUT,
        "users/me",
        None,
        Some(user_id),
        None,
        Some(serde_json::to_value(input)?),
        true,
    )
    .await?;
    Ok(data.user)
}

/// 签到；同 UTC 日重复签到 Worker 返回 409；返回统计 + 本次获得积分
pub async fn check_in(base_url: &str, user_id: &str) -> IcodeResult<CheckInResult> {
    send(
        base_url,
        Method::POST,
        "users/me/check-in",
        None,
        Some(user_id),
        None,
        None,
        true,
    )
    .await
}

/// 我的帖子
pub async fn list_my_posts(
    base_url: &str,
    user_id: &str,
    cursor: Option<String>,
    limit: Option<u32>,
) -> IcodeResult<MyPostsData> {
    let mut query = Vec::new();
    if let Some(c) = cursor {
        query.push(("cursor".to_string(), c));
    }
    if let Some(l) = limit {
        query.push(("limit".to_string(), l.to_string()));
    }
    send(
        base_url,
        Method::GET,
        "users/me/posts",
        if query.is_empty() { None } else { Some(query) },
        Some(user_id),
        None,
        None,
        false,
    )
    .await
}

/// 我的回复（含所在帖子标题）
pub async fn list_my_replies(
    base_url: &str,
    user_id: &str,
    cursor: Option<String>,
    limit: Option<u32>,
) -> IcodeResult<MyRepliesData> {
    let mut query = Vec::new();
    if let Some(c) = cursor {
        query.push(("cursor".to_string(), c));
    }
    if let Some(l) = limit {
        query.push(("limit".to_string(), l.to_string()));
    }
    send(
        base_url,
        Method::GET,
        "users/me/replies",
        if query.is_empty() { None } else { Some(query) },
        Some(user_id),
        None,
        None,
        false,
    )
    .await
}

/// 编辑自己的帖子（title / content / section 至少一项；Worker 校验归属）
pub async fn update_my_post(
    base_url: &str,
    user_id: &str,
    post_id: i64,
    input: &UpdateMyPostInput,
) -> IcodeResult<()> {
    send::<serde_json::Value>(
        base_url,
        Method::PUT,
        &format!("users/me/posts/{post_id}"),
        None,
        Some(user_id),
        None,
        Some(serde_json::to_value(input)?),
        true,
    )
    .await?;
    Ok(())
}

/// 删除自己的帖子（Worker 级联删除其全部回复与相关举报）
pub async fn delete_my_post(
    base_url: &str,
    user_id: &str,
    post_id: i64,
) -> IcodeResult<()> {
    send::<serde_json::Value>(
        base_url,
        Method::DELETE,
        &format!("users/me/posts/{post_id}"),
        None,
        Some(user_id),
        None,
        None,
        true,
    )
    .await?;
    Ok(())
}

/// 编辑自己的回复（≤ 1000 字 + 敏感词校验；Worker 校验归属）
pub async fn update_my_reply(
    base_url: &str,
    user_id: &str,
    reply_id: i64,
    content: &str,
) -> IcodeResult<()> {
    let body = serde_json::json!({ "content": content });
    send::<serde_json::Value>(
        base_url,
        Method::PUT,
        &format!("users/me/replies/{reply_id}"),
        None,
        Some(user_id),
        None,
        Some(body),
        true,
    )
    .await?;
    Ok(())
}

/// 删除自己的回复（顶层评论级联楼中楼；Worker 回减 reply_count）
pub async fn delete_my_reply(
    base_url: &str,
    user_id: &str,
    reply_id: i64,
) -> IcodeResult<()> {
    send::<serde_json::Value>(
        base_url,
        Method::DELETE,
        &format!("users/me/replies/{reply_id}"),
        None,
        Some(user_id),
        None,
        None,
        true,
    )
    .await?;
    Ok(())
}

/// 举报帖子 / 回复，返回 report_id
pub async fn report(
    base_url: &str,
    user_id: &str,
    input: &ReportInput,
) -> IcodeResult<i64> {
    let data: ReportCreated = send(
        base_url,
        Method::POST,
        "reports",
        None,
        Some(user_id),
        None,
        Some(serde_json::to_value(input)?),
        true,
    )
    .await?;
    Ok(data.report_id)
}

/// 全站治理开关（D11：用户端只读，用于前端禁用发帖 / 回复入口）
pub async fn get_site_governance(
    base_url: &str,
    user_id: &str,
) -> IcodeResult<SiteGovernance> {
    send(
        base_url,
        Method::GET,
        "site-settings",
        None,
        Some(user_id),
        None,
        None,
        false,
    )
    .await
}

/// 积分排行（offset 分页；Worker 侧聚合 points_ledger，过滤封禁用户，禁言用户仍展示）
pub async fn get_points_leaderboard(
    base_url: &str,
    user_id: &str,
    offset: Option<i64>,
    limit: Option<u32>,
) -> IcodeResult<PointsLeaderboardData> {
    let mut query = Vec::new();
    if let Some(o) = offset {
        query.push(("offset".to_string(), o.to_string()));
    }
    if let Some(l) = limit {
        query.push(("limit".to_string(), l.to_string()));
    }
    send(
        base_url,
        Method::GET,
        "points/leaderboard",
        if query.is_empty() { None } else { Some(query) },
        Some(user_id),
        None,
        None,
        false,
    )
    .await
}

/// 签到排行（offset 分页；Worker 侧返回累计 `total` 与连续 `streak` 两列表，共用同一分页）
pub async fn get_checkin_leaderboard(
    base_url: &str,
    user_id: &str,
    offset: Option<i64>,
    limit: Option<u32>,
) -> IcodeResult<CheckInLeaderboardData> {
    let mut query = Vec::new();
    if let Some(o) = offset {
        query.push(("offset".to_string(), o.to_string()));
    }
    if let Some(l) = limit {
        query.push(("limit".to_string(), l.to_string()));
    }
    send(
        base_url,
        Method::GET,
        "checkins/leaderboard",
        if query.is_empty() { None } else { Some(query) },
        Some(user_id),
        None,
        None,
        false,
    )
    .await
}

// ===== 管理员接口（§5.3：凭据由用户手动输入，客户端只持短期 adminToken）=====

/// 管理员登录，返回 adminToken
pub async fn admin_login(
    base_url: &str,
    input: &AdminLoginInput,
) -> IcodeResult<AdminLoginData> {
    send(
        base_url,
        Method::POST,
        "admin/login",
        None,
        None,
        None,
        Some(serde_json::to_value(input)?),
        true,
    )
    .await
}

/// 用户列表（封禁状态 / 发帖回复数）
pub async fn admin_list_users(
    base_url: &str,
    admin_token: &str,
) -> IcodeResult<Vec<AdminUserItem>> {
    let data: AdminUsersData = send(
        base_url,
        Method::GET,
        "admin/users",
        None,
        None,
        Some(admin_token),
        None,
        false,
    )
    .await?;
    Ok(data.users)
}

/// 封禁用户
pub async fn admin_ban_user(
    base_url: &str,
    admin_token: &str,
    user_id: &str,
    reason: Option<String>,
) -> IcodeResult<()> {
    let body = serde_json::json!({ "reason": reason });
    send::<serde_json::Value>(
        base_url,
        Method::POST,
        &format!("admin/users/{user_id}/ban"),
        None,
        None,
        Some(admin_token),
        Some(body),
        true,
    )
    .await?;
    Ok(())
}

/// 解封用户
pub async fn admin_unban_user(
    base_url: &str,
    admin_token: &str,
    user_id: &str,
) -> IcodeResult<()> {
    send::<serde_json::Value>(
        base_url,
        Method::POST,
        &format!("admin/users/{user_id}/unban"),
        None,
        None,
        Some(admin_token),
        None,
        true,
    )
    .await?;
    Ok(())
}

/// 禁言用户（D12：设置时长 / 永久 + 原因）
pub async fn admin_mute_user(
    base_url: &str,
    admin_token: &str,
    user_id: &str,
    input: &AdminMuteInput,
) -> IcodeResult<()> {
    send::<serde_json::Value>(
        base_url,
        Method::POST,
        &format!("admin/users/{user_id}/mute"),
        None,
        None,
        Some(admin_token),
        Some(serde_json::to_value(input)?),
        true,
    )
    .await?;
    Ok(())
}

/// 解除用户禁言
pub async fn admin_unmute_user(
    base_url: &str,
    admin_token: &str,
    user_id: &str,
) -> IcodeResult<()> {
    send::<serde_json::Value>(
        base_url,
        Method::POST,
        &format!("admin/users/{user_id}/unmute"),
        None,
        None,
        Some(admin_token),
        None,
        true,
    )
    .await?;
    Ok(())
}

/// 举报列表（待处理优先）
pub async fn admin_list_reports(
    base_url: &str,
    admin_token: &str,
) -> IcodeResult<Vec<AdminReportItem>> {
    let data: AdminReportsData = send(
        base_url,
        Method::GET,
        "admin/reports",
        None,
        None,
        Some(admin_token),
        None,
        false,
    )
    .await?;
    Ok(data.reports)
}

/// 处理举报（忽略 / 处置）
pub async fn admin_resolve_report(
    base_url: &str,
    admin_token: &str,
    report_id: i64,
) -> IcodeResult<()> {
    send::<serde_json::Value>(
        base_url,
        Method::POST,
        &format!("admin/reports/{report_id}/resolve"),
        None,
        None,
        Some(admin_token),
        None,
        true,
    )
    .await?;
    Ok(())
}

// ===== 管理员帖子管理（D10；Worker 端点不限流 D9）=====

/// 管理员帖子列表（所有用户，游标分页；`section` = Some 时按板块过滤）
pub async fn admin_list_posts(
    base_url: &str,
    admin_token: &str,
    cursor: Option<String>,
    limit: Option<u32>,
    section: Option<&str>,
) -> IcodeResult<AdminPostListData> {
    let mut query = Vec::new();
    if let Some(c) = cursor {
        query.push(("cursor".to_string(), c));
    }
    if let Some(l) = limit {
        query.push(("limit".to_string(), l.to_string()));
    }
    if let Some(s) = section {
        query.push(("section".to_string(), s.to_string()));
    }
    send(
        base_url,
        Method::GET,
        "admin/posts",
        if query.is_empty() { None } else { Some(query) },
        None,
        Some(admin_token),
        None,
        false,
    )
    .await
}

/// 管理员帖子详情 + 评论区（定位待处置回复）
pub async fn admin_get_post(
    base_url: &str,
    admin_token: &str,
    post_id: i64,
) -> IcodeResult<PostDetailData> {
    send(
        base_url,
        Method::GET,
        &format!("admin/posts/{post_id}"),
        None,
        None,
        Some(admin_token),
        None,
        false,
    )
    .await
}

/// 管理员编辑帖子（title / content / section 至少一项）
pub async fn admin_update_post(
    base_url: &str,
    admin_token: &str,
    post_id: i64,
    input: &AdminUpdatePostInput,
) -> IcodeResult<()> {
    send::<serde_json::Value>(
        base_url,
        Method::PUT,
        &format!("admin/posts/{post_id}"),
        None,
        None,
        Some(admin_token),
        Some(serde_json::to_value(input)?),
        true,
    )
    .await?;
    Ok(())
}

/// 管理员删除帖子（Worker 级联删除其全部回复与相关举报）
pub async fn admin_delete_post(
    base_url: &str,
    admin_token: &str,
    post_id: i64,
) -> IcodeResult<()> {
    send::<serde_json::Value>(
        base_url,
        Method::DELETE,
        &format!("admin/posts/{post_id}"),
        None,
        None,
        Some(admin_token),
        None,
        true,
    )
    .await?;
    Ok(())
}

/// 管理员编辑回复
pub async fn admin_update_reply(
    base_url: &str,
    admin_token: &str,
    reply_id: i64,
    content: &str,
) -> IcodeResult<()> {
    let body = serde_json::json!({ "content": content });
    send::<serde_json::Value>(
        base_url,
        Method::PUT,
        &format!("admin/replies/{reply_id}"),
        None,
        None,
        Some(admin_token),
        Some(body),
        true,
    )
    .await?;
    Ok(())
}

/// 管理员删除回复（顶层评论级联楼中楼）
pub async fn admin_delete_reply(
    base_url: &str,
    admin_token: &str,
    reply_id: i64,
) -> IcodeResult<()> {
    send::<serde_json::Value>(
        base_url,
        Method::DELETE,
        &format!("admin/replies/{reply_id}"),
        None,
        None,
        Some(admin_token),
        None,
        true,
    )
    .await?;
    Ok(())
}

// ===== 站点治理（D11：全站禁言 / 禁发帖 / 禁回复 + 帖子级锁定；端点不限流 D9）=====

/// 管理员读取全站治理开关
pub async fn admin_get_governance(
    base_url: &str,
    admin_token: &str,
) -> IcodeResult<SiteGovernance> {
    send(
        base_url,
        Method::GET,
        "admin/site-settings",
        None,
        None,
        Some(admin_token),
        None,
        false,
    )
    .await
}

/// 管理员更新全站治理开关（部分更新，返回最新完整状态）
pub async fn admin_update_governance(
    base_url: &str,
    admin_token: &str,
    input: &AdminUpdateGovernanceInput,
) -> IcodeResult<SiteGovernance> {
    send(
        base_url,
        Method::PUT,
        "admin/site-settings",
        None,
        None,
        Some(admin_token),
        Some(serde_json::to_value(input)?),
        true,
    )
    .await
}

/// 管理员锁定 / 解锁帖子（locked=1 时该帖禁止新增评论回复）
pub async fn admin_set_post_locked(
    base_url: &str,
    admin_token: &str,
    post_id: i64,
    locked: bool,
) -> IcodeResult<()> {
    let body = serde_json::json!({ "locked": locked });
    send::<serde_json::Value>(
        base_url,
        Method::POST,
        &format!("admin/posts/{post_id}/lock"),
        None,
        None,
        Some(admin_token),
        Some(body),
        true,
    )
    .await?;
    Ok(())
}

/// 管理员置顶 / 取消置顶帖子（置顶帖在列表排序时排在最前）
pub async fn admin_set_post_pin(
    base_url: &str,
    admin_token: &str,
    post_id: i64,
    pinned: bool,
) -> IcodeResult<()> {
    let body = serde_json::json!({ "pinned": pinned });
    send::<serde_json::Value>(
        base_url,
        Method::POST,
        &format!("admin/posts/{post_id}/pin"),
        None,
        None,
        Some(admin_token),
        Some(body),
        true,
    )
    .await?;
    Ok(())
}

// ===== 消息通知（2026-08-23 通知迭代）=====

/// 通知列表（游标分页；顺带返回未读数供小红点）
pub async fn list_notifications(
    base_url: &str,
    user_id: &str,
    cursor: Option<String>,
    limit: Option<u32>,
) -> IcodeResult<NotificationListData> {
    let mut query = Vec::new();
    if let Some(c) = cursor {
        query.push(("cursor".to_string(), c));
    }
    if let Some(l) = limit {
        query.push(("limit".to_string(), l.to_string()));
    }
    send(
        base_url,
        Method::GET,
        "users/me/notifications",
        if query.is_empty() { None } else { Some(query) },
        Some(user_id),
        None,
        None,
        false,
    )
    .await
}

/// 未读通知数（小红点）
pub async fn get_unread_count(
    base_url: &str,
    user_id: &str,
) -> IcodeResult<UnreadCountData> {
    send(
        base_url,
        Method::GET,
        "users/me/notifications/unread-count",
        None,
        Some(user_id),
        None,
        None,
        false,
    )
    .await
}

/// 全部标记已读（返回本次更新的条数）
pub async fn read_all_notifications(
    base_url: &str,
    user_id: &str,
) -> IcodeResult<ReadAllNotificationsData> {
    send(
        base_url,
        Method::POST,
        "users/me/notifications/read-all",
        None,
        Some(user_id),
        None,
        None,
        true,
    )
    .await
}
