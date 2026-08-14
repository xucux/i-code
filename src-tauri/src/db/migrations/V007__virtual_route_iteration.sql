-- V007：虚拟路由迭代（健康检查元数据 + 负载均衡权重 + 尝试历史）
--
-- 本迁移聚合本次虚拟供应商迭代的全部 schema 变更，原 V007/V008/V009 合并生成：
-- 1. 健康检查元数据：为 virtual_model_routes 增列
--    - consecutive_failures：连续探活失败次数（探活成功后清零）
--    - last_error_text：上次探活失败原因（探活成功后清空）
--    - last_check_duration_ms：上次探活耗时
--    - last_check_at：上次探活时间戳
-- 2. 负载均衡权重：为 virtual_model_routes 增列
--    - weight：load_balance 策略使用的权重（默认 1，0 表示不参与轮询）
-- 3. 路由尝试历史：新增 virtual_route_attempts 表，记录每次请求的路由尝试链，
--    供 UI 展示最近 N 次请求与每条路由的成功率/平均耗时；表按 30 天自动清理。
--
-- 配合 scheduler::health_check_loop、service::mark_route_healthy / mark_route_check_failed、
-- service::LoadBalanceSelector 与 gateway_runtime VirtualForwarder 使用。
-- 不修改既有 is_healthy / last_healthy_at 列，保持向后兼容。
-- fallback / on_all 策略忽略 weight 字段。

-- 1) 健康检查元数据列
ALTER TABLE virtual_model_routes ADD COLUMN consecutive_failures INTEGER NOT NULL DEFAULT 0;
ALTER TABLE virtual_model_routes ADD COLUMN last_error_text TEXT;
ALTER TABLE virtual_model_routes ADD COLUMN last_check_duration_ms INTEGER;
ALTER TABLE virtual_model_routes ADD COLUMN last_check_at TEXT;

-- 索引：调度器快速取出待探活路由（is_healthy=0 或 consecutive_failures>0）
CREATE INDEX IF NOT EXISTS idx_virtual_routes_health_check
  ON virtual_model_routes(is_healthy, consecutive_failures);

-- 2) 负载均衡权重列
ALTER TABLE virtual_model_routes ADD COLUMN weight INTEGER NOT NULL DEFAULT 1;

-- 3) 路由尝试历史表
CREATE TABLE IF NOT EXISTS virtual_route_attempts (
  id TEXT PRIMARY KEY,
  -- 关联 virtual_model_routes；外键级联删除，路由删除时历史一并清理
  virtual_route_id TEXT NOT NULL REFERENCES virtual_model_routes(id) ON DELETE CASCADE,
  -- 冗余 virtual_provider_id，便于按供应商维度查询统计
  virtual_provider_id TEXT NOT NULL,
  -- 关联 call_records 的 request_id，便于交叉跳转
  request_id TEXT NOT NULL,
  -- 第几条尝试（0-based）
  attempt_index INTEGER NOT NULL,
  -- 是否成功（0/1）
  success INTEGER NOT NULL,
  -- HTTP 状态码
  status_code INTEGER,
  -- 失败原因
  error_message TEXT,
  -- 该路由耗时（毫秒）
  duration_ms INTEGER NOT NULL,
  -- 尝试时间戳
  attempted_at TEXT NOT NULL
);

-- 按路由 + 时间倒序：UI 取最近 N 次
CREATE INDEX IF NOT EXISTS idx_attempts_route_time
  ON virtual_route_attempts(virtual_route_id, attempted_at DESC);
-- 按供应商 + 时间倒序：供应商维度的成功率统计
CREATE INDEX IF NOT EXISTS idx_attempts_provider_time
  ON virtual_route_attempts(virtual_provider_id, attempted_at DESC);
-- 按时间清理：scheduler 每天 0 点删除 30 天前的记录
CREATE INDEX IF NOT EXISTS idx_attempts_time
  ON virtual_route_attempts(attempted_at);

-- ===================== CLI 绑定虚拟供应商 =====================
-- 重建 cli_providers 表：
-- 1. 去掉 provider_id 对 providers 的外键约束：虚拟供应商绑定 provider_id 使用
--    `virtual:{virtual_provider_id}` 前缀，无法命中 providers 表，保留外键会导致插入失败。
-- 2. 去掉 cli_profile_id 对 cli_profiles 的外键约束：CLI 档案删除时不级联删除绑定，
--    便于虚拟供应商等非强关联场景的干净清理。
-- SQLite 不支持 ALTER ... DROP CONSTRAINT，需重建表。
-- 前置条件：迁移执行器（run_migrations）已临时关闭外键（PRAGMA foreign_keys=OFF），
-- 因此 DROP TABLE 不会触发对 cli_model_mappings 的级联删除。

CREATE TABLE cli_providers_new (
  id TEXT PRIMARY KEY,
  cli_profile_id TEXT NOT NULL,
  provider_id TEXT,
  display_name TEXT NOT NULL,
  route_mode INTEGER NOT NULL DEFAULT 0,
  gateway_base_url TEXT,
  direct_base_url TEXT,
  auth_json TEXT,
  balance_json TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  is_default INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

INSERT INTO cli_providers_new (
  id, cli_profile_id, provider_id, display_name, route_mode,
  gateway_base_url, direct_base_url, auth_json, balance_json,
  sort_order, is_default, created_at, updated_at
)
SELECT
  id, cli_profile_id, provider_id, display_name, route_mode,
  gateway_base_url, direct_base_url, auth_json, balance_json,
  sort_order, is_default, created_at, updated_at
FROM cli_providers;

DROP TABLE cli_providers;

ALTER TABLE cli_providers_new RENAME TO cli_providers;

-- 重建原索引（DROP TABLE 时随表一并删除）
CREATE INDEX IF NOT EXISTS idx_cli_providers_cli ON cli_providers(cli_profile_id, sort_order);
