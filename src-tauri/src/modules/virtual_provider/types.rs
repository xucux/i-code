//! # 虚拟供应商模块类型定义
//!
//! 与数据库 migration V001 中 `virtual_providers` / `virtual_models` /
//! `virtual_model_routes` 三表对齐。

use serde::{Deserialize, Serialize};

/// 虚拟供应商路由策略
///
/// 当前仅支持顺序回退（`fallback`）：按「冷却中最后 → 不健康最后 → priority 升序」
/// 逐条尝试，失败即切换。历史上存在的 `on_all` / `load_balance` 已废弃，
/// 由迁移 V013 归一为 `fallback`，新建 / 更新时拒绝写入其它值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VirtualProviderStrategy {
    /// 按优先级顺序尝试，失败则切换下一条（唯一支持的策略）
    #[default]
    Fallback,
}

impl VirtualProviderStrategy {
    /// 解析策略字符串；仅接受 `fallback`，其它值（含历史策略）返回 `None`
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "fallback" => Some(Self::Fallback),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fallback => "fallback",
        }
    }
}

/// 虚拟供应商 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualProvider {
    pub id: String,
    pub name: String,
    pub alias: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub is_enabled: bool,
    pub strategy: String,
    pub max_retries: i64,
    pub retry_interval_ms: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 虚拟模型 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualModel {
    pub id: String,
    pub virtual_provider_id: String,
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub is_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 对外暴露的虚拟模型（用于 `/v1/models` 接口）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExposedVirtualModel {
    /// 对外路由 ID：`{virtual_alias}/{model_id}`
    pub id: String,
    pub virtual_provider_id: String,
    /// 虚拟供应商 alias
    pub alias: String,
    pub model_id: String,
    pub display_name: String,
}

/// 虚拟路由失败类型
///
/// 由网关错误判定器（`gateway_runtime::forwarding::error_classifier`）产出，
/// 用于决定是否切换下一条路由，以及降级时的冷却时长。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteFailureKind {
    /// 请求级错误（参数非法 / 上下文超限 / 内容安全拒绝）：透传，不切换、不降级
    ClientRequest,
    /// 额度耗尽：切换 + 长冷却
    QuotaExhausted,
    /// 凭证失效 / 无权限：切换 + 长冷却
    AuthFailure,
    /// 模型不可用（不存在 / 无权使用）：切换 + 长冷却
    ModelUnavailable,
    /// 供应商配置错误（协议 / 路径不兼容）：切换 + 长冷却
    ProviderMisconfigured,
    /// 限流：切换 + 短冷却
    RateLimited,
    /// 上游不可用（5xx）：切换 + 短冷却
    UpstreamUnavailable,
    /// 上游超时：切换 + 短冷却
    Timeout,
}

impl RouteFailureKind {
    /// 字符串标识（落库到 `virtual_model_routes.failure_kind`）
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ClientRequest => "client_request",
            Self::QuotaExhausted => "quota_exhausted",
            Self::AuthFailure => "auth_failure",
            Self::ModelUnavailable => "model_unavailable",
            Self::ProviderMisconfigured => "provider_misconfigured",
            Self::RateLimited => "rate_limited",
            Self::UpstreamUnavailable => "upstream_unavailable",
            Self::Timeout => "timeout",
        }
    }

    /// 从字符串解析（无法识别时返回 None）
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "client_request" => Some(Self::ClientRequest),
            "quota_exhausted" => Some(Self::QuotaExhausted),
            "auth_failure" => Some(Self::AuthFailure),
            "model_unavailable" => Some(Self::ModelUnavailable),
            "provider_misconfigured" => Some(Self::ProviderMisconfigured),
            "rate_limited" => Some(Self::RateLimited),
            "upstream_unavailable" => Some(Self::UpstreamUnavailable),
            "timeout" => Some(Self::Timeout),
            _ => None,
        }
    }

    /// 是否为永久性故障（长冷却期间该路由不参与选路，全部路由都永久故障时才作为兜底尝试）
    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            Self::QuotaExhausted
                | Self::AuthFailure
                | Self::ModelUnavailable
                | Self::ProviderMisconfigured
        )
    }

    /// 是否属于请求级错误（不触发故障转移）
    pub fn is_client_request(&self) -> bool {
        matches!(self, Self::ClientRequest)
    }
}

/// 路由降级惩罚（与"是否切换路由"分离）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutePenalty {
    /// 不惩罚：该路由本身没有故障（如上下文超限、内容安全拒绝）
    None,
    /// 短冷却：限流 / 5xx / 超时，60 秒后自动恢复参与选路
    Short,
    /// 长冷却：额度 / 凭证 / 模型不可用 / 配置错误，30 分钟后才允许探活恢复
    Long,
}

