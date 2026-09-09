//! # 调用记录数据访问层
//!
//! 直接操作 `model_call_logs` 表，以及 V006 新增的
//! `model_call_stats_hourly` / `model_call_stats_daily` 聚合表。
//!
//! ## 聚合表写入策略
//!
//! 采用**定时窗口重算**：调度器（`modules/scheduler`）每 10 分钟调用
//! [`rebuild_windowed_stats`]，从 `model_call_logs` 明细表重新计算
//! 「当前时刻 − 10 分钟」所在小时桶与天桶，覆盖式写回聚合表
//! （DELETE + INSERT SELECT，天然幂等，不存在累加重复计数问题）。

use chrono::{Duration, Timelike, Utc};

use crate::core::id::generate_id;
use crate::db::{get_db_pool, DbConn};
use crate::error::{IcodeError, IcodeResult};

use super::types::{
    AggregatedStatsInput, AggregatedStatsRow, CreateModelCallLogInput, ListModelCallLogsInput,
    ModelCallLog, ModelCallStatsInput, ModelCallStatsRow, UpdateModelCallLogInput,
};

/// 聚合表窗口重算周期（秒）：每次重算「当前时刻 − 该周期」所在的小时/天桶。
/// 留出该延迟窗口是为了让流式请求的真实 usage 全部落库后再纳入聚合，避免缺计。
pub const AGG_STATS_REBUILD_INTERVAL_SECS: u64 = 600;

fn get_conn() -> IcodeResult<DbConn> {
    Ok(get_db_pool()?.get()?)
}

fn new_id() -> String {
    generate_id()
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

/// 插入一条调用记录（请求开始时）
///
/// 仅填充基础字段，完成信息由 [`update_call_log`] 在请求结束后补充。
pub fn insert_call_log(input: &CreateModelCallLogInput) -> IcodeResult<ModelCallLog> {
    let conn = get_conn()?;
    let id = new_id();
    let now = now();
    let cache_hit_i: i64 = 0;
    let route_mode_i = input.route_mode.as_i64();

    conn.execute(
        "INSERT INTO model_call_logs
            (id, provider_id, gateway_model_id, model_id, request_id, requested_at,
             cache_hit, route_mode, source, api_key_secret_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            id,
            input.provider_id,
            input.gateway_model_id,
            input.model_id,
            input.request_id,
            now,
            cache_hit_i,
            route_mode_i,
            input.source,
            input.api_key_secret_id,
        ],
    )?;

    find_call_log_by_id(&id)
}

/// 根据 ID 查询调用记录
pub fn find_call_log_by_id(id: &str) -> IcodeResult<ModelCallLog> {
    let conn = get_conn()?;
    // CALL_LOG_SELECT_SQL 不含 WHERE 条件，需拼接 id 过滤，避免参数数量不匹配
    let sql = format!("{} WHERE id = ?1", CALL_LOG_SELECT_SQL);
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], call_log_row_mapper)?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Err(IcodeError::not_found("ModelCallLog", Some(id))),
    }
}

