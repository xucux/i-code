//! # 调用记录模块 Tauri Command 声明
//!
//! 前端通过 `invoke('call_records_*', payload)` 调用这些命令。
//!
//! ## 命令清单
//!
//! - `call_records_list`：列出调用记录
//! - `call_records_get`：获取单条调用记录详情
//! - `gateway_model_call_stats`：查询模型调用统计（明细表实时聚合）
//! - `call_stats_aggregated`：查询聚合统计（预聚合表，高性能）
//! - `call_records_clear_stats`：清空模型调用统计数据（全部或指定时间范围）
//! - `call_records_today_tokens`：获取今日消耗的 total_tokens 总数

use tauri::State;

use crate::error::IcodeResult;

use super::service::CallRecordsHandle;
use super::types::{
    AggregatedStatsInput, AggregatedStatsRow, ListModelCallLogsInput, ModelCallLog,
    ModelCallStatsInput, ModelCallStatsRow,
};

/// 清空统计数据的输入参数
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearStatsInput {
    /// 开始时间（RFC3339），为空则清空全部
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_at: Option<String>,
    /// 结束时间（RFC3339），为空则清空全部
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_at: Option<String>,
}

/// 列出调用记录
///
/// 支持按 provider_id / model_id 过滤，默认返回最近 100 条。
#[tauri::command]
pub async fn call_records_list(
    state: State<'_, CallRecordsHandle>,
    input: Option<ListModelCallLogsInput>,
) -> IcodeResult<Vec<ModelCallLog>> {
    state.service().list_call_logs(input.unwrap_or_default())
}

/// 获取单条调用记录详情
#[tauri::command]
pub async fn call_records_get(
    state: State<'_, CallRecordsHandle>,
    id: String,
) -> IcodeResult<ModelCallLog> {
    state.service().get_call_log(&id)
}

/// 查询模型调用统计（明细表实时聚合）
///
/// V006 增强：支持 model_id、route_mode 过滤，增加 route_mode 分组与错误分布。
#[tauri::command]
pub async fn gateway_model_call_stats(
    input: ModelCallStatsInput,
) -> Result<Vec<ModelCallStatsRow>, String> {
    super::service::aggregate_call_stats(&input).map_err(|e| e.to_string())
}

/// 查询聚合统计（预聚合表，高性能）
///
/// 从 `model_call_stats_hourly` 或 `model_call_stats_daily` 表读取，
/// 无需 GROUP BY 扫描明细表，适合大盘趋势图和长时间跨度查询。
#[tauri::command]
pub async fn call_stats_aggregated(
    input: AggregatedStatsInput,
) -> Result<Vec<AggregatedStatsRow>, String> {
    super::service::query_aggregated_stats(&input).map_err(|e| e.to_string())
}

/// 清空模型调用统计数据
///
/// - 不传 `start_at` / `end_at` 时清空全部数据（明细表 + 两张聚合表）
/// - 传时间范围时仅清空指定范围内的数据
///
/// 返回受影响的行数（三表合计）。
#[tauri::command]
pub async fn call_records_clear_stats(
    input: Option<ClearStatsInput>,
) -> IcodeResult<u64> {
    let scope: Option<(String, String)> = match input {
        Some(ci) if ci.start_at.is_some() && ci.end_at.is_some() => {
            Some((ci.start_at.unwrap(), ci.end_at.unwrap()))
        }
        _ => None,
    };
    let scope_ref = scope.as_ref().map(|(s, e)| (s.as_str(), e.as_str()));
    super::service::clear_call_stats(scope_ref)
}

/// 获取今日消耗的 total_tokens 总数
///
/// 供系统托盘和仪表盘调用。
#[tauri::command]
pub async fn call_records_today_tokens() -> IcodeResult<i64> {
    super::service::get_today_tokens()
}
