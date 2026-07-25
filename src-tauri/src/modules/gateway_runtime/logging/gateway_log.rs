//! # 网关入口日志记录器
//!
//! 将 `LogRecord` 写入自研 logger 模块的环形缓冲区（`LogSource::Gateway`），
//! 按网关日志开关与最大 body 长度配置截断请求/响应体。

use crate::modules::gateway_runtime::client::truncate_body;
use crate::modules::gateway_runtime::service::GatewaySharedState;
use crate::modules::logger::types::{LogEntry, LogLevel, LogSource, LOG_TIME_FORMAT};

use super::recorder::{LogKind, LogRecord, LogRecorder, LogStatus};

/// 网关入口日志记录器
///
/// 写入自研 logger，按 `gateway_*` 系列开关控制是否记录请求/响应体，
/// 并按 `gateway_max_body_length` 截断。
pub struct GatewayLogRecorder;

impl LogRecorder for GatewayLogRecorder {
    fn record(&self, shared: &GatewaySharedState, entry: &LogRecord) {
        if !matches!(entry.kind, Some(LogKind::Gateway)) {
            return;
        }

        let settings = shared.logger_handle.service().get_settings();
        let gw_cfg = settings.to_gateway_config();
        drop(settings);

        let request_body = entry
            .request_body
            .as_deref()
            .filter(|_| gw_cfg.enable_gateway_request_log)
            .map(|s| truncate_body(s, gw_cfg.max_body_length));

        let response_body = entry
            .response_body
            .as_deref()
            .filter(|_| gw_cfg.enable_gateway_response_log)
            .map(|s| truncate_body(s, gw_cfg.max_body_length));

        let level = match entry.status {
            Some(LogStatus::Success) | None => LogLevel::Info,
            Some(LogStatus::Error | LogStatus::Failed) => LogLevel::Error,
        };

        let log_entry = LogEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Local::now().format(LOG_TIME_FORMAT).to_string(),
            level,
            source: LogSource::Gateway,
            method: entry.method.clone(),
            url: entry.url.clone(),
            status_code: entry.status_code,
            duration_ms: entry.duration_ms,
            prompt_tokens: entry.prompt_tokens.map(|v| v as u64),
            completion_tokens: entry.completion_tokens.map(|v| v as u64),
            total_tokens: entry.total_tokens.map(|v| v as u64),
            cached_tokens: entry.cached_tokens.map(|v| v as u64),
            error_message: entry.error_message.clone(),
            request_id: entry.request_id.clone(),
            model_id: entry.model_id.clone(),
            request_body,
            response_body,
            tags: entry.tags.clone(),
            file_name: None,
            line_number: None,
        };

        shared.logger_handle.service().write(log_entry);
    }
}