/// 更新调用记录完成信息
pub fn update_call_log(id: &str, input: &UpdateModelCallLogInput) -> IcodeResult<ModelCallLog> {
    let conn = get_conn()?;

    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut idx = 1usize;

    if let Some(v) = &input.completed_at {
        sets.push(format!("completed_at = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.duration_ms {
        sets.push(format!("duration_ms = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.status_code {
        sets.push(format!("status_code = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = &input.error_message {
        sets.push(format!("error_message = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.prompt_tokens {
        sets.push(format!("prompt_tokens = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.completion_tokens {
        sets.push(format!("completion_tokens = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.total_tokens {
        sets.push(format!("total_tokens = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.cached_tokens {
        sets.push(format!("cached_tokens = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.cache_hit {
        sets.push(format!("cache_hit = ?{idx}"));
        params.push(Box::new(v as i64));
        idx += 1;
    }
    if let Some(v) = input.time_to_first_token_ms {
        sets.push(format!("time_to_first_token_ms = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.price_per_1m_tokens {
        sets.push(format!("price_per_1m_tokens = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }

    if sets.is_empty() {
        return find_call_log_by_id(id);
    }

    let sql = format!(
        "UPDATE model_call_logs SET {} WHERE id = ?{}",
        sets.join(", "),
        idx
    );
    params.push(Box::new(id.to_string()));

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, params_ref.as_slice())?;
    if affected == 0 {
        return Err(IcodeError::not_found("ModelCallLog", Some(id)));
    }

    find_call_log_by_id(id)
}

/// 列出调用记录
///
/// 支持按 provider_id / model_id 过滤，按 requested_at 降序排列。
pub fn list_call_logs(input: &ListModelCallLogsInput) -> IcodeResult<Vec<ModelCallLog>> {
    let conn = get_conn()?;

    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut idx = 1usize;

    if let Some(provider_id) = &input.provider_id {
        conditions.push(format!("provider_id = ?{idx}"));
        params.push(Box::new(provider_id.clone()));
        idx += 1;
    }
    if let Some(model_id) = &input.model_id {
        conditions.push(format!("model_id = ?{idx}"));
        params.push(Box::new(model_id.clone()));
        idx += 1;
    }
    if let Some(api_key_secret_id) = &input.api_key_secret_id {
        conditions.push(format!("api_key_secret_id = ?{idx}"));
        params.push(Box::new(api_key_secret_id.clone()));
        idx += 1;
    }
    if let Some(start_at) = &input.start_at {
        conditions.push(format!("requested_at >= ?{idx}"));
        params.push(Box::new(start_at.clone()));
        idx += 1;
    }
    if let Some(end_at) = &input.end_at {
        conditions.push(format!("requested_at <= ?{idx}"));
        params.push(Box::new(end_at.clone()));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "{} {} ORDER BY requested_at DESC LIMIT ?{} OFFSET ?{}",
        CALL_LOG_SELECT_SQL,
        where_clause,
        params.len() + 1,
        params.len() + 2
    );
    params.push(Box::new(input.limit));
    params.push(Box::new(input.offset));

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_ref.as_slice(), call_log_row_mapper)?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 按供应商 + 模型 + 入口 + 路由模式聚合调用统计
///
/// V006 增强：增加 route_mode 分组、model_id 过滤、错误分布计数。
pub fn aggregate_call_stats(input: &ModelCallStatsInput) -> IcodeResult<Vec<ModelCallStatsRow>> {
    let conn = get_conn()?;

    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut idx = 1usize;

    // 默认统计最近 24 小时
    let default_end = Utc::now().to_rfc3339();
    let default_start = (Utc::now() - Duration::hours(24)).to_rfc3339();
    conditions.push(format!("m.requested_at >= ?{idx}"));
    params.push(Box::new(input.start_at.clone().unwrap_or(default_start)));
    idx += 1;
    conditions.push(format!("m.requested_at <= ?{idx}"));
    params.push(Box::new(input.end_at.clone().unwrap_or(default_end)));
    idx += 1;

    if let Some(source) = &input.source {
        conditions.push(format!("m.source = ?{idx}"));
        params.push(Box::new(source.clone()));
        idx += 1;
    }

    if let Some(provider_id) = &input.provider_id {
        conditions.push(format!("m.provider_id = ?{idx}"));
        params.push(Box::new(provider_id.clone()));
        idx += 1;
    }

    if let Some(model_id) = &input.model_id {
        conditions.push(format!("m.model_id = ?{idx}"));
        params.push(Box::new(model_id.clone()));
        idx += 1;
    }

    if let Some(route_mode) = input.route_mode {
        conditions.push(format!("m.route_mode = ?{idx}"));
        params.push(Box::new(route_mode));
        idx += 1;
    }
    if let Some(api_key_secret_id) = &input.api_key_secret_id {
        conditions.push(format!("m.api_key_secret_id = ?{idx}"));
        params.push(Box::new(api_key_secret_id.clone()));
    }

    let where_clause = conditions.join(" AND ");

    let sql = format!(
        "SELECT
            m.provider_id,
            p.display_name AS provider_name,
            m.model_id,
            m.source,
            m.route_mode,
            COALESCE(m.api_key_secret_id, '') AS api_key_secret_id,
            COUNT(*) AS request_count,
            SUM(CASE WHEN m.status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END) AS success_count,
            COALESCE(SUM(m.total_tokens), 0) AS total_tokens,
            COALESCE(SUM(m.cached_tokens), 0) AS cached_tokens,
            SUM(CASE WHEN m.cache_hit THEN 1 ELSE 0 END) AS cache_hit_count,
            COALESCE(SUM(COALESCE(m.total_tokens, 0) * COALESCE(m.price_per_1m_tokens, 0.0) / 1000000.0), 0.0) AS cost_cny,
            COALESCE(AVG(m.duration_ms), 0.0) AS avg_duration_ms,
            COALESCE(AVG(m.time_to_first_token_ms), 0.0) AS avg_time_to_first_token_ms,
            COALESCE(AVG(CASE
                WHEN m.completion_tokens > 0 AND m.duration_ms > COALESCE(m.time_to_first_token_ms, 0)
                THEN CAST(m.completion_tokens AS REAL) * 1000.0 / (m.duration_ms - COALESCE(m.time_to_first_token_ms, 0))
                ELSE NULL
            END), 0.0) AS avg_tokens_per_second,
            SUM(CASE WHEN m.status_code BETWEEN 400 AND 499 THEN 1 ELSE 0 END) AS error_count_4xx,
            SUM(CASE WHEN m.status_code >= 500 THEN 1 ELSE 0 END) AS error_count_5xx
        FROM model_call_logs m
        JOIN providers p ON p.id = m.provider_id
        WHERE {}
        GROUP BY m.provider_id, m.model_id, m.source, m.route_mode, m.api_key_secret_id
        ORDER BY request_count DESC",
        where_clause
    );

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_ref.as_slice(), |row| {
        let request_count: i64 = row.get(6)?;
        let success_count: i64 = row.get(7)?;
        let total_tokens: i64 = row.get(8)?;
        let cached_tokens: i64 = row.get(9)?;
        let cache_hit_count: i64 = row.get(10)?;
        let cost_cny: f64 = row.get(11)?;

        let success_rate = if request_count > 0 {
            success_count as f64 * 100.0 / request_count as f64
        } else { 0.0 };
        let cache_hit_rate = if request_count > 0 {
            cache_hit_count as f64 * 100.0 / request_count as f64
        } else { 0.0 };
        let cost_per_1m_tokens = if total_tokens > 0 {
            cost_cny * 1_000_000.0 / total_tokens as f64
        } else { 0.0 };

        Ok(ModelCallStatsRow {
            provider_id: row.get(0)?,
            provider_name: row.get(1)?,
            model_id: row.get(2)?,
            source: row.get(3)?,
            route_mode: row.get(4)?,
            api_key_secret_id: row.get(5)?,
            request_count,
            success_count,
            success_rate,
            total_tokens,
            cached_tokens,
            cache_hit_rate,
            cost_cny,
            cost_ratio: 0.0, // 由 service 层二次计算
            cost_per_1m_tokens,
            avg_duration_ms: row.get(12)?,
            avg_time_to_first_token_ms: row.get(13)?,
            avg_tokens_per_second: row.get(14)?,
            error_count_4xx: row.get(15)?,
            error_count_5xx: row.get(16)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

const CALL_LOG_SELECT_SQL: &str = "SELECT id, provider_id, gateway_model_id, model_id, request_id,
    requested_at, completed_at, duration_ms, status_code, error_message,
    prompt_tokens, completion_tokens, total_tokens, cached_tokens, cache_hit, route_mode,
    source, time_to_first_token_ms, price_per_1m_tokens, api_key_secret_id
    FROM model_call_logs";

fn call_log_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<ModelCallLog> {
    let cache_hit: i64 = row.get(14)?;
    Ok(ModelCallLog {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        gateway_model_id: row.get(2)?,
        model_id: row.get(3)?,
        request_id: row.get(4)?,
        requested_at: row.get(5)?,
        completed_at: row.get(6)?,
        duration_ms: row.get(7)?,
        status_code: row.get(8)?,
        error_message: row.get(9)?,
        prompt_tokens: row.get(10)?,
        completion_tokens: row.get(11)?,
        total_tokens: row.get(12)?,
        cached_tokens: row.get(13)?,
        cache_hit: cache_hit != 0,
        route_mode: row.get(15)?,
        source: row.get(16)?,
        time_to_first_token_ms: row.get(17)?,
        price_per_1m_tokens: row.get(18)?,
        api_key_secret_id: row.get(19)?,
    })
}

// ===== 聚合表：定时窗口重算 =====

/// 小时级聚合表桶重算 SQL（覆盖式）
///
/// 事务内先删除目标小时的旧聚合行，再从明细表按同一小时桶重新聚合插入：
/// - `?1`：小时桶（形如 `2026-09-05T03:00:00+00:00`）
/// - 聚合口径与「明细」视图（`aggregate_call_stats`）完全一致，均以
///   `COALESCE(total_tokens, 0)` 等口径从 `model_call_logs` 派生
const DELETE_HOURLY_BUCKET_SQL: &str =
    "DELETE FROM model_call_stats_hourly WHERE time_bucket = ?1";
const INSERT_HOURLY_BUCKET_SQL: &str = "
INSERT INTO model_call_stats_hourly (
  id, provider_id, model_id, source, route_mode, api_key_secret_id, time_bucket,
  request_count, success_count, error_count_4xx, error_count_5xx,
  total_tokens, cached_tokens, cache_hit_count,
  cost_usd, sum_duration_ms, sum_ttft_ms, sum_output_tps,
  created_at, updated_at
)
SELECT
  COALESCE(api_key_secret_id, '') || '|' || provider_id || '|' || model_id || '|' || source
    || '|' || route_mode || '|' || strftime('%Y-%m-%dT%H:00:00', requested_at) AS id,
  provider_id,
  model_id,
  source,
  route_mode,
  COALESCE(api_key_secret_id, ''),
  strftime('%Y-%m-%dT%H:00:00', requested_at) || '+00:00' AS time_bucket,
  COUNT(*) AS request_count,
  SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END) AS success_count,
  SUM(CASE WHEN status_code BETWEEN 400 AND 499 THEN 1 ELSE 0 END) AS error_count_4xx,
  SUM(CASE WHEN status_code >= 500 THEN 1 ELSE 0 END) AS error_count_5xx,
  COALESCE(SUM(COALESCE(total_tokens, 0)), 0) AS total_tokens,
  COALESCE(SUM(COALESCE(cached_tokens, 0)), 0) AS cached_tokens,
  COALESCE(SUM(cache_hit), 0) AS cache_hit_count,
  COALESCE(SUM(COALESCE(total_tokens, 0) * COALESCE(price_per_1m_tokens, 0.0) / 1000000.0), 0.0) AS cost_usd,
  COALESCE(SUM(COALESCE(duration_ms, 0)), 0) AS sum_duration_ms,
  COALESCE(SUM(COALESCE(time_to_first_token_ms, 0)), 0) AS sum_ttft_ms,
  COALESCE(SUM(CASE
    WHEN completion_tokens > 0 AND duration_ms > 0
    THEN CAST(completion_tokens AS REAL) * 1000.0 / duration_ms
    ELSE 0.0
  END), 0.0) AS sum_output_tps,
  strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
  strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
FROM model_call_logs
WHERE strftime('%Y-%m-%dT%H:00:00', requested_at) || '+00:00' = ?1
GROUP BY provider_id, model_id, source, route_mode, COALESCE(api_key_secret_id, ''),
  strftime('%Y-%m-%dT%H:00:00', requested_at);
";

/// 天级聚合表桶重算 SQL（覆盖式），见小时级说明
const DELETE_DAILY_BUCKET_SQL: &str =
    "DELETE FROM model_call_stats_daily WHERE time_bucket = ?1";
const INSERT_DAILY_BUCKET_SQL: &str = "
INSERT INTO model_call_stats_daily (
  id, provider_id, model_id, source, route_mode, api_key_secret_id, time_bucket,
  request_count, success_count, error_count_4xx, error_count_5xx,
  total_tokens, cached_tokens, cache_hit_count,
  cost_usd, sum_duration_ms, sum_ttft_ms, sum_output_tps,
  created_at, updated_at
)
SELECT
  COALESCE(api_key_secret_id, '') || '|' || provider_id || '|' || model_id || '|' || source
    || '|' || route_mode || '|' || strftime('%Y-%m-%dT%H:%M:%S', requested_at, 'start of day') AS id,
  provider_id,
  model_id,
  source,
  route_mode,
  COALESCE(api_key_secret_id, ''),
  strftime('%Y-%m-%dT%H:%M:%S', requested_at, 'start of day') || '+00:00' AS time_bucket,
  COUNT(*) AS request_count,
  SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END) AS success_count,
  SUM(CASE WHEN status_code BETWEEN 400 AND 499 THEN 1 ELSE 0 END) AS error_count_4xx,
  SUM(CASE WHEN status_code >= 500 THEN 1 ELSE 0 END) AS error_count_5xx,
  COALESCE(SUM(COALESCE(total_tokens, 0)), 0) AS total_tokens,
  COALESCE(SUM(COALESCE(cached_tokens, 0)), 0) AS cached_tokens,
  COALESCE(SUM(cache_hit), 0) AS cache_hit_count,
  COALESCE(SUM(COALESCE(total_tokens, 0) * COALESCE(price_per_1m_tokens, 0.0) / 1000000.0), 0.0) AS cost_usd,
  COALESCE(SUM(COALESCE(duration_ms, 0)), 0) AS sum_duration_ms,
  COALESCE(SUM(COALESCE(time_to_first_token_ms, 0)), 0) AS sum_ttft_ms,
  COALESCE(SUM(CASE
    WHEN completion_tokens > 0 AND duration_ms > 0
    THEN CAST(completion_tokens AS REAL) * 1000.0 / duration_ms
    ELSE 0.0
  END), 0.0) AS sum_output_tps,
  strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
  strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
FROM model_call_logs
WHERE strftime('%Y-%m-%dT%H:%M:%S', requested_at, 'start of day') || '+00:00' = ?1
GROUP BY provider_id, model_id, source, route_mode, COALESCE(api_key_secret_id, ''),
  strftime('%Y-%m-%dT%H:%M:%S', requested_at, 'start of day');
";

/// 定时窗口重算聚合表
///
/// 由调度器每 [`AGG_STATS_REBUILD_INTERVAL_SECS`] 秒调用一次：
/// 取「当前时刻 − 重算周期」所在的小时桶与天桶（留出延迟窗口让流式请求的
/// usage 全部落库），对两个桶各执行一次覆盖式重算（DELETE + INSERT SELECT）。
/// 覆盖式写入天然幂等，聚合值 100% 由明细派生，不存在累加重复计数。
/// 整个重算在单个事务中完成，查询侧始终看到完整数据。
pub fn rebuild_windowed_stats() -> IcodeResult<()> {
    let mut conn = get_conn()?;

    // 目标时刻 = 当前时刻 − 重算周期，其所属时/天桶即本次重算对象
    let target = Utc::now() - Duration::seconds(AGG_STATS_REBUILD_INTERVAL_SECS as i64);
    let hour_bucket = target
        .with_minute(0)
        .and_then(|dt| dt.with_second(0))
        .and_then(|dt| dt.with_nanosecond(0))
        .unwrap_or(target)
        .to_rfc3339();
    let day_bucket = target
        .with_hour(0)
        .and_then(|dt| dt.with_minute(0))
        .and_then(|dt| dt.with_second(0))
        .and_then(|dt| dt.with_nanosecond(0))
        .unwrap_or(target)
        .to_rfc3339();

    let tx = conn.transaction()?;
    tx.execute(DELETE_HOURLY_BUCKET_SQL, rusqlite::params![&hour_bucket])
        .map_err(|e| IcodeError::database(format!("删除小时聚合桶失败：{e}")))?;
    tx.execute(INSERT_HOURLY_BUCKET_SQL, rusqlite::params![&hour_bucket])
        .map_err(|e| IcodeError::database(format!("重算小时聚合桶失败：{e}")))?;
    tx.execute(DELETE_DAILY_BUCKET_SQL, rusqlite::params![&day_bucket])
        .map_err(|e| IcodeError::database(format!("删除天聚合桶失败：{e}")))?;
    tx.execute(INSERT_DAILY_BUCKET_SQL, rusqlite::params![&day_bucket])
        .map_err(|e| IcodeError::database(format!("重算天聚合桶失败：{e}")))?;
    tx.commit()?;

    Ok(())
}

// ===== 聚合表：查询 =====

/// 查询聚合统计
///
/// 根据时间粒度选择数据源：
/// - `hourly` / `daily`：从预聚合表读取。
/// - `thirtySeconds` / `oneMinute` / `twoMinutes` / `fiveMinutes` / `tenMinutes` / `thirtyMinutes`：
///   从 `model_call_logs` 明细表实时 GROUP BY 聚合，用于图表展示不同时间窗口的细粒度趋势。
pub fn query_aggregated_stats(input: &AggregatedStatsInput) -> IcodeResult<Vec<AggregatedStatsRow>> {
    let conn = get_conn()?;

    let mut result = if input.granularity.is_pre_aggregated() {
        query_pre_aggregated_stats(&conn, input)?
    } else {
        query_realtime_aggregated_stats(&conn, input)?
    };

    // 批量补全供应商显示名称：聚合结果只存 provider_id，需用 IN 查询反查 providers 表
    if !result.is_empty() {
        let provider_ids: Vec<String> = result
            .iter()
            .map(|r| r.provider_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        if !provider_ids.is_empty() {
            let placeholders = provider_ids
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 1))
                .collect::<Vec<_>>()
                .join(", ");
            let name_sql = format!(
                "SELECT id, display_name FROM providers WHERE id IN ({})",
                placeholders
            );
            let name_params: Vec<&dyn rusqlite::ToSql> =
                provider_ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
            let mut name_stmt = conn.prepare(&name_sql)?;
            let name_map: std::collections::HashMap<String, String> = name_stmt
                .query_map(name_params.as_slice(), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();
            for row in &mut result {
                if let Some(name) = name_map.get(&row.provider_id) {
                    row.provider_name = name.clone();
                }
            }
        }
    }

    Ok(result)
}

/// 从预聚合表查询聚合统计（hourly / daily）
fn query_pre_aggregated_stats(
    conn: &DbConn,
    input: &AggregatedStatsInput,
) -> IcodeResult<Vec<AggregatedStatsRow>> {
    let table = match input.granularity {
        super::types::StatsGranularity::Hourly => "model_call_stats_hourly",
        super::types::StatsGranularity::Daily => "model_call_stats_daily",
        _ => unreachable!("pre-aggregated granularity expected"),
    };

    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut idx = 1usize;

    // 默认最近 24 小时
    let default_end = Utc::now().to_rfc3339();
    let default_start = (Utc::now() - Duration::hours(24)).to_rfc3339();
    conditions.push(format!("time_bucket >= ?{idx}"));
    params.push(Box::new(input.start_at.clone().unwrap_or(default_start)));
    idx += 1;
    conditions.push(format!("time_bucket <= ?{idx}"));
    params.push(Box::new(input.end_at.clone().unwrap_or(default_end)));
    idx += 1;

    if let Some(source) = &input.source {
        conditions.push(format!("source = ?{idx}"));
        params.push(Box::new(source.clone()));
        idx += 1;
    }
    if let Some(provider_id) = &input.provider_id {
        conditions.push(format!("provider_id = ?{idx}"));
        params.push(Box::new(provider_id.clone()));
        idx += 1;
    }
    if let Some(model_id) = &input.model_id {
        conditions.push(format!("model_id = ?{idx}"));
        params.push(Box::new(model_id.clone()));
        idx += 1;
    }
    if let Some(route_mode) = input.route_mode {
        conditions.push(format!("route_mode = ?{idx}"));
        params.push(Box::new(route_mode));
        idx += 1;
    }
    if let Some(api_key_secret_id) = &input.api_key_secret_id {
        conditions.push(format!("api_key_secret_id = ?{idx}"));
        params.push(Box::new(api_key_secret_id.clone()));
    }

    let where_clause = conditions.join(" AND ");

    let sql = format!(
        "SELECT
            provider_id, model_id, source, route_mode, api_key_secret_id, time_bucket,
            request_count, success_count,
            error_count_4xx, error_count_5xx,
            total_tokens, cached_tokens, cache_hit_count,
            cost_usd, sum_duration_ms, sum_ttft_ms, sum_output_tps
         FROM {table}
         WHERE {where_clause}
         ORDER BY time_bucket ASC, request_count DESC"
    );

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_ref.as_slice(), map_aggregated_row)?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 从明细表实时聚合（thirtySeconds / oneMinute / twoMinutes / fiveMinutes / tenMinutes / thirtyMinutes）
///
/// 通过 `strftime('%s', requested_at)` 计算 UNIX 时间戳并对桶宽取整，
/// 在查询时动态生成时间桶，避免为细粒度单独维护预聚合表。
fn query_realtime_aggregated_stats(
    conn: &DbConn,
    input: &AggregatedStatsInput,
) -> IcodeResult<Vec<AggregatedStatsRow>> {
    let bucket_secs: i64 = match input.granularity {
        super::types::StatsGranularity::ThirtySeconds => 30,
        super::types::StatsGranularity::OneMinute => 60,
        super::types::StatsGranularity::TwoMinutes => 120,
        super::types::StatsGranularity::FiveMinutes => 300,
        super::types::StatsGranularity::TenMinutes => 600,
        super::types::StatsGranularity::ThirtyMinutes => 1800,
        _ => unreachable!("realtime granularity expected"),
    };

    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut idx = 1usize;

    // 默认最近 24 小时；按 requested_at 过滤明细记录
    let default_end = Utc::now().to_rfc3339();
    let default_start = (Utc::now() - Duration::hours(24)).to_rfc3339();
    conditions.push(format!("m.requested_at >= ?{idx}"));
    params.push(Box::new(input.start_at.clone().unwrap_or(default_start)));
    idx += 1;
    conditions.push(format!("m.requested_at <= ?{idx}"));
    params.push(Box::new(input.end_at.clone().unwrap_or(default_end)));
    idx += 1;

    if let Some(source) = &input.source {
        conditions.push(format!("m.source = ?{idx}"));
        params.push(Box::new(source.clone()));
        idx += 1;
    }
    if let Some(provider_id) = &input.provider_id {
        conditions.push(format!("m.provider_id = ?{idx}"));
        params.push(Box::new(provider_id.clone()));
        idx += 1;
    }
    if let Some(model_id) = &input.model_id {
        conditions.push(format!("m.model_id = ?{idx}"));
        params.push(Box::new(model_id.clone()));
        idx += 1;
    }
    if let Some(route_mode) = input.route_mode {
        conditions.push(format!("m.route_mode = ?{idx}"));
        params.push(Box::new(route_mode));
        idx += 1;
    }
    if let Some(api_key_secret_id) = &input.api_key_secret_id {
        conditions.push(format!("m.api_key_secret_id = ?{idx}"));
        params.push(Box::new(api_key_secret_id.clone()));
    }

    let where_clause = conditions.join(" AND ");

    let sql = format!(
        "SELECT
            m.provider_id,
            m.model_id,
            m.source,
            m.route_mode,
            COALESCE(m.api_key_secret_id, '') AS api_key_secret_id,
            strftime('%Y-%m-%dT%H:%M:%S', datetime((strftime('%s', m.requested_at) / {bucket_secs}) * {bucket_secs}, 'unixepoch')) || '+00:00' AS time_bucket,
            COUNT(*) AS request_count,
            SUM(CASE WHEN m.status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END) AS success_count,
            SUM(CASE WHEN m.status_code BETWEEN 400 AND 499 THEN 1 ELSE 0 END) AS error_count_4xx,
            SUM(CASE WHEN m.status_code >= 500 THEN 1 ELSE 0 END) AS error_count_5xx,
            COALESCE(SUM(m.total_tokens), 0) AS total_tokens,
            COALESCE(SUM(m.cached_tokens), 0) AS cached_tokens,
            SUM(CASE WHEN m.cache_hit THEN 1 ELSE 0 END) AS cache_hit_count,
            COALESCE(SUM(COALESCE(m.total_tokens, 0) * COALESCE(m.price_per_1m_tokens, 0.0) / 1000000.0), 0.0) AS cost_usd,
            COALESCE(SUM(m.duration_ms), 0) AS sum_duration_ms,
            COALESCE(SUM(m.time_to_first_token_ms), 0) AS sum_ttft_ms,
            COALESCE(SUM(CASE
                WHEN m.completion_tokens > 0 AND m.duration_ms > COALESCE(m.time_to_first_token_ms, 0)
                THEN CAST(m.completion_tokens AS REAL) * 1000.0 / (m.duration_ms - COALESCE(m.time_to_first_token_ms, 0))
                ELSE NULL
            END), 0.0) AS sum_output_tps
         FROM model_call_logs m
         WHERE {where_clause}
         GROUP BY m.provider_id, m.model_id, m.source, m.route_mode, m.api_key_secret_id, time_bucket
         ORDER BY time_bucket ASC, request_count DESC"
    );

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_ref.as_slice(), map_aggregated_row)?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 将聚合查询行映射为 `AggregatedStatsRow`
///
/// 列顺序与预聚合表、实时聚合查询保持一致。
fn map_aggregated_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AggregatedStatsRow> {
    let request_count: i64 = row.get(6)?;
    let success_count: i64 = row.get(7)?;
    let error_count_4xx: i64 = row.get(8)?;
    let error_count_5xx: i64 = row.get(9)?;
    let total_tokens: i64 = row.get(10)?;
    let cached_tokens: i64 = row.get(11)?;
    let cache_hit_count: i64 = row.get(12)?;
    let cost_cny: f64 = row.get(13)?;
    let sum_duration_ms: i64 = row.get(14)?;
    let sum_ttft_ms: i64 = row.get(15)?;
    let sum_output_tps: f64 = row.get(16)?;

    let success_rate = if request_count > 0 {
        success_count as f64 * 100.0 / request_count as f64
    } else { 0.0 };
    let error_rate_4xx = if request_count > 0 {
        error_count_4xx as f64 * 100.0 / request_count as f64
    } else { 0.0 };
    let error_rate_5xx = if request_count > 0 {
        error_count_5xx as f64 * 100.0 / request_count as f64
    } else { 0.0 };
    let cache_hit_rate = if request_count > 0 {
        cache_hit_count as f64 * 100.0 / request_count as f64
    } else { 0.0 };
    let avg_duration_ms = if request_count > 0 {
        sum_duration_ms as f64 / request_count as f64
    } else { 0.0 };
    let avg_time_to_first_token_ms = if request_count > 0 {
        sum_ttft_ms as f64 / request_count as f64
    } else { 0.0 };
    let avg_tokens_per_second = if request_count > 0 {
        sum_output_tps / request_count as f64
    } else { 0.0 };

    Ok(AggregatedStatsRow {
        provider_id: row.get(0)?,
        provider_name: String::new(),
        model_id: row.get(1)?,
        source: row.get(2)?,
        route_mode: row.get(3)?,
        api_key_secret_id: row.get(4)?,
        time_bucket: row.get(5)?,
        request_count,
        success_count,
        success_rate,
        error_count_4xx,
        error_count_5xx,
        error_rate_4xx,
        error_rate_5xx,
        total_tokens,
        cached_tokens,
        cache_hit_rate,
        cost_cny,
        avg_duration_ms,
        avg_time_to_first_token_ms,
        avg_tokens_per_second,
    })
}

/// 清空模型调用统计数据
///
/// `scope` 为 `None` 时清空全部数据；为 `Some((start_at, end_at))` 时仅清空指定时间范围。
/// 同时清空明细表 `model_call_logs` 和两张聚合表 `model_call_stats_hourly` / `model_call_stats_daily`。
/// 返回受影响的总行数。
pub fn clear_call_stats(
    scope: Option<(&str, &str)>,
) -> IcodeResult<u64> {
    let conn = get_conn()?;

    let (where_clause, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = match scope {
        Some((start_at, end_at)) => {
            (String::from("WHERE requested_at >= ?1 AND requested_at <= ?2"),
             vec![Box::new(start_at.to_string()), Box::new(end_at.to_string())])
        }
        None => (String::new(), Vec::new()),
    };

    // 聚合表使用 time_bucket 过滤
    let (agg_where, agg_params): (String, Vec<Box<dyn rusqlite::ToSql>>) = match scope {
        Some((start_at, end_at)) => {
            (String::from("WHERE time_bucket >= ?1 AND time_bucket <= ?2"),
             vec![Box::new(start_at.to_string()), Box::new(end_at.to_string())])
        }
        None => (String::new(), Vec::new()),
    };

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let agg_params_ref: Vec<&dyn rusqlite::ToSql> = agg_params.iter().map(|p| p.as_ref()).collect();

    let deleted_logs = conn.execute(
        &format!("DELETE FROM model_call_logs {where_clause}"),
        params_ref.as_slice(),
    )? as u64;

    let deleted_hourly = conn.execute(
        &format!("DELETE FROM model_call_stats_hourly {agg_where}"),
        agg_params_ref.as_slice(),
    )? as u64;

    let deleted_daily = conn.execute(
        &format!("DELETE FROM model_call_stats_daily {agg_where}"),
        agg_params_ref.as_slice(),
    )? as u64;

    Ok(deleted_logs + deleted_hourly + deleted_daily)
}

/// 查询今日消耗的 total_tokens 总数
///
/// 从 `model_call_logs` 表聚合今天 00:00:00 UTC 至今的所有 total_tokens。
pub fn get_today_tokens() -> IcodeResult<i64> {
    let conn = get_conn()?;
    let now = Utc::now();
    let today_start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap_or(now.date_naive().and_hms_opt(0, 0, 0).unwrap_or_default())
        .and_utc()
        .to_rfc3339();

    let total: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(total_tokens), 0) FROM model_call_logs WHERE requested_at >= ?1",
            [today_start],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(total)
}
