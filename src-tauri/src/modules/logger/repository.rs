//! # 日志模块内存环形缓冲区
//!
//! 使用 `VecDeque` 实现 FIFO 环形缓冲区。
//! 超过容量时自动淘汰最旧条目。
//!
//! ## 设计要点
//!
//! - 线程安全：通过 `Mutex` 保护（写入路径为热路径，需低开销）
//! - 默认容量 5000 条
//! - 导出与查询都从内存读取，无文件 IO

use std::collections::VecDeque;
use std::sync::Mutex;

use super::types::LogEntry;

/// 内存环形缓冲区
pub struct LogRingBuffer {
    inner: Mutex<VecDeque<LogEntry>>,
    capacity: usize,
}

impl LogRingBuffer {
    /// 创建指定容量的缓冲区
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    /// 追加一条日志；超过容量时淘汰最旧条目
    pub fn push(&self, entry: LogEntry) {
        let mut buf = self.inner.lock().expect("日志缓冲区 Mutex 中毒");
        if buf.len() >= self.capacity {
            buf.pop_front();
        }
        buf.push_back(entry);
    }

    /// 读取所有日志（按时间正序）
    pub fn list_all(&self) -> Vec<LogEntry> {
        let buf = self.inner.lock().expect("日志缓冲区 Mutex 中毒");
        buf.iter().cloned().collect()
    }

    /// 读取最近 N 条
    pub fn list_recent(&self, n: usize) -> Vec<LogEntry> {
        let buf = self.inner.lock().expect("日志缓冲区 Mutex 中毒");
        let len = buf.len();
        let start = len.saturating_sub(n);
        buf.iter().skip(start).cloned().collect()
    }

    /// 清空缓冲区
    pub fn clear(&self) {
        let mut buf = self.inner.lock().expect("日志缓冲区 Mutex 中毒");
        buf.clear();
    }

    /// 当前条数
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .expect("日志缓冲区 Mutex 中毒")
            .len()
    }

    /// 是否为空
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for LogRingBuffer {
    fn default() -> Self {
        Self::new(5_000)
    }
}

// ===== log_settings 数据库读写 =====

use crate::db::get_db_pool;
use crate::error::IcodeResult;
use super::types::{LogSettings, LogLevel};

/// 从 log_settings 表读取配置（单行，id='default'）
pub fn get_log_settings() -> IcodeResult<LogSettings> {
    let pool = get_db_pool()?;
    let conn = pool.get()?;
    let mut settings = LogSettings::default();

    let result = conn.query_row(
        "SELECT buffer_size, log_dir, max_retention_days, enable_file_persistence, \
         max_file_size_mb, max_file_count, file_log_level, \
         enable_request_log, enable_response_log, forward_max_body_length, \
         enable_gateway_request_log, enable_gateway_response_log, gateway_max_body_length, \
         enable_command_log, enable_command_request_log, enable_command_response_log, \
         command_max_body_length \
         FROM log_settings WHERE id = 'default'",
        [],
        |row| {
            settings.buffer_size = row.get::<_, i64>(0)? as usize;
            settings.log_dir = row.get::<_, String>(1)?;
            settings.max_retention_days = row.get::<_, i64>(2)? as u32;
            settings.enable_file_persistence = row.get::<_, i64>(3)? != 0;
            settings.max_file_size_mb = row.get::<_, i64>(4)? as u64;
            settings.max_file_count = row.get::<_, i64>(5)? as u32;
            let fll: String = row.get::<_, String>(6)?;
            settings.file_log_level = LogLevel::from_str(&fll);
            settings.enable_request_log = row.get::<_, i64>(7)? != 0;
            settings.enable_response_log = row.get::<_, i64>(8)? != 0;
            settings.forward_max_body_length = row.get::<_, i64>(9)? as usize;
            settings.enable_gateway_request_log = row.get::<_, i64>(10)? != 0;
            settings.enable_gateway_response_log = row.get::<_, i64>(11)? != 0;
            settings.gateway_max_body_length = row.get::<_, i64>(12)? as usize;
            settings.enable_command_log = row.get::<_, i64>(13)? != 0;
            settings.enable_command_request_log = row.get::<_, i64>(14)? != 0;
            settings.enable_command_response_log = row.get::<_, i64>(15)? != 0;
            settings.command_max_body_length = row.get::<_, i64>(16)? as usize;
            Ok(())
        },
    );

    match result {
        Ok(_) => Ok(settings),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(LogSettings::default()),
        Err(e) => Err(crate::error::IcodeError::database(format!("读取 log_settings 失败: {e}"))),
    }
}

