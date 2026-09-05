//! # 数据库表名与 schema 版本常量
//!
//! 所有业务模块通过 [`TABLE_NAMES`] 查询表名，避免硬编码字符串。
//! 新增表时在此处追加常量，并在 [`TABLE_NAMES`] 数组中登记。

#![allow(dead_code)]

/// 当前 schema 版本号
///
/// 与 `src-tauri/src/db/migrations/V{n}__*.sql` 文件名中的 `{n}` 对应。
/// 每次新增增量迁移时同步更新。
pub const SCHEMA_VERSION: u32 = 11;

/// 所有业务表名常量
///
/// 命名规范：`TABLE_*` 全大写 + 下划线。
/// 与 `docs/database.md` §4 中的表名保持一致。
pub mod table {
    /// 应用全局设置单例表
    pub const APP_SETTINGS: &str = "app_settings";
    /// 全局配置键值表
    pub const GLOBAL_CONFIGS: &str = "global_configs";
    /// 敏感凭据表（加密存储）
    pub const SECRETS: &str = "secrets";
    /// AI Gateway 供应商表
    pub const PROVIDERS: &str = "providers";
    /// 供应商级附加请求头
    pub const PROVIDER_EXTRA_HEADERS: &str = "provider_extra_headers";
    /// 供应商级附加请求体参数
    pub const PROVIDER_EXTRA_BODY: &str = "provider_extra_body";
    /// 模型完整配置表
    pub const MODEL_CONFIGS: &str = "model_configs";
    /// 模型级附加请求头
    pub const MODEL_CONFIG_EXTRA_HEADERS: &str = "model_config_extra_headers";
    /// 模型级附加请求体参数
    pub const MODEL_CONFIG_EXTRA_BODY: &str = "model_config_extra_body";
    /// 网关暴露模型表
    pub const GATEWAY_MODELS: &str = "gateway_models";
    /// 官方模型拉取缓存
    pub const OFFICIAL_MODEL_CACHE: &str = "official_model_cache";
    /// 内置模型列表（种子数据）
    pub const BUILTIN_MODELS: &str = "builtin_models";
    /// 内置模型 × 供应商类型关联
    pub const BUILTIN_MODEL_PROVIDERS: &str = "builtin_model_providers";
    /// 内置模型覆盖配置
    pub const BUILTIN_MODEL_OVERRIDES: &str = "builtin_model_overrides";
    /// 内置供应商预设
    pub const BUILTIN_PROVIDERS: &str = "builtin_providers";
    /// 内置供应商支持的认证方式
    pub const BUILTIN_PROVIDER_AUTH_TYPES: &str = "builtin_provider_auth_types";
    /// 内置供应商推荐模型关联
    pub const BUILTIN_PROVIDER_MODELS: &str = "builtin_provider_models";
    /// 内置模型别名
    pub const BUILTIN_MODEL_ALIASES: &str = "builtin_model_aliases";
    /// CLI 配置档案表
    pub const CLI_PROFILES: &str = "cli_profiles";
    /// CLI 绑定的供应商
    pub const CLI_PROVIDERS: &str = "cli_providers";
    /// CLI 模型映射
    pub const CLI_MODEL_MAPPINGS: &str = "cli_model_mappings";
    /// 模型调用统计（按天聚合）
    pub const MODEL_CALL_STATS_DAILY: &str = "model_call_stats_daily";
    /// 模型调用统计（按小时聚合）
    pub const MODEL_CALL_STATS_HOURLY: &str = "model_call_stats_hourly";
    /// 模型调用记录（用量统计）
    pub const MODEL_CALL_LOGS: &str = "model_call_logs";
    /// 虚拟供应商表
    pub const VIRTUAL_PROVIDERS: &str = "virtual_providers";
    /// 虚拟模型表
    pub const VIRTUAL_MODELS: &str = "virtual_models";
    /// 虚拟模型路由表
    pub const VIRTUAL_MODEL_ROUTES: &str = "virtual_model_routes";
    /// 虚拟路由尝试历史表
    pub const VIRTUAL_ROUTE_ATTEMPTS: &str = "virtual_route_attempts";
    /// 额度监控脚本模板表
    pub const SCRIPT_TEMPLATES: &str = "script_templates";
    /// 图像生成历史表
    pub const MEDIA_GENERATIONS: &str = "media_generations";
    /// 视频生成任务表
    pub const MEDIA_VIDEO_TASKS: &str = "media_video_tasks";
    /// 迁移版本记录表
    pub const SCHEMA_MIGRATIONS: &str = "schema_migrations";
    /// 网关设置单例表
    pub const GATEWAY_SETTINGS: &str = "gateway_settings";
    /// 网关认证 API Key 表
    pub const GATEWAY_AUTH_KEYS: &str = "gateway_auth_keys";
    /// 日志配置单例表
    pub const LOG_SETTINGS: &str = "log_settings";
    /// WebDAV 配置表
    pub const WEBDAV_CONFIGS: &str = "webdav_configs";
}

/// 所有表名集合，用于备份模块遍历与 schema 一致性校验
pub const TABLE_NAMES: &[&str] = &[
    table::APP_SETTINGS,
    table::GLOBAL_CONFIGS,
    table::SECRETS,
    table::PROVIDERS,
    table::PROVIDER_EXTRA_HEADERS,
    table::PROVIDER_EXTRA_BODY,
    table::MODEL_CONFIGS,
    table::MODEL_CONFIG_EXTRA_HEADERS,
    table::MODEL_CONFIG_EXTRA_BODY,
    table::GATEWAY_MODELS,
    table::OFFICIAL_MODEL_CACHE,
    table::BUILTIN_MODELS,
    table::BUILTIN_MODEL_PROVIDERS,
    table::BUILTIN_MODEL_OVERRIDES,
    table::BUILTIN_PROVIDERS,
    table::BUILTIN_PROVIDER_AUTH_TYPES,
    table::BUILTIN_PROVIDER_MODELS,
    table::BUILTIN_MODEL_ALIASES,
    table::CLI_PROFILES,
    table::CLI_PROVIDERS,
    table::CLI_MODEL_MAPPINGS,
    table::MODEL_CALL_LOGS,
    table::VIRTUAL_PROVIDERS,
    table::VIRTUAL_MODELS,
    table::VIRTUAL_MODEL_ROUTES,
    table::VIRTUAL_ROUTE_ATTEMPTS,
    table::SCRIPT_TEMPLATES,
    table::MEDIA_GENERATIONS,
    table::MEDIA_VIDEO_TASKS,
    table::SCHEMA_MIGRATIONS,
    table::GATEWAY_SETTINGS,
    table::GATEWAY_AUTH_KEYS,
    table::LOG_SETTINGS,
    table::WEBDAV_CONFIGS,
];
