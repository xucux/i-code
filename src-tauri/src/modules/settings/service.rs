//! # 应用设置业务服务层
//!
//! 提供 `app_settings` 的读取、更新、字段级便捷操作。
//! Service 层负责 JSON 字段的序列化/反序列化，对 Commands 层屏蔽存储细节。
//!
//! ## 句柄模式
//!
//! [`SettingsServiceHandle`] 为零状态句柄，直接持有 [`SettingsService`]。
//! 由于 `app_settings` 是单例表，Service 无需缓存数据，每次读取直接查库。
//! 全局只创建一个 Handle，通过 Tauri State 共享。

use std::sync::Arc;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::shared::{ProxyConfig, RetryConfig, TimeoutConfig};

use super::repository::{self, AppSettingsRow, UpdateSettingsFields};
use super::types::{AppSettingsDto, Locale, LogLevel, Theme, TitleBarInfoConfig, UpdateSettingsInput};
use crate::modules::backup::types::BackupSettings;

/// 通用密码（config_key）最小长度
const CONFIG_KEY_MIN_LEN: usize = 1;

/// 通用密码（config_key）最大长度
const CONFIG_KEY_MAX_LEN: usize = 20;

/// Settings Service 在 Tauri State 中的句柄
///
/// 使用 `Arc` 包装以便在多线程间共享。
/// 与 `SecretServiceHandle` 不同，Settings 无需启动期加载缓存数据，
/// 每次调用都直接查库，避免缓存一致性问题。
#[derive(Clone, Default)]
pub struct SettingsServiceHandle {
    pub inner: Arc<SettingsService>,
}

impl SettingsServiceHandle {
    /// 创建 Settings Service 句柄
    ///
    /// 无需启动期初始化参数，直接构造即可。
    /// 在 `main.rs` 的 `setup` 钩子中调用并 `app.manage()`。
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SettingsService),
        }
    }

    /// 获取内部 Service 引用
    pub fn service(&self) -> &SettingsService {
        &self.inner
    }
}

/// Settings Service 业务逻辑
///
/// 通过 `SettingsServiceHandle::service()` 获取引用，
/// 在 Tauri Command 中通过 `State<SettingsServiceHandle>` 访问。
#[derive(Default)]
pub struct SettingsService;

impl SettingsService {
    /// 读取应用设置
    ///
    /// 将 `app_settings` 表中的标量字段与 JSON 字段合并为强类型 DTO。
    /// JSON 字段反序列化失败时返回 INTERNAL 错误（数据库被破坏的信号）。
    pub fn get_settings(&self) -> IcodeResult<AppSettingsDto> {
        let row = repository::find()?;
        self.row_to_dto(&row)
    }

    /// 更新应用设置
    ///
    /// 仅更新传入字段，其余保持原值。
    /// 更新完成后返回最新 DTO。
    pub fn update_settings(&self, input: UpdateSettingsInput) -> IcodeResult<AppSettingsDto> {
        let mut fields = UpdateSettingsFields::default();

        if let Some(theme) = input.theme {
            fields.theme = Some(theme.as_str().to_string());
        }
        if let Some(locale) = input.locale {
            fields.locale = Some(locale.as_str().to_string());
        }
        // Option<Option<String>>：外层 Some 表示需要更新，内层 None 表示置空
        if let Some(config_key) = input.config_key {
            if let Some(ref pwd) = config_key {
                if !pwd.is_empty() && (pwd.len() < CONFIG_KEY_MIN_LEN || pwd.len() > CONFIG_KEY_MAX_LEN) {
                    return Err(IcodeError::validation(format!(
                        "通用密码长度必须在 {CONFIG_KEY_MIN_LEN}-{CONFIG_KEY_MAX_LEN} 位之间"
                    )));
                }
            }
            fields.config_key = Some(config_key);
        }
        if let Some(proxy) = input.global_proxy {
            fields.global_proxy_json = Some(Some(serde_json::to_string(&proxy)?));
        }
        if let Some(timeout_ms) = input.network_timeout_ms {
            fields.network_timeout_ms = Some(timeout_ms as i64);
        }
        if let Some(retry) = input.network_retry {
            fields.network_retry_json = Some(Some(serde_json::to_string(&retry)?));
        }
        // TimeoutConfig 暂仅取 connection 字段合并到 network_timeout_ms
        // 完整的双字段超时（连接/响应）需后续 ai_gateway 模块支持
        if let Some(timeout) = input.network_timeout {
            fields.network_timeout_ms = Some(timeout.connection as i64);
        }
        if let Some(store_in_keychain) = input.store_secrets_in_keychain {
            fields.store_secrets_in_keychain = Some(store_in_keychain);
        }
        if let Some(global_proxy_enabled) = input.global_proxy_enabled {
            fields.global_proxy_enabled = Some(global_proxy_enabled);
        }
        if let Some(titlebar_info) = input.titlebar_info {
            fields.titlebar_info_json = Some(serde_json::to_string(&titlebar_info)?);
        }
        if let Some(backup_settings) = input.backup_settings {
            fields.backup_settings_json = Some(serde_json::to_string(&backup_settings)?);
        }
        if let Some(auto_start_enabled) = input.auto_start_enabled {
            fields.auto_start_enabled = Some(auto_start_enabled);
        }
        if let Some(gateway_last_running) = input.gateway_last_running {
            fields.gateway_last_running = Some(gateway_last_running);
        }
        if let Some(log_level) = input.log_level {
            fields.log_level = Some(log_level.as_str().to_string());
        }

        repository::update(&fields)?;
        self.get_settings()
    }

