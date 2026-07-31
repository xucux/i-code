-- V003: providers 表新增 OAuth 认证相关字段
-- - auth_expires_at: OAuth 授权过期时间（ISO8601 字符串，由 OAuth 成功流程回填）
-- - auth_method: 认证方式（与 AuthConfig 中的 OAuth 类型对应，便于定时扫描与展示）

ALTER TABLE providers ADD COLUMN auth_expires_at TEXT;
ALTER TABLE providers ADD COLUMN auth_method TEXT;
