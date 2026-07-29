//! # 按天 + 按大小双维度滚动的文件 appender
//!
//! `tracing-appender` 原生仅支持按时间滚动，本 appender 额外支持按大小分片：
//!
//! - 按天创建文件：`i-code-YYYY-MM-DD.log`
//! - 超过 `max_size` 分片：`i-code-YYYY-MM-DD.1.log`、`i-code-YYYY-MM-DD.2.log`
//! - 启动时清理超过 `max_days` 天的旧文件
//!
//! ## 文件命名规则
//!
//! ```text
//! i-code-2026-07-29.log        # 当天首个文件
//! i-code-2026-07-29.1.log      # 超过 20MB 后
//! i-code-2026-07-29.2.log      # 再次超过
//! i-code-2026-07-30.log        # 跨天切换，序号重置
//! ```
//!
//! ## 边界情况
//!
//! - 进程重启后，`new()` 会打开当天已有文件继续追加（`append(true)`），
//!   `current_size` 从 metadata 读取
//! - 跨天但进程未重启：下次 `write` 时检测到日期变化，自动切换
//! - 旧文件清理仅在 `new()` 时执行一次，运行时不重复扫描
//!
//! ## 与 `tracing_appender::non_blocking` 的兼容性
//!
//! `SizeAwareFileAppender` 实现 `std::io::Write + Send + 'static`，可直接传入
//! `tracing_appender::non_blocking()`。`non_blocking` 会启动后台线程，所有写入
//! 异步化，不阻塞日志热路径。

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use chrono::{Datelike, Local};

/// 按天 + 按大小滚动的文件 appender
pub struct SizeAwareFileAppender {
    inner: Mutex<Inner>,
}

struct Inner {
    log_dir: PathBuf,
    prefix: String,
    suffix: String,
    max_size: u64,
    max_days: u32,
    current_file: Option<File>,
    current_size: u64,
    current_date: String,
    current_seq: u32,
}

impl SizeAwareFileAppender {
    /// 创建 appender
    ///
    /// - `log_dir`：日志目录
    /// - `prefix`：文件名前缀（如 `"i-code"`）
    /// - `suffix`：文件名后缀（如 `"log"`）
    /// - `max_size`：单文件最大字节数（如 `20 * 1024 * 1024` = 20MB）
    /// - `max_days`：保留天数（如 `30`）
    pub fn new(
        log_dir: impl AsRef<Path>,
        prefix: &str,
        suffix: &str,
        max_size: u64,
        max_days: u32,
    ) -> io::Result<Self> {
        let log_dir = log_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&log_dir)?;

        // 启动时清理超过 max_days 的旧文件
        Self::cleanup_old_files(&log_dir, prefix, suffix, max_days)?;

        let today = today_string();
        let path = Self::build_path(&log_dir, prefix, &today, 0, suffix);
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let size = file.metadata()?.len();

        Ok(Self {
            inner: Mutex::new(Inner {
                log_dir,
                prefix: prefix.to_string(),
                suffix: suffix.to_string(),
                max_size,
                max_days,
                current_file: Some(file),
                current_size: size,
                current_date: today,
                current_seq: 0,
            }),
        })
    }

    /// 构建文件路径
    ///
    /// - `seq=0` 时：`prefix-YYYY-MM-DD.suffix`
    /// - `seq>0` 时：`prefix-YYYY-MM-DD.N.suffix`
    fn build_path(dir: &Path, prefix: &str, date: &str, seq: u32, suffix: &str) -> PathBuf {
        if seq == 0 {
            dir.join(format!("{}-{}.{}", prefix, date, suffix))
        } else {
            dir.join(format!("{}-{}.{}.{}", prefix, date, seq, suffix))
        }
    }

    /// 清理超过 `max_days` 天的旧文件
    ///
    /// 仅清理匹配 `prefix-*.suffix` 的文件，按 mtime 判断。
    fn cleanup_old_files(dir: &Path, prefix: &str, suffix: &str, max_days: u32) -> io::Result<()> {
        let cutoff = SystemTime::now() - Duration::from_secs(max_days as u64 * 86400);
        if !dir.exists() {
            return Ok(());
        }
        let starts_with = format!("{}-", prefix);
        let ends_with = format!(".{}", suffix);
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            // 仅清理匹配 prefix-*.suffix 的文件
            if !name.starts_with(&starts_with) || !name.ends_with(&ends_with) {
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                if let Ok(mtime) = meta.modified() {
                    if mtime < cutoff {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
        Ok(())
    }

    /// 检查并执行滚动（跨天或超大小）
    fn rotate_if_needed(&self, inner: &mut Inner, write_len: usize) -> io::Result<()> {
        let today = today_string();

        // 跨天：重置序号，创建新日期文件
        if today != inner.current_date {
            inner.current_date = today.clone();
            inner.current_seq = 0;
            let path = Self::build_path(&inner.log_dir, &inner.prefix, &today, 0, &inner.suffix);
            inner.current_file = Some(OpenOptions::new().create(true).append(true).open(&path)?);
            inner.current_size = inner.current_file.as_ref().unwrap().metadata()?.len();
        }

        // 超大小：序号 +1，创建分片文件
        if inner.current_size + write_len as u64 > inner.max_size {
            inner.current_seq += 1;
            let path = Self::build_path(
                &inner.log_dir,
                &inner.prefix,
                &inner.current_date,
                inner.current_seq,
                &inner.suffix,
            );
            inner.current_file = Some(OpenOptions::new().create(true).append(true).open(&path)?);
            inner.current_size = 0;
        }

        Ok(())
    }
}

impl Write for SizeAwareFileAppender {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "mutex poisoned"))?;
        self.rotate_if_needed(&mut inner, buf.len())?;
        if let Some(ref mut file) = inner.current_file {
            let written = file.write(buf)?;
            inner.current_size += written as u64;
            Ok(written)
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "no file"))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "mutex poisoned"))?;
        if let Some(ref mut file) = inner.current_file {
            file.flush()
        } else {
            Ok(())
        }
    }
}

/// 获取当天日期字符串（`YYYY-MM-DD`，本地时区）
fn today_string() -> String {
    let now = Local::now();
    format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_build_path_seq0() {
        let dir = Path::new("/tmp");
        let path = SizeAwareFileAppender::build_path(dir, "i-code", "2026-07-29", 0, "log");
        assert_eq!(path.file_name().unwrap(), "i-code-2026-07-29.log");
    }

    #[test]
    fn test_build_path_seq1() {
        let dir = Path::new("/tmp");
        let path = SizeAwareFileAppender::build_path(dir, "i-code", "2026-07-29", 1, "log");
        assert_eq!(path.file_name().unwrap(), "i-code-2026-07-29.1.log");
    }

    #[test]
    fn test_size_rollover() {
        let tmp = TempDir::new().unwrap();
        // max_size=100 字节，触发大小滚动
        let mut appender = SizeAwareFileAppender::new(tmp.path(), "test", "log", 100, 30).unwrap();
        // 写入 150 字节，应触发分片
        let data = vec![b'x'; 150];
        appender.write_all(&data).unwrap();
        appender.flush().unwrap();
        // 验证存在分片文件
        let files: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().count();
        assert!(files >= 2, "应存在至少 2 个文件（原文件 + 分片）");
    }
}
