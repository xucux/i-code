//! # 虚拟供应商运行时设置
//!
//! 探活与「全部失活兜底」节奏属于全局运行时配置，存放于 `global_configs`
//! （group = `virtual_provider`），无需数据库迁移；修改后下一轮调度立即生效。
//!
//! - `health_check_interval_secs`：降级路由探活间隔（默认 3600s / 1h）
//! - `all_failed_fallback_interval_secs`：全部路由永久失活时的兜底尝试间隔（默认 1800s / 30min）
//! - `attempts_keep_days`：路由尝试历史保留天数（默认 3 天）

use crate::db::{get_db_pool, global_config};
use crate::error::{IcodeError, IcodeResult};

const GROUP: &str = "virtual_provider";
const HEALTH_KEY: &str = "health_check_interval_secs";
const FALLBACK_KEY: &str = "all_failed_fallback_interval_secs";
const ATTEMPTS_KEEP_KEY: &str = "attempts_keep_days";

/// 探活间隔默认值（秒）：1 小时
pub const DEFAULT_HEALTH_CHECK_INTERVAL_SECS: u64 = 3600;
/// 全部失活兜底间隔默认值（秒）：30 分钟
pub const DEFAULT_ALL_FAILED_FALLBACK_INTERVAL_SECS: u64 = 1800;
/// 允许配置的最小间隔（秒）
pub const MIN_INTERVAL_SECS: u64 = 30;
/// 允许配置的最大间隔（秒）：24 小时
pub const MAX_INTERVAL_SECS: u64 = 24 * 3600;
/// 路由尝试历史默认保留天数
pub const DEFAULT_ATTEMPTS_KEEP_DAYS: u32 = 3;
/// 路由尝试历史允许保留的最小天数
pub const MIN_ATTEMPTS_KEEP_DAYS: u32 = 1;
/// 路由尝试历史允许保留的最大天数
pub const MAX_ATTEMPTS_KEEP_DAYS: u32 = 365;

/// 读取一个无符号整数配置，非法或缺失时返回默认值
fn read_u32(key: &str, default: u32, min: u32, max: u32) -> u32 {
    get_db_pool()
        .ok()
        .and_then(|pool| pool.get().ok())
        .and_then(|conn| {
            global_config::get_global_config(&conn, GROUP, key)
                .ok()
                .flatten()
        })
        .and_then(|raw| raw.trim().parse::<u32>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

/// 读取一个间隔配置（秒），非法或缺失时返回默认值
fn read_interval_secs(key: &str, default: u64) -> u64 {
    read_u32(key, default as u32, MIN_INTERVAL_SECS as u32, MAX_INTERVAL_SECS as u32) as u64
}

/// 降级路由探活间隔（秒）
pub fn health_check_interval_secs() -> u64 {
    read_interval_secs(HEALTH_KEY, DEFAULT_HEALTH_CHECK_INTERVAL_SECS)
}

/// 全部路由永久失活时的兜底尝试间隔（秒）
pub fn all_failed_fallback_interval_secs() -> u64 {
    read_interval_secs(FALLBACK_KEY, DEFAULT_ALL_FAILED_FALLBACK_INTERVAL_SECS)
}

/// 路由尝试历史保留天数
pub fn attempts_keep_days() -> u32 {
    read_u32(
        ATTEMPTS_KEEP_KEY,
        DEFAULT_ATTEMPTS_KEEP_DAYS,
        MIN_ATTEMPTS_KEEP_DAYS,
        MAX_ATTEMPTS_KEEP_DAYS,
    )
}

fn write_interval_secs(key: &str, secs: u64) -> IcodeResult<()> {
    if !(MIN_INTERVAL_SECS..=MAX_INTERVAL_SECS).contains(&secs) {
        return Err(IcodeError::validation(format!(
            "间隔需在 {} 秒到 {} 秒之间",
            MIN_INTERVAL_SECS, MAX_INTERVAL_SECS
        )));
    }
    let conn = get_db_pool()?.get()?;
    global_config::set_global_config(&conn, GROUP, key, &secs.to_string())
}

fn write_attempts_keep_days(days: u32) -> IcodeResult<()> {
    if !(MIN_ATTEMPTS_KEEP_DAYS..=MAX_ATTEMPTS_KEEP_DAYS).contains(&days) {
        return Err(IcodeError::validation(format!(
            "保留天数需在 {} 天到 {} 天之间",
            MIN_ATTEMPTS_KEEP_DAYS, MAX_ATTEMPTS_KEEP_DAYS
        )));
    }
    let conn = get_db_pool()?.get()?;
    global_config::set_global_config(&conn, GROUP, ATTEMPTS_KEEP_KEY, &days.to_string())
}

/// 更新探活 / 兜底间隔 / 尝试历史保留天数（`None` 表示不修改）
pub fn set_intervals(
    health_check: Option<u64>,
    all_failed_fallback: Option<u64>,
    attempts_keep: Option<u32>,
) -> IcodeResult<()> {
    if let Some(v) = health_check {
        write_interval_secs(HEALTH_KEY, v)?;
    }
    if let Some(v) = all_failed_fallback {
        write_interval_secs(FALLBACK_KEY, v)?;
    }
    if let Some(v) = attempts_keep {
        write_attempts_keep_days(v)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interval_bounds() {
        assert_eq!(DEFAULT_HEALTH_CHECK_INTERVAL_SECS, 3600);
        assert_eq!(DEFAULT_ALL_FAILED_FALLBACK_INTERVAL_SECS, 1800);
        assert!(MIN_INTERVAL_SECS < MAX_INTERVAL_SECS);
    }
}
