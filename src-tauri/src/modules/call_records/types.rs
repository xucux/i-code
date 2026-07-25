//! # 调用记录模块类型定义
//!
//! 与数据库 migration V001 中 `model_call_logs` 表对齐。
//! V006 迁移新增 `model_call_stats_hourly` / `model_call_stats_daily` 聚合表。
//!
//! ## 路由模式
//!
//! - `Direct`（1）：直接请求真实供应商。
//! - `VirtualFallback`（2）：通过虚拟供应商的 fallback 策略路由。
//!
//! ## 聚合表策略
//!
//! 采用**累加计算（UPSERT ON CONFLICT 累加）**，每次请求完成时实时累加，
//! 避免查询时 GROUP BY 全表扫描明细表。

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// 路由模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RouteMode {
    /// 直接请求真实供应商
    Direct = 1,
    /// 虚拟供应商故障转移
    VirtualFallback = 2,
}

impl RouteMode {
    pub fn as_i64(&self) -> i64 {
        *self as i64
    }

    pub fn from_i64(v: i64) -> Option<Self> {
        match v {
            1 => Some(Self::Direct),
            2 => Some(Self::VirtualFallback),
            _ => None,
        }
    }
}

/// 模型调用记录 DTO
///
/// 与 `model_call_logs` 表结构对齐。
/// V004 迁移追加 `source`、`time_to_first_token_ms`、`price_per_1m_tokens` 三列。
/// V013 迁移追加 `api_key_secret_id` 列。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCallLog {
    pub id: String,
    pub provider_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_model_id: Option<String>,
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub requested_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<i64>,
    pub cache_hit: bool,
    pub route_mode: i64,
    /// 请求入口来源：`cli` / `gateway` / `internal`
    pub source: String,
    /// 流式响应首字延迟（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_to_first_token_ms: Option<i64>,
    /// 记录时单价快照（CNY / 1M tokens）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_per_1m_tokens: Option<f64>,
    /// 请求使用的 Gateway API Key 明文（命中 `gateway_auth_keys` 时）
    ///
    /// V013 新增。内部 CLI 豁免或开放模式下可能为 None。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_secret_id: Option<String>,
}

/// 创建调用记录的输入参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateModelCallLogInput {
    pub provider_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_model_id: Option<String>,
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub route_mode: RouteMode,
    /// 请求入口来源：`cli` / `gateway` / `internal`，默认 `gateway`
    #[serde(default = "default_source")]
    pub source: String,
    /// 请求使用的 Gateway API Key 明文，V013 新增
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_secret_id: Option<String>,
}

fn default_source() -> String {
    "gateway".to_string()
}

/// 更新调用记录完成信息的输入参数
///
/// V004 迁移追加 `time_to_first_token_ms`、`price_per_1m_tokens`，
/// 供响应拦截器在请求完成后补充首字延迟与单价快照。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateModelCallLogInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_hit: Option<bool>,
    /// 流式响应首字延迟（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_to_first_token_ms: Option<i64>,
    /// 记录时单价快照（CNY / 1M tokens）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_per_1m_tokens: Option<f64>,
}

/// 列出调用记录的查询参数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListModelCallLogsInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// 请求发起时间起始过滤（ISO 8601，含）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_at: Option<String>,
    /// 请求发起时间结束过滤（ISO 8601，含）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_at: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    /// 按请求使用的 Gateway API Key 过滤，V013 新增
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_secret_id: Option<String>,
}

fn default_limit() -> i64 {
    100
}

/// 模型调用统计查询输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCallStatsInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// 路由模式过滤：1=直连，2=虚拟故障转移
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_mode: Option<i64>,
    /// 按请求使用的 Gateway API Key 过滤，V013 新增
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_secret_id: Option<String>,
}

/// 模型调用统计输出行
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCallStatsRow {
    pub provider_id: String,
    pub provider_name: String,
    pub model_id: String,
    pub source: String,
    /// 路由模式：1=直连，2=虚拟故障转移
    pub route_mode: i64,
    /// 请求使用的 Gateway API Key 明文，空字符串表示无/未识别，V013 新增
    pub api_key_secret_id: String,
    pub request_count: i64,
    pub success_count: i64,
    pub success_rate: f64,
    pub total_tokens: i64,
    pub cached_tokens: i64,
    pub cache_hit_rate: f64,
    /// 总花费金额（CNY，元）
    pub cost_cny: f64,
    pub cost_ratio: f64,
    /// 每百万 Token 成本（CNY / 1M tokens）
    pub cost_per_1m_tokens: f64,
    pub avg_duration_ms: f64,
    pub avg_time_to_first_token_ms: f64,
    pub avg_tokens_per_second: f64,
    /// 4xx 错误数
    pub error_count_4xx: i64,
    /// 5xx 错误数
    pub error_count_5xx: i64,
}