impl RoutePenalty {
    /// 冷却秒数；`None` 表示不写入冷却时间
    pub fn cooldown_secs(&self) -> Option<i64> {
        match self {
            Self::None => None,
            Self::Short => Some(60),
            Self::Long => Some(30 * 60),
        }
    }
}

/// 虚拟模型路由 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualModelRoute {
    pub id: String,
    pub virtual_model_id: String,
    pub target_provider_id: String,
    pub target_model_id: String,
    pub priority: i64,
    pub enabled: bool,
    pub max_retries: i64,
    pub retry_interval_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<i64>,
    pub is_healthy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_healthy_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_headers_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_body_json: Option<String>,
    /// 连续探活失败次数（探活成功后清零）
    pub consecutive_failures: i64,
    /// 上次探活失败原因（探活成功后清空）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_text: Option<String>,
    /// 上次探活耗时（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_check_duration_ms: Option<i64>,
    /// 上次探活时间戳
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_check_at: Option<String>,
    /// 负载均衡权重（默认 1，0 表示不参与轮询）；仅在 load_balance 策略下生效
    pub weight: i64,
    /// 冷却截止时间（RFC3339）；未到期前该路由不参与选路（永久故障仅作最后兜底）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown_until: Option<String>,
    /// 最近一次失败类型（对应 [`RouteFailureKind::as_str`]）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<String>,
    /// 最近一次真实路由尝试时间（RFC3339，请求侧转发；非探活）
    ///
    /// 用于「全部路由永久失活」时按兜底间隔判断是否允许再尝试一次。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_attempted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建虚拟供应商输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateVirtualProviderInput {
    pub name: String,
    pub alias: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub is_enabled: bool,
    #[serde(default)]
    pub strategy: Option<String>,
    #[serde(default = "default_provider_max_retries")]
    pub max_retries: i64,
    #[serde(default = "default_provider_retry_interval_ms")]
    pub retry_interval_ms: i64,
}

/// 更新虚拟供应商输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateVirtualProviderInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_interval_ms: Option<i64>,
}

/// 创建虚拟模型输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateVirtualModelInput {
    pub virtual_provider_id: String,
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub is_enabled: bool,
}

/// 更新虚拟模型输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateVirtualModelInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enabled: Option<bool>,
}

/// 创建虚拟模型路由输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateVirtualModelRouteInput {
    pub virtual_model_id: String,
    pub target_provider_id: String,
    pub target_model_id: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default = "default_route_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub max_retries: i64,
    #[serde(default = "default_route_retry_interval_ms")]
    pub retry_interval_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<i64>,
}

/// 更新虚拟模型路由输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateVirtualModelRouteInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_interval_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<i64>,
}

fn default_route_enabled() -> bool {
    true
}

fn default_route_retry_interval_ms() -> i64 {
    1000
}

fn default_provider_max_retries() -> i64 {
    3
}

fn default_provider_retry_interval_ms() -> i64 {
    1000
}

/// 保存虚拟模型完整输入（包含子级路由）
///
/// 用于 `virtual_model_save` 命令，前端把父级虚拟模型与全部子级真实模型路由一次性提交，
/// 后端在事务中完成创建/更新，并重新关联子级路由。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveVirtualModelInput {
    /// 虚拟模型 ID，None 表示新建，Some 表示更新
    pub id: Option<String>,
    /// 所属虚拟供应商 ID，新建时必填
    pub virtual_provider_id: String,
    /// 虚拟模型对外标识
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub is_enabled: bool,
    /// 子级真实模型路由列表
    pub routes: Vec<SaveVirtualModelRouteInput>,
}

/// 保存虚拟模型时携带的单条子级路由输入
///
/// 运行态字段（健康 / 冷却 / 重试 / 超时 / 权重）不在此输入中：保存时由
/// repository 按 `(provider_id, model_id)` 从已有路由继承，避免编辑弹窗覆盖运行态。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveVirtualModelRouteInput {
    pub target_provider_id: String,
    pub target_model_id: String,
    pub priority: i64,
    /// 是否启用该路由（前端传入以支持单独禁用）
    pub enabled: bool,
    /// 路由级附加请求头（JSON 对象），覆盖供应商级同名头
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_headers: Option<serde_json::Value>,
    /// 路由级附加请求体参数（JSON 对象），浅合并到请求体
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_body: Option<serde_json::Value>,
}

