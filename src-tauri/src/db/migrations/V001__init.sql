-- ===== i-code 数据库初始迁移 V001__init（基线重置版） =====
-- 由 main.sql 聚合生成的完整表结构 + 索引 + 默认数据。
-- 本迁移为唯一基线版本，后续表结构变更新增迁移。
-- 所有 CREATE TABLE 使用 IF NOT EXISTS，便于在已有数据库上重跑（配合 schema_migrations 清空策略）。

PRAGMA foreign_keys = ON;

-- ===== 网关设置单例表 =====
CREATE TABLE IF NOT EXISTS "gateway_settings" (
  id TEXT PRIMARY KEY DEFAULT 'default',
  gateway_host TEXT NOT NULL DEFAULT '127.0.0.1',
  gateway_port INTEGER NOT NULL DEFAULT 54321,
  default_api_key_secret_id TEXT,
  is_enabled INTEGER NOT NULL DEFAULT 1,
  log_level TEXT NOT NULL DEFAULT 'minimal',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ===== 应用全局设置单例表 =====
CREATE TABLE IF NOT EXISTS app_settings (
  id TEXT PRIMARY KEY DEFAULT 'default',
  theme TEXT NOT NULL DEFAULT 'dark',
  locale TEXT NOT NULL DEFAULT 'zh-CN',
  global_proxy_json TEXT,
  network_timeout_ms INTEGER DEFAULT 120000,
  network_retry_json TEXT,
  store_secrets_in_keychain INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
, global_proxy_enabled INTEGER NOT NULL DEFAULT 0, config_key TEXT, titlebar_info_json TEXT NOT NULL DEFAULT '{"showTokens":true,"showRpm":true,"showLatency":false,"showMemory":true,"showGatewayStatus":true}', backup_settings_json TEXT, auto_start_enabled INTEGER NOT NULL DEFAULT 0, gateway_last_running INTEGER NOT NULL DEFAULT 0, log_level TEXT NOT NULL DEFAULT 'info');

-- ===== 全局配置表 =====
-- 用于存储不适合放入单例表的键值对配置，如 OAuth 预设 client_id / client_secret。
CREATE TABLE IF NOT EXISTS global_configs (
  id TEXT PRIMARY KEY,
  "group" TEXT NOT NULL,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  description TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE("group", key)
);

-- ===== 迁移版本记录表 =====
-- 注意：迁移执行器会以 CREATE TABLE IF NOT EXISTS 方式预先创建本表，此处保留是为了基线完整性。
CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);

-- ===== 内置模型列表（种子数据） =====
CREATE TABLE IF NOT EXISTS builtin_models (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  family TEXT,
  provider_type TEXT,
  max_input_tokens INTEGER,
  max_output_tokens INTEGER,
  tokenizer TEXT,
  token_count_multiplier REAL NOT NULL DEFAULT 1.0,
  stream INTEGER,
  temperature REAL,
  top_k INTEGER,
  top_p REAL,
  frequency_penalty REAL,
  presence_penalty REAL,
  parallel_tool_calling INTEGER,
  service_tier TEXT,
  verbosity TEXT,
  capabilities_json TEXT,
  thinking_json TEXT,
  multi_agent_json TEXT,
  web_search_json TEXT,
  memory_tool INTEGER,
  preset_templates_json TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);

