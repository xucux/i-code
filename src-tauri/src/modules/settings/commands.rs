//! # 应用设置模块 Tauri Command 声明
//!
//! 前端通过 `invoke('settings_*', payload)` 调用这些命令。
//! Commands 层仅做参数校验与 Service 调用，不包含业务逻辑。

use tauri::{AppHandle, Emitter, Manager, State};

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
    state.service().get_settings()
}

/// 更新应用设置
///
/// 仅更新传入的字段，其余保持原值。
/// 更新完成后返回最新设置 DTO，前端可直接替换缓存，并广播
/// `settings:changed` 事件，方便标题栏等跨组件监听方实时刷新。
/// 若更新了 `log_level`，会实时同步到 tauri-plugin-log 的全局过滤级别。
///
/// # 参数
/// - `input`：部分更新输入，字段全部可选
#[tauri::command]
pub async fn settings_update(
    state: State<'_, SettingsServiceHandle>,
    app_handle: AppHandle,
    input: UpdateSettingsInput,
) -> IcodeResult<AppSettingsDto> {
    let dto = state.service().update_settings(input)?;
    // 实时应用日志级别变更
    log::set_max_level(dto.log_level.to_level_filter());
    // 广播设置变更事件，payload 携带最新的标题栏信息配置
    let _ = app_handle.emit("settings:changed", &dto.titlebar_info);
    Ok(dto)
}

/// 获取 tauri-plugin-log 日志文件目录
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
