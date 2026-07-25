//! # 全局雪花 ID 生成器
//!
//! 基于 `rs-snowflake` 为数据库主键提供趋势递增的 64 位分布式 ID。
//! 所有表级 ID（providers、workspaces、secrets 等）统一通过 [`generate_id`] 生成，
//! 以十进制字符串形式存入 SQLite TEXT 主键列。

use once_cell::sync::Lazy;
use snowflake::SnowflakeIdGenerator;
use std::sync::Mutex;

/// 机器 ID 与节点 ID
///
/// 桌面端单实例运行，固定为 1/1 即可满足唯一性。
const MACHINE_ID: i32 = 1;
const NODE_ID: i32 = 1;

/// 全局雪花 ID 生成器
///
/// 使用 Mutex 保护内部序列号状态；桌面端并发极低，锁竞争可忽略。
static SNOWFLAKE: Lazy<Mutex<SnowflakeIdGenerator>> =
    Lazy::new(|| Mutex::new(SnowflakeIdGenerator::new(MACHINE_ID, NODE_ID)));

/// 生成一个新的雪花 ID 字符串
pub fn generate_id() -> String {
    SNOWFLAKE.lock().unwrap().generate().to_string()
}

/// 判断字符串是否为雪花 ID 格式
///
/// 仅用于兼容旧版 Secret 引用识别等场景。新业务中 Secret 引用应统一使用
/// `$SECRET:{id}$` 格式，不依赖此函数。
pub fn is_snowflake_id(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| b.is_ascii_digit())
        && s.parse::<i64>().map(|v| v > 0).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id_is_snowflake_format() {
        let id = generate_id();
        assert!(
            is_snowflake_id(&id),
            "生成的 ID 应符合雪花 ID 格式: {}",
            id
        );
    }

    #[test]
    fn test_is_snowflake_id_rejects_invalid() {
        assert!(!is_snowflake_id(""));
        assert!(!is_snowflake_id("abc"));
        assert!(!is_snowflake_id("12-34"));
        assert!(!is_snowflake_id("0"));
        assert!(is_snowflake_id("1234567890123456789"));
    }
}