    /// 设置主题
    #[allow(dead_code)]
    pub fn set_theme(&self, theme: Theme) -> IcodeResult<AppSettingsDto> {
        self.update_settings(UpdateSettingsInput {
            theme: Some(theme),
            ..Default::default()
        })
    }

    /// 设置语言
    #[allow(dead_code)]
    pub fn set_locale(&self, locale: Locale) -> IcodeResult<AppSettingsDto> {
        self.update_settings(UpdateSettingsInput {
            locale: Some(locale),
            ..Default::default()
        })
    }

    /// 将数据库行转换为 DTO
    ///
    /// 负责解析 JSON 字段、转换枚举类型。
    fn row_to_dto(&self, row: &AppSettingsRow) -> IcodeResult<AppSettingsDto> {
        // 解析主题枚举，未知值降级为 Dark
        let theme = Theme::from_str(&row.theme)
            .ok_or_else(|| IcodeError::internal(format!("未知主题值: {}", row.theme)))?;

        // 解析语言枚举
        let locale = Locale::from_str(&row.locale)
            .ok_or_else(|| IcodeError::internal(format!("未知语言值: {}", row.locale)))?;

        // 解析全局代理 JSON（可选）
        let global_proxy: Option<ProxyConfig> = match &row.global_proxy_json {
            Some(json) if !json.is_empty() => Some(serde_json::from_str(json)?),
            _ => None,
        };

        // 解析全局重试策略 JSON（可选）
        let network_retry: Option<RetryConfig> = match &row.network_retry_json {
            Some(json) if !json.is_empty() => Some(serde_json::from_str(json)?),
            _ => None,
        };

        // 网络超时从 i64 转 u32
        let network_timeout_ms = row.network_timeout_ms.unwrap_or(120_000);
        let network_timeout_ms = if network_timeout_ms >= 0 && network_timeout_ms <= u32::MAX as i64
        {
            network_timeout_ms as u32
        } else {
            return Err(IcodeError::internal(format!(
                "网络超时 {} 超出 u32 范围",
                network_timeout_ms
            )));
        };

        // 解析标题栏信息配置 JSON
        let titlebar_info: TitleBarInfoConfig = match &row.titlebar_info_json {
            Some(json) if !json.is_empty() => serde_json::from_str(json).unwrap_or_default(),
            _ => TitleBarInfoConfig::default(),
        };
        let titlebar_info_json = serde_json::to_string(&titlebar_info)?;

        // 解析备份设置 JSON
        let backup_settings: BackupSettings = match &row.backup_settings_json {
            Some(json) if !json.is_empty() => serde_json::from_str(json).unwrap_or_default(),
            _ => BackupSettings::default(),
        };

        // 解析全局日志级别，未知值降级为 Info
        let log_level = LogLevel::from_str(&row.log_level).unwrap_or_default();

        Ok(AppSettingsDto {
            theme,
            locale,
            global_proxy,
            global_proxy_enabled: row.global_proxy_enabled,
            network_timeout_ms,
            network_retry,
            store_secrets_in_keychain: row.store_secrets_in_keychain,
            config_key: row.config_key.clone(),
            titlebar_info,
            titlebar_info_json,
            backup_settings,
            auto_start_enabled: row.auto_start_enabled,
            gateway_last_running: row.gateway_last_running,
            log_level,
            created_at: row.created_at.clone(),
            updated_at: row.updated_at.clone(),
        })
    }
}

/// 从 TimeoutConfig 提取连接超时毫秒数
///
/// 用于将复合 TimeoutConfig 简化为 app_settings.network_timeout_ms 标量。
/// 后续 ai_gateway 模块可支持完整的双字段超时。
#[allow(dead_code)]
pub fn timeout_to_ms(timeout: &TimeoutConfig) -> u32 {
    // 截断到 u32 范围
    if timeout.connection > u32::MAX as u64 {
        u32::MAX
    } else {
        timeout.connection as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_to_ms() {
        let timeout = TimeoutConfig {
            connection: 25000,
            response: 120000,
        };
        assert_eq!(timeout_to_ms(&timeout), 25000);
    }

    #[test]
    fn test_timeout_to_ms_overflow() {
        let timeout = TimeoutConfig {
            connection: u64::MAX,
            response: u64::MAX,
        };
        // 超出 u32 范围时截断到 u32::MAX
        assert_eq!(timeout_to_ms(&timeout), u32::MAX);
    }

    #[test]
    fn test_handle_clone() {
        // 验证 Handle 可克隆（Tauri State 要求 Clone）
        let handle = SettingsServiceHandle::new();
        let _clone = handle.clone();
    }

    #[test]
    fn test_update_settings_input_default() {
        let input = UpdateSettingsInput::default();
        assert!(input.theme.is_none());
        assert!(input.locale.is_none());
        assert!(input.global_proxy_enabled.is_none());
    }
}