-- ===== AI Gateway 供应商 =====
CREATE TABLE IF NOT EXISTS providers (
  id TEXT PRIMARY KEY,
  slug TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL,
  provider_type TEXT NOT NULL,
  base_url TEXT NOT NULL,
  use_raw_base_url INTEGER NOT NULL DEFAULT 0,
  transport TEXT,
  service_tier TEXT,
  auth_json TEXT,
  balance_provider_json TEXT,
  timeout_json TEXT,
  retry_json TEXT,
  proxy_json TEXT,
  auto_fetch_official_models INTEGER NOT NULL DEFAULT 0,
  context_cache_json TEXT,
  well_known_template_id TEXT,
  is_enabled INTEGER NOT NULL DEFAULT 1,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- ===== 日志配置单例表 =====
CREATE TABLE IF NOT EXISTS log_settings (
  id              TEXT PRIMARY KEY DEFAULT 'default',

  -- 基础设置
  buffer_size     INTEGER NOT NULL DEFAULT 5000,    -- 内存缓冲队列大小
  log_dir         TEXT    NOT NULL DEFAULT '',       -- 日志文件目录（空则使用默认）
  max_retention_days INTEGER NOT NULL DEFAULT 30,   -- 日志文件保留天数
  enable_file_persistence INTEGER NOT NULL DEFAULT 0, -- 是否启用文件持久化（0=否，1=是）
  max_file_size_mb  INTEGER NOT NULL DEFAULT 10,    -- 单个日志文件大小上限（MB）
  max_file_count    INTEGER NOT NULL DEFAULT 7,     -- 保留的日志文件数量
  file_log_level    TEXT    DEFAULT 'INFO',          -- 文件写入级别阈值

  -- 转发详细日志配置（首次安装默认开启请求/响应记录）
  enable_request_log  INTEGER NOT NULL DEFAULT 1,   -- 是否记录转发请求体
  enable_response_log INTEGER NOT NULL DEFAULT 1,   -- 是否记录转发响应体
  forward_max_body_length INTEGER NOT NULL DEFAULT 4096, -- 转发日志最大记录长度

  -- Command 交互日志配置
  enable_command_log  INTEGER NOT NULL DEFAULT 1,   -- 是否记录 Command 调用
  enable_command_request_log INTEGER NOT NULL DEFAULT 1, -- 是否记录请求参数
  enable_command_response_log INTEGER NOT NULL DEFAULT 1, -- 是否记录响应数据
  command_max_body_length INTEGER NOT NULL DEFAULT 4096,  -- Command 日志最大记录长度
  enable_gateway_request_log INTEGER NOT NULL DEFAULT 1,
  enable_gateway_response_log INTEGER NOT NULL DEFAULT 1,
  gateway_max_body_length INTEGER NOT NULL DEFAULT 4096
);

-- ===== 模型调用统计（按天聚合） =====
CREATE TABLE IF NOT EXISTS "model_call_stats_daily" (
  id TEXT PRIMARY KEY,
  provider_id TEXT NOT NULL,
  model_id TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'gateway',
  route_mode INTEGER NOT NULL DEFAULT 1,
  api_key_secret_id TEXT NOT NULL DEFAULT '',
  time_bucket TEXT NOT NULL,
  request_count INTEGER NOT NULL DEFAULT 0,
  success_count INTEGER NOT NULL DEFAULT 0,
  error_count_4xx INTEGER NOT NULL DEFAULT 0,
  error_count_5xx INTEGER NOT NULL DEFAULT 0,
  total_tokens INTEGER NOT NULL DEFAULT 0,
  cached_tokens INTEGER NOT NULL DEFAULT 0,
  cache_hit_count INTEGER NOT NULL DEFAULT 0,
  cost_usd REAL NOT NULL DEFAULT 0.0,
  sum_duration_ms INTEGER NOT NULL DEFAULT 0,
  sum_ttft_ms INTEGER NOT NULL DEFAULT 0,
  sum_output_tps REAL NOT NULL DEFAULT 0.0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(provider_id, model_id, source, route_mode, api_key_secret_id, time_bucket)
);

-- ===== WebDAV 配置表 =====
CREATE TABLE IF NOT EXISTS webdav_configs (
  id TEXT PRIMARY KEY,                      -- 配置唯一标识（UUID）
  name TEXT NOT NULL,                       -- 配置显示名称
  url TEXT NOT NULL,                        -- WebDAV 服务器 URL
  username TEXT NOT NULL,                   -- 用户名
  password TEXT NOT NULL,                   -- 密码（明文存储）
  remote_path TEXT NOT NULL DEFAULT '/i-code-backups/', -- 远程目录路径
  strict_ssl INTEGER NOT NULL DEFAULT 1,    -- 是否校验 TLS 证书（0=否，1=是）
  preset TEXT NOT NULL DEFAULT 'custom',    -- 服务预设（jianguoyun/koofr/nextcloud/custom）
  sort_order INTEGER NOT NULL DEFAULT 0,    -- 排序权重
  is_enabled INTEGER NOT NULL DEFAULT 1,    -- 是否启用（0=禁用，1=启用）
  created_at TEXT NOT NULL,                 -- 创建时间（ISO 8601）
  updated_at TEXT NOT NULL                  -- 更新时间（ISO 8601）
);

-- ===== 内置供应商预设 =====
CREATE TABLE IF NOT EXISTS builtin_providers (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  category TEXT NOT NULL,
  provider_type TEXT NOT NULL,
  base_url TEXT NOT NULL,
  use_raw_base_url INTEGER NOT NULL DEFAULT 0,
  default_auth_json TEXT,
  default_balance_provider_json TEXT,
  extra_headers_json TEXT,
  extra_body_json TEXT,
  auto_fetch_official_models INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);

-- ===== 模型完整配置表 =====
CREATE TABLE IF NOT EXISTS model_configs (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  family TEXT,
  max_input_tokens INTEGER,
  max_output_tokens INTEGER,
  tokenizer TEXT,
  token_count_multiplier REAL NOT NULL DEFAULT 1.0,
  stream INTEGER,
  temperature REAL,
  top_k INTEGER,
  top_p REAL,
  frequency_penalty REAL,
  presence_penalty REAL,
  parallel_tool_calling INTEGER,
  service_tier TEXT,
  verbosity TEXT,
  capabilities_json TEXT,
  thinking_json TEXT,
  multi_agent_json TEXT,
  web_search_json TEXT,
  memory_tool INTEGER,
  preset_templates_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
, price_per_1m_tokens REAL);

-- ===== 网关认证 API Key 表 =====
CREATE TABLE IF NOT EXISTS gateway_auth_keys (
  id TEXT PRIMARY KEY,
  -- API Key 名称
  name TEXT NOT NULL,
  -- API Key 描述
  description TEXT,
  -- API Key 值（$SECRET:{uuid}$ 引用或明文）
  api_key_secret_id TEXT,
  -- 是否启用
  is_enabled INTEGER NOT NULL DEFAULT 1,
  -- 过期时间（NULL 表示永不过期）
  expires_at TEXT,
  -- 排序
  sort_order INTEGER NOT NULL DEFAULT 0,
  -- 最后使用时间
  last_used_at TEXT,
  -- 创建时间
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  -- 更新时间
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ===== 敏感凭据表（加密存储） =====
CREATE TABLE IF NOT EXISTS secrets (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  encrypted_value BLOB NOT NULL,
  label TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- ===== 虚拟供应商表 =====
CREATE TABLE IF NOT EXISTS virtual_providers (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  alias TEXT NOT NULL UNIQUE,
  display_name TEXT,
  is_enabled INTEGER NOT NULL DEFAULT 1,
  strategy TEXT NOT NULL DEFAULT 'on_all',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
, max_retries INTEGER NOT NULL DEFAULT 3, retry_interval_ms INTEGER NOT NULL DEFAULT 1000);

-- ===== CLI 配置档案表 =====
CREATE TABLE IF NOT EXISTS cli_profiles (
  id TEXT PRIMARY KEY,
  slug TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL,
  cli_type TEXT NOT NULL,
  config_file_path TEXT,
  proxy_json TEXT,
  is_enabled INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- ===== 模型调用统计（按小时聚合） =====
CREATE TABLE IF NOT EXISTS "model_call_stats_hourly" (
  id TEXT PRIMARY KEY,
  provider_id TEXT NOT NULL,
  model_id TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'gateway',
  route_mode INTEGER NOT NULL DEFAULT 1,
  api_key_secret_id TEXT NOT NULL DEFAULT '',
  time_bucket TEXT NOT NULL,
  request_count INTEGER NOT NULL DEFAULT 0,
  success_count INTEGER NOT NULL DEFAULT 0,
  error_count_4xx INTEGER NOT NULL DEFAULT 0,
  error_count_5xx INTEGER NOT NULL DEFAULT 0,
  total_tokens INTEGER NOT NULL DEFAULT 0,
  cached_tokens INTEGER NOT NULL DEFAULT 0,
  cache_hit_count INTEGER NOT NULL DEFAULT 0,
  cost_usd REAL NOT NULL DEFAULT 0.0,
  sum_duration_ms INTEGER NOT NULL DEFAULT 0,
  sum_ttft_ms INTEGER NOT NULL DEFAULT 0,
  sum_output_tps REAL NOT NULL DEFAULT 0.0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(provider_id, model_id, source, route_mode, api_key_secret_id, time_bucket)
);

-- ===== 内置模型别名 =====
CREATE TABLE IF NOT EXISTS builtin_model_aliases (
  builtin_model_id TEXT NOT NULL REFERENCES builtin_models(id) ON DELETE CASCADE,
  alias TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY(builtin_model_id, alias)
);

-- ===== 内置模型覆盖配置 =====
CREATE TABLE IF NOT EXISTS builtin_model_overrides (
  id TEXT PRIMARY KEY,
  builtin_model_id TEXT NOT NULL REFERENCES builtin_models(id) ON DELETE CASCADE,
  matcher_type TEXT NOT NULL,
  matcher_value TEXT NOT NULL,
  override_config_json TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);

-- ===== 内置模型 × 供应商类型关联 =====
CREATE TABLE IF NOT EXISTS builtin_model_providers (
  builtin_model_id TEXT NOT NULL REFERENCES builtin_models(id) ON DELETE CASCADE,
  provider_type TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY(builtin_model_id, provider_type)
);

-- ===== 官方模型拉取缓存 =====
CREATE TABLE IF NOT EXISTS official_model_cache (
  provider_id TEXT PRIMARY KEY REFERENCES providers(id) ON DELETE CASCADE,
  models_json TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  expires_at TEXT,
  error_message TEXT
);

-- ===== 供应商额度快照缓存表 =====
CREATE TABLE IF NOT EXISTS provider_balance_snapshots (
  provider_id TEXT PRIMARY KEY REFERENCES providers(id) ON DELETE CASCADE,
  snapshot_json TEXT NOT NULL,                               -- BalanceSnapshot 序列化 JSON
  updated_at TEXT NOT NULL                                   -- 最近一次刷新时间（ISO 8601）
);

-- ===== 额度监控脚本模板 =====
-- 用户自定义 Rhai 脚本，用于对接未内置的供应商额度接口
CREATE TABLE IF NOT EXISTS script_templates (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  kind TEXT NOT NULL,                                        -- 模板类型：本期固定 balance
  status TEXT NOT NULL DEFAULT 'draft',                      -- draft / active / disabled
  description TEXT,
  script_body TEXT NOT NULL DEFAULT '',
  engine TEXT NOT NULL DEFAULT 'rhai',                       -- 预留多引擎；本期仅 rhai
  default_timeout_ms INTEGER NOT NULL DEFAULT 15000,
  allowed_hosts_json TEXT,                                   -- JSON 字符串数组，额外允许 host
  snippet_id TEXT,                                           -- 创建时选用的内置 snippet 标识
  last_test_at TEXT,
  last_test_ok INTEGER,
  last_test_message TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_script_templates_kind_status
  ON script_templates(kind, status, sort_order);

CREATE INDEX IF NOT EXISTS idx_script_templates_status
  ON script_templates(status);

-- ===== 供应商级附加请求体参数 =====
CREATE TABLE IF NOT EXISTS provider_extra_body (
  provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  value_json TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(provider_id, key)
);

-- ===== 供应商级附加请求头 =====
CREATE TABLE IF NOT EXISTS provider_extra_headers (
  provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(provider_id, key)
);

-- ===== 内置供应商认证方式 =====
CREATE TABLE IF NOT EXISTS builtin_provider_auth_types (
  builtin_provider_id TEXT NOT NULL REFERENCES builtin_providers(id) ON DELETE CASCADE,
  auth_method TEXT NOT NULL,
  is_default INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  PRIMARY KEY(builtin_provider_id, auth_method)
);

-- ===== 内置供应商与模型关联 =====
CREATE TABLE IF NOT EXISTS builtin_provider_models (
  builtin_provider_id TEXT NOT NULL REFERENCES builtin_providers(id) ON DELETE CASCADE,
  builtin_model_id TEXT NOT NULL REFERENCES builtin_models(id) ON DELETE CASCADE,
  declared_model_id TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  PRIMARY KEY(builtin_provider_id, builtin_model_id)
);

-- ===== 网关暴露模型表 =====
CREATE TABLE IF NOT EXISTS gateway_models (
  id TEXT PRIMARY KEY,
  provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  model_config_id TEXT NOT NULL REFERENCES model_configs(id) ON DELETE CASCADE,
  model_id TEXT NOT NULL,
  display_name TEXT,
  family TEXT,
  source TEXT NOT NULL DEFAULT 'manual',
  is_exposed INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(provider_id, model_id)
);

-- ===== 模型级附加请求体参数 =====
CREATE TABLE IF NOT EXISTS model_config_extra_body (
  model_config_id TEXT NOT NULL REFERENCES model_configs(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  value_json TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(model_config_id, key)
);

-- ===== 模型级附加请求头 =====
CREATE TABLE IF NOT EXISTS model_config_extra_headers (
  model_config_id TEXT NOT NULL REFERENCES model_configs(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(model_config_id, key)
);

-- ===== 虚拟模型表 =====
CREATE TABLE IF NOT EXISTS virtual_models (
  id TEXT PRIMARY KEY,
  virtual_provider_id TEXT NOT NULL REFERENCES virtual_providers(id) ON DELETE CASCADE,
  model_id TEXT NOT NULL,
  display_name TEXT,
  is_enabled INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(virtual_provider_id, model_id)
);

-- ===== CLI 档案绑定的供应商 =====
CREATE TABLE IF NOT EXISTS cli_providers (
  id TEXT PRIMARY KEY,
  cli_profile_id TEXT NOT NULL REFERENCES cli_profiles(id) ON DELETE CASCADE,
  provider_id TEXT REFERENCES providers(id) ON DELETE SET NULL,
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

-- ===== 模型调用日志 =====
CREATE TABLE IF NOT EXISTS model_call_logs (
  id TEXT PRIMARY KEY,
  provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  gateway_model_id TEXT REFERENCES gateway_models(id) ON DELETE SET NULL,
  model_id TEXT NOT NULL,
  request_id TEXT,
  requested_at TEXT NOT NULL,
  completed_at TEXT,
  duration_ms INTEGER,
  status_code INTEGER,
  error_message TEXT,
  prompt_tokens INTEGER,
  completion_tokens INTEGER,
  total_tokens INTEGER,
  cached_tokens INTEGER,
  cache_hit INTEGER NOT NULL DEFAULT 0,
  route_mode INTEGER NOT NULL DEFAULT 1,
  source TEXT NOT NULL DEFAULT 'gateway',
  time_to_first_token_ms INTEGER,
  price_per_1m_tokens REAL,
  api_key_secret_id TEXT
);

-- ===== 虚拟模型路由（故障转移目标） =====
CREATE TABLE IF NOT EXISTS virtual_model_routes (
  id TEXT PRIMARY KEY,
  virtual_model_id TEXT NOT NULL REFERENCES virtual_models(id) ON DELETE CASCADE,
  target_provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  target_model_id TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  enabled INTEGER NOT NULL DEFAULT 1,
  max_retries INTEGER NOT NULL DEFAULT 0,
  timeout_ms INTEGER,
  is_healthy INTEGER NOT NULL DEFAULT 1,
  last_healthy_at TEXT,
  extra_headers_json TEXT,
  extra_body_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  retry_interval_ms INTEGER NOT NULL DEFAULT 1000
);

-- ===== CLI 模型角色映射 =====
CREATE TABLE IF NOT EXISTS cli_model_mappings (
  id TEXT PRIMARY KEY,
  cli_provider_id TEXT NOT NULL REFERENCES cli_providers(id) ON DELETE CASCADE,
  cli_model_alias TEXT NOT NULL,
  gateway_model_id TEXT,
  raw_model_id TEXT,
  input_mode TEXT NOT NULL DEFAULT 'select',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(cli_provider_id, cli_model_alias)
);

-- ===== 索引 =====
CREATE INDEX IF NOT EXISTS idx_providers_enabled ON providers(is_enabled, sort_order);
CREATE INDEX IF NOT EXISTS idx_gateway_models_provider ON gateway_models(provider_id, is_exposed);
CREATE INDEX IF NOT EXISTS idx_gateway_models_config ON gateway_models(model_config_id);
CREATE INDEX IF NOT EXISTS idx_model_call_logs_provider ON model_call_logs(provider_id, requested_at);
CREATE INDEX IF NOT EXISTS idx_model_call_logs_model ON model_call_logs(model_id, requested_at);
CREATE INDEX IF NOT EXISTS idx_model_call_logs_requested ON model_call_logs(requested_at);
CREATE INDEX IF NOT EXISTS idx_virtual_models_provider ON virtual_models(virtual_provider_id, is_enabled);
CREATE INDEX IF NOT EXISTS idx_virtual_model_routes_model ON virtual_model_routes(virtual_model_id, priority, enabled);
CREATE INDEX IF NOT EXISTS idx_builtin_model_overrides_model ON builtin_model_overrides(builtin_model_id, sort_order);
CREATE INDEX IF NOT EXISTS idx_builtin_providers_category ON builtin_providers(category, sort_order);
CREATE INDEX IF NOT EXISTS idx_builtin_provider_models_provider ON builtin_provider_models(builtin_provider_id, sort_order);
CREATE INDEX IF NOT EXISTS idx_cli_providers_cli ON cli_providers(cli_profile_id, sort_order);
CREATE INDEX IF NOT EXISTS idx_webdav_configs_enabled ON webdav_configs(is_enabled, sort_order);

-- ===== 默认数据 =====

-- 应用默认设置（单例）
INSERT OR IGNORE INTO app_settings (id, theme, locale, store_secrets_in_keychain, created_at, updated_at)
VALUES ('default', 'dark', 'zh-CN', 1, datetime('now'), datetime('now'));

-- 网关默认设置（单例）
INSERT OR IGNORE INTO gateway_settings (id) VALUES ('default');

-- 日志默认设置（单例）
INSERT OR IGNORE INTO log_settings (id) VALUES ('default');

-- OAuth 预设凭据
-- Antigravity OAuth
INSERT OR IGNORE INTO global_configs (id, "group", key, value, description, created_at)
VALUES ('oauth-antigravity-client-id', 'oauth', 'antigravity_client_id', '1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com', 'Antigravity OAuth client_id', datetime('now'));

INSERT OR IGNORE INTO global_configs (id, "group", key, value, description, created_at)
VALUES ('oauth-antigravity-client-secret', 'oauth', 'antigravity_client_secret', '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', 'Antigravity OAuth client_secret', datetime('now'));

-- Google Gemini OAuth
INSERT OR IGNORE INTO global_configs (id, "group", key, value, description, created_at)
VALUES ('oauth-google-gemini-client-id', 'oauth', 'google_gemini_client_id', '1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com', 'Google Gemini OAuth client_id', datetime('now'));

INSERT OR IGNORE INTO global_configs (id, "group", key, value, description, created_at)
VALUES ('oauth-google-gemini-client-secret', 'oauth', 'google_gemini_client_secret', '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', 'Google Gemini OAuth client_secret', datetime('now'));
