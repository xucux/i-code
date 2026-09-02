# i-code 数据库设计文档

> 版本：v0.1.0  
> 存储引擎：SQLite 3  
> 运行环境：Tauri 2.x 桌面端本地持久化

## 1. 设计目标

i-code 使用 SQLite 作为唯一本地配置存储，支撑以下核心能力：

1. **AI Gateways**：多供应商 CRUD、模型列表、代理与认证配置
2. **CLI 管理**：CLI 实例、路由模式、模型映射、额度信息
3. **网关路由**：对外模型 ID 格式为 `{providerId}/{modelId}`，网关层解析后路由到真实供应商

设计参考：`参考项目/vscode-unify-chat-provider-7.12.3/src` 中的 `ProviderConfig`、`ModelConfig`、`AuthConfig`、`ProxyConfig` 等类型定义。

---

## 2. 全局约定

### 2.1 主键与标识

| 约定 | 说明 |
|------|------|
| 表主键 | 统一使用 `TEXT` UUID（`uuid v4`） |
| 业务唯一键 | `providers.slug`、`cli_profiles.slug` 全局唯一 |
| 时间戳 | `created_at` / `updated_at` 使用 ISO 8601 UTC 文本或 Unix 毫秒整数 |

### 2.2 敏感数据

| 字段类型 | 存储策略 |
|----------|----------|
| API Key / OAuth Token | **禁止明文落库**；存入 `secrets` 表（加密）或系统密钥链，配置中仅存 `$SECRET:{uuid}$` 引用 |
| 代理认证 | 同上 |
| 内部 CLI 代理通信 | 可跳过网关 API Key 认证（应用层白名单） |

### 2.3 JSON 扩展字段

复杂嵌套结构（如 `extra_headers`、`capabilities`、`thinking`）使用 `TEXT` JSON 列存储，读写时在应用层做 schema 校验。

### 2.4 模型 ID 路由规则

```
对外暴露 ID = {provider_slug}/{model_id}
网关解析：
  provider_slug → providers 表
  model_id      → gateway_models 表（或 provider 内置 models JSON）
内部 CLI 路由模式：
  请求 base_url → 本地网关
  Authorization → 可选（内部通信可忽略）
  model 字段    → 保持 provider_slug/model_id 前缀
```

---

## 3. ER 关系概览

```mermaid
erDiagram
    app_settings ||--o{ providers : configures
    app_settings ||--|| gateway_settings : references
    providers ||--o{ gateway_models : exposes
    providers ||--o{ provider_extra_headers : sends
    providers ||--o{ provider_extra_body : sends
    providers }o--o| secrets : references
    gateway_settings }o--o| secrets : references
    gateway_settings ||--o{ gateway_auth_keys : owns

    gateway_models }o--|| model_configs : uses
    model_configs ||--o{ model_config_extra_headers : sends
    model_configs ||--o{ model_config_extra_body : sends

    builtin_models ||--o{ builtin_model_providers : adapts_to
    builtin_models ||--o{ builtin_model_aliases : aliased_as
    builtin_models ||--o{ builtin_model_overrides : overrides
    builtin_providers ||--o{ builtin_provider_models : recommends
    builtin_providers ||--o{ builtin_provider_auth_types : supports
    builtin_provider_models }o--|| builtin_models : references

    cli_profiles ||--o{ cli_providers : binds
    providers ||--o{ cli_providers : routes_to
    cli_profiles ||--o{ cli_model_mappings : maps

    providers ||--o{ model_call_logs : logs
    gateway_models ||--o{ model_call_logs : logs
```

---

## 4. 表结构定义

