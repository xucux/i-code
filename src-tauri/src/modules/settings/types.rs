//! # 应用设置模块类型定义
//!
//! 与前端 `src/modules/settings/types.ts` 对齐。
//! 序列化字段使用 camelCase，确保前后端 JSON 字段名一致。

use serde::{Deserialize, Serialize};

use crate::modules::backup::types::BackupSettings;
use crate::modules::shared::{ProxyConfig, RetryConfig, TimeoutConfig};

/// 主题枚举
///
/// 对应 `app_settings.theme` 列，支持 8 种主题：
/// - `light` / `dark`：标准浅色/深色
/// - `claude-light` / `claude-dark`：Claude 风格
/// - `deepseek-light` / `deepseek-dark`：DeepSeek 风格
/// - `nvidia-light` / `nvidia-dark`：NVIDIA 风格
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    Light,
    Dark,
    ClaudeLight,
    ClaudeDark,
    DeepseekLight,
    DeepseekDark,
    NvidiaLight,
    NvidiaDark,
}

impl Theme {
    /// 从字符串解析为 Theme；未知值返回 None
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "claude-light" => Some(Self::ClaudeLight),
            "claude-dark" => Some(Self::ClaudeDark),
            "deepseek-light" => Some(Self::DeepseekLight),
            "deepseek-dark" => Some(Self::DeepseekDark),
            "nvidia-light" => Some(Self::NvidiaLight),
            "nvidia-dark" => Some(Self::NvidiaDark),
            _ => None,
        }
    }

    /// 转换为数据库存储的字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::ClaudeLight => "claude-light",
            Self::ClaudeDark => "claude-dark",
            Self::DeepseekLight => "deepseek-light",
            Self::DeepseekDark => "deepseek-dark",
            Self::NvidiaLight => "nvidia-light",
            Self::NvidiaDark => "nvidia-dark",
        }
    }
}

/// 语言枚举
///
/// 对应 `app_settings.locale` 列。
/// 新增语言时需同步 `src/i18n/locales/` 下的翻译文件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locale {
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "zh-TW")]
    ZhTw,
    #[serde(rename = "en")]
    En,
    #[serde(rename = "ja")]
    Ja,
}

impl Locale {
    /// 从字符串解析为 Locale；未知值返回 None
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "zh-CN" => Some(Self::ZhCn),
            "zh-TW" => Some(Self::ZhTw),
            "en" => Some(Self::En),
            "ja" => Some(Self::Ja),
            _ => None,
        }
    }

    /// 转换为数据库存储的字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::ZhTw => "zh-TW",
            Self::En => "en",
            Self::Ja => "ja",
        }
    }
}

/// 全局日志级别枚举
///
/// 对应 `app_settings.log_level` 列，控制 tauri-plugin-log 的输出级别。
/// 默认 `Info`，可在设置页面实时调整。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// 从字符串解析；未知值返回 None
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "trace" => Some(Self::Trace),
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" => Some(Self::Warn),
            "error" => Some(Self::Error),
            _ => None,
        }
    }

    /// 转换为数据库存储的字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    /// 转换为 log crate 的 LevelFilter
    ///
    /// 保留供其他场景使用（如直接调用 `log::set_max_level`）；
    /// 本项目主要使用 [`Self::to_tracing_level`] 配合 `AtomicLevelFilter`。
    pub fn to_level_filter(&self) -> log::LevelFilter {
        match self {
            Self::Trace => log::LevelFilter::Trace,
            Self::Debug => log::LevelFilter::Debug,
            Self::Info => log::LevelFilter::Info,
            Self::Warn => log::LevelFilter::Warn,
            Self::Error => log::LevelFilter::Error,
        }
    }

    /// 转换为 `tracing::Level`，供 `AtomicLevelFilter::set_level` 使用
    ///
    /// `tracing` 的 `log` feature 会将 `log::` 宏桥接为 tracing event，
    /// 实际过滤级别由 `AtomicLevelFilter` 控制。
    pub fn to_tracing_level(&self) -> tracing::Level {
        match self {
            Self::Trace => tracing::Level::TRACE,
            Self::Debug => tracing::Level::DEBUG,
            Self::Info => tracing::Level::INFO,
            Self::Warn => tracing::Level::WARN,
            Self::Error => tracing::Level::ERROR,
        }
    }
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

