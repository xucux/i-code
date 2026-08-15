//! # 社区 Worker REST API 客户端（§7.2）
//!
//! - `base_url` 由调用方传入（读取自 `app_settings.community.base_url`）
//! - 复用 `shared::apply_global_proxy` 应用全局代理
//! - `X-User-Id` 从本地状态注入；`X-App-Token` 为编译期常量
//! - 响应先验 `code`：非 0 转业务错误；429 → 限流文案；错误不暴露 SQL / 堆栈

use std::time::Duration;

use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Method;
use serde::de::DeserializeOwned;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::shared;

use super::types::{
    AdminLoginData, AdminLoginInput, AdminReportItem, AdminUserItem, CheckInStats, CreatePostInput,
    CreateReplyInput, MyPostsData, MyRepliesData, PostDetailData, PostListData, ProfileData,
    ProfileUser, ReportInput, UpdateProfileInput,
};

/// App Token：与 Worker 侧 `APP_TOKEN`（wrangler.toml `[vars]`）保持一致，
/// 防止外部脚本直接刷接口（§5.1）
pub const APP_TOKEN: &str = "i-code-community-app-token-v1";

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
        .user_agent(concat!("i-code/", env!("CARGO_PKG_VERSION")));
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

/// 帖子列表（游标分页）
pub async fn list_posts(
    base_url: &str,
    user_id: &str,
    cursor: Option<String>,
    limit: Option<u32>,
) -> IcodeResult<PostListData> {
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

/// 签到；同 UTC 日重复签到 Worker 返回 409
pub async fn check_in(base_url: &str, user_id: &str) -> IcodeResult<CheckInStats> {
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