### 4.1 `app_settings` — 应用全局设置

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PK | 固定单例 `default` |
| `theme` | TEXT | NOT NULL | `light` / `dark` / `claude-light` / `claude-dark` / `deepseek-light` / `deepseek-dark` / `nvidia-light` / `nvidia-dark` |
| `locale` | TEXT | NOT NULL | `zh-CN` / `zh-TW` / `en` / `ja` |
| `global_proxy_json` | TEXT | | 全局代理 `ProxyConfig` |
| `network_timeout_ms` | INTEGER | | 默认请求超时 |
| `network_retry_json` | TEXT | | 全局重试策略 `RetryConfig` JSON，见 §5.8 |
| `store_secrets_in_keychain` | INTEGER | NOT NULL | 1=系统密钥链，0=本地加密 |
| `config_key` | TEXT | | AES 配置密钥，用于加密部分落库数据 |
| `titlebar_info_json` | TEXT | NOT NULL | 标题栏信息展示配置 JSON，见 §5.15 |
| `created_at` | TEXT | NOT NULL | |
| `updated_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE app_settings (
  id TEXT PRIMARY KEY DEFAULT 'default',
  theme TEXT NOT NULL DEFAULT 'dark',
  locale TEXT NOT NULL DEFAULT 'zh-CN',
  global_proxy_json TEXT,
  network_timeout_ms INTEGER DEFAULT 120000,
  network_retry_json TEXT,
  store_secrets_in_keychain INTEGER NOT NULL DEFAULT 1,
  config_key TEXT,
  titlebar_info_json TEXT NOT NULL DEFAULT '{"showTokens":true,"showRpm":true,"showLatency":false,"showMemory":true,"showGatewayStatus":true}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

---

### 4.2 `secrets` — 敏感凭据

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PK | UUID |
| `kind` | TEXT | NOT NULL | `api-key` / `oauth-token` / `proxy-auth` / `gateway-key` |
| `encrypted_value` | BLOB | NOT NULL | AES-GCM 密文或密钥链句柄索引 |
| `label` | TEXT | | 展示用标签 |
| `created_at` | TEXT | NOT NULL | |
| `updated_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE secrets (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  encrypted_value BLOB NOT NULL,
  label TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

配置引用格式：`$SECRET:{id}$`

---

### 4.3 `providers` — AI Gateway 供应商

对应参考项目 `ProviderConfig`（不含 `models` 数组与 extra headers/body，模型拆到 `gateway_models`，附加头/体拆到 §4.4、§4.5）。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PK | UUID |
| `slug` | TEXT | UNIQUE NOT NULL | 路由用供应商标识，如 `openai-main` |
| `display_name` | TEXT | NOT NULL | 展示名称 |
| `provider_type` | TEXT | NOT NULL | 协议类型，见 §5.1 |
| `base_url` | TEXT | NOT NULL | API Base URL |
| `use_raw_base_url` | INTEGER | DEFAULT 0 | |
| `transport` | TEXT | | `auto` / `sse` / `websocket` |
| `service_tier` | TEXT | | |
| `auth_json` | TEXT | | 认证配置 `AuthConfig` JSON（多态联合类型），密钥用 `$SECRET:{uuid}$` 引用，见 §5.4 |
| `balance_provider_json` | TEXT | | 额度监控配置 `BalanceConfig` JSON，见 §5.10 |
| `timeout_json` | TEXT | | 超时配置 `TimeoutConfig` JSON，见 §5.7 |
| `retry_json` | TEXT | | 重试配置 `RetryConfig` JSON，见 §5.8 |
| `proxy_json` | TEXT | | 供应商级代理 `ProxyConfig` JSON，见 §5.3 |
| `auto_fetch_official_models` | INTEGER | DEFAULT 0 | 是否自动从供应商 API 拉取官方模型列表 |
| `context_cache_json` | TEXT | | 上下文缓存 `ContextCacheConfig` JSON，见 §5.9 |
| `well_known_template_id` | TEXT | | 来源预设模板 ID |
| `is_enabled` | INTEGER | NOT NULL DEFAULT 1 | |
| `sort_order` | INTEGER | DEFAULT 0 | |
| `created_at` | TEXT | NOT NULL | |
| `updated_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE providers (
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

CREATE INDEX idx_providers_enabled ON providers(is_enabled, sort_order);
```

---

### 4.4 `provider_extra_headers` — 供应商附加请求头

将 `providers.extra_headers_json` 拆分为行式键值对，便于 UI 单独增删改、按 key 去重/覆盖。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `provider_id` | TEXT | PK, FK | → `providers.id` |
| `key` | TEXT | PK | HTTP Header 名 |
| `value` | TEXT | NOT NULL | Header 值，支持 `$SECRET:{uuid}$` |
| `sort_order` | INTEGER | DEFAULT 0 | |
| `created_at` | TEXT | NOT NULL | |
| `updated_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE provider_extra_headers (
  provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(provider_id, key)
);
```

---

### 4.5 `provider_extra_body` — 供应商附加请求体参数

将 `providers.extra_body_json` 拆分为行式键值对，value 为任意 JSON。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `provider_id` | TEXT | PK, FK | → `providers.id` |
| `key` | TEXT | PK | 请求体参数名 |
| `value_json` | TEXT | NOT NULL | JSON 值 |
| `sort_order` | INTEGER | DEFAULT 0 | |
| `created_at` | TEXT | NOT NULL | |
| `updated_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE provider_extra_body (
  provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  value_json TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(provider_id, key)
);
```

---

### 4.6 `model_configs` — 模型完整配置

对应原 `gateway_models.config_json`。将 `ModelConfig` 核心字段拆分为列，避免运行时每次请求都解析整个 JSON；复杂嵌套对象仍用 JSON 列存储。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PK | UUID |
| `name` | TEXT | NOT NULL | 展示名称 |
| `family` | TEXT | | 模型族 |
| `max_input_tokens` | INTEGER | | 最大输入/上下文 Token 数 |
| `max_output_tokens` | INTEGER | | 最大输出 Token 数 |
| `tokenizer` | TEXT | | Tokenizer ID |
| `token_count_multiplier` | REAL | DEFAULT 1.0 | Token 计数乘数 |
| `stream` | INTEGER | | 是否流式 |
| `temperature` | REAL | | 采样温度 |
| `top_k` | INTEGER | | Top-K |
| `top_p` | REAL | | Top-P |
| `frequency_penalty` | REAL | | 频率惩罚 |
| `presence_penalty` | REAL | | 存在惩罚 |
| `parallel_tool_calling` | INTEGER | | 是否并行工具调用 |
| `service_tier` | TEXT | | `auto` / `standard` / `flex` / `scale` / `priority` |
| `verbosity` | TEXT | | `low` / `medium` / `high` |
| `capabilities_json` | TEXT | | 模型能力 `ModelCapabilities` JSON，见 §5.6 |
| `thinking_json` | TEXT | | 思考配置 `ThinkingConfig` JSON，见 §5.11 |
| `multi_agent_json` | TEXT | | 多 Agent 配置 |
| `web_search_json` | TEXT | | 联网搜索配置 |
| `memory_tool` | INTEGER | | 是否启用原生记忆工具 |
| `preset_templates_json` | TEXT | | 预设模板数组 `PresetTemplate[]`，见 §5.14 |
| `created_at` | TEXT | NOT NULL | |
| `updated_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE model_configs (
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
);
```

---

### 4.7 `model_config_extra_headers` — 模型配置附加请求头

`model_configs` 级别的附加请求头，合并优先级高于供应商级别。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `model_config_id` | TEXT | PK, FK | → `model_configs.id` |
| `key` | TEXT | PK | |
| `value` | TEXT | NOT NULL | 支持 `$SECRET:{uuid}$` |
| `sort_order` | INTEGER | DEFAULT 0 | |
| `created_at` | TEXT | NOT NULL | |
| `updated_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE model_config_extra_headers (
  model_config_id TEXT NOT NULL REFERENCES model_configs(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(model_config_id, key)
);
```

---

### 4.8 `model_config_extra_body` — 模型配置附加请求体参数

`model_configs` 级别的附加请求体参数。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `model_config_id` | TEXT | PK, FK | → `model_configs.id` |
| `key` | TEXT | PK | |
| `value_json` | TEXT | NOT NULL | |
| `sort_order` | INTEGER | DEFAULT 0 | |
| `created_at` | TEXT | NOT NULL | |
| `updated_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE model_config_extra_body (
  model_config_id TEXT NOT NULL REFERENCES model_configs(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  value_json TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(model_config_id, key)
);
```

---

### 4.9 `gateway_models` — 网关暴露模型

仅暴露已配置且启用的模型。对应 `ModelConfig` 的暴露层，实际配置存于 `model_configs`。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PK | UUID |
| `provider_id` | TEXT | FK NOT NULL | → `providers.id` |
| `model_config_id` | TEXT | FK NOT NULL | → `model_configs.id` |
| `model_id` | TEXT | NOT NULL | 真实模型 ID，如 `gpt-4.1` |
| `display_name` | TEXT | | 暴露层展示名，为空时回退 `model_configs.name` |
| `family` | TEXT | | 暴露层模型族，为空时回退 `model_configs.family` |
| `source` | TEXT | NOT NULL | `manual` / `builtin` / `official` |
| `is_exposed` | INTEGER | NOT NULL DEFAULT 1 | 是否对外暴露 |
| `created_at` | TEXT | NOT NULL | |
| `updated_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE gateway_models (
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

CREATE INDEX idx_gateway_models_provider ON gateway_models(provider_id, is_exposed);
CREATE INDEX idx_gateway_models_config ON gateway_models(model_config_id);
```

对外路由 ID：`providers.slug + '/' + gateway_models.model_id`

---

### 4.10 `official_model_cache` — 官方模型拉取缓存

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `provider_id` | TEXT | PK, FK | → `providers.id` |
| `models_json` | TEXT | NOT NULL | 拉取结果 |
| `fetched_at` | TEXT | NOT NULL | |
| `expires_at` | TEXT | | |
| `error_message` | TEXT | | 最近一次失败原因 |

```sql
CREATE TABLE official_model_cache (
  provider_id TEXT PRIMARY KEY REFERENCES providers(id) ON DELETE CASCADE,
  models_json TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  expires_at TEXT,
  error_message TEXT
);
```

---

### 4.11 `builtin_models` — 内置模型列表

对应参考项目 `well-known/models.ts` 中的 `WELL_KNOWN_MODELS`。存储所有已知的模型定义，供用户从"从内置模型列表添加"时选择。结构与 `model_configs` 对齐，避免种子数据与应用数据模型不一致。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PK | 模型标识符，如 `gpt-4.1` |
| `display_name` | TEXT | NOT NULL | 展示名称，如 `GPT-4.1` |
| `family` | TEXT | | 模型族 |
| `provider_type` | TEXT | | 限定供应商类型（为空则通用） |
| `max_input_tokens` | INTEGER | | 最大输入 Token 数 |
| `max_output_tokens` | INTEGER | | 最大输出 Token 数 |
| `tokenizer` | TEXT | | |
| `token_count_multiplier` | REAL | DEFAULT 1.0 | |
| `stream` | INTEGER | | |
| `temperature` | REAL | | |
| `top_k` | INTEGER | | |
| `top_p` | REAL | | |
| `frequency_penalty` | REAL | | |
| `presence_penalty` | REAL | | |
| `parallel_tool_calling` | INTEGER | | |
| `service_tier` | TEXT | | |
| `verbosity` | TEXT | | |
| `capabilities_json` | TEXT | | 模型能力 `ModelCapabilities` JSON，见 §5.6 |
| `thinking_json` | TEXT | | 思考配置 `ThinkingConfig` JSON，见 §5.11 |
| `multi_agent_json` | TEXT | | |
| `web_search_json` | TEXT | | |
| `memory_tool` | INTEGER | | |
| `preset_templates_json` | TEXT | | 预设模板数组，见 §5.14 |
| `sort_order` | INTEGER | DEFAULT 0 | 排序优先级 |
| `created_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE builtin_models (
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
```

---

### 4.12 `builtin_model_providers` — 内置模型 × 供应商类型关联

一个内置模型可能适配多个供应商类型（如 `gpt-4.1` 可同时用于 `openai-responses` 和 `openai-chat-completion`）。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `builtin_model_id` | TEXT | PK, FK | → `builtin_models.id` |
| `provider_type` | TEXT | PK | → 见 §5.1 `provider_type` |
| `created_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE builtin_model_providers (
  builtin_model_id TEXT NOT NULL REFERENCES builtin_models(id) ON DELETE CASCADE,
  provider_type TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY(builtin_model_id, provider_type)
);
```

---

### 4.13 `builtin_model_overrides` — 内置模型供应商覆盖配置

对应原 `builtin_models.overrides_json` 中的对象形式覆盖项。字符串形式别名已规范化到 `builtin_model_aliases`。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PK | UUID |
| `builtin_model_id` | TEXT | FK NOT NULL | → `builtin_models.id` |
| `matcher_type` | TEXT | NOT NULL | `provider_type` / `name` / `pattern` |
| `matcher_value` | TEXT | NOT NULL | 匹配值 |
| `override_config_json` | TEXT | NOT NULL | 覆盖的 `Partial<ModelConfig>` JSON |
| `sort_order` | INTEGER | DEFAULT 0 | |
| `created_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE builtin_model_overrides (
  id TEXT PRIMARY KEY,
  builtin_model_id TEXT NOT NULL REFERENCES builtin_models(id) ON DELETE CASCADE,
  matcher_type TEXT NOT NULL,
  matcher_value TEXT NOT NULL,
  override_config_json TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);

CREATE INDEX idx_builtin_model_overrides_model ON builtin_model_overrides(builtin_model_id, sort_order);
```

> 匹配规则：当用户选定的供应商与任意一条记录的 `(matcher_type, matcher_value)` 匹配时，将 `override_config_json` 合并到基础配置。同一模型的多条覆盖按 `sort_order` 依次应用。

---

### 4.14 `builtin_providers` — 内置供应商预设

对应参考项目 `well-known/providers.ts` 中的 `WELL_KNOWN_PROVIDERS`。存储“从内置模型列表中添加”时可一键选择的供应商预设。认证方式 ID 列表拆到 §4.15。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PK | 供应商标识符，如 `openai` / `google-ai-studio` / `anthropic` |
| `display_name` | TEXT | NOT NULL | 展示名称，如 `Open AI` |
| `category` | TEXT | NOT NULL | UI 分组，如 `General` / `Experimental` |
| `provider_type` | TEXT | NOT NULL | 协议类型，见 §5.1 |
| `base_url` | TEXT | NOT NULL | 默认 API Base URL |
| `use_raw_base_url` | INTEGER | DEFAULT 0 | 是否原样使用 base URL |
| `default_auth_json` | TEXT | | 默认 `AuthConfig` JSON（可选） |
| `default_balance_provider_json` | TEXT | | 默认 `BalanceConfig` JSON（可选） |
| `extra_headers_json` | TEXT | | 默认附加请求头（种子阶段可保留 JSON，运行时映射到 `provider_extra_headers`） |
| `extra_body_json` | TEXT | | 默认附加请求体（同上） |
| `auto_fetch_official_models` | INTEGER | DEFAULT 0 | 是否默认开启官方模型自动拉取 |
| `sort_order` | INTEGER | DEFAULT 0 | 排序优先级 |
| `created_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE builtin_providers (
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

CREATE INDEX idx_builtin_providers_category ON builtin_providers(category, sort_order);
```

---

### 4.15 `builtin_provider_auth_types` — 内置供应商支持的认证方式

将 `builtin_providers.auth_types_json` 字符串数组拆分为行式关联表，支持 UI 按认证方式筛选和设置默认首选认证。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `builtin_provider_id` | TEXT | PK, FK | → `builtin_providers.id` |
| `auth_method` | TEXT | PK | 见 §5.4，如 `api-key` / `oauth2` |
| `is_default` | INTEGER | DEFAULT 0 | 是否该供应商的默认认证方式 |
| `sort_order` | INTEGER | DEFAULT 0 | |
| `created_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE builtin_provider_auth_types (
  builtin_provider_id TEXT NOT NULL REFERENCES builtin_providers(id) ON DELETE CASCADE,
  auth_method TEXT NOT NULL,
  is_default INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  PRIMARY KEY(builtin_provider_id, auth_method)
);
```

---

### 4.16 `builtin_provider_models` — 内置供应商 × 内置模型关联

对应 `WELL_KNOWN_PROVIDERS[i].models` 数组。一个内置供应商预设默认关联一组内置模型，用户在“从内置模型列表中添加”时按供应商筛选模型。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `builtin_provider_id` | TEXT | PK, FK | → `builtin_providers.id` |
| `builtin_model_id` | TEXT | PK, FK | → `builtin_models.id` |
| `declared_model_id` | TEXT | | 供应商声明的模型 ID 别名 |
| `sort_order` | INTEGER | DEFAULT 0 | |
| `created_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE builtin_provider_models (
  builtin_provider_id TEXT NOT NULL REFERENCES builtin_providers(id) ON DELETE CASCADE,
  builtin_model_id TEXT NOT NULL REFERENCES builtin_models(id) ON DELETE CASCADE,
  declared_model_id TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  PRIMARY KEY(builtin_provider_id, builtin_model_id)
);

CREATE INDEX idx_builtin_provider_models_provider ON builtin_provider_models(builtin_provider_id, sort_order);
```

---

### 4.17 `builtin_model_aliases` — 内置模型别名

规范化内置模型覆盖配置中的字符串别名。这些别名用于在官方 API 返回的模型 ID 与内置模型之间建立匹配关系（如 `claude-3.5-sonnet` 是 `claude-3-5-sonnet` 的别名）。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `builtin_model_id` | TEXT | PK, FK | → `builtin_models.id` |
| `alias` | TEXT | PK | 替代模型 ID |
| `created_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE builtin_model_aliases (
  builtin_model_id TEXT NOT NULL REFERENCES builtin_models(id) ON DELETE CASCADE,
  alias TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY(builtin_model_id, alias)
);
```

---

### 4.18 `cli_profiles` — CLI 配置档案

每个受管 CLI（Claude Code、Codex、Gemini CLI 等）一条记录。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PK | UUID |
| `slug` | TEXT | UNIQUE NOT NULL | 如 `claude-code` |
| `display_name` | TEXT | NOT NULL | |
| `cli_type` | TEXT | NOT NULL | 见 §5.2 |
| `config_file_path` | TEXT | | CLI 实际配置文件路径 |
| `proxy_json` | TEXT | | CLI 级代理 |
| `is_enabled` | INTEGER | NOT NULL DEFAULT 1 | |
| `created_at` | TEXT | NOT NULL | |
| `updated_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE cli_profiles (
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
```

---

### 4.19 `cli_providers` — CLI 绑定的供应商

CLI 可从 Gateway 列表选择供应商；支持路由模式。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PK | UUID |
| `cli_profile_id` | TEXT | FK NOT NULL | |
| `provider_id` | TEXT | FK | 直连 Gateway 供应商；路由模式时必填 |
| `display_name` | TEXT | NOT NULL | CLI 内展示名 |
| `route_mode` | INTEGER | NOT NULL DEFAULT 0 | 1=走本地网关代理 |
| `gateway_base_url` | TEXT | | 路由模式下的网关地址，**为空时运行时从 `app_settings.gateway_host:gateway_port` 动态拼接** |
| `direct_base_url` | TEXT | | 非路由模式直连地址，`route_mode=0` 时必填 |
| `auth_json` | TEXT | | CLI 侧认证（可与 Gateway 分离） |
| `balance_json` | TEXT | | 额度展示缓存 `BalanceSnapshot` JSON，见 §5.10 |
| `sort_order` | INTEGER | DEFAULT 0 | |
| `is_default` | INTEGER | DEFAULT 0 | |
| `created_at` | TEXT | NOT NULL | |
| `updated_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE cli_providers (
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

CREATE INDEX idx_cli_providers_cli ON cli_providers(cli_profile_id, sort_order);
```

---

### 4.20 `cli_model_mappings` — CLI 模型映射

支持从列表选择或手动输入。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PK | UUID |
| `cli_provider_id` | TEXT | FK NOT NULL | |
| `cli_model_alias` | TEXT | NOT NULL | CLI 内使用的模型名 |
| `gateway_model_id` | TEXT | | 路由模式下 `slug/model_id` |
| `raw_model_id` | TEXT | | 非路由模式真实模型 ID |
| `input_mode` | TEXT | NOT NULL | `select` / `manual` |
| `created_at` | TEXT | NOT NULL | |
| `updated_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE cli_model_mappings (
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
```

---

### 4.21 `model_call_logs` — 模型调用记录

记录每个模型每次被调用的详细信息，用于用量统计、成本分析和缓存效率评估。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PK | UUID |
| `provider_id` | TEXT | FK NOT NULL | → `providers.id` |
| `gateway_model_id` | TEXT | FK | → `gateway_models.id`（可为空，直连模式） |
| `model_id` | TEXT | NOT NULL | 实际调用的模型 ID |
| `request_id` | TEXT | | 网关生成的唯一请求追踪 ID |
| `requested_at` | TEXT | NOT NULL | 请求发起时间 |
| `completed_at` | TEXT | | 响应完成时间 |
| `duration_ms` | INTEGER | | 请求耗时（毫秒） |
| `status_code` | INTEGER | | HTTP 状态码 |
| `error_message` | TEXT | | 错误信息（如有） |
| `prompt_tokens` | INTEGER | | 提示 Token 数 |
| `completion_tokens` | INTEGER | | 补全 Token 数 |
| `total_tokens` | INTEGER | | 总 Token 数 |
| `cached_tokens` | INTEGER | | 缓存命中 Token 数 |
| `cache_hit` | INTEGER | DEFAULT 0 | 是否命中缓存（1=是，0=否） |
| `route_mode` | INTEGER | | 路由模式：1=直连，2=虚拟供应商故障转移 |
| `source` | TEXT | DEFAULT 'gateway' | 请求入口：`cli` / `gateway` / `internal` |
| `time_to_first_token_ms` | INTEGER | | 流式响应首字延迟（毫秒） |
| `price_per_1m_tokens` | REAL | | 记录时单价快照（USD / 1M tokens） |

```sql
CREATE TABLE model_call_logs (
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
  price_per_1m_tokens REAL
);

CREATE INDEX idx_model_call_logs_provider ON model_call_logs(provider_id, requested_at);
CREATE INDEX idx_model_call_logs_model ON model_call_logs(model_id, requested_at);
CREATE INDEX idx_model_call_logs_requested ON model_call_logs(requested_at);
CREATE INDEX idx_model_call_logs_stats ON model_call_logs(provider_id, model_id, source, requested_at);
```

> 写入时机：由 `gateway-runtime` 的响应拦截器在每次请求完成后**异步写入**（通过后台队列批量提交，不阻塞响应返回）。`cached_tokens` 和 `cache_hit` 从响应头或响应体中提取；`source` 由调用上下文决定；`time_to_first_token_ms` 在流式响应首 chunk 到达时记录；`price_per_1m_tokens` 在写入时按模型定价快照填充，避免后续改价影响历史统计。
>
> 参考 `development.md §5.12`（call-records 模块）和 `development.md §5.13`（拦截器链）。
>
> 历史变更：
> - V004 迁移追加 `source`、`time_to_first_token_ms`、`price_per_1m_tokens` 三列及 `idx_model_call_logs_stats` 索引。

---

### 4.22 `gateway_settings` — 网关设置

单例表（`id = 'default'`），管理本地 AI Gateway 的监听地址、端口、默认 API Key 与启用状态。原 `app_settings` 中的 `gateway_host`、`gateway_port`、`gateway_api_key_secret_id` 字段已通过 V005 迁移至此表。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PK | 固定单例 `default` |
| `gateway_host` | TEXT | NOT NULL | 默认 `127.0.0.1`，支持 `0.0.0.0` |
| `gateway_port` | INTEGER | NOT NULL | 默认 `54321` |
| `default_api_key_secret_id` | TEXT | FK → secrets | 对外默认 API Key（内部 CLI 可豁免） |
| `is_enabled` | INTEGER | NOT NULL DEFAULT 1 | 网关是否启用 |
| `log_level` | TEXT | NOT NULL DEFAULT 'minimal' | 请求日志级别：`none` / `minimal` / `detailed` |
| `created_at` | TEXT | NOT NULL | |
| `updated_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE gateway_settings (
  id TEXT PRIMARY KEY DEFAULT 'default',
  gateway_host TEXT NOT NULL DEFAULT '127.0.0.1',
  gateway_port INTEGER NOT NULL DEFAULT 54321,
  default_api_key_secret_id TEXT,
  is_enabled INTEGER NOT NULL DEFAULT 1,
  log_level TEXT NOT NULL DEFAULT 'minimal',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

---

### 4.23 `gateway_auth_keys` — 网关认证 API Key

管理多个网关认证 API Key，支持启用/禁用、过期时间、排序与最近使用时间记录。运行时校验请求中的 `Authorization: Bearer {key}` 或 `X-API-Key: {key}`。

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PK | UUID |
| `name` | TEXT | NOT NULL | API Key 名称 |
| `description` | TEXT | | 描述 |
| `api_key_secret_id` | TEXT | FK → secrets | Secret 引用 |
| `is_enabled` | INTEGER | NOT NULL DEFAULT 1 | |
| `expires_at` | TEXT | | 过期时间，NULL 表示永不过期 |
| `sort_order` | INTEGER | NOT NULL DEFAULT 0 | |
| `last_used_at` | TEXT | | 最近使用时间 |
| `created_at` | TEXT | NOT NULL | |
| `updated_at` | TEXT | NOT NULL | |

```sql
CREATE TABLE gateway_auth_keys (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  api_key_secret_id TEXT,
  is_enabled INTEGER NOT NULL DEFAULT 1,
  expires_at TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  last_used_at TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

---

### 4.24 `log_settings` — 日志配置表

> 单行配置表，`id` 固定为 `'default'`，所有日志相关配置集中管理。

| 列名 | 类型 | 约束 | 默认值 | 说明 |
|------|------|------|--------|------|
| `id` | TEXT | PRIMARY KEY | `'default'` | 固定单行 |
| `buffer_size` | INTEGER | NOT NULL | 5000 | 内存缓冲队列大小（条数） |
| `log_dir` | TEXT | NOT NULL | `''` | 日志文件目录（空使用默认） |
| `max_retention_days` | INTEGER | NOT NULL | 30 | 日志文件保留天数 |
| `enable_file_persistence` | INTEGER | NOT NULL | 0 | 是否启用文件持久化（0=否，1=是） |
| `max_file_size_mb` | INTEGER | NOT NULL | 10 | 单个日志文件大小上限（MB） |
| `max_file_count` | INTEGER | NOT NULL | 7 | 保留的日志文件数量 |
| `file_log_level` | TEXT | | `'INFO'` | 文件写入级别阈值 |
| `enable_request_log` | INTEGER | NOT NULL | 0 | 是否记录转发请求体 |
| `enable_response_log` | INTEGER | NOT NULL | 0 | 是否记录转发响应体 |
| `forward_max_body_length` | INTEGER | NOT NULL | 4096 | 转发日志最大记录长度（字符） |
| `enable_command_log` | INTEGER | NOT NULL | 1 | 是否记录 Command 调用 |
| `enable_command_request_log` | INTEGER | NOT NULL | 0 | 是否记录 Command 请求参数 |
| `enable_command_response_log` | INTEGER | NOT NULL | 0 | 是否记录 Command 响应数据 |
| `command_max_body_length` | INTEGER | NOT NULL | 4096 | Command 日志最大记录长度（字符） |

### 设计要点

- **单行表**：全局只有一行配置，`id` 固定为 `'default'`
- **启动加载**：LoggerService 初始化时从 DB 读取配置，构建缓冲区和文件写入线程
- **运行时更新**：前端通过 `log_set_settings` Command 更新，同时写入 DB 和内存
- **布尔字段**：使用 INTEGER 0/1 表示，与 SQLite 惯例一致
- **迁移文件**：`V007__add_log_settings_table.sql`

---

### 4.25 `virtual_providers` — 虚拟供应商（故障转移组）

聚合多个真实供应商并按策略进行故障转移的虚拟层。对外表现为普通供应商，模型 ID 格式为 `{virtual_alias}/{model_id}`。

| 列名 | 类型 | 约束 | 默认值 | 说明 |
|------|------|------|--------|------|
| `id` | TEXT | PRIMARY KEY | | UUID |
| `name` | TEXT | NOT NULL UNIQUE | | 内部名称 |
| `alias` | TEXT | NOT NULL UNIQUE | | 对外路由别名，用于 model_id 前缀 |
| `display_name` | TEXT | | | 展示名称 |
| `is_enabled` | INTEGER | NOT NULL | 1 | 是否启用 |
| `strategy` | TEXT | NOT NULL | `'on_all'` | 路由策略：`fallback` / `on_all` / `load_balance` |
| `max_retries` | INTEGER | NOT NULL | 3 | 单条路由最大重试次数 |
| `retry_interval_ms` | INTEGER | NOT NULL | 1000 | 两次重试之间的间隔（毫秒） |
| `created_at` | TEXT | NOT NULL | | 创建时间 |
| `updated_at` | TEXT | NOT NULL | | 更新时间 |

```sql
CREATE TABLE virtual_providers (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  alias TEXT NOT NULL UNIQUE,
  display_name TEXT,
  is_enabled INTEGER NOT NULL DEFAULT 1,
  strategy TEXT NOT NULL DEFAULT 'on_all',
  max_retries INTEGER NOT NULL DEFAULT 3,
  retry_interval_ms INTEGER NOT NULL DEFAULT 1000,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

### 4.26 `virtual_models` — 虚拟供应商模型

虚拟供应商对外暴露的模型标识，背后通过 `virtual_model_routes` 映射到一组真实模型。

| 列名 | 类型 | 约束 | 默认值 | 说明 |
|------|------|------|--------|------|
| `id` | TEXT | PRIMARY KEY | | UUID |
| `virtual_provider_id` | TEXT | NOT NULL FK | | → `virtual_providers.id` |
| `model_id` | TEXT | NOT NULL | | 对外虚拟模型 ID |
| `display_name` | TEXT | | | 展示名称 |
| `is_enabled` | INTEGER | NOT NULL | 1 | 是否启用 |
| `created_at` | TEXT | NOT NULL | | 创建时间 |
| `updated_at` | TEXT | NOT NULL | | 更新时间 |

```sql
CREATE TABLE virtual_models (
  id TEXT PRIMARY KEY,
  virtual_provider_id TEXT NOT NULL REFERENCES virtual_providers(id) ON DELETE CASCADE,
  model_id TEXT NOT NULL,
  display_name TEXT,
  is_enabled INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(virtual_provider_id, model_id)
);
```

### 4.27 `virtual_model_routes` — 虚拟模型路由（故障转移目标）

一条从虚拟模型到真实供应商模型的映射。`fallback` 策略下按 `priority` 升序尝试；失败超过虚拟供应商级 `max_retries` 后该路由健康度降级。

| 列名 | 类型 | 约束 | 默认值 | 说明 |
|------|------|------|--------|------|
| `id` | TEXT | PRIMARY KEY | | UUID |
| `virtual_model_id` | TEXT | NOT NULL FK | | → `virtual_models.id` |
| `target_provider_id` | TEXT | NOT NULL FK | | → `providers.id` |
| `target_model_id` | TEXT | NOT NULL | | 目标真实模型 ID |
| `priority` | INTEGER | NOT NULL | 0 | 优先级，数值越小越优先 |
| `enabled` | INTEGER | NOT NULL | 1 | 是否启用 |
| `max_retries` | INTEGER | NOT NULL | 0 | 路由级最大重试次数（保留字段，网关优先使用虚拟供应商级配置） |
| `retry_interval_ms` | INTEGER | NOT NULL | 1000 | 路由级重试间隔（毫秒） |
| `timeout_ms` | INTEGER | | | 路由级超时（毫秒），为空表示不限制 |
| `is_healthy` | INTEGER | NOT NULL | 1 | 健康状态 |
| `last_healthy_at` | TEXT | | | 上次健康时间 |
| `extra_headers_json` | TEXT | | | 额外请求头 JSON |
| `extra_body_json` | TEXT | | | 额外请求体 JSON |
| `created_at` | TEXT | NOT NULL | | 创建时间 |
| `updated_at` | TEXT | NOT NULL | | 更新时间 |

```sql
CREATE TABLE virtual_model_routes (
  id TEXT PRIMARY KEY,
  virtual_model_id TEXT NOT NULL REFERENCES virtual_models(id) ON DELETE CASCADE,
  target_provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  target_model_id TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  enabled INTEGER NOT NULL DEFAULT 1,
  max_retries INTEGER NOT NULL DEFAULT 0,
  retry_interval_ms INTEGER NOT NULL DEFAULT 1000,
  timeout_ms INTEGER,
  is_healthy INTEGER NOT NULL DEFAULT 1,
  last_healthy_at TEXT,
  extra_headers_json TEXT,
  extra_body_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

### 4.28 `script_templates` — 额度监控脚本模板

用户自定义 Rhai 脚本，用于对接未内置的供应商额度接口。供应商通过 `balance_provider_json.method = "script"` + `scriptTemplateId` 引用；仅 `status = active` 可被正式额度刷新使用。

| 列名 | 类型 | 约束 | 默认值 | 说明 |
|------|------|------|--------|------|
| `id` | TEXT | PRIMARY KEY | | 雪花 ID |
| `name` | TEXT | NOT NULL | | 展示名称 |
| `slug` | TEXT | UNIQUE NOT NULL | | 稳定标识，如 `my-deepseek-balance` |
| `kind` | TEXT | NOT NULL | | 模板类型，本期固定 `balance` |
| `status` | TEXT | NOT NULL | `'draft'` | `draft` / `active` / `disabled` |
| `description` | TEXT | | | 说明 |
| `script_body` | TEXT | NOT NULL | `''` | Rhai 源码 |
| `engine` | TEXT | NOT NULL | `'rhai'` | 预留多引擎；本期仅 `rhai` |
| `default_timeout_ms` | INTEGER | NOT NULL | 15000 | 默认超时 |
| `allowed_hosts_json` | TEXT | | | JSON 字符串数组，额外允许 host |
| `snippet_id` | TEXT | | | 创建时选用的内置 snippet 标识 |
| `last_test_at` | TEXT | | | 最近试运行时间 ISO8601 |
| `last_test_ok` | INTEGER | | | 0/1 |
| `last_test_message` | TEXT | | | 最近试运行摘要（脱敏） |
| `sort_order` | INTEGER | NOT NULL | 0 | |
| `created_at` | TEXT | NOT NULL | | |
| `updated_at` | TEXT | NOT NULL | | |

```sql
CREATE TABLE IF NOT EXISTS script_templates (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  kind TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'draft',
  description TEXT,
  script_body TEXT NOT NULL DEFAULT '',
  engine TEXT NOT NULL DEFAULT 'rhai',
  default_timeout_ms INTEGER NOT NULL DEFAULT 15000,
  allowed_hosts_json TEXT,
  snippet_id TEXT,
  last_test_at TEXT,
  last_test_ok INTEGER,
  last_test_message TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

---

## 5. 枚举与 JSON Schema 引用

### 5.1 `provider_type`

继承参考项目 `ProviderType`：

```
anthropic                     # Anthropic Messages API
claude-code                   # Claude Code CLI 模式
google-ai-studio              # Google AI Studio (Gemini API)
google-vertex-ai              # Google Vertex AI
google-antigravity            # Google Antigravity
google-gemini-cli             # Google Gemini CLI
github-copilot                # GitHub Copilot
openai-chat-completion        # OpenAI Chat Completions API
openai-codex                  # OpenAI Codex
openai-responses              # OpenAI Responses API (新)
xai-grok-build                # xAI Grok Build
ollama                        # Ollama 本地
```

### 5.2 `cli_type`

```
claude-code | codex | gemini-cli | cursor-agent | custom
```

### 5.3 代理配置 JSON

i-code 的代理配置分**两层**，分别存于 `app_settings`（全局）与 `providers`（供应商级），
由 `src-tauri/src/modules/shared/mod.rs` 中的 `ProxyConfig` / `ProviderProxyConfig` 承载。
完整架构、决策矩阵、日志规范见 [`proxy.md`](./proxy.md)。

#### 5.3.1 全局代理 `ProxyConfig`（`app_settings.global_proxy_json`）

```json
{
  "type": "direct | system | http | socks",
  "url": "http://user:pass@127.0.0.1:7890",
  "noProxy": ["localhost", "127.0.0.1"]
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `type` | `'direct'` / `'system'` / `'http'` / `'socks'` | 是 | `direct`=直连；`system`=读取系统 `HTTP_PROXY`/`HTTPS_PROXY` 环境变量；`http`=HTTP 代理；`socks`=SOCKS5 代理 |
| `url` | `string` | 仅 `http` / `socks` | 代理 URL，可在 URL 中携带认证 `http://user:pass@host:port` 或 `socks5://user:pass@host:port` |
| `noProxy` | `string[]` | 否 | 绕过代理的主机白名单（`NO_PROXY` 等价物） |

> 全局代理是否生效由 `app_settings.global_proxy_enabled` 总开关控制：
> - `enabled = false` → **强制直连**（`no_proxy()`），不读取系统环境变量；
> - `enabled = true` → 按 `ProxyConfig.type` 应用。
>
> 「全局代理开关 = 应用级网络策略总开关」：供应商代理策略为 `global` 且全局开关关闭时，
> **回退直连**而非读取系统环境变量，避免代理工具残留环境变量导致直连可达的供应商也失败。

#### 5.3.2 供应商级代理 `ProviderProxyConfig`（`providers.proxy_json`）

```json
{
  "type": "global | direct | socks | http",
  "url": "socks5://127.0.0.1:1080"
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `type` | `'global'` / `'direct'` / `'socks'` / `'http'` | 是 | `global`=使用全局代理（开关关闭时回退直连）；`direct`=直连；`socks`=SOCKS5；`http`=HTTP |
| `url` | `string` | 仅 `socks` / `http` | 代理 URL，可含认证信息 |

> `providers.proxy_json` 字段缺失（`None`）等价于 `global`，与显式 `{"type":"global"}` 同义。
> 前端「全局代理」模式**始终序列化** `proxyJson`（含 `{"type":"global"}`），
> 确保从其他模式切换回 `global` 时能覆盖 DB 旧值（修复历史缺陷见 [`proxy.md`](./proxy.md) §4）。

### 5.4 `AuthConfig` JSON（完整）

认证方法为多态联合类型，由 `method` 字段区分。所有认证对象均可选包含 `label`（UI 标签）和 `description`（UI 描述）。

**`method: 'none'`** — 无认证
```json
{
  "method": "none",
  "label": "No Auth",
  "description": "No authentication required"
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `method` | `'none'` | 是 | 无认证 |
| `label` / `description` | `string` | 否 | UI 展示 |

**`method: 'api-key'`** — API Key 认证（最常用）
```json
{
  "method": "api-key",
  "label": "OpenAI",
  "description": "My OpenAI Key",
  "apiKey": "$SECRET:uuid$"
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `method` | `'api-key'` | 是 | 认证方式 |
| `label` / `description` | `string` | 否 | UI 展示 |
| `apiKey` | `string` | 否 | API Key 原文或 `$SECRET:{uuid}$` / `$UCPSECRET:{uuid}$` 引用 |

**`method: 'oauth2'`** — 通用 OAuth 2.0 认证
```json
{
  "method": "oauth2",
  "label": "Google",
  "description": "Google OAuth",
  "identityId": "usr_xxx",
  "token": "$SECRET:uuid$",
  "oauth": {
    "grantType": "authorization_code",
    "authorizationUrl": "https://accounts.google.com/o/oauth2/auth",
    "tokenUrl": "https://oauth2.googleapis.com/token",
    "clientId": "xxx.apps.googleusercontent.com",
    "clientSecret": "$SECRET:uuid$",
    "scopes": ["https://www.googleapis.com/auth/cloud-platform"],
    "pkce": true,
    "redirectUri": "http://127.0.0.1:port/callback"
  }
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `method` | `'oauth2'` | 是 | |
| `label` / `description` | `string` | 否 | UI 展示 |
| `identityId` | `string` | 否 | 已授权身份标识（运行时写入） |
| `token` | `string` | 否 | 持久化 token 或 secret 引用（运行时写入） |
| `oauth` | `object` | 是 | OAuth 端点配置 |

| `oauth` 子字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `grantType` | `'authorization_code'` / `'client_credentials'` / `'device_code'` | 是 | OAuth 2.0 授权类型 |
| `authorizationUrl` | `string` | 仅 `authorization_code` | 授权端点 |
| `tokenUrl` | `string` | 是 | Token 端点 |
| `deviceAuthorizationUrl` | `string` | 仅 `device_code` | 设备授权端点 |
| `clientId` | `string` | 除 device_code 可选外必填 | 客户端 ID |
| `clientSecret` | `string` | `client_credentials` 必填；其余可选 | 客户端 Secret（secret 引用） |
| `revocationUrl` | `string` | 否 | Token 吊销端点 |
| `scopes` | `string[]` | 否 | OAuth 作用域 |
| `pkce` | `boolean` | 否 | 是否启用 PKCE，默认 true |
| `redirectUri` | `string` | 否 | 回调 URI，未指定时自动生成 |

**`method: 'google-vertex-ai-auth'`** — Google Vertex AI 认证
```json
{
  "method": "google-vertex-ai-auth",
  "label": "Vertex AI",
  "subType": "service-account",
  "projectId": "my-project",
  "location": "us-central1",
  "keyFilePath": "/path/to/key.json"
}
```

| 子字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `subType` | `'adc'` / `'service-account'` / `'api-key'` | 是 | 认证子类型 |
| `projectId` | `string` | ADC 必填；service-account 可选 | GCP 项目 ID |
| `location` | `string` | ADC / service-account 必填 | GCP 区域，如 `us-central1` |
| `keyFilePath` | `string` | 仅 service-account | 服务账号 JSON 密钥文件路径 |
| `apiKey` | `string` | 仅 api-key | API Key 或 secret 引用 |

**`method: 'antigravity-oauth'`** — Google Antigravity OAuth
```json
{
  "method": "antigravity-oauth",
  "label": "Antigravity",
  "identityId": "usr_xxx",
  "token": "$SECRET:uuid$",
  "projectId": "my-project",
  "managedProjectId": "managed-xxx",
  "tier": "free",
  "tierId": "tier_xxx",
  "email": "user@example.com"
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `identityId` / `token` | `string` | 运行时身份/token |
| `projectId` | `string` | 用户提供的项目 ID（duetProject） |
| `managedProjectId` | `string` | Cloud Code Assist 托管项目 ID |
| `tier` | `'free'` / `'paid'` | 账户层级 |
| `tierId` | `string` | 精确层级标识 |
| `email` | `string` | 授权邮箱 |

**`method: 'google-gemini-oauth'`** — Google Gemini CLI OAuth
```json
{
  "method": "google-gemini-oauth",
  "label": "Gemini CLI",
  "identityId": "usr_xxx",
  "token": "$SECRET:uuid$",
  "projectId": "my-project",
  "oauthType": "code_assist",
  "managedProjectId": "managed-xxx",
  "tier": "free",
  "tierId": "tier_xxx",
  "email": "user@example.com"
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `identityId` / `token` | `string` | 运行时身份/token |
| `projectId` | `string` | 用户项目 ID（回退用） |
| `oauthType` | `'code_assist'` / `'ai_studio'` / `'google_one'` | Gemini OAuth 账户类型，默认 `code_assist` |
| `managedProjectId` / `tier` / `tierId` / `email` | | 同 Antigravity |

**`method: 'openai-codex'`** — OpenAI Codex 认证
```json
{
  "method": "openai-codex",
  "label": "OpenAI Codex",
  "identityId": "usr_xxx",
  "token": "$SECRET:uuid$",
  "accountId": "acc_xxx",
  "email": "user@example.com"
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `identityId` / `token` | `string` | 运行时身份/token |
| `accountId` | `string` | ChatGPT organization/subscription account ID（用于 `ChatGPT-Account-Id` header） |
| `email` | `string` | 授权邮箱 |

**`method: 'claude-code'`** — Claude Code 认证
```json
{
  "method": "claude-code",
  "label": "Claude Code",
  "identityId": "usr_xxx",
  "token": "$SECRET:uuid$",
  "email": "user@example.com"
}
```

**`method: 'xai-grok-oauth'`** — xAI Grok Build OAuth
```json
{
  "method": "xai-grok-oauth",
  "label": "xAI Grok",
  "identityId": "usr_xxx",
  "token": "$SECRET:uuid$",
  "email": "user@example.com"
}
```

**`method: 'github-copilot'`** — GitHub Copilot 认证
```json
{
  "method": "github-copilot",
  "label": "GitHub Copilot",
  "identityId": "usr_xxx",
  "token": "$SECRET:uuid$",
  "enterpriseUrl": "github.mycompany.com",
  "email": "user@example.com"
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `identityId` / `token` / `email` | `string` | 运行时身份/token/邮箱 |
| `enterpriseUrl` | `string` | 企业域名（含可选端口），未设置时默认为 `github.com` |

---

### 5.5 `ModelConfig` JSON（完整）

对应 `model_configs` 表及关联的 `model_config_extra_headers` / `model_config_extra_body`。标量字段（`name`、`maxOutputTokens`、`temperature` 等）已拆为 `model_configs` 列；复杂嵌套对象（`capabilities`、`thinking`、`multi-agent`、`webSearch`、`presetTemplates`）仍以 JSON 列存储；`extraHeaders` / `extraBody` 拆为键值对关联表。

```json
{
  "name": "GPT-4.1",
  "family": "gpt-4",
  "maxInputTokens": 128000,
  "maxOutputTokens": 64000,
  "tokenizer": "openai",
  "tokenCountMultiplier": 1.0,
  "stream": true,
  "temperature": 0.7,
  "topK": 40,
  "topP": 0.95,
  "frequencyPenalty": 0,
  "presencePenalty": 0,
  "parallelToolCalling": true,
  "serviceTier": "auto",
  "verbosity": "medium",
  "capabilities": { "toolCalling": true, "imageInput": true },
  "thinking": { "type": "enabled", "effort": "medium" },
  "multi-agent": { "enabled": false },
  "webSearch": { "enabled": false },
  "memoryTool": false,
  "extraHeaders": { "X-Custom-Header": "value" },
  "extraBody": { "reasoning_format": "parsed" },
  "presetTemplates": [ ]
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | `string` | 展示名称 |
| `family` | `string` | 模型族，如 `gpt-4`、`claude-3` |
| `maxInputTokens` | `number` | 最大输入/上下文 Token 数 |
| `maxOutputTokens` | `number` | 最大输出 Token 数 |
| `tokenizer` | `string` | Tokenizer ID，如 `'default'` / `'openai'` / `'deepseek'` / `'claude'` / `'char4'` / `'conservative'` 等 |
| `tokenCountMultiplier` | `number` | Token 计数乘数，默认 1.0 |
| `stream` | `boolean` | 是否流式响应 |
| `temperature` | `number` | 采样温度，通常 [0-2] |
| `topK` | `number` | Top-K 采样 |
| `topP` | `number` | Top-P 采样 [0-1] |
| `frequencyPenalty` | `number` | 频率惩罚，通常 [-2-2] |
| `presencePenalty` | `number` | 存在惩罚，通常 [-2-2] |
| `parallelToolCalling` | `boolean` | 是否并行工具调用 |
| `serviceTier` | `'auto'` / `'standard'` / `'flex'` / `'scale'` / `'priority'` | 服务层/处理层 |
| `verbosity` | `'low'` / `'medium'` / `'high'` | 响应详细程度 |
| `capabilities` | `object` | 模型能力，见 §5.6 |
| `thinking` | `object` | 思考/推理配置，见 §5.11 |
| `multi-agent` | `object` | 原生多 Agent 执行配置，见下表 |
| `webSearch` | `object` | 原生联网搜索配置，见下表 |
| `memoryTool` | `boolean` | 是否启用原生记忆工具 |
| `extraHeaders` | `Record<string,string>` | 附加 HTTP 请求头，运行时由 `model_config_extra_headers` 表组装 |
| `extraBody` | `Record<string,unknown>` | 附加请求体参数，运行时由 `model_config_extra_body` 表组装 |
| `presetTemplates` | `PresetTemplate[]` | 请求时预设模板，见 §5.14 |

**`multi-agent` 子字段**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `enabled` | `boolean` | 是否启用多 Agent 执行 |
| `maxConcurrentSubagents` | `number` | 最大并发子 Agent 数 |

**`webSearch` 子字段**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `enabled` | `boolean` | 是否启用原生联网搜索，默认 false |
| `maxUses` | `number` | 单次请求最大搜索次数 |
| `allowedDomains` | `string[]` | 仅允许搜索这些域名 |
| `blockedDomains` | `string[]` | 禁止搜索这些域名 |
| `userLocation` | `object` | 本地化搜索结果位置信息 |
| `userLocation.type` | `'approximate'` | 位置类型 |
| `userLocation.city` / `region` / `country` / `timezone` | `string` | 城市/地区/国家/时区 |

**`presetTemplates` 元素结构**（完整定义见 §5.14）：
```json
{
  "id": "reasoning-effort",
  "name": "Reasoning Effort",
  "default": "medium",
  "presets": [
    { "id": "high", "name": "High", "description": "High reasoning effort", "config": { "thinking": { "effort": "high" } } },
    { "id": "medium", "name": "Medium", "config": { "thinking": { "effort": "medium" } } }
  ]
}
```

### 5.6 `ModelCapabilities` JSON

对应 `providers` 下及 `ModelConfig.capabilities` 中的能力描述。

```json
{
  "toolCalling": true,
  "imageInput": false,
  "editTools": "find-replace"
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `toolCalling` | `boolean` / `number` | 是否支持工具/函数调用；`true` 表示支持，`false` 表示不支持，`number` 表示最大工具数 |
| `imageInput` | `boolean` | 是否支持图片/视觉输入 |
| `editTools` | `'find-replace'` / `'multi-find-replace'` / `'apply-patch'` / `'code-rewrite'` | 编辑器工具提示，供 IDE 选择代码编辑策略 |

### 5.7 `TimeoutConfig` JSON

```json
{
  "connection": 60000,
  "response": 300000
}
```

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `connection` | `number` | 60000 | TCP 连接超时（毫秒） |
| `response` | `number` | 300000 | SSE 流响应超时（毫秒），每次收到数据块后重置 |

### 5.8 `RetryConfig` JSON

```json
{
  "maxRetries": 3,
  "initialDelayMs": 1000,
  "maxDelayMs": 30000,
  "backoffMultiplier": 2,
  "jitterFactor": 0.1,
  "statusCodes": [408, 409, 429]
}
```

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `maxRetries` | `number` | 3 | 最大重试次数 |
| `initialDelayMs` | `number` | 1000 | 初始延迟（毫秒） |
| `maxDelayMs` | `number` | 30000 | 最大延迟（毫秒） |
| `backoffMultiplier` | `number` | 2 | 退避倍率 |
| `jitterFactor` | `number` | 0.1 | 抖动因子（0=无抖动，1=完全随机） |
| `statusCodes` | `number[]` | [408, 409, 429] | 触发重试的 HTTP 状态码 |

### 5.9 `ContextCacheConfig` JSON

```json
{
  "type": "only-free",
  "ttl": 300
}
```

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `type` | `'only-free'` / `'allow-paid'` | `'only-free'` | 缓存策略：仅免费 / 允许付费 |
| `ttl` | `number` | 300 | 缓存 TTL（秒） |

### 5.10 `BalanceConfig` JSON（额度监控）

多态联合类型，由 `method` 区分。常用方法：

| `method` 值 | 说明 | 额外字段 |
|------|------|------|
| `'none'` | 不监控 | 无 |
| `'moonshot-ai'` | Moonshot AI | 无 |
| `'kimi-code'` | Kimi Code | 无 |
| `'newapi'` | New API | `userId?`, `systemToken?`, `quotaTransform?` |
| `'deepseek'` | DeepSeek | 无 |
| `'openrouter'` | OpenRouter | 无 |
| `'siliconflow'` | SiliconFlow | 无 |
| `'aihubmix'` | AIHubMix | 无 |
| `'claude-relay-service'` | Claude Relay | `baseUrl?` |
| `'antigravity'` | Antigravity | 无 |
| `'gemini-cli'` | Gemini CLI | 无 |
| `'codex'` | Codex | 无 |
| `'synthetic'` | Synthetic | 无 |
| `'minimax'` | MiniMax | 无 |
| `'script'` | 自定义 Rhai 脚本 | `scriptTemplateId`, `timeoutMs?`, `allowedHosts?` |

`script` 方法扩展字段示例：
```json
{
  "method": "script",
  "scriptTemplateId": "550e8400-e29b-41d4-a716-446655440000",
  "timeoutMs": 12000
}
```

| `script` 子字段 | 类型 | 说明 |
|------|------|------|
| `scriptTemplateId` | `string` | 引用 `script_templates.id`；正式刷新要求模板 `status=active` |
| `timeoutMs` | `number` | 可选，单次查询超时（毫秒），默认取模板 `default_timeout_ms` |
| `allowedHosts` | `string[]` | 可选，额外允许的 HTTP host 白名单 |

`newapi` 方法扩展字段示例：
```json
{
  "method": "newapi",
  "userId": "usr_xxx",
  "systemToken": "$SECRET:uuid$",
  "quotaTransform": {
    "quotaField": "quota",
    "extraQuotaFields": ["bonus_quota"],
    "divisor": 500000,
    "multiplier": 1
  }
}
```

| `newapi` 子字段 | 类型 | 说明 |
|------|------|------|
| `userId` | `string` | 可选用户 ID，用于查询账户级额度 |
| `systemToken` | `string` | 可选系统 token（明文或 secret 引用） |
| `quotaTransform.quotaField` | `string` | 主额度字段名，默认 `quota` |
| `quotaTransform.extraQuotaFields` | `string[]` | 额外累加额度字段名 |
| `quotaTransform.divisor` | `number` | 原始额度转换除数，默认 `500000` |
| `quotaTransform.multiplier` | `number` | 除法后乘数，默认 `1` |

`claude-relay-service` 方法扩展字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| `baseUrl` | `string` | 可选自定义 Claude Relay Service `apiStats` API 地址 |

---

#### 额度快照 `BalanceSnapshot` JSON

`cli_providers.balance_json` 及运行时额度缓存使用以下结构：

```json
{
  "updatedAt": 1720000000000,
  "items": [
    { "id": "balance", "type": "amount", "period": "current", "direction": "remaining", "value": 12.34, "currencySymbol": "$", "primary": true },
    { "id": "tokens", "type": "token", "period": "month", "used": 1000, "limit": 5000, "remaining": 4000 },
    { "id": "expires", "type": "time", "kind": "expiresAt", "value": "2026-08-01T00:00:00Z", "timestampMs": 1759267200000 },
    { "id": "status", "type": "status", "period": "current", "value": "ok", "message": "Account active" }
  ]
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `updatedAt` | `number` | 快照更新时间戳（毫秒） |
| `items` | `BalanceMetric[]` | 额度指标数组 |

**`BalanceMetric` 联合类型**：

| 子类型 | 字段 | 说明 |
|------|------|------|
| `amount` | `direction`, `value`, `currencySymbol?` | 金额类指标：`remaining` / `used` / `limit` |
| `integer` | `direction`, `value` | 整数类指标 |
| `token` | `used?`, `limit?`, `remaining?` | Token 用量指标 |
| `percent` | `value`, `basis?` | 百分比指标：`remaining` / `used` |
| `time` | `kind`, `value`, `timestampMs?` | 时间指标：`expiresAt` / `resetAt` |
| `status` | `value`, `message?` | 状态指标：`ok` / `unlimited` / `exhausted` / `error` / `unavailable` |

公共基座字段：`id`, `type`, `period`, `periodLabel?`, `scope?`, `primary?`, `label?`。

### 5.11 `ThinkingConfig` JSON（思考/推理配置）

```json
{
  "type": "enabled",
  "budgetTokens": 16000,
  "effort": "high",
  "summary": "auto",
  "mode": "standard",
  "context": "all_turns"
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `type` | `'enabled'` / `'disabled'` / `'auto'` | 思考模式 |
| `budgetTokens` | `number` | 思考 Token 预算上限 |
| `effort` | `'max'` / `'xhigh'` / `'high'` / `'medium'` / `'low'` / `'minimal'` / `'none'` | 思考努力程度 |
| `summary` | `'none'` / `'auto'` / `'concise'` / `'detailed'` | 思考摘要级别 |
| `mode` | `'standard'` / `'pro'` | 推理模式 |
| `context` | `'auto'` / `'current_turn'` / `'all_turns'` | 推理上下文保留策略 |

### 5.12 `ExtraHeaders` / `ExtraBody` 存储

`providers` 与 `model_configs` 的附加头/体均已拆分为键值对关联表：

- 供应商级：`provider_extra_headers` / `provider_extra_body`
- 模型级：`model_config_extra_headers` / `model_config_extra_body`

运行时按 `(provider_id → model_config_id)` 两级合并：模型级同名 key 覆盖供应商级。

```json
// 运行时内存对象形态
{
  "extraHeaders": {
    "X-Custom-Header": "value",
    "X-API-Version": "2024-01"
  },
  "extraBody": {
    "reasoning_format": "parsed",
    "custom_param": 42
  }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `extraHeaders` | `Record<string, string>` | 附加 HTTP 请求头；值支持 `$SECRET:{uuid}$` |
| `extraBody` | `Record<string, unknown>` | 附加请求体参数；数据库中以 JSON 文本存储 |

### 5.13 `WellKnownModelOverride` JSON（内置模型覆盖）

内置模型可针对特定供应商类型提供配置覆盖。对象形式覆盖已拆分为 `builtin_model_overrides` 表；字符串形式别名已规范化到 `builtin_model_aliases` 表。

```json
[
  {
    "matchers": [
      { "type": "openai-responses" },
      { "name": "My Custom Provider" }
    ],
    "config": {
      "id": "gpt-4.1-custom",
      "maxOutputTokens": 32000,
      "capabilities": { "toolCalling": false }
    }
  },
  "gpt-4.1-alt-id"
]
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `matchers` | `array` | 匹配条件数组（`{ type?: string, name?: string, pattern?: string }`），任一匹配即生效；映射到 `builtin_model_overrides.matcher_type` / `matcher_value` |
| `config` | `object` | 覆盖的 `ModelConfig` 字段（含 `id`）；映射到 `builtin_model_overrides.override_config_json` |
| 字符串元素 | `string` | 简写：表示替代模型 ID 别名；映射到 `builtin_model_aliases.alias` |

---

### 5.14 `PresetTemplate` JSON（预设模板）

`ModelConfig.presetTemplates` 中每个元素的完整结构。

```json
{
  "id": "reasoning-effort",
  "name": "Reasoning Effort",
  "default": "medium",
  "presets": [
    {
      "id": "high",
      "name": "High",
      "description": "Use high reasoning effort",
      "config": {
        "thinking": { "effort": "high" }
      }
    },
    {
      "id": "medium",
      "name": "Medium",
      "config": {
        "thinking": { "effort": "medium" }
      }
    }
  ]
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id` | `string` | 是 | 模板唯一标识，如 `reasoning-effort`、`reasoning-mode`、`thinking-mode` |
| `name` | `string` | 是 | UI 展示名称 |
| `default` | `string` | 是 | 默认选中的 preset ID |
| `presets` | `PresetTemplatePreset[]` | 是 | 可选预设项列表 |

**`presets` 元素字段**：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id` | `string` | 是 | 预设项唯一标识 |
| `name` | `string` | 是 | UI 展示名称 |
| `description` | `string` | 否 | UI 说明 |
| `config` | `PresetTemplateOverrideConfig` | 是 | 该预设项覆盖的模型配置字段 |

**`config` 可覆盖字段**（`PresetTemplateOverrideConfig`）：

| 字段 | 类型 | 说明 |
|------|------|------|
| `maxOutputTokens` | `number` | 最大输出 Token 数 |
| `stream` | `boolean` | 是否流式 |
| `temperature` / `topK` / `topP` | `number` | 采样参数 |
| `frequencyPenalty` / `presencePenalty` | `number` | 惩罚参数 |
| `parallelToolCalling` | `boolean` | 是否并行工具调用 |
| `serviceTier` | `ServiceTier` | 服务层 |
| `verbosity` | `'low'` / `'medium'` / `'high'` | 响应详细程度 |
| `thinking` | `Partial<ThinkingConfig>` | 思考配置（合并策略，独立生效） |
| `webSearch` | `object` | 联网搜索配置 |
| `memoryTool` | `boolean` | 是否启用记忆工具 |
| `extraHeaders` | `Record<string,string>` | 附加请求头 |
| `extraBody` | `Record<string,unknown>` | 附加请求体参数 |

> 多个 `PresetTemplate` 同时生效时，非 `thinking` 字段按模板顺序覆盖；`thinking` 字段采用子集合并策略，使不同思考维度模板可以组合。

### 5.15 `TitleBarInfoConfig` JSON（标题栏信息配置）

控制自定义标题栏中间区域展示的信息项，存储于 `app_settings.titlebar_info_json`。

```json
{
  "showTokens": true,
  "showRpm": true,
  "showLatency": false,
  "showMemory": true,
  "showGatewayStatus": true
}
```

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `showTokens` | `boolean` | `true` | 展示近 1 小时 Token 消耗总数 |
| `showRpm` | `boolean` | `true` | 展示近 1 分钟请求数（RPM） |
| `showLatency` | `boolean` | `false` | 展示近 1 分钟平均请求延迟 |
| `showMemory` | `boolean` | `true` | 展示应用进程内存占用 |
| `showGatewayStatus` | `boolean` | `true` | 展示本地网关运行状态 |

> 标题栏中间大胶囊最多同时展示 3 项，启用项超过 3 个时按优先级取舍：网关状态 > 内存 > Tokens > RPM > Latency。
> 该配置由 V009 迁移添加，对应迁移文件 `V009__add_titlebar_info_config.sql`。

---

## 6. 核心业务流程

### 6.1 供应商 CRUD

```
CREATE provider → optional link secret → insert gateway_models
UPDATE provider → invalidate official_model_cache
DELETE provider → CASCADE gateway_models / cli_providers.provider_id SET NULL
```

### 6.2 模型暴露策略

网关 `/v1/models` 仅返回：

```sql
SELECT p.slug, m.model_id, m.display_name
FROM gateway_models m
JOIN providers p ON p.id = m.provider_id
WHERE p.is_enabled = 1 AND m.is_exposed = 1;
```

### 6.3 CLI 路由模式

```
route_mode = 1:
  CLI base_url = http://{gateway_host}:{gateway_port}
  model = {provider_slug}/{model_id}
  网关拆分后查 providers + gateway_models 转发

route_mode = 0:
  CLI 直连 cli_providers.direct_base_url
  model = cli_model_mappings.raw_model_id
```

### 6.4 从内置模型列表添加

对应参考项目“从内置模型列表中添加”功能：

```
1. 用户选择内置供应商预设 → builtin_providers
2. 查询该供应商推荐的内置模型 → builtin_provider_models JOIN builtin_models
3. 用户勾选模型 → 按供应商类型匹配 builtin_model_overrides
4. 生成 model_configs 记录：
   - 复制 builtin_models 的标量列与 capabilities_json / thinking_json 等
   - 合并匹配的 builtin_model_overrides.override_config_json
   - 若存在 declared_model_id，将其写入 gateway_models.model_id
5. 生成 gateway_models 记录：
   - model_id = 若存在 declared_model_id 则使用，否则 builtin_models.id
   - model_config_id = 上一步生成的 model_configs.id
   - source = 'builtin'
6. 插入 gateway_models 并对外暴露
```

内置模型匹配官方 API 返回模型 ID 时，优先使用 `builtin_model_aliases.alias` 和 `builtin_models.id` 做包含/精确匹配。

---

## 7. 迁移与版本

### 7.1 Schema 版本表

```sql
CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);
```

### 7.2 初始迁移 `V1__init.sql`

包含本文 §4 全部 `CREATE TABLE` 与索引，并插入默认数据：

```sql
-- 应用默认设置
INSERT INTO app_settings (id, theme, locale, gateway_host, gateway_port, store_secrets_in_keychain, created_at, updated_at)
VALUES ('default', 'dark', 'zh-CN', '127.0.0.1', 8787, 1, datetime('now'), datetime('now'));

-- 内置模型列表（来源：参考项目 well-known/models.ts 中的 WELL_KNOWN_MODELS）
-- INSERT INTO builtin_models (id, display_name, family, provider_type, max_input_tokens, max_output_tokens, tokenizer, token_count_multiplier, stream, temperature, top_k, top_p, frequency_penalty, presence_penalty, parallel_tool_calling, service_tier, verbosity, capabilities_json, thinking_json, multi_agent_json, web_search_json, memory_tool, preset_templates_json, sort_order, created_at)
-- VALUES (...);

-- 内置供应商预设（来源：参考项目 well-known/providers.ts 中的 WELL_KNOWN_PROVIDERS）
-- INSERT INTO builtin_providers (id, display_name, category, provider_type, base_url, use_raw_base_url, default_auth_json, default_balance_provider_json, extra_headers_json, extra_body_json, auto_fetch_official_models, sort_order, created_at)
-- VALUES (...);

-- 内置供应商支持的认证方式
-- INSERT INTO builtin_provider_auth_types (builtin_provider_id, auth_method, is_default, sort_order, created_at)
-- VALUES (...);

-- 内置供应商推荐模型
-- INSERT INTO builtin_provider_models (builtin_provider_id, builtin_model_id, declared_model_id, sort_order, created_at)
-- VALUES (...);

-- 内置模型别名（来源：well-known/models.ts 中的字符串别名）
-- INSERT INTO builtin_model_aliases (builtin_model_id, alias, created_at)
-- VALUES (...);

-- 内置模型对象覆盖配置（来源：well-known/models.ts 中的对象覆盖项）
-- INSERT INTO builtin_model_overrides (id, builtin_model_id, matcher_type, matcher_value, override_config_json, sort_order, created_at)
-- VALUES (...);

-- 内置模型适配的供应商类型（可按 capabilities/provider_type 推导，也可显式种子化）
-- INSERT INTO builtin_model_providers (builtin_model_id, provider_type, created_at)
-- VALUES (...);

INSERT INTO schema_migrations (version, applied_at) VALUES (1, datetime('now'));
```

> 内置数据（`builtin_*` 表）建议由构建脚本从参考项目的 `well-known/models.ts` 与 `well-known/providers.ts` 自动导出并生成种子 SQL，避免人工维护大段模型清单。

---

## 8. 索引与性能建议

| 场景 | 建议 |
|------|------|
| 网关高频查模型 | `idx_gateway_models_provider` + `idx_gateway_models_config`；应用层内存缓存 `Map<gateway_model_id, ModelConfig>` |
| 模型配置组装 | 启动时或首次访问时 JOIN `model_configs` + `model_config_extra_headers` + `model_config_extra_body`，避免运行时反复解析 JSON |
| 供应商附加头/体 | 按 `provider_id` 批量读取 `provider_extra_headers` / `provider_extra_body`，应用层合并为对象 |
| 内置模型列表加载 | `idx_builtin_providers_category` + `idx_builtin_provider_models_provider`；启动时全量载入内存 |
| 内置模型覆盖匹配 | `idx_builtin_model_overrides_model`；按 `builtin_model_id` 预加载并按 `matcher_type` 分组 |
| 内置模型别名匹配 | `builtin_model_aliases` 按 `alias` 建立索引或启动时构建 `Map<alias, builtin_model_id>` |
| 认证方式筛选 | `builtin_provider_auth_types` 按 `auth_method` 建立索引，支持 UI 按认证方式过滤供应商 |
| 官方模型刷新 | `official_model_cache.expires_at` 做 TTL |
| 密钥解析 | 启动时预热 `secrets` LRU 缓存 |

---

## 9. 文件位置（规划）

| 路径 | 说明 |
|------|------|
| `{app_data}/i-code/i-code.db` | SQLite 主库 |
| `src-tauri/src/db/migrations/` | Rust 侧迁移 SQL |
| `src-tauri/src/db/repositories/` | 按模块划分的仓储层 |
| `src/modules/*/types.ts` | 前端共享类型（与 DB JSON 字段对齐） |

---

## 10. 后续扩展（非 v0.1 范围）

- 配置导入/导出（参考 `provider-ops.ts` 冲突解决）
- 余额定时刷新任务表 `balance_snapshots`
- 多用户 / 云同步
