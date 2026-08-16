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
    AdminLoginData, AdminLoginInput, AdminMuteInput, AdminPostListData, AdminReportItem,
    AdminUpdateGovernanceInput, AdminUpdatePostInput, AdminUserItem, CheckInResult,
    CommunityLocalState, CreatePostInput, CreateReplyInput, MyPostsData, MyRepliesData,
    PostDetailData, PostListData, ProfileData, ProfileUser, ReportInput, SiteGovernance,
    UpdateProfileInput,
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

    /// 校验门禁已开启并返回 (本地状态, user_id)
    fn require_ready(&self) -> IcodeResult<(CommunityLocalState, String)> {
        let state = self.get_local_state()?;
        if !state.enabled {
            return Err(IcodeError::forbidden("社区尚未开启"));
        }
        let user_id = state
            .user_id
            .clone()
            .ok_or_else(|| IcodeError::forbidden("社区身份尚未初始化"))?;
        Ok((state, user_id))
    }

    /// 校验门禁已开启并返回 base_url（管理员接口，不要求 user_id）
    fn require_enabled_base(&self) -> IcodeResult<String> {
        let state = self.get_local_state()?;
        if !state.enabled {
            return Err(IcodeError::forbidden("社区尚未开启"));
        }
        Ok(state.base_url)
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
        let (state, user_id) = self.require_ready()?;
        let section = Self::normalize_section_filter(section.as_deref())?;
        client::list_posts(&state.base_url, &user_id, cursor, limit, section).await
    }

    pub async fn get_post(&self, post_id: i64) -> IcodeResult<PostDetailData> {
        let (state, user_id) = self.require_ready()?;
        client::get_post(&state.base_url, &user_id, post_id).await
    }

    pub async fn create_post(&self, mut input: CreatePostInput) -> IcodeResult<i64> {
        let (state, user_id) = self.require_ready()?;
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
        client::create_post(&state.base_url, &user_id, &input).await
    }

    pub async fn create_reply(&self, post_id: i64, input: CreateReplyInput) -> IcodeResult<i64> {
        let (state, user_id) = self.require_ready()?;
        client::create_reply(&state.base_url, &user_id, post_id, &input).await
    }

    // ===== 用户中心 =====

    /// 我的资料 + 签到统计；顺带把昵称/头像同步回本地缓存（§7.3：以 /users/me 为准）
    pub async fn get_profile(&self) -> IcodeResult<ProfileData> {
        let (state, user_id) = self.require_ready()?;
        let data = client::get_profile(&state.base_url, &user_id).await?;
        self.sync_profile_cache(&data.user)?;
        Ok(data)
    }

    /// 改昵称 / 头像；成功后同步本地缓存
    pub async fn update_profile(&self, input: UpdateProfileInput) -> IcodeResult<ProfileUser> {
        let (state, user_id) = self.require_ready()?;
        let user = client::update_profile(&state.base_url, &user_id, &input).await?;
        self.sync_profile_cache(&user)?;
        Ok(user)
    }

    /// 签到（重复签到由 Worker 返回 409）；返回统计 + 本次获得积分
    pub async fn check_in(&self) -> IcodeResult<CheckInResult> {
        let (state, user_id) = self.require_ready()?;
        client::check_in(&state.base_url, &user_id).await
    }

    pub async fn get_my_posts(
        &self,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> IcodeResult<MyPostsData> {
        let (state, user_id) = self.require_ready()?;
        client::list_my_posts(&state.base_url, &user_id, cursor, limit).await
    }

    pub async fn get_my_replies(
        &self,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> IcodeResult<MyRepliesData> {
        let (state, user_id) = self.require_ready()?;
        client::list_my_replies(&state.base_url, &user_id, cursor, limit).await
    }

    /// 举报
    pub async fn report(&self, input: ReportInput) -> IcodeResult<i64> {
        let (state, user_id) = self.require_ready()?;
        client::report(&state.base_url, &user_id, &input).await
    }

    /// 全站治理开关（D11：用户端只读，前端据此禁用发帖 / 回复入口）
    pub async fn get_site_governance(&self) -> IcodeResult<SiteGovernance> {
        let (state, user_id) = self.require_ready()?;
        client::get_site_governance(&state.base_url, &user_id).await
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
            if c.chars().count() > 5000 {
                return Err(IcodeError::validation("内容不能超过 5000 字"));
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

    // ===== 内部辅助 =====

    /// 同步昵称 / 头像到本地缓存
    fn sync_profile_cache(&self, user: &ProfileUser) -> IcodeResult<()> {
        let mut state = self.get_local_state()?;
        state.nickname = Some(user.nickname.clone());
        state.avatar_index = Some(user.avatar_index);
        repository::set_local_state(&state)
    }
}
