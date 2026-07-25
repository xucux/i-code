//! # 日志业务服务层
//!
//! 提供日志写入、查询、导出、清理功能。
//! 写入为非阻塞操作（仅锁 Mutex 短暂时间）。
//!
//! ## 实时推送
//!
//! 新日志写入后通过 Tauri Event `log:new-entry` 推送到前端。
//! 事件发送由 Commands 层负责（Service 层不持有 AppHandle）。

use std::sync::Arc;
use std::sync::mpsc;

use crate::error::IcodeResult;

use super::repository::LogRingBuffer;
use super::types::{LogEntry, LogExportFormat, LogExportResult, LogFilter, LogRollingConfig, LogSettings};

/// Logger Service 在 Tauri State 中的句柄
pub struct LoggerServiceHandle {
    inner: Arc<LoggerService>,
}

impl LoggerServiceHandle {
    pub fn new(config: LogRollingConfig) -> Self {
        Self {
            inner: Arc::new(LoggerService::new(config)),
        }
    }

    /// 使用默认配置创建
    pub fn with_default() -> Self {
        Self::new(LogRollingConfig::default())
    }

    pub fn service(&self) -> &LoggerService {
        &self.inner
    }
}

impl Clone for LoggerServiceHandle {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Logger Service 业务逻辑
pub struct LoggerService {
    buffer: LogRingBuffer,
    #[allow(dead_code)]
    config: LogRollingConfig,
    /// 日志文件异步写入通道
    log_file_tx: Option<mpsc::Sender<String>>,
    /// 统一日志配置（从 DB 加载，运行时可更新）
    log_settings: std::sync::RwLock<LogSettings>,
}

impl LoggerService {
    pub fn new(config: LogRollingConfig) -> Self {
        // 尝试从 DB 加载配置，失败则使用默认值
        let settings = super::repository::get_log_settings().unwrap_or_default();

        let buffer = LogRingBuffer::new(settings.buffer_size);

        // 初始化文件持久化
        let log_file_tx = if settings.enable_file_persistence {
            let log_dir = if settings.log_dir.is_empty() {
                Self::default_log_dir()
            } else {
                std::path::PathBuf::from(&settings.log_dir)
            };
            let tx = Self::start_file_writer(&log_dir, settings.max_retention_days);
            Some(tx)
        } else {
            None
        };

        Self {
            buffer,
            config,
            log_file_tx,
            log_settings: std::sync::RwLock::new(settings),
        }
    }

