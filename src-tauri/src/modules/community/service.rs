//! # 社区业务服务层
//!
//! 编排职责：
//! - 门禁开关与本地状态（§7.3）：读取 / 写入 `app_settings.community_json`
//! - 设备身份生成（§3.1 方案 A）：机器标识（MachineGuid 等）加盐 SHA-256 → 64 hex；
//!   读取失败兜底本机随机 UUID（同样加盐哈希，保证 64 hex 与跨重启稳定）
//! - 转发到 Worker REST API（`client.rs`）
//!
//! 隐私约束（§1.4 / §9）：`user_id` 与原始设备标识**禁止写入任何日志**。

use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::error::{IcodeError, IcodeResult};

use super::client;
use super::repository;
use super::types::{
    AccountAuthInput, AdminLoginData, AdminLoginInput, AdminMuteInput, AdminPostListData,
    AdminReportItem, AdminShareListData, AdminUpdateGovernanceInput, AdminUpdatePostInput,
    AdminUserItem, AuthResult, CheckInLeaderboardData, CheckInResult, CommunityLocalState,
    CreatePostInput, CreateReplyInput, LogoutData, MyPostsData, MyRepliesData,
    NotificationListData, PointsLeaderboardData, PostDetailData, PostLikeData, PostListData,
    PostTipData, PostTipListData, ProfileData, ProfileUser, ReadAllNotificationsData, ReportInput,
    ShareLink, ShareLinkInput, ShareLinkListData, SiteGovernance, TipPostInput, UnreadCountData,
    UpdateMyPostInput, UpdateProfileInput,
};

/// 社区 Service 句柄（Tauri State）
#[derive(Clone)]
pub struct CommunityHandle {
    inner: Arc<CommunityService>,
}

impl CommunityHandle {
    /// 创建社区 Service 句柄（无启动期依赖）
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CommunityService),
        }
    }

    /// 获取内部 Service 引用
    pub fn service(&self) -> &CommunityService {
        &self.inner
    }
}

impl Default for CommunityHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// 社区业务逻辑
pub struct CommunityService;

/// 设备身份加盐常量（应用标识，§3.1）
const USER_ID_SALT: &str = "com.icode.app:community:v1";

/// 生成 64 hex 设备身份
///
/// 优先读取系统机器标识（Windows MachineGuid / macOS IOPlatformUUID / Linux machine-id），
/// 读取失败时回退到本机随机 UUID。两者均拼盐后 SHA-256 → 64 hex，
/// 保证输出格式一致且不直接暴露原始标识。
fn generate_user_id() -> String {
    let raw = machine_uid::get()
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let mut hasher = Sha256::new();
    hasher.update(USER_ID_SALT.as_bytes());
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}

impl CommunityService {
    /// 读取社区本地状态（不存在时返回默认值）
    pub fn get_local_state(&self) -> IcodeResult<CommunityLocalState> {
        Ok(repository::get_local_state()?.unwrap_or_default())
    }

    /// 设置门禁开关
    ///
    /// 开启时若未生成设备身份则先生成并持久化（此后跨重启保持稳定）。
    pub fn set_enabled(&self, enabled: bool) -> IcodeResult<CommunityLocalState> {
        let mut state = self.get_local_state()?;
        state.enabled = enabled;
        if enabled && state.user_id.is_none() {
            state.user_id = Some(generate_user_id());
        }
        repository::set_local_state(&state)?;
        Ok(state)
    }

    /// 校验门禁已开启并返回 (本地状态, user_id, auth_token)
    ///
    /// 2026-08-31 鉴权迭代：业务接口一律依赖已签发的会话 token；
    /// token 缺失视为未登录（UNAUTHORIZED，前端跳登录卡）。
    fn require_ready(&self) -> IcodeResult<(CommunityLocalState, String, String)> {
        let state = self.get_local_state()?;
        if !state.enabled {
            return Err(IcodeError::forbidden("社区尚未开启"));
        }
        let user_id = state
            .user_id
            .clone()
            .ok_or_else(|| IcodeError::forbidden("社区身份尚未初始化"))?;
        let auth_token = state
            .auth_token
            .clone()
            .ok_or_else(|| IcodeError::unauthorized("社区登录已失效，请重新登录"))?;
        Ok((state, user_id, auth_token))
    }