/// 更新 log_settings 表
pub fn update_log_settings(settings: &LogSettings) -> IcodeResult<LogSettings> {
    let pool = get_db_pool()?;
    let conn = pool.get()?;

    conn.execute(
        "UPDATE log_settings SET \
         buffer_size = ?1, log_dir = ?2, max_retention_days = ?3, \
         enable_file_persistence = ?4, max_file_size_mb = ?5, max_file_count = ?6, \
         file_log_level = ?7, enable_request_log = ?8, enable_response_log = ?9, \
         forward_max_body_length = ?10, enable_gateway_request_log = ?11, \
         enable_gateway_response_log = ?12, gateway_max_body_length = ?13, \
         enable_command_log = ?14, enable_command_request_log = ?15, \
         enable_command_response_log = ?16, command_max_body_length = ?17 \
         WHERE id = 'default'",
        rusqlite::params![
            settings.buffer_size as i64,
            settings.log_dir,
            settings.max_retention_days as i64,
            settings.enable_file_persistence as i64,
            settings.max_file_size_mb as i64,
            settings.max_file_count as i64,
            settings.file_log_level.as_ref().map(|l| l.as_str()).unwrap_or("INFO"),
            settings.enable_request_log as i64,
            settings.enable_response_log as i64,
            settings.forward_max_body_length as i64,
            settings.enable_gateway_request_log as i64,
            settings.enable_gateway_response_log as i64,
            settings.gateway_max_body_length as i64,
            settings.enable_command_log as i64,
            settings.enable_command_request_log as i64,
            settings.enable_command_response_log as i64,
            settings.command_max_body_length as i64,
        ],
    ).map_err(|e| crate::error::IcodeError::database(format!("更新 log_settings 失败: {e}")))?;

    // 重新读取返回最新值
    get_log_settings()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::logger::types::{LogLevel, LogSource};

    fn make_entry(id: &str) -> LogEntry {
        LogEntry {
            id: id.to_string(),
            timestamp: format!("2026-07-15T00:00:0{id}Z", id = id),
            level: LogLevel::Info,
            source: LogSource::Gateway,
            method: None,
            url: None,
            status_code: None,
            duration_ms: None,
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            cached_tokens: None,
            error_message: None,
            request_id: None,
            model_id: None,
            request_body: None,
            response_body: None,
            tags: Vec::new(),
            file_name: None,
            line_number: None,
        }
    }

    #[test]
    fn test_push_and_list() {
        let buf = LogRingBuffer::new(100);
        buf.push(make_entry("1"));
        buf.push(make_entry("2"));
        assert_eq!(buf.len(), 2);
        let all = buf.list_all();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, "1");
    }

    #[test]
    fn test_capacity_eviction() {
        let buf = LogRingBuffer::new(3);
        for i in 0..5 {
            buf.push(make_entry(&i.to_string()));
        }
        assert_eq!(buf.len(), 3);
        let all = buf.list_all();
        // 最旧的 0、1 被淘汰
        assert_eq!(all[0].id, "2");
        assert_eq!(all[2].id, "4");
    }

    #[test]
    fn test_list_recent() {
        let buf = LogRingBuffer::new(100);
        for i in 0..5 {
            buf.push(make_entry(&i.to_string()));
        }
        let recent = buf.list_recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].id, "3");
        assert_eq!(recent[1].id, "4");
    }

    #[test]
    fn test_clear() {
        let buf = LogRingBuffer::new(100);
        buf.push(make_entry("1"));
        assert!(!buf.is_empty());
        buf.clear();
        assert!(buf.is_empty());
    }
}
