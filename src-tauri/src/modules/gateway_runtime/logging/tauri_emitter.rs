//! # tauri-plugin-log 输出器
//!
//! 将 `LogRecord` 通过 `tauri-plugin-log`（`log` crate）输出到终端、WebView 控制台
//! 和日志文件。**不截断**请求/响应体，提供完整诊断信息，与自研 logger 互补。
//!
//! 输出策略：
//! - `info`：URL、状态码、耗时、Token
//! - `debug`：请求体、响应体（完整）
//! - `warn`：非 2xx 或调用失败，同时打印请求/响应体/错误信息

use crate::modules::gateway_runtime::service::GatewaySharedState;

use super::recorder::{LogRecord, LogRecorder, LogStatus};

/// tauri-plugin-log 输出器
pub struct TauriLogEmitter;

impl TauriLogEmitter {
    /// 通道标签：Gateway / Provider API
    fn channel_label(record: &LogRecord) -> &'static str {
        match record
            .method
            .as_deref()
            .and_then(|_| record.url.as_deref())
        {
            _ => "Gateway", // 默认网关入口标签；转发场景通过 tags 区分
        }
    }
}

impl LogRecorder for TauriLogEmitter {
    fn record(&self, _shared: &GatewaySharedState, record: &LogRecord) {
        let channel = Self::channel_label(record);
        let method = record.method.as_deref().unwrap_or("-");
        let url = record.url.as_deref().unwrap_or("-");
        let status = record
            .status_code
            .map(|s| s.to_string())
            .unwrap_or_else(|| "-".to_string());
        let duration = record.duration_ms.unwrap_or(0);
        let pt = record
            .prompt_tokens
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());
        let ct = record
            .completion_tokens
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());
        let tt = record
            .total_tokens
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());
        let rid = record.request_id.as_deref().unwrap_or("-");

        log::info!(
            "{} | {} {} | status={} | duration={}ms | tokens={}/{}/{} | request_id={}",
            channel,
            method,
            url,
            status,
            duration,
            pt,
            ct,
            tt,
            rid
        );

        if let Some(body) = record.request_body.as_deref() {
            log::debug!(
                "{} request body | {} {} | request_id={} | {}",
                channel,
                method,
                url,
                rid,
                body
            );
        }

        if let Some(body) = record.response_body.as_deref() {
            log::debug!(
                "{} response body | {} {} | request_id={} | {}",
                channel,
                method,
                url,
                rid,
                body
            );
        }

        let is_failure = matches!(
            record.status,
            Some(LogStatus::Error | LogStatus::Failed)
        );
        if is_failure {
            log::warn!(
                "{} non-200/error | {} {} | status={} | duration={}ms | request_id={} | error={} | request_body={} | response_body={}",
                channel,
                method,
                url,
                status,
                duration,
                rid,
                record.error_message.as_deref().unwrap_or("-"),
                record.request_body.as_deref().unwrap_or("-"),
                record.response_body.as_deref().unwrap_or("-")
            );
        }
    }
}
