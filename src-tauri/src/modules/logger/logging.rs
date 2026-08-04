#![allow(dead_code)]

//! # 自研内存 Logger 全局工具接口
//!
//! 提供不依赖 `tauri::AppHandle` 的后端便捷写入接口。
//! **与 `tauri-plugin-log` / `log::` 宏完全隔离**，写入自研内存环形缓冲区。
//!
//! ## 使用方式
//!
//! 在 [`main.rs`](crate::main) 初始化 [`LoggerServiceHandle`] 后调用：
//!
//! ```rust
//! use crate::modules::logger;
//! logger::set_global_logger_handle(logger_handle.clone());
//! ```
//!
//! 之后在任意后端代码中：
//!
//! ```rust
//! use crate::modules::logger::Log;
//! Log::info("模块启动完成");
//! Log::error_with_loc("发生错误", Some("module.rs"), Some(42));
//! ```
//!
//! ## 与 tauri-plugin-log 的区别
//!
//! | 维度 | `log::info!` / tauri-plugin-log | `Log::info` / 自研 logger |
//! |------|----------------------------------|---------------------------|
//! | 输出目标 | 终端、WebView 控制台、日志文件 | 内存环形缓冲区 + 日志页面 + 可选文件 |
//! | 用途 | 开发调试、运行时追踪 | 业务运行时诊断、运维可见 |
//! | 生命周期 | 跟随 `log` crate logger | 跟随 `LoggerServiceHandle` |
//! | 级别控制 | `app_settings.log_level` | `log_settings` 中的过滤配置 |
//!
//! 两者互不干扰，调整一方不会影响另一方。

use std::sync::OnceLock;

use super::service::LoggerServiceHandle;
use super::types::{LogLevel, LogSource, LOG_TIME_FORMAT};

/// 全局 Logger 句柄
///
/// 应用启动时注册一次，之后 [`Log`] 的所有方法通过它写入内存缓冲区。
static GLOBAL_LOGGER_HANDLE: OnceLock<LoggerServiceHandle> = OnceLock::new();

/// 注册全局 Logger 句柄
///
/// 应在应用启动时调用一次。若重复调用，第二次及以后会被忽略。
/// 未注册时 [`Log`] 的方法会静默丢弃，不会 panic。
pub fn set_global_logger_handle(handle: LoggerServiceHandle) {
    let _ = GLOBAL_LOGGER_HANDLE.set(handle);
}

fn get_handle() -> Option<&'static LoggerServiceHandle> {
    GLOBAL_LOGGER_HANDLE.get()
}

/// 构造并写入一条系统日志
fn write(level: LogLevel, message: &str, file_name: Option<&str>, line_number: Option<u32>) {
    let Some(handle) = get_handle() else {
        return;
    };

    use super::types::LogEntry;
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
        error_message: Some(message.to_string()),
        request_id: None,
        model_id: None,
        request_headers: None,
        request_body: None,
        response_body: None,
        tags: Vec::new(),
        file_name: file_name.map(|s| s.to_string()),
        line_number,
    };
    handle.service().write(entry);
}

/// 全局自研内存日志工具
///
/// 所有方法均为非阻塞写入；未初始化或写入失败均静默忽略，不影响业务逻辑。
/// 不会调用 `log::` 宏，因此与 `tauri-plugin-log` 完全隔离。
#[allow(dead_code)]
pub struct Log;

impl Log {
    /// DEBUG 级别日志
    pub fn debug(message: &str) {
        write(LogLevel::Debug, message, None, None);
    }
    /// DEBUG 级别日志（带源码位置）
    pub fn debug_with_loc(message: &str, file_name: &str, line_number: u32) {
        write(LogLevel::Debug, message, Some(file_name), Some(line_number));
    }
    /// INFO 级别日志
    pub fn info(message: &str) {
        write(LogLevel::Info, message, None, None);
    }
    /// INFO 级别日志（带源码位置）
    pub fn info_with_loc(message: &str, file_name: &str, line_number: u32) {
        write(LogLevel::Info, message, Some(file_name), Some(line_number));
    }
    /// WARN 级别日志
    pub fn warn(message: &str) {
        write(LogLevel::Warn, message, None, None);
    }
    /// WARN 级别日志（带源码位置）
    pub fn warn_with_loc(message: &str, file_name: &str, line_number: u32) {
        write(LogLevel::Warn, message, Some(file_name), Some(line_number));
    }
    /// ERROR 级别日志
    pub fn error(message: &str) {
        write(LogLevel::Error, message, None, None);
    }
    /// ERROR 级别日志（带源码位置）
    pub fn error_with_loc(message: &str, file_name: &str, line_number: u32) {
        write(LogLevel::Error, message, Some(file_name), Some(line_number));
    }
}