    /// 校验门禁已开启并返回 base_url（管理员接口，不要求 user_id）
    fn require_enabled_base(&self) -> IcodeResult<String> {
        let state = self.get_local_state()?;
        if !state.enabled {
            return Err(IcodeError::forbidden("社区尚未开启"));
        }
        Ok(state.base_url)
    }

    // ===== 鉴权（2026-08-31 迭代，docs/proposals/community-auth-accounts.md）=====

    /// 匿名进入：以本机机器码身份换取匿名 token
    ///
    /// 老用户升级（已有 userId 无 token）自动补换；维持 anonymous 模式，不自动迁移为账号。
    pub async fn auth_anonymous(&self) -> IcodeResult<AuthResult> {
        let state = self.get_local_state()?;
        if !state.enabled {
            return Err(IcodeError::forbidden("社区尚未开启"));
        }
        let user_id = state
            .user_id
            .clone()
            .ok_or_else(|| IcodeError::forbidden("社区身份尚未初始化"))?;
        let result = self.map_auth_failure(client::auth_anonymous(&state.base_url, &user_id).await)?;
        self.apply_auth(&result, None)?;
        Ok(result)
    }

    /// 账号登录：Worker 校验密码后签发 account token（自选：Worker 侧按 IP 防爆破）
    pub async fn auth_login(&self, input: AccountAuthInput) -> IcodeResult<AuthResult> {
        let state = self.get_local_state()?;
        let result = self.map_auth_failure(client::auth_login(&state.base_url, &input).await)?;
        self.apply_auth(&result, Some(input.username))?;
        Ok(result)
    }

    /// 注册账号（D4：Worker 创建全新独立身份，账号与设备解耦）
    pub async fn auth_register(&self, input: AccountAuthInput) -> IcodeResult<AuthResult> {
        let state = self.get_local_state()?;
        let result = self.map_auth_failure(client::auth_register(&state.base_url, &input).await)?;
        self.apply_auth(&result, Some(input.username))?;
        Ok(result)
    }

