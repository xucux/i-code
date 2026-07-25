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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveVirtualModelRouteInput {
    pub target_provider_id: String,
    pub target_model_id: String,
    pub priority: i64,
    pub enabled: bool,
    pub is_healthy: bool,
    pub max_retries: i64,
    pub retry_interval_ms: i64,
}

/// 路由解析结果
///
/// 由 Service 层返回给 gateway_runtime，用于实际请求转发
#[derive(Debug, Clone)]
pub struct ResolvedVirtualRoute {
    /// 目标真实供应商 ID
    pub target_provider_id: String,
    /// 目标真实模型 ID
    pub target_model_id: String,
    /// 当前是第几条路由（用于日志）
    pub route_index: usize,
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
