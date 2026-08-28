//! # 社区模块 Tauri Command 声明（§7.1）
//!
//! 前端通过 `invoke('community_*', payload)` 调用。
//! 标量参数 snake_case（Tauri 自动映射前端 camelCase）；复杂对象用 DTO。
//! 错误统一 `IcodeResult`（`IcodeError`），前端经 `toIcodeError` 处理。

use tauri::State;

use crate::error::IcodeResult;

use super::service::CommunityHandle;
use super::types::{
    AdminLoginData, AdminLoginInput, AdminMuteInput, AdminPostListData, AdminReportItem,
    AdminShareListData, AdminUpdateGovernanceInput, AdminUpdatePostInput, AdminUserItem,
    CheckInLeaderboardData, CheckInResult, CommunityLocalState, CreatePostInput, CreateReplyInput,
    MyPostsData, MyRepliesData, NotificationListData, PointsLeaderboardData, PostDetailData,
    PostLikeData, PostListData, PostTipData, PostTipListData, ProfileData, ProfileUser,
    ReadAllNotificationsData, ReportInput, ShareLink, ShareLinkInput, ShareLinkListData,
    SiteGovernance, TipPostInput, UnreadCountData, UpdateMyPostInput, UpdateProfileInput,
};

/// 列表分页上限（与 Worker 侧 MAX_LIST_LIMIT 对齐）
const MAX_LIST_LIMIT: u32 = 50;

/// 校验分页 limit（可选参数）
fn validate_limit(limit: Option<u32>) -> IcodeResult<()> {
    if let Some(l) = limit {
        if l == 0 || l > MAX_LIST_LIMIT {
            return Err(crate::error::IcodeError::validation(format!(
                "limit 须在 1-{MAX_LIST_LIMIT} 之间"
            )));
        }
    }
    Ok(())
}

// ===== 帖子 =====

/// 帖子列表（游标分页；`section` 可选：chat=闲聊 / eggs=领鸡蛋 / tech=技术，缺省 = 最近/全部）
#[tauri::command]
pub async fn community_get_posts(
    state: State<'_, CommunityHandle>,
    cursor: Option<String>,
    limit: Option<u32>,
    section: Option<String>,
) -> IcodeResult<PostListData> {
    validate_limit(limit)?;
    state.service().get_posts(cursor, limit, section).await
}

/// 帖子详情 + 评论区（含楼中楼）
#[tauri::command]
pub async fn community_get_post(
    state: State<'_, CommunityHandle>,
    post_id: i64,
) -> IcodeResult<PostDetailData> {
    state.service().get_post(post_id).await
}

/// 发帖
#[tauri::command]
pub async fn community_create_post(
    state: State<'_, CommunityHandle>,
    input: CreatePostInput,
) -> IcodeResult<i64> {
    state.service().create_post(input).await
}

/// 回复 / 楼中楼
#[tauri::command]
pub async fn community_create_reply(
    state: State<'_, CommunityHandle>,
    post_id: i64,
    input: CreateReplyInput,
) -> IcodeResult<i64> {
    state.service().create_reply(post_id, input).await
}

/// 点赞帖子（点赞迭代：作者不能自赞由 Worker 校验；1 赞 = 作者 +1 积分）
#[tauri::command]
pub async fn community_like_post(
    state: State<'_, CommunityHandle>,
    post_id: i64,
) -> IcodeResult<PostLikeData> {
    state.service().like_post(post_id).await
}

/// 取消点赞（作者积分同步扣回）
#[tauri::command]
pub async fn community_unlike_post(
    state: State<'_, CommunityHandle>,
    post_id: i64,
) -> IcodeResult<PostLikeData> {
    state.service().unlike_post(post_id).await
}

/// 打赏帖子（1~66 积分；不能自赏；每人每帖一次不可撤销）
#[tauri::command]
pub async fn community_tip_post(
    state: State<'_, CommunityHandle>,
    post_id: i64,
    input: TipPostInput,
) -> IcodeResult<PostTipData> {
    state.service().tip_post(post_id, input.amount).await
}