/// 标题栏信息展示配置
///
/// 控制自定义标题栏中间区域展示的信息项。
/// 与前端 `TitleBarInfoConfig` 对齐。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleBarInfoConfig {
    /// 展示 Token 消耗总数
    #[serde(default = "default_true")]
    pub show_tokens: bool,
    /// 展示每分钟请求数（RPM）
    #[serde(default = "default_true")]
    pub show_rpm: bool,
    /// 展示平均请求延迟
    #[serde(default = "default_false")]
    pub show_latency: bool,
    /// 展示应用内存占用
    #[serde(default = "default_true")]
    pub show_memory: bool,
    /// 展示网关运行状态
    #[serde(default = "default_true")]
    pub show_gateway_status: bool,
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

impl Default for TitleBarInfoConfig {
    fn default() -> Self {
        Self {
            show_tokens: true,
            show_rpm: true,
            show_latency: false,
            show_memory: true,
            show_gateway_status: true,
        }
    }
}

/// 应用全局设置 DTO
///
/// 将 `app_settings` 表中的 JSON 字段解析为强类型对象，
/// 便于前端表单直接绑定与校验。
/// 与前端 `AppSettingsDto` 对齐。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsDto {
    pub theme: Theme,
    pub locale: Locale,
    /// 全局代理配置（解析后的 ProxyConfig）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_proxy: Option<ProxyConfig>,
    /// 默认请求超时（毫秒），默认 120000
    pub network_timeout_ms: u32,
    /// 全局重试策略（解析后的 RetryConfig）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_retry: Option<RetryConfig>,
    /// 是否使用系统密钥链存储敏感数据
    /// - `true`：系统密钥链（v0.1 暂未实现）
    /// - `false`：本地 AES-GCM 加密
    pub store_secrets_in_keychain: bool,
    /// 全局代理开关（关闭=直连，开启=所有请求走代理）
    pub global_proxy_enabled: bool,
    /// 通用密码（1-20 位），经 SHA-256 派生为 AES-256-GCM 密钥
    /// 用于加密 API Key、Token 等 Secret，以及 WebDAV 远端备份文件
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_key: Option<String>,
    /// 标题栏信息展示配置（结构化对象）
    #[serde(default)]
    pub titlebar_info: TitleBarInfoConfig,
    /// 标题栏信息展示配置 JSON（原始字符串，与数据库字段对应）
    #[serde(default = "default_titlebar_info_json")]
    pub titlebar_info_json: String,
    /// 备份设置（结构化对象）
    #[serde(default)]
    pub backup_settings: BackupSettings,
    /// 开机自启开关（true=注册开机自启，false=取消）
    #[serde(default)]
    pub auto_start_enabled: bool,
    /// 网关上次关闭时的运行状态（true=运行中，false=已关闭）
    /// 用于开机自启时决定是否自动恢复网关
    #[serde(default)]
    pub gateway_last_running: bool,
    /// 全局日志级别，控制 tauri-plugin-log 输出过滤
    #[serde(default)]
    pub log_level: LogLevel,
    /// 创建时间（RFC3339）
    pub created_at: String,
    /// 更新时间（RFC3339）
    pub updated_at: String,
}

/// 标题栏信息展示配置默认值（JSON 字符串）
pub fn default_titlebar_info_json() -> String {
    serde_json::to_string(&TitleBarInfoConfig::default()).unwrap_or_else(|_| {
        r#"{"showTokens":true,"showRpm":true,"showLatency":false,"showMemory":true,"showGatewayStatus":true}"#.to_string()
    })
}

