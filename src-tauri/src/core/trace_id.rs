//! # trace_id 生成器（雪花 ID 转 32 进制）
//!
//! 简化版雪花算法，生成趋势递增的 64 位整数，转 32 进制字符串。
//! 用于网关请求、Tauri Command、定时任务、托盘菜单、模块启动等所有操作入口的链路追踪。
//!
//! ## 设计要点
//!
//! - 41 位毫秒时间戳（约 69 年）
//! - 10 位机器 ID（桌面应用固定为 1）
//! - 12 位序列号（同毫秒内递增，每毫秒 4096 个）
//! - 64 位整数转 32 进制（字符集 `0-9a-v`），最多 13 字符
//!
//! ## 优势对比 UUID
//!
//! | 维度 | UUID v4 | 雪花 ID 转 32 进制 |
//! |------|---------|-------------------|
//! | 长度 | 36 字符 | 13 字符 |
//! | 排序 | 随机 | 趋势递增 |
//! | 依赖 | uuid crate | 纯标准库 |

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// 32 进制字符集：0-9a-v
const BASE32_CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuv";

/// 简化版雪花 ID 生成器（单机，无需 worker ID 协调）
///
/// 结构：`[41 位毫秒时间戳] [10 位机器 ID] [12 位序列号]`
/// 转成 32 进制后约 13 字符，趋势递增，便于日志按时间近似排序。
pub struct TraceIdGenerator {
    /// 机器 ID 左移 12 位（10 位机器 ID << 12）
    machine_id_shifted: u64,
    /// 上次生成的时间戳（毫秒）
    last_timestamp: AtomicU64,
    /// 同毫秒内序列号
    sequence: AtomicU64,
}

impl TraceIdGenerator {
    /// 创建生成器，machine_id 取低 10 位
    pub const fn new(machine_id: u16) -> Self {
        Self {
            machine_id_shifted: ((machine_id & 0x3FF) as u64) << 12,
            last_timestamp: AtomicU64::new(0),
            sequence: AtomicU64::new(0),
        }
    }

    /// 生成下一个 64 位整数 ID
    pub fn next_id(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // CAS 循环确保同毫秒内序列号递增
        loop {
            let last = self.last_timestamp.load(Ordering::Relaxed);
            if now > last {
                // 新毫秒，重置序列号
                if self
                    .last_timestamp
                    .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
                {
                    self.sequence.store(0, Ordering::Relaxed);
                    break;
                }
            } else if now == last {
                // 同毫秒，序列号递增
                let seq = self.sequence.fetch_add(1, Ordering::Relaxed);
                if seq < 0xFFF {
                    break;
                }
                // 序列号耗尽，自旋等待下一毫秒（桌面应用几乎不会触发）
                continue;
            } else {
                // 时钟回拨，用 last_timestamp 保证递增
                break;
            }
        }

        let ts = self.last_timestamp.load(Ordering::Relaxed);
        let seq = self.sequence.load(Ordering::Relaxed) & 0xFFF;
        (ts << 22) | self.machine_id_shifted | seq
    }

    /// 生成 32 进制字符串（小写 0-9a-v），最多 13 字符
    pub fn next_trace_id(&self) -> String {
        encode_base32(self.next_id())
    }
}

/// 64 位整数转 32 进制字符串
fn encode_base32(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut buf = [0u8; 13];
    let mut i = 12;
    loop {
        buf[i] = BASE32_CHARS[(n & 0x1F) as usize];
        n >>= 5;
        // 仅在还有剩余位时递减 i，避免 i == 0 时 usize 下溢 panic
        if n == 0 {
            break;
        }
        i -= 1;
    }
    String::from_utf8(buf[i..].to_vec()).unwrap()
}

/// 全局生成器实例（machine_id 固定为 1，桌面应用无需区分多实例）
static GLOBAL_GENERATOR: TraceIdGenerator = TraceIdGenerator::new(1);

/// 生成新的 trace_id（32 进制字符串，约 13 字符）
pub fn next_trace_id() -> String {
    GLOBAL_GENERATOR.next_trace_id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_base32() {
        assert_eq!(encode_base32(0), "0");
        assert_eq!(encode_base32(1), "1");
        assert_eq!(encode_base32(31), "v");
        assert_eq!(encode_base32(32), "10");
        // u64::MAX 转 32 进制应为 13 字符
        let s = encode_base32(u64::MAX);
        assert_eq!(s.len(), 13);
    }

    #[test]
    fn test_next_trace_id_unique() {
        let gen = TraceIdGenerator::new(1);
        let id1 = gen.next_trace_id();
        let id2 = gen.next_trace_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_next_trace_id_monotonic() {
        let gen = TraceIdGenerator::new(1);
        let id1 = gen.next_id();
        let id2 = gen.next_id();
        assert!(id2 >= id1, "trace_id 应趋势递增");
    }

    #[test]
    fn test_next_trace_id_length() {
        // 全局生成器产生的 trace_id 长度不超过 13
        let id = next_trace_id();
        assert!(id.len() <= 13);
        assert!(id.chars().all(|c| c.is_ascii_digit() || ('a'..='v').contains(&c)));
    }
}
