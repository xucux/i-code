//! # 日志模块 Tauri Command 声明
//!
//! 前端通过 `invoke('log_*', payload)` 调用这些命令。

use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::IcodeResult;

use super::service::LoggerServiceHandle;
use super::types::{LogEntry, LogExportFormat, LogExportResult, LogFilter, LogLevel, LogSource, CommandLogConfig, LogSettings};

/// 事件名：新日志写入
pub const EVENT_LOG_NEW_ENTRY: &str = "log:new-entry";
/// 事件名：日志缓冲区被清空
pub const EVENT_LOG_CLEARED: &str = "log:cleared";

/// 查询日志
///
/// 按 `filter` 过滤返回匹配条目。`filter` 为空时返回全部。
#[tauri::command]
pub async fn log_list(
    state: State<'_, LoggerServiceHandle>,
    filter: Option<LogFilter>,
) -> IcodeResult<Vec<LogEntry>> {
    let filter = filter.unwrap_or_default();
    state.service().query(&filter)
}

/// 获取最近 N 条日志
///
/// 比 `log_list` 更高效，不执行过滤。
#[tauri::command]
pub async fn log_recent(
    state: State<'_, LoggerServiceHandle>,
    limit: Option<usize>,
) -> IcodeResult<Vec<LogEntry>> {
    Ok(state.service().list_recent(limit.unwrap_or(100)))
}

/// 写入一条日志
///
/// 主要供 `gateway-runtime` 在拦截器中调用。
/// 前端通常不直接调用此命令。
/// 写入后通过 Tauri Event `log:new-entry` 推送到前端。
#[tauri::command]
pub async fn log_write(
    app: AppHandle,
    state: State<'_, LoggerServiceHandle>,
    entry: LogEntry,
) -> IcodeResult<()> {
    state.service().write(entry.clone());
    // 推送事件到前端，前端通过 listen() 接收
    let _ = app.emit(EVENT_LOG_NEW_ENTRY, &entry);
    Ok(())
}

/// 清空日志
///
/// 清空完成后广播 `log:cleared` 事件，前端无需轮询即可感知。
#[tauri::command]
pub async fn log_clear(
    app: AppHandle,
    state: State<'_, LoggerServiceHandle>,
) -> IcodeResult<()> {
    state.service().clear();
    let _ = app.emit(EVENT_LOG_CLEARED, ());
    Ok(())
}

/// 获取当前日志条数
#[tauri::command]
pub async fn log_count(state: State<'_, LoggerServiceHandle>) -> IcodeResult<usize> {
    Ok(state.service().count())
}

/// 导出日志到文件
///
/// 按 `filter` 过滤后导出为 JSON 或 CSV，保存到应用缓存目录。
#[tauri::command]
pub async fn log_export(
    state: State<'_, LoggerServiceHandle>,
    app: AppHandle,
    filter: Option<LogFilter>,
    format: LogExportFormat,
) -> IcodeResult<LogExportResult> {
    let filter = filter.unwrap_or_default();
    let export_dir = app
        .path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir());
    state.service().export(&filter, format, &export_dir)
}

/// 通用日志写入（前端调用 log.info / log.error 等记录消息）
///
/// errorMessage 字段承载日志消息内容，source 固定为 System。
#[tauri::command]
pub async fn log_message(
    app: AppHandle,
    state: State<'_, LoggerServiceHandle>,
    level: LogLevel,
    message: String,
    file_name: Option<String>,
    line_number: Option<u32>,
) -> IcodeResult<()> {
    use super::types::LOG_TIME_FORMAT;
    let entry = LogEntry {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Local::now().format(LOG_TIME_FORMAT).to_string(),
        level,
        source: LogSource::System,
        method: None,
        url: None,
        status_code: None,
        duration_ms: None,
        prompt_tokens: None,
        completion_tokens: None,
        total_tokens: None,
        cached_tokens: None,
        error_message: Some(message),
            request_id: None,
            model_id: None,
            request_headers: None,
            request_body: None,
            response_body: None,
            tags: Vec::new(),
        file_name,
        line_number,
    };
    state.service().write(entry.clone());
    let _ = app.emit(EVENT_LOG_NEW_ENTRY, &entry);
    Ok(())
}

/// 读取 Command 交互日志配置
#[tauri::command]
pub async fn log_get_command_config(
    state: State<'_, LoggerServiceHandle>,
) -> IcodeResult<CommandLogConfig> {
    Ok(state.service().get_settings().to_command_config())
}

/// 更新 Command 交互日志配置
#[tauri::command]
pub async fn log_set_command_config(
    state: State<'_, LoggerServiceHandle>,
    config: CommandLogConfig,
) -> IcodeResult<CommandLogConfig> {
    let mut settings = state.service().get_settings();
    settings.enable_command_log = config.enable_command_log;
    settings.enable_command_request_log = config.enable_command_request_log;
    settings.enable_command_response_log = config.enable_command_response_log;
    settings.command_max_body_length = config.max_body_length;
    let updated = state.service().update_settings(&settings)?;
    Ok(updated.to_command_config())
}

/// 读取统一日志设置
#[tauri::command]
pub async fn log_get_settings(
    state: State<'_, LoggerServiceHandle>,
) -> IcodeResult<LogSettings> {
    Ok(state.service().get_settings())
}

/// 更新统一日志设置
///
/// 同时更新数据库和内存中的配置。
/// 转发详细日志和 Command 日志配置由 GatewaySharedState 中的引用自动生效。
#[tauri::command]
pub async fn log_set_settings(
    state: State<'_, LoggerServiceHandle>,
    settings: LogSettings,
) -> IcodeResult<LogSettings> {
    state.service().update_settings(&settings)
}