    /// 获取默认日志目录（可执行文件所在目录/logs）
    fn default_log_dir() -> std::path::PathBuf {
        // 优先使用可执行文件所在目录，失败则回退到临时目录
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.join("logs")))
            .unwrap_or_else(|| {
                dirs::data_dir()
                    .unwrap_or_else(|| std::env::temp_dir())
                    .join("i-code")
                    .join("logs")
            })
    }

    /// 启动后台文件写入线程
    ///
    /// 通过 mpsc channel 接收日志行，异步追加到当天日志文件。
    /// 启动时清理超过 max_retention_days 的过期日志文件。
    fn start_file_writer(log_dir: &std::path::Path, max_retention_days: u32) -> mpsc::Sender<String> {
        let (tx, rx) = mpsc::channel::<String>();
        let log_dir = log_dir.to_path_buf();

        std::thread::spawn(move || {
            // 启动时清理过期日志
            Self::cleanup_old_logs(&log_dir, max_retention_days);

            // 当前写入的文件日期和文件句柄
            let mut current_date: Option<String> = None;
            let mut current_file: Option<std::fs::File> = None;

            while let Ok(line) = rx.recv() {
                // 计算当前日期
                let today = chrono::Local::now().format("%Y-%m-%d").to_string();

                // 日期变化时切换文件
                if current_date.as_ref() != Some(&today) {
                    current_date = Some(today.clone());
                    // 确保目录存在
                    let _ = std::fs::create_dir_all(&log_dir);
                    let file_path = log_dir.join(format!("i-code-{today}.log"));
                    current_file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(file_path)
                        .ok();
                }

                // 写入一行
                if let Some(file) = current_file.as_mut() {
                    use std::io::Write;
                    let _ = writeln!(file, "{}", line);
                }
            }
        });

        tx
    }

    /// 清理超过指定天数的日志文件
    fn cleanup_old_logs(log_dir: &std::path::Path, max_retention_days: u32) {
        if !log_dir.exists() {
            return;
        }
        let cutoff = chrono::Local::now() - chrono::Duration::days(max_retention_days as i64);
        let cutoff_str = cutoff.format("%Y-%m-%d").to_string();

        if let Ok(entries) = std::fs::read_dir(log_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                // 匹配 i-code-YYYY-MM-DD.log 格式
                if let Some(date_part) = name.strip_prefix("i-code-").and_then(|s| s.strip_suffix(".log")) {
                    if date_part < cutoff_str.as_str() {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
        }
    }

    /// 获取当前日志配置
    pub fn get_settings(&self) -> LogSettings {
        self.log_settings.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// 更新日志配置（写入 DB + 更新内存）
    pub fn update_settings(&self, settings: &LogSettings) -> IcodeResult<LogSettings> {
        let updated = super::repository::update_log_settings(settings)?;
        if let Ok(mut guard) = self.log_settings.write() {
            *guard = updated.clone();
        }
        Ok(updated)
    }

    /// 将 LogEntry 格式化为 pipe 分隔的日志行
    fn format_pipe_line(entry: &LogEntry) -> String {
        let ts = &entry.timestamp;
        let tags = entry.tags.join(",");
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            ts,
            entry.level.as_str(),
            entry.source.as_str(),
            entry.method.as_deref().unwrap_or(""),
            entry.url.as_deref().unwrap_or(""),
            entry.status_code.map(|c| c.to_string()).unwrap_or_default(),
            entry.duration_ms.map(|d| d.to_string()).unwrap_or_default(),
            entry.prompt_tokens.map(|t| t.to_string()).unwrap_or_default(),
            entry.completion_tokens.map(|t| t.to_string()).unwrap_or_default(),
            entry.total_tokens.map(|t| t.to_string()).unwrap_or_default(),
            entry.cached_tokens.map(|t| t.to_string()).unwrap_or_default(),
            entry.error_message.as_deref().unwrap_or(""),
            entry.request_id.as_deref().unwrap_or(""),
            entry.model_id.as_deref().unwrap_or(""),
            entry.file_name.as_deref().unwrap_or(""),
            entry.line_number.map(|n| n.to_string()).unwrap_or_default(),
            entry.request_body.as_deref().unwrap_or(""),
            entry.response_body.as_deref().unwrap_or(""),
            tags,
        )
    }

    /// 写入一条系统日志（非阻塞）
    ///
    /// 供后端模块直接调用，无需 AppHandle。
    /// 自动设置 source = System，生成 UUID 和时间戳。
    pub fn log_system(&self, level: super::types::LogLevel, message: &str, file_name: Option<&str>) {
        use super::types::{LogEntry, LogSource, LOG_TIME_FORMAT};
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
            request_body: None,
            response_body: None,
            tags: Vec::new(),
            file_name: file_name.map(|s| s.to_string()),
            line_number: None,
        };
        self.write(entry);
    }

    /// 写入一条日志（非阻塞）
    ///
    /// 同时写入内存环形缓冲区和日志文件（如果开启）。
    pub fn write(&self, entry: LogEntry) {
        self.buffer.push(entry.clone());
        // 异步写入日志文件
        if let Some(tx) = &self.log_file_tx {
            let line = Self::format_pipe_line(&entry);
            let _ = tx.send(line);
        }
    }

    /// 查询日志
    ///
    /// 按 `filter` 过滤，返回匹配条目（按时间倒序，最新在前）。
    /// v0.1 返回全部匹配，分页由前端处理。
    pub fn query(&self, filter: &LogFilter) -> IcodeResult<Vec<LogEntry>> {
        let all = self.buffer.list_all();
        let mut result: Vec<LogEntry> = all
            .into_iter()
            .filter(|entry| Self::matches(entry, filter))
            .collect();
        // 按时间倒序排列（最新在前）
        result.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(result)
    }

    /// 获取最近 N 条日志
    pub fn list_recent(&self, limit: usize) -> Vec<LogEntry> {
        self.buffer.list_recent(limit)
    }

    /// 清空日志
    pub fn clear(&self) {
        self.buffer.clear();
    }

    /// 当前日志条数
    pub fn count(&self) -> usize {
        self.buffer.len()
    }

    /// 导出日志到文件
    ///
    /// 按 `filter` 过滤后导出为 JSON 或 CSV。
    /// 文件保存到应用临时目录，返回文件路径。
    pub fn export(
        &self,
        filter: &LogFilter,
        format: LogExportFormat,
        export_dir: &std::path::Path,
    ) -> IcodeResult<LogExportResult> {
        let entries = self.query(filter)?;

        // 确保导出目录存在
        if !export_dir.exists() {
            std::fs::create_dir_all(export_dir)?;
        }

        let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let (ext, content) = match format {
            LogExportFormat::Json => {
                ("json", serde_json::to_string_pretty(&entries)?)
            }
            LogExportFormat::Csv => {
                ("csv", Self::to_csv(&entries))
            }
        };
        let file_name = format!("i-code-logs-{timestamp}.{ext}");
        let file_path = export_dir.join(file_name);
        std::fs::write(&file_path, content)?;

        Ok(LogExportResult {
            file_path: file_path.to_string_lossy().to_string(),
            count: entries.len(),
            format,
        })
    }

    /// 判断日志是否匹配过滤条件
    fn matches(entry: &LogEntry, filter: &LogFilter) -> bool {
        // 级别过滤
        if !filter.levels.is_empty() && !filter.levels.contains(&entry.level) {
            return false;
        }
        // 来源过滤
        if !filter.sources.is_empty() && !filter.sources.contains(&entry.source) {
            return false;
        }
        // 状态码过滤
        if !filter.status_codes.is_empty() {
            match entry.status_code {
                Some(code) if filter.status_codes.contains(&code) => {}
                _ => return false,
            }
        }
        // 关键词模糊匹配（URL、errorMessage）
        if let Some(keyword) = &filter.keyword {
            let kw = keyword.to_lowercase();
            let url_match = entry
                .url
                .as_ref()
                .map(|u| u.to_lowercase().contains(&kw))
                .unwrap_or(false);
            let err_match = entry
                .error_message
                .as_ref()
                .map(|e| e.to_lowercase().contains(&kw))
                .unwrap_or(false);
            if !url_match && !err_match {
                return false;
            }
        }
        // 时间范围过滤
        if let Some(range) = &filter.time_range {
            if let Some(from) = &range.from {
                if entry.timestamp.as_str() < from.as_str() {
                    return false;
                }
            }
            if let Some(to) = &range.to {
                if entry.timestamp.as_str() > to.as_str() {
                    return false;
                }
            }
        }
        // 请求 ID 精确匹配
        if let Some(req_id) = &filter.request_id {
            if entry.request_id.as_deref() != Some(req_id.as_str()) {
                return false;
            }
        }
        true
    }

    /// 将日志条目转为 CSV 字符串
    fn to_csv(entries: &[LogEntry]) -> String {
        let mut buf = String::new();
        // 表头
        buf.push_str(
            "id,timestamp,level,source,method,url,statusCode,durationMs,promptTokens,completionTokens,totalTokens,cachedTokens,errorMessage,requestId,modelId,requestBody,responseBody,tags,fileName,lineNumber\n",
        );
        for e in entries {
            // CSV 字段转义：包含逗号、引号、换行的字段用双引号包裹，内部双引号转义为两个双引号
            let esc = |s: &str| {
                if s.contains(',') || s.contains('"') || s.contains('\n') {
                    format!("\"{}\"", s.replace('"', "\"\""))
                } else {
                    s.to_string()
                }
            };
            let tags = e.tags.join(",");
            buf.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                esc(&e.id),
                esc(&e.timestamp),
                e.level.as_str(),
                e.source.as_str(),
                e.method.as_deref().unwrap_or(""),
                e.url.as_deref().map(esc).unwrap_or_default(),
                e.status_code.map(|c| c.to_string()).unwrap_or_default(),
                e.duration_ms.map(|d| d.to_string()).unwrap_or_default(),
                e.prompt_tokens.map(|t| t.to_string()).unwrap_or_default(),
                e.completion_tokens
                    .map(|t| t.to_string())
                    .unwrap_or_default(),
                e.total_tokens.map(|t| t.to_string()).unwrap_or_default(),
                e.cached_tokens.map(|t| t.to_string()).unwrap_or_default(),
                e.error_message.as_deref().map(esc).unwrap_or_default(),
                e.request_id.as_deref().map(esc).unwrap_or_default(),
                e.model_id.as_deref().map(esc).unwrap_or_default(),
                e.request_body.as_deref().map(esc).unwrap_or_default(),
                e.response_body.as_deref().map(esc).unwrap_or_default(),
                esc(&tags),
                e.file_name.as_deref().map(esc).unwrap_or_default(),
                e.line_number.map(|n| n.to_string()).unwrap_or_default(),
            ));
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::logger::types::{LogLevel, LogSource};

    fn make_entry(id: &str, level: LogLevel, source: LogSource) -> LogEntry {
        LogEntry {
            id: id.to_string(),
            timestamp: format!("2026-07-15T00:00:0{id}Z", id = id),
            level,
            source,
            method: Some("POST".to_string()),
            url: Some(format!("https://api.example.com/req-{id}")),
            status_code: Some(200),
            duration_ms: Some(100),
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            cached_tokens: None,
            error_message: None,
            request_id: Some(format!("req-{id}")),
            model_id: Some(format!("openai/model-{id}")),
            request_body: None,
            response_body: None,
            tags: Vec::new(),
            file_name: None,
            line_number: None,
        }
    }

    #[test]
    fn test_write_and_query() {
        let svc = LoggerService::new(LogRollingConfig::default());
        svc.write(make_entry("1", LogLevel::Info, LogSource::Gateway));
        svc.write(make_entry("2", LogLevel::Error, LogSource::ProviderApi));

        let all = svc.query(&LogFilter::default()).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_filter_by_level() {
        let svc = LoggerService::new(LogRollingConfig::default());
        svc.write(make_entry("1", LogLevel::Info, LogSource::Gateway));
        svc.write(make_entry("2", LogLevel::Error, LogSource::ProviderApi));

        let filter = LogFilter {
            levels: vec![LogLevel::Error],
            ..Default::default()
        };
        let result = svc.query(&filter).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].level, LogLevel::Error);
    }

    #[test]
    fn test_filter_by_keyword() {
        let svc = LoggerService::new(LogRollingConfig::default());
        svc.write(make_entry("1", LogLevel::Info, LogSource::Gateway));

        // 匹配 URL
        let filter = LogFilter {
            keyword: Some("req-1".to_string()),
            ..Default::default()
        };
        let result = svc.query(&filter).unwrap();
        assert_eq!(result.len(), 1);

        // 不匹配
        let filter = LogFilter {
            keyword: Some("nonexistent".to_string()),
            ..Default::default()
        };
        let result = svc.query(&filter).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_filter_by_request_id() {
        let svc = LoggerService::new(LogRollingConfig::default());
        svc.write(make_entry("1", LogLevel::Info, LogSource::Gateway));
        svc.write(make_entry("2", LogLevel::Info, LogSource::Gateway));

        let filter = LogFilter {
            request_id: Some("req-2".to_string()),
            ..Default::default()
        };
        let result = svc.query(&filter).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].request_id.as_deref(), Some("req-2"));
    }

    #[test]
    fn test_clear() {
        let svc = LoggerService::new(LogRollingConfig::default());
        svc.write(make_entry("1", LogLevel::Info, LogSource::Gateway));
        assert_eq!(svc.count(), 1);
        svc.clear();
        assert_eq!(svc.count(), 0);
    }

    #[test]
    fn test_csv_export() {
        let svc = LoggerService::new(LogRollingConfig::default());
        svc.write(make_entry("1", LogLevel::Info, LogSource::Gateway));

        let tmp = std::env::temp_dir();
        let result = svc
            .export(&LogFilter::default(), LogExportFormat::Csv, &tmp)
            .unwrap();
        assert_eq!(result.count, 1);
        let content = std::fs::read_to_string(&result.file_path).unwrap();
        assert!(content.contains("id,timestamp,level"));
        assert!(content.contains("req-1"));
    }
}
