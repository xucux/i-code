//! # 虚拟供应商模块类型定义
//!
//! 与数据库 migration V001 中 `virtual_providers` / `virtual_models` /
//! `virtual_model_routes` 三表对齐。

use serde::{Deserialize, Serialize};

/// 虚拟供应策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualProviderStrategy {
    /// 同时请求所有可用路由（v0.1 未实现）
    OnAll,
    /// 按优先级顺序尝试，失败则切换下一条（默认）
    Fallback,
    /// 按权重轮询（v0.1 未实现）
    LoadBalance,
}

impl Default for VirtualProviderStrategy {
    fn default() -> Self {
        Self::Fallback
    }
}

impl VirtualProviderStrategy {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "on_all" => Some(Self::OnAll),
            "fallback" => Some(Self::Fallback),
            "load_balance" => Some(Self::LoadBalance),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OnAll => "on_all",
            Self::Fallback => "fallback",
            Self::LoadBalance => "load_balance",
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

/// 路由权重默认值（load_balance 策略下生效）
fn default_route_weight() -> i64 {
    1
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveVirtualModelRouteInput {
    pub target_provider_id: String,
    pub target_model_id: String,
    pub priority: i64,
    /// 是否启用该路由（原硬编码 true，改为前端传入以支持单独禁用）
    pub enabled: bool,
    pub is_healthy: bool,
    pub max_retries: i64,
    pub retry_interval_ms: i64,
    /// 路由级超时（毫秒），覆盖供应商级配置；None 表示继承
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<i64>,
    /// 路由级附加请求头（JSON 对象），覆盖供应商级同名头
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_headers: Option<serde_json::Value>,
    /// 路由级附加请求体参数（JSON 对象），浅合并到请求体
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_body: Option<serde_json::Value>,
    /// 负载均衡权重（默认 1，0 表示不参与轮询）；仅在 load_balance 策略下生效
    #[serde(default = "default_route_weight")]
    pub weight: i64,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_roundtrip() {
        for s in [VirtualProviderStrategy::OnAll, VirtualProviderStrategy::Fallback, VirtualProviderStrategy::LoadBalance] {
            let text = s.as_str();
            assert_eq!(VirtualProviderStrategy::from_str(text), Some(s));
        }
        assert_eq!(VirtualProviderStrategy::from_str("unknown"), None);
    }
}
