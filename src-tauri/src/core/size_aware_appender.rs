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
//!   `current_size` 从 metadata 读取；滚动时以 `create_new` 独占创建新分片，
//!   不会重复追加旧分片，避免多实例 / 多次重启把同一分片撑到超过 `max_size`
//! - 单次写入超过剩余容量时按 `max_size` 自动拆分，每个分片不超过 `max_size`
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

/// SSE 专属日志文件前缀（`i-code-sse.YYYY-MM-DD-HH.log`，由 `tracing_appender::rolling::hourly` 生成）。
/// 该前缀下的文件与主日志**共用** `SizeAwareFileAppender` 的清理逻辑与保留天数，
/// 在应用启动时统一按 `max_days` 清理，避免内部高频 chunk 文件无限累积。
pub const SSE_LOG_PREFIX: &str = "i-code-sse";

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

        // 防御：max_size 至少为 1 字节，避免配置为 0 时每次写入都滚动
        let max_size = max_size.max(1);

        // 启动时清理超过 max_days 的旧文件。
        // 主日志（i-code-*）与 SSE 专属文件（i-code-sse.*）共用同一清理逻辑与保留天数：
        // 二者在同一 `new()` 触发时统一按 `max_days` 清理，保证配置单一、行为一致。
        Self::cleanup_old_files(&log_dir, &[prefix, SSE_LOG_PREFIX], suffix, max_days)?;

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
    /// 仅清理以任一 `prefixes[*]` 开头、以 `.suffix` 结尾的文件，按 mtime 判断。
    /// 传入多个前缀即可让多类日志文件共用同一保留天数（如主日志 `i-code` 与 SSE `i-code-sse`）。
    fn cleanup_old_files(dir: &Path, prefixes: &[&str], suffix: &str, max_days: u32) -> io::Result<()> {
        let cutoff = SystemTime::now() - Duration::from_secs(max_days as u64 * 86400);
        if !dir.exists() {
            return Ok(());
        }
        let ends_with = format!(".{}", suffix);
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            // 仅清理匹配任一 prefix 开头、以 .suffix 结尾的文件
            if !name.ends_with(&ends_with) {
                continue;
            }
            if !prefixes.iter().any(|p| name.starts_with(p)) {
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

    /// 以独占方式创建下一个分片文件，序号从 `start_seq` 起，跳过已被其他实例占用的序号。
    ///
    /// 使用 `create_new`（O_EXCL）保证同一序号只会被一个进程创建成功，其余实例自动顺延，
    /// 从根上避免多进程 / 多次重启追加到同一分片：这会撑大单个文件超过 `max_size`，
    /// 或让序号被反复复用（例如 `.2` 的写入时间早于 `.1`）。
    fn open_exclusive_segment(
        dir: &Path,
        prefix: &str,
        date: &str,
        start_seq: u32,
        suffix: &str,
    ) -> io::Result<(File, u32)> {
        let mut seq = start_seq;
        loop {
            let path = Self::build_path(dir, prefix, date, seq, suffix);
            match OpenOptions::new().create_new(true).write(true).open(&path) {
                Ok(file) => return Ok((file, seq)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    let Some(next) = seq.checked_add(1) else {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "日志分片序号溢出",
                        ));
                    };
                    seq = next;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// 执行写入，并在跨天 / 超出 `max_size` 时滚动。
    ///
    /// 与旧实现不同，这里按 `max_size` **拆分**传入的 buffer：若数据超过当前文件剩余容量，
    /// 先写满当前文件，滚动到新分片后再写剩余部分，保证每个分片不超过 `max_size`
    /// （修复了单块写入越过阈值导致出现 ~40MB 分片的问题）。
    fn write_impl(&self, inner: &mut Inner, buf: &[u8]) -> io::Result<usize> {
        // 跨天：切换到当天主文件，并重置序号
        let today = today_string();
        if today != inner.current_date {
            inner.current_date = today.clone();
            inner.current_seq = 0;
            let path = Self::build_path(&inner.log_dir, &inner.prefix, &today, 0, &inner.suffix);
            inner.current_file = Some(OpenOptions::new().create(true).append(true).open(&path)?);
            inner.current_size = inner.current_file.as_ref().unwrap().metadata()?.len();
        }

        let mut written_total = 0usize;
        let mut rest = buf;
        while !rest.is_empty() {
            // 当前文件剩余容量；已满则滚动到新分片
            let remaining = inner.max_size.saturating_sub(inner.current_size);
            if remaining == 0 {
                let (file, seq) = Self::open_exclusive_segment(
                    &inner.log_dir,
                    &inner.prefix,
                    &inner.current_date,
                    inner.current_seq.saturating_add(1),
                    &inner.suffix,
                )?;
                inner.current_seq = seq;
                inner.current_file = Some(file);
                inner.current_size = 0;
                continue;
            }

            // 单次最多写满当前文件
            let to_write = (rest.len() as u64).min(remaining) as usize;
            let file = inner
                .current_file
                .as_mut()
                .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no file"))?;
            let written = file.write(&rest[..to_write])?;
            if written == 0 {
                // 底层拒绝写入（如磁盘满），避免无限循环
                return if written_total == 0 {
                    Err(io::Error::new(io::ErrorKind::WriteZero, "日志文件写入返回 0"))
                } else {
                    Ok(written_total)
                };
            }
            inner.current_size += written as u64;
            written_total += written;
            rest = &rest[written..];
        }
        Ok(written_total)
    }
}

impl Write for SizeAwareFileAppender {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "mutex poisoned"))?;
        self.write_impl(&mut inner, buf)
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
    use std::io::Read;
    use tempfile::TempDir;

    /// 按字典序列出目录内所有文件名
    fn files_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// 按写入顺序（主文件在前，分片序号递增）拼接全部内容
    fn read_all_in_order(dir: &Path, prefix: &str, date: &str) -> Vec<u8> {
        let mut out = Vec::new();
        let mut f = std::fs::File::open(dir.join(format!("{prefix}-{date}.log"))).unwrap();
        f.read_to_end(&mut out).unwrap();
        let mut seq = 1u32;
        while let Some(mut f) =
            std::fs::File::open(dir.join(format!("{prefix}-{date}.{seq}.log"))).ok()
        {
            f.read_to_end(&mut out).unwrap();
            seq += 1;
        }
        out
    }

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

    /// 单块写入超过 max_size 时应被自动拆分为多个分片：每个 ≤ max_size 且内容不丢不乱
    #[test]
    fn test_size_rollover_splits_large_write() {
        let tmp = TempDir::new().unwrap();
        let max_size: u64 = 100;
        let mut appender =
            SizeAwareFileAppender::new(tmp.path(), "test", "log", max_size, 30).unwrap();
        // 350B 一次写入，应拆为 100 + 100 + 100 + 50 共 4 个文件
        let data: Vec<u8> = (0..350u32).map(|i| (i % 251) as u8).collect();
        appender.write_all(&data).unwrap();
        appender.flush().unwrap();
        drop(appender);

        let files = files_in(tmp.path());
        assert_eq!(files.len(), 4, "应生成 4 个文件，实际: {files:?}");
        for name in &files {
            let len = std::fs::metadata(tmp.path().join(name)).unwrap().len();
            assert!(len <= max_size, "分片 {name} 大小 {len} 超过 max_size");
        }
        let merged = read_all_in_order(
            tmp.path(),
            "test",
            &chrono::Local::now().format("%Y-%m-%d").to_string(),
        );
        assert_eq!(merged, data, "拆分后内容丢失或乱序");
    }

    /// 模拟重启：已被占用的分片序号不再复用，滚动会独占创建新分片
    #[test]
    fn test_restart_does_not_reuse_existing_segment() {
        let tmp = TempDir::new().unwrap();
        let max_size: u64 = 100;
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();

        // 第一次运行：150B → 主文件 100B + .1 分片 50B（内容 'x'）
        {
            let mut a =
                SizeAwareFileAppender::new(tmp.path(), "i-code", "log", max_size, 30).unwrap();
            a.write_all(&vec![b'x'; 150]).unwrap();
            a.flush().unwrap();
        }
        // 第二次运行（重启）：主文件已满，滚动不应 append 到 .1，而应独占创建 .2
        {
            let mut a =
                SizeAwareFileAppender::new(tmp.path(), "i-code", "log", max_size, 30).unwrap();
            a.write_all(&vec![b'y'; 150]).unwrap();
            a.flush().unwrap();
        }

        let seg1 = tmp.path().join(format!("i-code-{date}.1.log"));
        let seg2 = tmp.path().join(format!("i-code-{date}.2.log"));
        let seg3 = tmp.path().join(format!("i-code-{date}.3.log"));

        // .1 是第一次运行独占创建的，内容只能是 'x'，不会被二次追加
        assert_eq!(std::fs::metadata(&seg1).unwrap().len(), 50);
        let mut buf1 = Vec::new();
        std::fs::File::open(&seg1).unwrap().read_to_end(&mut buf1).unwrap();
        assert!(buf1.iter().all(|&b| b == b'x'), ".1 被二次追加或内容被污染");

        // .2 / .3 由第二次运行独占创建，内容为 'y'
        let mut buf2 = Vec::new();
        std::fs::File::open(&seg2).unwrap().read_to_end(&mut buf2).unwrap();
        assert_eq!(buf2.len(), 100);
        assert!(buf2.iter().all(|&b| b == b'y'));
        assert_eq!(std::fs::metadata(&seg3).unwrap().len(), 50);
    }
}