/// 更新应用设置的输入参数
///
/// 所有字段均为可选，仅传递需要变更的字段。
/// `network_timeout` 字段为旧名，v0.2 起统一使用 `network_timeout_ms` 标量。
/// 保留 `network_timeout` 是为兼容前端表单可能存在的复合 TimeoutConfig。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<Locale>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_proxy: Option<ProxyConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_timeout_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_retry: Option<RetryConfig>,
    /// 供应商级 TimeoutConfig；写入时合并到 `network_timeout_ms`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_timeout: Option<TimeoutConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_secrets_in_keychain: Option<bool>,
    /// 全局代理开关（关闭=直连，开启=所有请求走代理）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_proxy_enabled: Option<bool>,
    /// 通用密码（1-20 位）；传 `null` 表示清空，不传表示不更新
    /// 经 SHA-256 派生为 AES-256-GCM 密钥，用于加密 Secret 与 WebDAV 远端备份文件
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_key: Option<Option<String>>,
    /// 标题栏信息展示配置 JSON（旧字段，保留兼容）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titlebar_info_json: Option<String>,
    /// 标题栏信息展示配置（结构化对象，优先使用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titlebar_info: Option<TitleBarInfoConfig>,
    /// 备份设置（结构化对象）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_settings: Option<BackupSettings>,
    /// 开机自启开关
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_start_enabled: Option<bool>,
    /// 网关上次关闭时的运行状态
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_last_running: Option<bool>,
    /// 全局日志级别
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<LogLevel>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_roundtrip() {
        for theme in [
            Theme::Light,
            Theme::Dark,
            Theme::ClaudeLight,
            Theme::ClaudeDark,
            Theme::DeepseekLight,
            Theme::DeepseekDark,
            Theme::NvidiaLight,
            Theme::NvidiaDark,
        ] {
            let s = theme.as_str();
            assert_eq!(Theme::from_str(s), Some(theme));
        }
        assert_eq!(Theme::from_str("unknown"), None);
    }

    #[test]
    fn test_locale_roundtrip() {
        assert_eq!(Locale::from_str("zh-CN"), Some(Locale::ZhCn));
        assert_eq!(Locale::from_str("zh-TW"), Some(Locale::ZhTw));
        assert_eq!(Locale::from_str("en"), Some(Locale::En));
        assert_eq!(Locale::from_str("ja"), Some(Locale::Ja));
        assert_eq!(Locale::from_str("fr"), None);
        assert_eq!(Locale::ZhCn.as_str(), "zh-CN");
        assert_eq!(Locale::ZhTw.as_str(), "zh-TW");
        assert_eq!(Locale::En.as_str(), "en");
        assert_eq!(Locale::Ja.as_str(), "ja");
    }

    #[test]
    fn test_theme_serde() {
        // kebab-case 序列化
        let json = serde_json::to_string(&Theme::ClaudeDark).unwrap();
        assert_eq!(json, "\"claude-dark\"");
        // 反序列化
        let t: Theme = serde_json::from_str("\"deepseek-light\"").unwrap();
        assert_eq!(t, Theme::DeepseekLight);
    }

    #[test]
    fn test_locale_serde() {
        let json = serde_json::to_string(&Locale::ZhCn).unwrap();
        assert_eq!(json, "\"zh-CN\"");
        let l: Locale = serde_json::from_str("\"en\"").unwrap();
        assert_eq!(l, Locale::En);
    }

    #[test]
    fn test_update_settings_input_skip_none() {
        // 仅传 theme 字段，其余字段应被跳过
        let input = UpdateSettingsInput {
            theme: Some(Theme::Dark),
            ..Default::default()
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"theme\":\"dark\""));
        assert!(!json.contains("locale"));
        assert!(!json.contains("gatewayHost"));
    }
}
