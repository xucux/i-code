-- =============================================================
-- V011 幂等累加：修复预聚合表重复累加导致的统计数值膨胀
-- =============================================================
-- 背景：流式请求在转发链路中会对同一调用记录执行两次 finish
-- （forwarder 先以估算 prompt_tokens 完成一次，SSE 流结束后再以真实
-- usage 完成一次）。明细表 model_call_logs 为 UPDATE 覆盖（只有一行），
-- 而预聚合表此前无条件 UPSERT 累加，导致同一调用被重复计数，
-- 「聚合汇总」远大于「明细汇总」。
-- 本迁移：
--   1) model_call_logs 增加 stats_accumulated 标记列（幂等累加开关）
--   2) 从明细表全量重建 model_call_stats_hourly / model_call_stats_daily，
--      修复历史重复累加的脏数据（重建口径与「明细」视图完全一致）
--   3) 历史记录统一标记为已累加，防止后续任何路径再次累加

ALTER TABLE model_call_logs ADD COLUMN stats_accumulated INTEGER NOT NULL DEFAULT 0;

-- 重建小时级聚合表
DELETE FROM model_call_stats_hourly;
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
GROUP BY provider_id, model_id, source, route_mode, COALESCE(api_key_secret_id, ''),
  strftime('%Y-%m-%dT%H:00:00', requested_at);

-- 重建天级聚合表
DELETE FROM model_call_stats_daily;
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
GROUP BY provider_id, model_id, source, route_mode, COALESCE(api_key_secret_id, ''),
  strftime('%Y-%m-%dT%H:%M:%S', requested_at, 'start of day');

-- 历史记录一律视为已累加（迁移已重建聚合表），防止旧记录被再次累加
UPDATE model_call_logs SET stats_accumulated = 1;