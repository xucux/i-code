-- 虚拟路由冷却窗口：区分永久性 / 瞬时性故障，冷却期内不参与选路
-- cooldown_until：冷却截止时间（RFC3339）；为空表示未处于冷却
-- failure_kind：最近一次失败类型（quota_exhausted / auth_failure / model_unavailable /
--               provider_misconfigured / rate_limited / upstream_unavailable / timeout）
ALTER TABLE virtual_model_routes ADD COLUMN cooldown_until TEXT;
ALTER TABLE virtual_model_routes ADD COLUMN failure_kind TEXT;

CREATE INDEX IF NOT EXISTS idx_virtual_model_routes_cooldown
  ON virtual_model_routes(cooldown_until);