/// 帖内打赏列表（游标分页；实名展示打赏人）
#[tauri::command]
pub async fn community_list_post_tips(
    state: State<'_, CommunityHandle>,
    post_id: i64,
    cursor: Option<String>,
    limit: Option<u32>,
) -> IcodeResult<PostTipListData> {
    validate_limit(limit)?;
    state.service().list_post_tips(post_id, cursor, limit).await
}

// ===== 用户中心 =====

/// 我的资料 + 签到统计
#[tauri::command]
pub async fn community_get_profile(
    state: State<'_, CommunityHandle>,
) -> IcodeResult<ProfileData> {
    state.service().get_profile().await
}

/// 改昵称 / 头像
#[tauri::command]
pub async fn community_update_profile(
    state: State<'_, CommunityHandle>,
    input: UpdateProfileInput,
) -> IcodeResult<ProfileUser> {
    state.service().update_profile(input).await
}

/// 签到（同 UTC 日重复 → Worker 409）；返回统计 + 本次获得积分
#[tauri::command]
pub async fn community_check_in(
    state: State<'_, CommunityHandle>,
) -> IcodeResult<CheckInResult> {
    state.service().check_in().await
}

/// 我的帖子
#[tauri::command]
pub async fn community_get_my_posts(
    state: State<'_, CommunityHandle>,
    cursor: Option<String>,
    limit: Option<u32>,
) -> IcodeResult<MyPostsData> {
    validate_limit(limit)?;
    state.service().get_my_posts(cursor, limit).await
}

/// 我的回复（含所在帖子标题）
#[tauri::command]
pub async fn community_get_my_replies(
    state: State<'_, CommunityHandle>,
    cursor: Option<String>,
    limit: Option<u32>,
) -> IcodeResult<MyRepliesData> {
    validate_limit(limit)?;
    state.service().get_my_replies(cursor, limit).await
}

/// 编辑自己的帖子（title / content / section 至少一项）
#[tauri::command]
pub async fn community_update_my_post(
    state: State<'_, CommunityHandle>,
    post_id: i64,
    input: UpdateMyPostInput,
) -> IcodeResult<()> {
    state.service().update_my_post(post_id, input).await
}

/// 删除自己的帖子（Worker 级联删除其全部回复与相关举报）
#[tauri::command]
pub async fn community_delete_my_post(
    state: State<'_, CommunityHandle>,
    post_id: i64,
) -> IcodeResult<()> {
    state.service().delete_my_post(post_id).await
}

/// 编辑自己的回复
#[tauri::command]
pub async fn community_update_my_reply(
    state: State<'_, CommunityHandle>,
    reply_id: i64,
    content: String,
) -> IcodeResult<()> {
    state.service().update_my_reply(reply_id, &content).await
}

/// 删除自己的回复（顶层评论级联楼中楼）
#[tauri::command]
pub async fn community_delete_my_reply(
    state: State<'_, CommunityHandle>,
    reply_id: i64,
) -> IcodeResult<()> {
    state.service().delete_my_reply(reply_id).await
}

/// 举报帖子 / 回复
#[tauri::command]
pub async fn community_report(
    state: State<'_, CommunityHandle>,
    input: ReportInput,
) -> IcodeResult<i64> {
    state.service().report(input).await
}

/// 全站治理开关（D11：用户端只读）
#[tauri::command]
pub async fn community_get_site_governance(
    state: State<'_, CommunityHandle>,
) -> IcodeResult<SiteGovernance> {
    state.service().get_site_governance().await
}

/// 积分排行（offset 分页；Worker 侧过滤封禁用户，禁言用户仍展示）
#[tauri::command]
pub async fn community_get_points_leaderboard(
    state: State<'_, CommunityHandle>,
    offset: Option<i64>,
    limit: Option<u32>,
) -> IcodeResult<PointsLeaderboardData> {
    validate_limit(limit)?;
    state.service().get_points_leaderboard(offset, limit).await
}

/// 签到排行（offset 分页；Worker 侧返回累计 `total` 与连续 `streak` 两列表）
#[tauri::command]
pub async fn community_get_checkin_leaderboard(
    state: State<'_, CommunityHandle>,
    offset: Option<i64>,
    limit: Option<u32>,
) -> IcodeResult<CheckInLeaderboardData> {
    validate_limit(limit)?;
    state.service().get_checkin_leaderboard(offset, limit).await
}