/// 路由解析结果
///
/// 由 Service 层返回给 gateway_runtime，用于实际请求转发
#[expect(dead_code)]
#[derive(Debug, Clone)]
pub struct ResolvedVirtualRoute {
    /// 目标真实供应商 ID
    pub target_provider_id: String,
    /// 目标真实模型 ID
    pub target_model_id: String,
    /// 当前是第几条路由（用于日志）
    pub route_index: usize,
}

/// 虚拟路由尝试历史 DTO
///
/// 对应 `virtual_route_attempts` 表，由 `VirtualForwarder` 在每条路由尝试结束后异步写入。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualRouteAttempt {
    pub id: String,
    pub virtual_route_id: String,
    pub virtual_provider_id: String,
    pub request_id: String,
    /// 第几条尝试（0-based）
    pub attempt_index: i64,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub duration_ms: i64,
    pub attempted_at: String,
}

/// 路由维度统计聚合
///
/// 由 repository 聚合查询返回，供 UI 展示每条路由的成功率/平均耗时。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteAttemptStats {
    pub virtual_route_id: String,
    /// 总尝试次数
    pub total: i64,
    /// 成功次数
    pub success_count: i64,
    /// 失败次数
    pub failure_count: i64,
    /// 成功率（0-100，整数百分比）
    pub success_rate: i64,
    /// 平均耗时（毫秒）
    pub avg_duration_ms: i64,
    /// 最近一次失败原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// 最近一次尝试时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_attempted_at: Option<String>,
}

/// 单条路由探活记录
///
/// 对应 `virtual_route_probe_logs` 表，由调度器自动探活与手动「测试路由」写入。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualRouteProbe {
    pub id: String,
    pub virtual_route_id: String,
    pub virtual_provider_id: String,
    /// 来源：`auto`（调度器自动探活）/ `manual`（手动测试路由）
    pub source: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<i64>,
    pub duration_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// 失败类型（见 [`RouteFailureKind::as_str`]），无法判定 / 手动失败时为 None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<String>,
    pub probed_at: String,
}

/// 路由维度探活统计聚合
///
/// 供 UI 展示每条路由的探活成功率 / 平均耗时 / 最近失败原因。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteProbeStats {
    pub virtual_route_id: String,
    /// 总探活次数
    pub total: i64,
    /// 成功次数
    pub success_count: i64,
    /// 失败次数（含无法判定）
    pub failure_count: i64,
    /// 成功率（0-100，整数百分比）
    pub success_rate: i64,
    /// 平均耗时（毫秒）
    pub avg_duration_ms: i64,
    /// 最近一次失败原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// 最近一次探活时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_probed_at: Option<String>,
}

/// 失败请求尝试链中的单条路由尝试
///
/// 由 `virtual_route_attempts` 关联路由与供应商表得到，供 UI 展示“本次请求走了哪几条路由”。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestAttemptStep {
    /// 尝试序号（0 起）
    pub attempt_index: i64,
    pub virtual_route_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_model_id: Option<String>,
    /// 目标供应商展示名（路由已被删除时为 None）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_provider_name: Option<String>,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub duration_ms: i64,
    pub attempted_at: String,
}

/// 按 request_id 聚合的失败请求尝试链
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestAttemptChain {
    pub request_id: String,
    /// 虚拟供应商 alias（attempts 表中的占位字段归一化后的展示值）
    pub virtual_provider_alias: String,
    /// 该请求首条尝试时间
    pub started_at: String,
    /// 尝试顺序（attempt_index 升序）
    pub attempts: Vec<RequestAttemptStep>,
}

/// 虚拟路由探活 / 全部失活兜底间隔配置
///
/// 由 `virtual_health_check_config_get` 返回；间隔单位均为秒，
/// `min/max` 为允许配置范围（30s ~ 24h）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualHealthCheckConfig {
    /// 降级路由探活间隔（秒）
    pub health_check_interval_secs: u64,
    /// 全部路由永久失活时的兜底尝试间隔（秒）
    pub all_failed_fallback_interval_secs: u64,
    /// 路由尝试历史保留天数
    pub attempts_keep_days: u32,
    pub default_health_check_interval_secs: u64,
    pub default_all_failed_fallback_interval_secs: u64,
    pub default_attempts_keep_days: u32,
    pub min_interval_secs: u64,
    pub max_interval_secs: u64,
    pub min_attempts_keep_days: u32,
    pub max_attempts_keep_days: u32,
}

/// 更新探活 / 兜底间隔 / 历史保留天数输入（None 表示不修改）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualHealthCheckConfigSetInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_check_interval_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_failed_fallback_interval_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempts_keep_days: Option<u32>,
}

