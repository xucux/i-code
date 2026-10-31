-- 最近一次真实路由尝试时间（请求侧转发，非探活）
-- 用于「全部路由永久失活」时按兜底间隔判定是否允许再尝试一次
ALTER TABLE virtual_model_routes ADD COLUMN last_attempted_at TEXT;