// ===== 消息通知（2026-08-23 通知迭代）=====

/// 通知列表（游标分页；顺带返回未读数供小红点）
#[tauri::command]
pub async fn community_get_notifications(
    state: State<'_, CommunityHandle>,
    cursor: Option<String>,
    limit: Option<u32>,
) -> IcodeResult<NotificationListData> {
    validate_limit(limit)?;
    state.service().get_notifications(cursor, limit).await
}

/// 未读通知数（小红点）
#[tauri::command]
pub async fn community_get_unread_count(
    state: State<'_, CommunityHandle>,
) -> IcodeResult<UnreadCountData> {
    state.service().get_unread_count().await
}

/// 全部标记已读（返回本次更新的条数）
#[tauri::command]
pub async fn community_read_all_notifications(
    state: State<'_, CommunityHandle>,
) -> IcodeResult<ReadAllNotificationsData> {
    state.service().read_all_notifications().await
}

// ===== 门禁与本地状态（不进 Worker）=====

/// 读取社区本地状态（门禁开关 / 身份 / 昵称头像缓存）
#[tauri::command]
pub async fn community_get_local_state(
    state: State<'_, CommunityHandle>,
) -> IcodeResult<CommunityLocalState> {
    Ok(state.service().get_local_state()?)
}

/// 设置门禁开关（开启时若未生成身份则自动生成）
#[tauri::command]
pub async fn community_set_enabled(
    state: State<'_, CommunityHandle>,
    enabled: bool,
) -> IcodeResult<CommunityLocalState> {
    Ok(state.service().set_enabled(enabled)?)
}

// ===== 管理员（§5.3）=====

/// 管理员登录（用户输入固定凭据，Worker 校验，返回短期 adminToken）
#[tauri::command]
pub async fn community_admin_login(
    state: State<'_, CommunityHandle>,
    input: AdminLoginInput,
) -> IcodeResult<AdminLoginData> {
    state.service().admin_login(input).await
}

/// 用户列表
#[tauri::command]
pub async fn community_admin_get_users(
    state: State<'_, CommunityHandle>,
    admin_token: String,
) -> IcodeResult<Vec<AdminUserItem>> {
    state.service().admin_list_users(&admin_token).await
}

/// 封禁用户
#[tauri::command]
pub async fn community_admin_ban(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    user_id: String,
    reason: Option<String>,
) -> IcodeResult<()> {
    state.service().admin_ban_user(&admin_token, &user_id, reason).await
}

/// 解封用户
#[tauri::command]
pub async fn community_admin_unban(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    user_id: String,
) -> IcodeResult<()> {
    state.service().admin_unban_user(&admin_token, &user_id).await
}

/// 禁言用户（D12：设置时长 / 永久 + 原因）
#[tauri::command]
pub async fn community_admin_mute_user(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    user_id: String,
    input: AdminMuteInput,
) -> IcodeResult<()> {
    state.service().admin_mute_user(&admin_token, &user_id, input).await
}

/// 解除用户禁言
#[tauri::command]
pub async fn community_admin_unmute_user(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    user_id: String,
) -> IcodeResult<()> {
    state.service().admin_unmute_user(&admin_token, &user_id).await
}

/// 举报列表
#[tauri::command]
pub async fn community_admin_get_reports(
    state: State<'_, CommunityHandle>,
    admin_token: String,
) -> IcodeResult<Vec<AdminReportItem>> {
    state.service().admin_list_reports(&admin_token).await
}

/// 处理举报（忽略 / 处置）
#[tauri::command]
pub async fn community_admin_resolve_report(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    report_id: i64,
) -> IcodeResult<()> {
    state.service().admin_resolve_report(&admin_token, report_id).await
}

// ===== 管理员帖子管理（D10）=====

/// 管理员帖子列表（所有用户，游标分页；section 可选过滤）
#[tauri::command]
pub async fn community_admin_get_posts(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    cursor: Option<String>,
    limit: Option<u32>,
    section: Option<String>,
) -> IcodeResult<AdminPostListData> {
    validate_limit(limit)?;
    state.service().admin_list_posts(&admin_token, cursor, limit, section).await
}

