//! # 调用记录业务服务层
//!
//! 提供模型调用记录的写入与查询。
//!
//! ## 使用方式
//!
//! 在 `gateway_runtime/upstream.rs` 转发请求前调用 [`start_call`] 创建记录，
//! 请求结束后调用 [`finish_call`] 补充完成信息（耗时、状态码、错误、token 数）。
//!
//! 聚合表（`model_call_stats_hourly` / `model_call_stats_daily`）由调度器
//! 每 10 分钟定时从明细表窗口重算（见 `repository::rebuild_windowed_stats`），
//! 本层写入调用记录时不再实时累加，避免增量计数不一致。
//!
//! V002 新增：`clear_call_stats` 清空统计数据（全部或指定时间范围）；
//! `get_today_tokens` 供系统托盘显示今日 token 消耗。

use std::sync::Arc;

use chrono::Utc;

use crate::error::IcodeResult;

use super::repository;
use super::types::{
    AggregatedStatsInput, AggregatedStatsRow, CreateModelCallLogInput, ListModelCallLogsInput,
    ModelCallLog, ModelCallStatsInput, ModelCallStatsRow, UpdateModelCallLogInput,
};

/// 调用记录 Service 在 Tauri State 中的句柄
///
/// 使用 `Arc` 包装以便在多线程间共享。
#[derive(Clone)]
pub struct CallRecordsHandle {
    inner: Arc<CallRecordsService>,
}

impl CallRecordsHandle {
    /// 创建调用记录句柄
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CallRecordsService::new()),
        }
    }

    /// 获取内部 Service 引用
    pub fn service(&self) -> &CallRecordsService {
        &self.inner
    }
}

impl Default for CallRecordsHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// 调用记录业务逻辑
pub struct CallRecordsService;

impl CallRecordsService {
    /// 创建 Service 实例
    pub fn new() -> Self {
        Self
    }

    /// 开始记录一次调用
    ///
    /// 在向上游发送请求前调用，写入 `provider_id`、`model_id`、`request_id`、
    /// `requested_at`、`route_mode` 等初始字段。
    /// 返回记录 ID，供后续 [`finish_call_with_duration`] 更新。
    pub fn start_call(&self, input: CreateModelCallLogInput) -> IcodeResult<ModelCallLog> {
        repository::insert_call_log(&input)
    }

    /// 完成一次调用记录（含耗时）
    ///
    /// 由上层传入从请求开始到响应完成的耗时（毫秒）。
    // 该方法当前无调用方（网关与媒体生成均使用 *_and_tokens* 系列）；
    // 用 allow 而非 expect：expect(dead_code) 需 Rust 1.81+ 与新版 rust-analyzer，
    // 旧版分析器会将其误报为未知属性导致 IDE 标红。
    #[allow(dead_code)]
    pub fn finish_call_with_duration(
        &self,
        id: &str,
        duration_ms: i64,
        status_code: Option<i64>,
        error_message: Option<String>,
    ) -> IcodeResult<ModelCallLog> {
        self.finish_call_with_duration_and_tokens(
            id,
            duration_ms,
            status_code,
            error_message,
            None,
        )
    }

    /// 完成一次调用记录（含耗时与估算 token 数）
    ///
    /// 当上游响应不含 usage 字段时，`prompt_tokens` 由 tokenizer 估算补充。
    /// 聚合表由调度器定时从明细表窗口重算，本方法只更新明细记录。
    pub fn finish_call_with_duration_and_tokens(
        &self,
        id: &str,
        duration_ms: i64,
        status_code: Option<i64>,
        error_message: Option<String>,
        prompt_tokens: Option<i64>,
    ) -> IcodeResult<ModelCallLog> {
        let completed_at = Utc::now().to_rfc3339();
        repository::update_call_log(
            id,
            &UpdateModelCallLogInput {
                completed_at: Some(completed_at.clone()),
                duration_ms: Some(duration_ms),
                status_code,
                error_message,
                prompt_tokens,
                completion_tokens: None,
                total_tokens: None,
                cached_tokens: None,
                cache_hit: None,
                time_to_first_token_ms: None,
                price_per_1m_tokens: None,
            },
        )
    }

    /// 完成一次调用记录（含完整 usage 数据）
    ///
    /// 从上游响应解析的 usage 字段通过 `UpdateModelCallLogInput` 传入，
    /// 包含 completion_tokens / total_tokens / cached_tokens / price_per_1m_tokens 等。
    /// 聚合表由调度器定时从明细表窗口重算，本方法只更新明细记录。
    pub fn finish_call_with_duration_and_tokens_full(
        &self,
        id: &str,
        duration_ms: i64,
        input: &UpdateModelCallLogInput,
    ) -> IcodeResult<ModelCallLog> {
        // duration_ms 已包含在 input 中；此处保持位置参数签名兼容历史调用方
        let _ = duration_ms;
        repository::update_call_log(id, input)
    }

    /// 列出调用记录
    pub fn list_call_logs(
        &self,
        input: ListModelCallLogsInput,
    ) -> IcodeResult<Vec<ModelCallLog>> {
        repository::list_call_logs(&input)
    }

    /// 获取单条调用记录
    pub fn get_call_log(&self, id: &str) -> IcodeResult<ModelCallLog> {
        repository::find_call_log_by_id(id)
    }
}

/// 查询模型调用统计（明细表实时聚合）
pub fn aggregate_call_stats(input: &ModelCallStatsInput) -> IcodeResult<Vec<ModelCallStatsRow>> {
    let mut rows = repository::aggregate_call_stats(input)?;

    // 计算费用占比
    let total_cost: f64 = rows.iter().map(|r| r.cost_cny).sum();
    if total_cost > 0.0 {
        for row in &mut rows {
            row.cost_ratio = row.cost_cny / total_cost;
        }
    }

    Ok(rows)
}

/// 查询聚合统计（从预聚合表读取，高性能）
pub fn query_aggregated_stats(input: &AggregatedStatsInput) -> IcodeResult<Vec<AggregatedStatsRow>> {
    repository::query_aggregated_stats(input)
}

/// 清空模型调用统计数据
///
/// - `scope` 为 `None` 时清空全部（明细表 + 两张聚合表）
/// - `scope` 为 `Some((start_at, end_at))` 时仅清空指定时间范围的数据
///
/// 返回受影响的行数（三表合计）。
pub fn clear_call_stats(scope: Option<(&str, &str)>) -> IcodeResult<u64> {
    repository::clear_call_stats(scope)
}

/// 获取今日消耗的 total_tokens 总数
///
/// 供系统托盘和仪表盘调用。
pub fn get_today_tokens() -> IcodeResult<i64> {
    repository::get_today_tokens()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_clone() {
        let handle = CallRecordsHandle::new();
        let _cloned = handle.clone();
    }
}
