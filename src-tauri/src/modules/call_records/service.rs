//! # 调用记录业务服务层
//!
//! 提供模型调用记录的写入与查询。
//!
//! ## 使用方式
//!
//! 在 `gateway_runtime/upstream.rs` 转发请求前调用 [`start_call`] 创建记录，
//! 请求结束后调用 [`finish_call`] 补充完成信息（耗时、状态码、错误、token 数）。
//!
//! V006 新增：`finish_call` 完成时同步累加写入 `model_call_stats_hourly` /
//! `model_call_stats_daily` 两张聚合表。
//!
//! V002 新增：`clear_call_stats` 清空统计数据（全部或指定时间范围）；
//! `get_today_tokens` 供系统托盘显示今日 token 消耗。

use std::sync::Arc;

use chrono::{Timelike, Utc};

use crate::error::IcodeResult;

use super::repository;
use super::types::{
    AggregatedStatsInput, AggregatedStatsRow, CreateModelCallLogInput, ListModelCallLogsInput,
    ModelCallLog, ModelCallStatsInput, ModelCallStatsRow, StatsAccumulate,
    UpdateModelCallLogInput,
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
    /// 完成后同步累加写入 `model_call_stats_hourly` / `model_call_stats_daily` 聚合表。
    pub fn finish_call_with_duration_and_tokens(
        &self,
        id: &str,
        duration_ms: i64,
        status_code: Option<i64>,
        error_message: Option<String>,
        prompt_tokens: Option<i64>,
    ) -> IcodeResult<ModelCallLog> {
        let completed_at = Utc::now().to_rfc3339();
        let log = repository::update_call_log(
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
        )?;

        // 同步累加写入聚合表（失败不影响主流程）
        let _ = self.accumulate_stats_from_log(&log, status_code, duration_ms, prompt_tokens);

        Ok(log)
    }

    /// 完成一次调用记录（含完整 usage 数据）
    ///
    /// 从上游响应解析的 usage 字段通过 `UpdateModelCallLogInput` 传入，
    /// 包含 completion_tokens / total_tokens / cached_tokens / price_per_1m_tokens 等。
    /// 完成后同步累加写入聚合表。
    pub fn finish_call_with_duration_and_tokens_full(
        &self,
        id: &str,
        duration_ms: i64,
        input: &UpdateModelCallLogInput,
    ) -> IcodeResult<ModelCallLog> {
        let log = repository::update_call_log(id, input)?;

        // 同步累加写入聚合表（失败不影响主流程）
        let _ = self.accumulate_stats_from_log(
            &log,
            input.status_code,
            duration_ms,
            input.prompt_tokens,
        );

        Ok(log)
    }

    /// 根据完成的调用记录构造增量数据并累加写入两张聚合表
    ///
    /// 按 `requested_at` 计算所属小时桶与天桶，UPSERT 累加。
    fn accumulate_stats_from_log(
        &self,
        log: &ModelCallLog,
        status_code: Option<i64>,
        duration_ms: i64,
        prompt_tokens: Option<i64>,
    ) -> IcodeResult<()> {
        // 解析 requested_at 为 chrono DateTime 用于计算时间桶
        let requested_at = chrono::DateTime::parse_from_rfc3339(&log.requested_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .ok();

        // 计算时间桶
        let (hour_bucket, day_bucket) = if let Some(dt) = requested_at {
            let hour = dt.with_minute(0).unwrap_or(dt).with_second(0).unwrap_or(dt)
                .with_nanosecond(0).unwrap_or(dt);
            let day = dt.with_hour(0).unwrap_or(dt).with_minute(0).unwrap_or(dt)
                .with_second(0).unwrap_or(dt).with_nanosecond(0).unwrap_or(dt);
            (hour.to_rfc3339(), day.to_rfc3339())
        } else {
            // 解析失败时用当前时间
            let now = chrono::Utc::now();
            let hour = now.with_minute(0).unwrap_or(now).with_second(0).unwrap_or(now)
                .with_nanosecond(0).unwrap_or(now);
            let day = now.with_hour(0).unwrap_or(now).with_minute(0).unwrap_or(now)
                .with_second(0).unwrap_or(now).with_nanosecond(0).unwrap_or(now);
            (hour.to_rfc3339(), day.to_rfc3339())
        };

        // 构造增量数据
        let sc = status_code.unwrap_or(0);
        let is_success = sc >= 200 && sc <= 299;
        let is_4xx = sc >= 400 && sc <= 499;
        let is_5xx = sc >= 500;

        // 使用日志记录中的实际 total_tokens（来自上游 usage 或 prompt+completion 估算）
        let total_tokens = log.total_tokens.unwrap_or_else(|| {
            // 回退：从 prompt_tokens + completion_tokens 估算
            let pt = prompt_tokens.unwrap_or(log.prompt_tokens.unwrap_or(0));
            let ct = log.completion_tokens.unwrap_or(0);
            pt + ct
        });

        // 估算费用：total_tokens × price_per_1m_tokens / 1M
        // model_configs.price_per_1m_tokens 单位为元 / 1M tokens，因此结果单位为 CNY。
        let price = log.price_per_1m_tokens.unwrap_or(0.0);
        let cost_cny = if total_tokens > 0 && price > 0.0 {
            total_tokens as f64 * price / 1_000_000.0
        } else {
            0.0
        };

        // 输出速率估算
        let output_tps = if let Some(ct) = log.completion_tokens {
            if ct > 0 && duration_ms > 0 {
                ct as f64 * 1000.0 / duration_ms as f64
            } else {
                0.0
            }
        } else {
            0.0
        };

        let ttft_ms = log.time_to_first_token_ms.unwrap_or(0);

        // 小时级累加
        let acc_hourly = StatsAccumulate {
            provider_id: log.provider_id.clone(),
            model_id: log.model_id.clone(),
            source: log.source.clone(),
            route_mode: log.route_mode,
            api_key_secret_id: log.api_key_secret_id.clone().unwrap_or_default(),
            time_bucket: hour_bucket,
            is_success,
            is_4xx,
            is_5xx,
            total_tokens,
            cached_tokens: log.cached_tokens.unwrap_or(0),
            cache_hit: log.cache_hit,
            cost_cny,
            duration_ms,
            ttft_ms,
            output_tps,
        };

        // 天级累加（同维度，不同 time_bucket）
        let acc_daily = StatsAccumulate {
            time_bucket: day_bucket,
            ..acc_hourly.clone()
        };

        // 写入两张聚合表（任一失败不影响另一张）
        let _ = repository::accumulate_stats_hourly(&acc_hourly);
        let _ = repository::accumulate_stats_daily(&acc_daily);

        Ok(())
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