    /// 匿名身份升级账号（D3）：需先有匿名 token；成功后本地模式切换为 account
    pub async fn auth_bind(&self, input: AccountAuthInput) -> IcodeResult<AuthResult> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        let result = self.map_auth_failure(client::auth_bind(&state.base_url, &auth_token, &input).await)?;
        self.apply_auth(&result, Some(input.username))?;
        Ok(result)
    }

    /// 登出：远端吊销会话（best-effort）并清空本地登录态（回到登录卡）
    pub async fn auth_logout(&self) -> IcodeResult<LogoutData> {
        let state = self.get_local_state()?;
        if let Some(token) = state.auth_token.clone() {
            // 远端吊销失败（已过期 / 网络异常）不阻断本地登出
            let _ = client::auth_logout(&state.base_url, &token).await;
        }
        self.clear_login()
    }

    /// 保存登录态到本地（token / mode / username，并同步昵称头像缓存）
    fn apply_auth(&self, result: &AuthResult, username: Option<String>) -> IcodeResult<()> {
        let mut state = self.get_local_state()?;
        state.auth_token = Some(result.token.clone());
        state.auth_mode = Some(result.mode.clone());
        state.username = username;
        state.nickname = Some(result.user.nickname.clone());
        state.avatar_index = Some(result.user.avatar_index);
        repository::set_local_state(&state)
    }

    /// 清空本地登录态（登出 / token 失效），保留门禁与设备身份
    fn clear_login(&self) -> IcodeResult<LogoutData> {
        let mut state = self.get_local_state()?;
        state.auth_token = None;
        state.auth_mode = None;
        state.username = None;
        repository::set_local_state(&state)?;
        Ok(LogoutData { ok: true })
    }

    /// 业务调用 401 包装：Worker 返回会话过期/失效时清除本地登录态
    /// （D1：不做自动续期，需用户重新登录；前端收到 UNAUTHORIZED 后展示登录卡）
    fn map_auth_failure<T>(&self, r: IcodeResult<T>) -> IcodeResult<T> {
        match r {
            Err(e) if e.code == "UNAUTHORIZED" => {
                let _ = self.clear_login();
                Err(e)
            }
            other => other,
        }
    }

    // ===== 帖子 =====

    /// 校验板块参数：Some 时必须为合法枚举值；返回归一化后的过滤值
    fn normalize_section_filter(section: Option<&str>) -> IcodeResult<Option<&str>> {
        match section {
            None => Ok(None),
            Some(s) if super::types::is_valid_section(s) => Ok(Some(s)),
            Some(s) => Err(IcodeError::validation(format!(
                "板块非法：{s}（须为 chat / eggs / tech）"
            ))),
        }
    }

    pub async fn get_posts(
        &self,
        cursor: Option<String>,
        limit: Option<u32>,
        section: Option<String>,
    ) -> IcodeResult<PostListData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        let section = Self::normalize_section_filter(section.as_deref())?;
        self.map_auth_failure(client::list_posts(&state.base_url, &auth_token, cursor, limit, section).await)
    }

    pub async fn get_post(&self, post_id: i64) -> IcodeResult<PostDetailData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::get_post(&state.base_url, &auth_token, post_id).await)
    }

    pub async fn create_post(&self, mut input: CreatePostInput) -> IcodeResult<i64> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        // 板块归一化：None → 闲聊；非法值直接拒绝（与 Worker 400 语义一致，提前拦截）
        let section = match input.section.as_deref() {
            None => "chat".to_string(),
            Some(s) if super::types::is_valid_section(s) => s.to_string(),
            Some(s) => {
                return Err(IcodeError::validation(format!(
                    "板块非法：{s}（须为 chat / eggs / tech）"
                )))
            }
        };
        input.section = Some(section);
        self.map_auth_failure(client::create_post(&state.base_url, &auth_token, &input).await)
    }

    pub async fn create_reply(&self, post_id: i64, input: CreateReplyInput) -> IcodeResult<i64> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::create_reply(&state.base_url, &auth_token, post_id, &input).await)
    }

    /// 点赞帖子（点赞迭代：作者不能自赞由 Worker 校验；1 赞 = 作者 +1 积分）
    pub async fn like_post(&self, post_id: i64) -> IcodeResult<PostLikeData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::like_post(&state.base_url, &auth_token, post_id).await)
    }

    /// 取消点赞（作者积分同步扣回）
    pub async fn unlike_post(&self, post_id: i64) -> IcodeResult<PostLikeData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::unlike_post(&state.base_url, &auth_token, post_id).await)
    }

    /// 打赏帖子（打赏迭代：单次 1~66 积分 / 不能自赏 / 每人每帖一次不可撤销；Worker 校验）
    pub async fn tip_post(&self, post_id: i64, amount: i64) -> IcodeResult<PostTipData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        // 金额前置校验（与 Worker 400 语义一致，提前拦截；Worker 侧仍兜底）
        if amount < 1 || amount > 66 {
            return Err(IcodeError::validation("打赏积分须为 1~66 的整数"));
        }
        let input = TipPostInput { amount };
        self.map_auth_failure(client::tip_post(&state.base_url, &auth_token, post_id, &input).await)
    }

    /// 帖内打赏列表（游标分页；实名展示打赏人）
    pub async fn list_post_tips(
        &self,
        post_id: i64,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> IcodeResult<PostTipListData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::list_post_tips(&state.base_url, &auth_token, post_id, cursor, limit).await)
    }

    // ===== 用户中心 =====

    /// 我的资料 + 签到统计；顺带把昵称/头像同步回本地缓存（§7.3：以 /users/me 为准）
    pub async fn get_profile(&self) -> IcodeResult<ProfileData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        let data = self.map_auth_failure(client::get_profile(&state.base_url, &auth_token).await)?;
        self.sync_profile_cache(&data.user)?;
        Ok(data)
    }

    /// 改昵称 / 头像；成功后同步本地缓存
    pub async fn update_profile(&self, input: UpdateProfileInput) -> IcodeResult<ProfileUser> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        let user = self.map_auth_failure(client::update_profile(&state.base_url, &auth_token, &input).await)?;
        self.sync_profile_cache(&user)?;
        Ok(user)
    }

    /// 签到（重复签到由 Worker 返回 409）；返回统计 + 本次获得积分
    pub async fn check_in(&self) -> IcodeResult<CheckInResult> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::check_in(&state.base_url, &auth_token).await)
    }

    pub async fn get_my_posts(
        &self,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> IcodeResult<MyPostsData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::list_my_posts(&state.base_url, &auth_token, cursor, limit).await)
    }

    pub async fn get_my_replies(
        &self,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> IcodeResult<MyRepliesData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::list_my_replies(&state.base_url, &auth_token, cursor, limit).await)
    }

    /// 编辑自己的帖子（title / content / section 至少一项，前置校验与发帖一致）
    pub async fn update_my_post(
        &self,
        post_id: i64,
        input: UpdateMyPostInput,
    ) -> IcodeResult<()> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        // 至少一项字段；提供字段时做与发帖一致的前置校验（Worker 侧仍兜底）
        if input.title.is_none() && input.content.is_none() && input.section.is_none() {
            return Err(IcodeError::validation("请提供要修改的字段"));
        }
        if let Some(title) = input.title.as_deref() {
            let t = title.trim();
            if t.is_empty() {
                return Err(IcodeError::validation("标题不能为空"));
            }
            if t.chars().count() > 80 {
                return Err(IcodeError::validation("标题不能超过 80 字"));
            }
        }
        if let Some(content) = input.content.as_deref() {
            let c = content.trim();
            if c.is_empty() {
                return Err(IcodeError::validation("内容不能为空"));
            }
            if c.chars().count() > 10000 {
                return Err(IcodeError::validation("内容不能超过 10000 字"));
            }
        }
        if let Some(section) = input.section.as_deref() {
            if !super::types::is_valid_section(section) {
                return Err(IcodeError::validation(format!(
                    "板块非法：{section}（须为 chat / eggs / tech）"
                )));
            }
        }
        self.map_auth_failure(client::update_my_post(&state.base_url, &auth_token, post_id, &input).await)
    }

    /// 删除自己的帖子（Worker 级联删除其全部回复与相关举报）
    pub async fn delete_my_post(&self, post_id: i64) -> IcodeResult<()> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::delete_my_post(&state.base_url, &auth_token, post_id).await)
    }

    /// 编辑自己的回复（前置校验与发回复一致）
    pub async fn update_my_reply(&self, reply_id: i64, content: &str) -> IcodeResult<()> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        let c = content.trim();
        if c.is_empty() {
            return Err(IcodeError::validation("回复内容不能为空"));
        }
        if c.chars().count() > 1000 {
            return Err(IcodeError::validation("回复不能超过 1000 字"));
        }
        self.map_auth_failure(client::update_my_reply(&state.base_url, &auth_token, reply_id, c).await)
    }

    /// 删除自己的回复（顶层评论级联楼中楼）
    pub async fn delete_my_reply(&self, reply_id: i64) -> IcodeResult<()> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::delete_my_reply(&state.base_url, &auth_token, reply_id).await)
    }

    /// 举报
    pub async fn report(&self, input: ReportInput) -> IcodeResult<i64> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::report(&state.base_url, &auth_token, &input).await)
    }

    /// 全站治理开关（D11：用户端只读，前端据此禁用发帖 / 回复入口）
    pub async fn get_site_governance(&self) -> IcodeResult<SiteGovernance> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::get_site_governance(&state.base_url, &auth_token).await)
    }

    /// 积分排行（offset 分页；Worker 侧过滤封禁用户，禁言用户仍展示）
    pub async fn get_points_leaderboard(
        &self,
        offset: Option<i64>,
        limit: Option<u32>,
    ) -> IcodeResult<PointsLeaderboardData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::get_points_leaderboard(&state.base_url, &auth_token, offset, limit).await)
    }

    /// 签到排行（offset 分页；Worker 侧返回累计 `total` 与连续 `streak` 两列表）
    pub async fn get_checkin_leaderboard(
        &self,
        offset: Option<i64>,
        limit: Option<u32>,
    ) -> IcodeResult<CheckInLeaderboardData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::get_checkin_leaderboard(&state.base_url, &auth_token, offset, limit).await)
    }

    // ===== 消息通知（2026-08-23 通知迭代）=====

    /// 通知列表（游标分页；顺带返回未读数供小红点）
    pub async fn get_notifications(
        &self,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> IcodeResult<NotificationListData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::list_notifications(&state.base_url, &auth_token, cursor, limit).await)
    }

    /// 未读通知数（小红点）
    pub async fn get_unread_count(&self) -> IcodeResult<UnreadCountData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::get_unread_count(&state.base_url, &auth_token).await)
    }

    /// 全部标记已读（返回本次更新的条数）
    pub async fn read_all_notifications(&self) -> IcodeResult<ReadAllNotificationsData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::read_all_notifications(&state.base_url, &auth_token).await)
    }

    // ===== 管理员 =====

    pub async fn admin_login(&self, input: AdminLoginInput) -> IcodeResult<AdminLoginData> {
        let base_url = self.require_enabled_base()?;
        client::admin_login(&base_url, &input).await
    }

    pub async fn admin_list_users(&self, admin_token: &str) -> IcodeResult<Vec<AdminUserItem>> {
        let base_url = self.require_enabled_base()?;
        client::admin_list_users(&base_url, admin_token).await
    }

    pub async fn admin_ban_user(
        &self,
        admin_token: &str,
        user_id: &str,
        reason: Option<String>,
    ) -> IcodeResult<()> {
        let base_url = self.require_enabled_base()?;
        client::admin_ban_user(&base_url, admin_token, user_id, reason).await
    }

    pub async fn admin_unban_user(
        &self,
        admin_token: &str,
        user_id: &str,
    ) -> IcodeResult<()> {
        let base_url = self.require_enabled_base()?;
        client::admin_unban_user(&base_url, admin_token, user_id).await
    }

    /// 禁言用户（D12：服务端校验 until 格式，委托 Worker 落库）
    pub async fn admin_mute_user(
        &self,
        admin_token: &str,
        user_id: &str,
        input: AdminMuteInput,
    ) -> IcodeResult<()> {
        let base_url = self.require_enabled_base()?;
        // until：None = 永久；Some 时须为合法 ISO 时间字符串（此处提前拦截，Worker 仍兜底）
        if let Some(until) = input.until.as_deref() {
            if chrono::DateTime::parse_from_rfc3339(until).is_err() {
                return Err(IcodeError::validation("until 须为合法 ISO 时间字符串"));
            }
        }
        client::admin_mute_user(&base_url, admin_token, user_id, &input).await
    }

    /// 解除用户禁言
    pub async fn admin_unmute_user(
        &self,
        admin_token: &str,
        user_id: &str,
    ) -> IcodeResult<()> {
        let base_url = self.require_enabled_base()?;
        client::admin_unmute_user(&base_url, admin_token, user_id).await
    }

    pub async fn admin_list_reports(
        &self,
        admin_token: &str,
    ) -> IcodeResult<Vec<AdminReportItem>> {
        let base_url = self.require_enabled_base()?;
        client::admin_list_reports(&base_url, admin_token).await
    }

    pub async fn admin_resolve_report(
        &self,
        admin_token: &str,
        report_id: i64,
    ) -> IcodeResult<()> {
        let base_url = self.require_enabled_base()?;
        client::admin_resolve_report(&base_url, admin_token, report_id).await
    }

    // ===== 管理员帖子管理（D10）=====

    /// 管理员帖子列表（所有用户，游标分页；section 可选过滤，非法值提前拦截）
    pub async fn admin_list_posts(
        &self,
        admin_token: &str,
        cursor: Option<String>,
        limit: Option<u32>,
        section: Option<String>,
    ) -> IcodeResult<AdminPostListData> {
        let base_url = self.require_enabled_base()?;
        let section = Self::normalize_section_filter(section.as_deref())?;
        client::admin_list_posts(&base_url, admin_token, cursor, limit, section).await
    }

    /// 管理员帖子详情 + 评论区（定位待处置回复）
    pub async fn admin_get_post(
        &self,
        admin_token: &str,
        post_id: i64,
    ) -> IcodeResult<PostDetailData> {
        let base_url = self.require_enabled_base()?;
        client::admin_get_post(&base_url, admin_token, post_id).await
    }

    /// 管理员编辑帖子（title / content / section 至少一项，提前做与发帖一致的校验）
    pub async fn admin_update_post(
        &self,
        admin_token: &str,
        post_id: i64,
        input: AdminUpdatePostInput,
    ) -> IcodeResult<()> {
        let base_url = self.require_enabled_base()?;
        // 至少一项字段；提供字段时做与发帖一致的前置校验（Worker 侧仍兜底）
        if input.title.is_none() && input.content.is_none() && input.section.is_none() {
            return Err(IcodeError::validation("请提供要修改的字段"));
        }
        if let Some(title) = input.title.as_deref() {
            let t = title.trim();
            if t.is_empty() {
                return Err(IcodeError::validation("标题不能为空"));
            }
            if t.chars().count() > 80 {
                return Err(IcodeError::validation("标题不能超过 80 字"));
            }
        }
        if let Some(content) = input.content.as_deref() {
            let c = content.trim();
            if c.is_empty() {
                return Err(IcodeError::validation("内容不能为空"));
            }
            if c.chars().count() > 10000 {
                return Err(IcodeError::validation("内容不能超过 10000 字"));
            }
        }
        if let Some(section) = input.section.as_deref() {
            if !super::types::is_valid_section(section) {
                return Err(IcodeError::validation(format!(
                    "板块非法：{section}（须为 chat / eggs / tech）"
                )));
            }
        }
        client::admin_update_post(&base_url, admin_token, post_id, &input).await
    }

    /// 管理员删除帖子（Worker 级联删除其全部回复与相关举报）
    pub async fn admin_delete_post(&self, admin_token: &str, post_id: i64) -> IcodeResult<()> {
        let base_url = self.require_enabled_base()?;
        client::admin_delete_post(&base_url, admin_token, post_id).await
    }

    /// 管理员编辑回复（前置校验与发回复一致）
    pub async fn admin_update_reply(
        &self,
        admin_token: &str,
        reply_id: i64,
        content: &str,
    ) -> IcodeResult<()> {
        let base_url = self.require_enabled_base()?;
        let c = content.trim();
        if c.is_empty() {
            return Err(IcodeError::validation("回复内容不能为空"));
        }
        if c.chars().count() > 1000 {
            return Err(IcodeError::validation("回复不能超过 1000 字"));
        }
        client::admin_update_reply(&base_url, admin_token, reply_id, c).await
    }

    /// 管理员删除回复（顶层评论级联楼中楼）
    pub async fn admin_delete_reply(&self, admin_token: &str, reply_id: i64) -> IcodeResult<()> {
        let base_url = self.require_enabled_base()?;
        client::admin_delete_reply(&base_url, admin_token, reply_id).await
    }

    // ===== 站点治理（D11）=====

    /// 管理员读取全站治理开关
    pub async fn admin_get_governance(&self, admin_token: &str) -> IcodeResult<SiteGovernance> {
        let base_url = self.require_enabled_base()?;
        client::admin_get_governance(&base_url, admin_token).await
    }

    /// 管理员更新全站治理开关（部分更新，返回最新完整状态）
    pub async fn admin_update_governance(
        &self,
        admin_token: &str,
        input: AdminUpdateGovernanceInput,
    ) -> IcodeResult<SiteGovernance> {
        let base_url = self.require_enabled_base()?;
        if input.mute_all.is_none() && input.post_locked.is_none() && input.reply_locked.is_none() {
            return Err(IcodeError::validation(
                "请提供要修改的开关（muteAll / postLocked / replyLocked）",
            ));
        }
        client::admin_update_governance(&base_url, admin_token, &input).await
    }

    /// 管理员锁定 / 解锁帖子（locked=1 时该帖禁止新增评论回复）
    pub async fn admin_set_post_locked(
        &self,
        admin_token: &str,
        post_id: i64,
        locked: bool,
    ) -> IcodeResult<()> {
        let base_url = self.require_enabled_base()?;
        client::admin_set_post_locked(&base_url, admin_token, post_id, locked).await
    }

    /// 管理员置顶 / 取消置顶帖子（置顶帖在列表排序时排在最前）
    pub async fn admin_set_post_pin(
        &self,
        admin_token: &str,
        post_id: i64,
        pinned: bool,
    ) -> IcodeResult<()> {
        let base_url = self.require_enabled_base()?;
        client::admin_set_post_pin(&base_url, admin_token, post_id, pinned).await
    }

    // ===== 帖子外链分享（2026-08-26 分享迭代，见 docs/proposals/community-post-share.md）=====

    /// 发起分享：作者本人 + 扣 100 积分（Worker 校验），返回带直链的链接
    pub async fn create_share_link(
        &self,
        post_id: i64,
        input: ShareLinkInput,
    ) -> IcodeResult<ShareLink> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        // maxViews：缺省 1000，范围 1~10000（提前拦截，Worker 仍兜底）
        let max_views = input.max_views.unwrap_or(1000);
        if max_views < 1 || max_views > 10000 {
            return Err(IcodeError::validation("maxViews 须为 1~10000 的整数"));
        }
        let normalized = ShareLinkInput {
            max_views: Some(max_views),
        };
        self.map_auth_failure(client::create_share_link(&state.base_url, &auth_token, post_id, &normalized).await)
    }

    /// 该帖分享列表（游标分页；仅作者本人可见，Worker 校验归属）
    pub async fn list_post_share_links(
        &self,
        post_id: i64,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> IcodeResult<ShareLinkListData> {
        let (state, _user_id, auth_token) = self.require_ready()?;
        self.map_auth_failure(client::list_post_share_links(&state.base_url, &auth_token, post_id, cursor, limit).await)
    }

    /// 管理员分享列表（全站 / 按帖子过滤，游标分页）
    pub async fn admin_list_share_links(
        &self,
        admin_token: &str,
        cursor: Option<String>,
        limit: Option<u32>,
        post_id: Option<i64>,
    ) -> IcodeResult<AdminShareListData> {
        let base_url = self.require_enabled_base()?;
        client::admin_list_share_links(&base_url, admin_token, cursor, limit, post_id).await
    }

    /// 管理员撤销任意分享（不返还积分）
    pub async fn admin_revoke_share_link(
        &self,
        admin_token: &str,
        pid: &str,
    ) -> IcodeResult<()> {
        let base_url = self.require_enabled_base()?;
        client::admin_revoke_share_link(&base_url, admin_token, pid).await
    }

    // ===== 内部辅助 =====

    /// 同步昵称 / 头像到本地缓存
    fn sync_profile_cache(&self, user: &ProfileUser) -> IcodeResult<()> {
        let mut state = self.get_local_state()?;
        state.nickname = Some(user.nickname.clone());
        state.avatar_index = Some(user.avatar_index);
        repository::set_local_state(&state)
    }
}