/// 单条路由测试结果
///
/// 由 `virtual_provider_route_test` 命令返回，供前端 toast 展示。
/// 不写入 `virtual_route_attempts`（避免污染统计）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteTestResult {
    /// 是否成功（2xx）
    pub success: bool,
    /// HTTP 状态码（网络错误时为 None）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    /// 请求耗时（毫秒）
    pub duration_ms: u64,
    /// 错误信息（失败时提供）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// alias 变更影响检查结果
///
/// 由 `virtual_provider_check_alias_impact` 命令返回，
/// 供前端在用户修改 alias 时展示影响范围警告。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasImpactResult {
    /// 旧 alias
    pub old_alias: String,
    /// 新 alias
    pub new_alias: String,
    /// 受影响的 CLI 模型映射数量（gateway_model_id 以旧 alias 为前缀）
    pub affected_cli_model_mappings: i64,
    /// 是否有影响（任一受影响计数 > 0）
    pub has_impact: bool,
}

// ===== 一键生成（三模型槽位）=====

/// 一键生成数据源 JSON 的虚拟供应商元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualSlotsProvider {
    /// 虚拟供应商别名（唯一）
    pub alias: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub max_retries: i64,
    #[serde(default)]
    pub retry_interval_ms: i64,
    #[serde(default)]
    pub is_enabled: bool,
}

/// 一键生成数据源 JSON 中单个槽位的匹配规则
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualSlotsMatchRule {
    /// 匹配优先级（越小越优先）；同时作为生成路由的 priority
    pub priority: i64,
    /// 匹配类型：exact / prefix / regex
    #[serde(default = "default_match_type")]
    pub r#type: String,
    /// exact / prefix 时的目标模型 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// regex 时的正则表达式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

fn default_match_type() -> String {
    "exact".to_string()
}

/// 一键生成数据源 JSON 中单个虚拟模型槽位
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualSlotsSlot {
    /// 槽位标识（程序内部引用，不直接落库）
    pub key: String,
    /// 虚拟模型对外 ID
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 用途：planning / coding / tool_use
    #[serde(default)]
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 实体模型匹配规则，按 priority 升序
    pub matches: Vec<VirtualSlotsMatchRule>,
}

/// 一键生成数据源 JSON（远程或内置）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualSlotsConfig {
    /// 数据源 schema 版本
    pub schema_version: i64,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    pub provider: VirtualSlotsProvider,
    pub slots: Vec<VirtualSlotsSlot>,
}

/// 一键生成输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateVirtualProviderInput {
    /// 可选，覆盖设置中的自定义数据源 URL；空/省略时读取已配置的 URL（为空则回退内置 JSON）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source_url: Option<String>,
    /// 可选，覆盖数据源 JSON 中的重试次数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<i64>,
    /// 可选，覆盖数据源 JSON 中的重试间隔
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_interval_ms: Option<i64>,
}

/// 单个槽位生成结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotGenerationResult {
    /// 槽位标识
    pub key: String,
    /// 生成的虚拟模型 ID
    pub model_id: String,
    /// 展示名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 命中并生成的子级路由数量
    pub route_count: i64,
    /// 未命中任何实体模型时标记
    #[serde(default)]
    pub empty: bool,
}

/// 一键生成结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateVirtualProviderResult {
    /// 本次生成或复用的虚拟供应商
    pub provider: VirtualProvider,
    /// 三个槽位的生成明细
    pub slots: Vec<SlotGenerationResult>,
}

/// 数据源配置读取结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualSlotsConfigDto {
    /// 用户已保存的自定义数据源 URL（可能为空字符串）
    pub data_source_url: String,
    /// 内置默认数据源 URL
    pub default_url: String,
    /// 实际生效的数据源 URL（dataSourceUrl 非空时为其，否则为 defaultUrl）
    pub effective_url: String,
    /// 是否使用默认 URL（用户未自定义）
    pub use_default: bool,
}

/// 数据源配置写入输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualSlotsConfigSetInput {
    /// 自定义数据源 URL；空字符串表示清空（恢复默认）
    pub data_source_url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_roundtrip() {
        assert_eq!(
            VirtualProviderStrategy::from_str("fallback"),
            Some(VirtualProviderStrategy::Fallback)
        );
        // 历史策略已移除：不再允许解析 / 保存
        assert_eq!(VirtualProviderStrategy::from_str("on_all"), None);
        assert_eq!(VirtualProviderStrategy::from_str("load_balance"), None);
        assert_eq!(VirtualProviderStrategy::from_str("unknown"), None);
    }
}