// ===== 聚合表相关类型 =====

/// 聚合时间粒度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatsGranularity {
    /// 30 秒级（仅实时聚合，不预写入聚合表）
    ThirtySeconds,
    /// 1 分钟级（仅实时聚合，不预写入聚合表）
    OneMinute,
    /// 10 分钟级（仅实时聚合，不预写入聚合表）
    TenMinutes,
    /// 30 分钟级（仅实时聚合，不预写入聚合表）
    ThirtyMinutes,
    /// 小时级（预聚合表）
    Hourly,
    /// 天级（预聚合表）
    Daily,
}

impl Default for StatsGranularity {
    fn default() -> Self {
        Self::Hourly
    }
}

impl StatsGranularity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ThirtySeconds => "thirtySeconds",
            Self::OneMinute => "oneMinute",
            Self::TenMinutes => "tenMinutes",
            Self::ThirtyMinutes => "thirtyMinutes",
            Self::Hourly => "hourly",
            Self::Daily => "daily",
        }
    }

    /// 是否为预聚合表支持的粒度
    pub fn is_pre_aggregated(&self) -> bool {
        matches!(self, Self::Hourly | Self::Daily)
    }
}

/// 聚合统计查询输入（查询聚合表）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedStatsInput {
    /// 时间粒度
    pub granularity: StatsGranularity,
    /// 开始时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_at: Option<String>,
    /// 结束时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_at: Option<String>,
    /// 请求来源过滤
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// 供应商过滤
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    /// 模型过滤
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// 路由模式过滤
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_mode: Option<i64>,
    /// 按请求使用的 Gateway API Key 过滤，V013 新增
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_secret_id: Option<String>,
}

/// 聚合统计输出行（从聚合表读取）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedStatsRow {
    pub provider_id: String,
    pub provider_name: String,
    pub model_id: String,
    pub source: String,
    pub route_mode: i64,
    /// 请求使用的 Gateway API Key 明文，空字符串表示无/未识别，V013 新增
    pub api_key_secret_id: String,
    /// 时间桶（整点/整天对齐的 ISO 8601）
    pub time_bucket: String,
    pub request_count: i64,
    pub success_count: i64,
    pub success_rate: f64,
    pub error_count_4xx: i64,
    pub error_count_5xx: i64,
    pub error_rate_4xx: f64,
    pub error_rate_5xx: f64,
    pub total_tokens: i64,
    pub cached_tokens: i64,
    pub cache_hit_rate: f64,
    /// 总花费金额（CNY，元）
    pub cost_cny: f64,
    pub avg_duration_ms: f64,
    pub avg_time_to_first_token_ms: f64,
    pub avg_tokens_per_second: f64,
}

/// 累加写入聚合表的增量数据
///
/// 每次请求完成时构造此结构，UPSERT 到小时/天聚合表。
#[derive(Debug, Clone)]
pub struct StatsAccumulate {
    pub provider_id: String,
    pub model_id: String,
    pub source: String,
    pub route_mode: i64,
    /// 请求使用的 Gateway API Key 明文，空字符串表示无/未识别，V013 新增
    pub api_key_secret_id: String,
    pub time_bucket: String,
    /// 是否成功（status_code 2xx）
    pub is_success: bool,
    /// 是否 4xx 错误
    pub is_4xx: bool,
    /// 是否 5xx 错误
    pub is_5xx: bool,
    /// total_tokens 增量
    pub total_tokens: i64,
    /// cached_tokens 增量
    pub cached_tokens: i64,
    /// 是否缓存命中
    pub cache_hit: bool,
    /// 费用增量（CNY，元）
    pub cost_cny: f64,
    /// duration_ms 增量
    pub duration_ms: i64,
    /// time_to_first_token_ms 增量
    pub ttft_ms: i64,
    /// output tokens_per_second 增量
    pub output_tps: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_mode_roundtrip() {
        assert_eq!(RouteMode::from_i64(1), Some(RouteMode::Direct));
        assert_eq!(RouteMode::from_i64(2), Some(RouteMode::VirtualFallback));
        assert_eq!(RouteMode::from_i64(99), None);
        assert_eq!(RouteMode::Direct.as_i64(), 1);
        assert_eq!(RouteMode::VirtualFallback.as_i64(), 2);
    }
}
