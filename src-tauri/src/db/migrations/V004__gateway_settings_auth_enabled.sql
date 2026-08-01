-- V004: gateway_settings 表新增 auth_enabled 列
-- - auth_enabled: 是否启用外部请求认证（默认 1 启用）
--   * 0：开放模式，外部请求无需认证即可访问网关
--   * 1：需要携带有效 API Key（默认 Gateway Key 或 gateway_auth_keys 中的记录）
--   注意：内部 CLI 通过 inner-cli-api 请求头豁免，不受此开关影响。
--   default_api_key_secret_id 仅在 auth_enabled=1 时参与校验，不控制认证开关。

ALTER TABLE gateway_settings ADD COLUMN auth_enabled INTEGER NOT NULL DEFAULT 1;
