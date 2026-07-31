//! # 应用设置模块 Tauri Command 声明
//!
//! 前端通过 `invoke('settings_*', payload)` 调用这些命令。
//! Commands 层仅做参数校验与 Service 调用，不包含业务逻辑。

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::core::atomic_filter::AtomicLevelFilter;
use crate::core::trace_id_layer::enter_operation_async;
use crate::error::{IcodeError, IcodeResult};

use super::service::SettingsServiceHandle;
use super::types::{AppSettingsDto, UpdateSettingsInput};

/// 读取应用设置
///
/// 返回包含解析后 JSON 字段的 DTO，前端可直接绑定到表单。
///
/// # 返回
/// `AppSettingsDto`：主题、语言、网关地址、代理、超时、重试等配置。
#[tauri::command]
pub async fn settings_get(
    state: State<'_, SettingsServiceHandle>,
) -> IcodeResult<AppSettingsDto> {
    enter_operation_async("settings_get", async {
        state.service().get_settings()
    })
    .await
}

/// 更新应用设置
///
/// 仅更新传入的字段，其余保持原值。
/// 更新完成后返回最新设置 DTO，前端可直接替换缓存，并广播
/// `settings:changed` 事件，方便标题栏等跨组件监听方实时刷新。
/// 若更新了 `log_level`，会实时同步到 `AtomicLevelFilter` 的全局过滤级别。
///
/// 业务日志：若更新了 `global_proxy` 或 `global_proxy_enabled`，
/// 写入 system 级业务 logger（代理 URL 脱敏，不含认证信息）。
///
/// # 参数
/// - `input`：部分更新输入，字段全部可选
#[tauri::command]
pub async fn settings_update(
    state: State<'_, SettingsServiceHandle>,
    app_handle: AppHandle,
    input: UpdateSettingsInput,
) -> IcodeResult<AppSettingsDto> {
    enter_operation_async("settings_update", async {
        // 提取代理变更信息用于业务日志（在 input 被消费前）
        // global_proxy 是强类型 ProxyConfig，to_log_string 已脱敏
        let proxy_log = input.global_proxy.as_ref().map(|p| p.to_log_string());
        let proxy_enabled_log = input.global_proxy_enabled;

        let dto = state.service().update_settings(input)?;

        // 业务日志：全局代理配置变更（脱敏，不含认证信息）
        if let Some(log_str) = proxy_log {
            let msg = format!("全局代理配置已更新 | {}", log_str);
            tracing::info!("{}", msg);
            crate::modules::logger::Log::info(&msg);
        }
        if let Some(enabled) = proxy_enabled_log {
            let msg = format!("全局代理开关已{}", if enabled { "开启" } else { "关闭" });
            tracing::info!("{}", msg);
            crate::modules::logger::Log::info(&msg);
        }

        // 实时应用日志级别变更：通过 Arc<AtomicLevelFilter> 调整全局过滤级别
        if let Some(atomic_filter) = app_handle.try_state::<Arc<AtomicLevelFilter>>() {
            atomic_filter.set_level(dto.log_level.to_tracing_level());
        }
        // 广播设置变更事件，payload 携带最新的标题栏信息配置
        let _ = app_handle.emit("settings:changed", &dto.titlebar_info);
        Ok(dto)
    })
    .await
}

/// 获取应用日志文件目录
///
/// 返回应用日志目录的绝对路径，不同平台位置不同：
/// - Windows: `%LOCALAPPDATA%\\com.icode.app\\logs`
/// - macOS: `~/Library/Logs/com.icode.app`
/// - Linux: `~/.config/com.icode.app/logs`
#[tauri::command]
pub async fn settings_log_dir(app_handle: AppHandle) -> IcodeResult<String> {
    let log_dir = app_handle
        .path()
        .app_log_dir()
        .map_err(|e| IcodeError::internal(format!("无法获取日志目录: {e}")))?
        .to_string_lossy()
        .into_owned();
    Ok(log_dir)
}

/// 获取应用配置/数据目录
///
/// 返回应用配置目录的绝对路径，与数据库（`i-code.db`）同目录，
/// 用于在设置页展示系统配置目录（提示词库、备份等均在此目录下）。
///
/// - Windows: `%APPDATA%\\com.icode.app`
/// - macOS: `~/Library/Application Support/com.icode.app`
/// - Linux: `~/.config/com.icode.app`
#[tauri::command]
pub async fn settings_config_dir(app_handle: AppHandle) -> IcodeResult<String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| IcodeError::internal(format!("无法获取应用配置目录: {e}")))?
        .to_string_lossy()
        .into_owned();
    Ok(config_dir)
}
