-- 虚拟路由探活历史表
--
-- 记录调度器自动探活与用户手动「测试路由」的每次结果，
-- 供虚拟供应商页「探活记录」Tab 展示（此前仅保留每条路由的最后一次探活状态）。
CREATE TABLE IF NOT EXISTS virtual_route_probe_logs (
    id TEXT PRIMARY KEY,
    virtual_route_id TEXT NOT NULL,
    virtual_provider_id TEXT NOT NULL,
    -- 来源：auto（调度器自动探活）/ manual（用户手动测试路由）
    source TEXT NOT NULL,
    success INTEGER NOT NULL,
    status_code INTEGER,
    duration_ms INTEGER NOT NULL,
    error_message TEXT,
    -- 失败类型（RouteFailureKind），无法判定 / 手动失败时为 NULL
    failure_kind TEXT,
    probed_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_virtual_route_probe_logs_route
    ON virtual_route_probe_logs(virtual_route_id, probed_at DESC);

CREATE INDEX IF NOT EXISTS idx_virtual_route_probe_logs_provider
    ON virtual_route_probe_logs(virtual_provider_id, probed_at DESC);