/// 管理员帖子详情 + 评论区（定位待处置回复）
#[tauri::command]
pub async fn community_admin_get_post(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    post_id: i64,
) -> IcodeResult<PostDetailData> {
    state.service().admin_get_post(&admin_token, post_id).await
}

/// 管理员编辑帖子（title / content / section 至少一项）
#[tauri::command]
pub async fn community_admin_update_post(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    post_id: i64,
    input: AdminUpdatePostInput,
) -> IcodeResult<()> {
    state.service().admin_update_post(&admin_token, post_id, input).await
}

/// 管理员删除帖子（级联删除其全部回复与相关举报）
#[tauri::command]
pub async fn community_admin_delete_post(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    post_id: i64,
) -> IcodeResult<()> {
    state.service().admin_delete_post(&admin_token, post_id).await
}

/// 管理员编辑回复
#[tauri::command]
pub async fn community_admin_update_reply(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    reply_id: i64,
    content: String,
) -> IcodeResult<()> {
    state.service().admin_update_reply(&admin_token, reply_id, &content).await
}

/// 管理员删除回复（顶层评论级联楼中楼）
#[tauri::command]
pub async fn community_admin_delete_reply(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    reply_id: i64,
) -> IcodeResult<()> {
    state.service().admin_delete_reply(&admin_token, reply_id).await
}

// ===== 站点治理（D11：全站禁言 / 禁发帖 / 禁回复 + 帖子级锁定）=====

/// 管理员读取全站治理开关
#[tauri::command]
pub async fn community_admin_get_governance(
    state: State<'_, CommunityHandle>,
    admin_token: String,
) -> IcodeResult<SiteGovernance> {
    state.service().admin_get_governance(&admin_token).await
}

/// 管理员更新全站治理开关（muteAll / postLocked / replyLocked 至少一项）
#[tauri::command]
pub async fn community_admin_update_governance(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    input: AdminUpdateGovernanceInput,
) -> IcodeResult<SiteGovernance> {
    state.service().admin_update_governance(&admin_token, input).await
}

/// 管理员锁定 / 解锁帖子（locked=1 时该帖禁止新增评论回复）
#[tauri::command]
pub async fn community_admin_set_post_locked(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    post_id: i64,
    locked: bool,
) -> IcodeResult<()> {
    state.service().admin_set_post_locked(&admin_token, post_id, locked).await
}

/// 管理员置顶 / 取消置顶帖子（置顶帖在列表排序时排在最前）
#[tauri::command]
pub async fn community_admin_set_post_pin(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    post_id: i64,
    pinned: bool,
) -> IcodeResult<()> {
    state.service().admin_set_post_pin(&admin_token, post_id, pinned).await
}

// ===== 帖子外链分享（2026-08-26 分享迭代，见 docs/proposals/community-post-share.md）=====

/// 发起分享（作者本人 + 扣 100 积分，Worker 校验），返回带直链的分享链接
#[tauri::command]
pub async fn community_create_share_link(
    state: State<'_, CommunityHandle>,
    post_id: i64,
    input: ShareLinkInput,
) -> IcodeResult<ShareLink> {
    state.service().create_share_link(post_id, input).await
}

/// 该帖分享列表（游标分页；仅作者本人可见，Worker 校验归属）
#[tauri::command]
pub async fn community_list_post_share_links(
    state: State<'_, CommunityHandle>,
    post_id: i64,
    cursor: Option<String>,
    limit: Option<u32>,
) -> IcodeResult<ShareLinkListData> {
    validate_limit(limit)?;
    state.service().list_post_share_links(post_id, cursor, limit).await
}

/// 管理员：分享列表（全站 / 按帖子过滤，游标分页）
#[tauri::command]
pub async fn community_admin_get_share_links(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    cursor: Option<String>,
    limit: Option<u32>,
    post_id: Option<i64>,
) -> IcodeResult<AdminShareListData> {
    validate_limit(limit)?;
    state.service().admin_list_share_links(&admin_token, cursor, limit, post_id).await
}

/// 管理员：撤销任意分享（不返还积分）
#[tauri::command]
pub async fn community_admin_revoke_share_link(
    state: State<'_, CommunityHandle>,
    admin_token: String,
    pid: String,
) -> IcodeResult<()> {
    state.service().admin_revoke_share_link(&admin_token, &pid).await
}
