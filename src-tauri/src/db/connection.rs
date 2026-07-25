//! # 数据库连接池管理
//!
//! 使用 `r2d2` + `r2d2_sqlite` 维护全局 SQLite 连接池。
//!
//! ## 设计要点
//!
//! - **单例连接池**：通过 [`once_cell::sync::Lazy`] 全局持有，避免每次调用 Command 都重新连接。
//! - **WAL 模式**：开启 `journal_mode = WAL`，提升并发写入性能，允许读写并发。
//! - **外键约束**：开启 `foreign_keys = ON`，保证级联删除/更新生效。
//! - **忙等待超时**：5000ms，避免短时间锁冲突立即报错。
//!
//! ## 使用方式
//!
//! ```ignore
//! use crate::db::get_db_pool;
//!
//! let conn = get_db_pool()?.get()?;
//! conn.execute("SELECT 1", [])?;
//! ```

use std::path::Path;
use std::sync::Mutex;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::error::{IcodeError, IcodeResult};

/// SQLite 连接池别名
pub type DbPool = Pool<SqliteConnectionManager>;

/// 池中获取的连接别名
pub type DbConn = r2d2::PooledConnection<SqliteConnectionManager>;

/// 全局连接池单例
///
/// 使用 `Mutex<Option<>>` 而非 `OnceLock`，因为备份恢复场景需要关闭并重新初始化连接池。
static DB_POOL: Mutex<Option<DbPool>> = Mutex::new(None);

/// 初始化全局数据库连接池
///
/// 应用启动时调用一次。后续 [`get_db_pool`] 从全局静态变量获取句柄。
///
/// # 参数
/// - `db_path`：数据库文件完整路径。若文件不存在会自动创建。
///
/// # 错误
/// - 数据库文件所在目录不存在时返回 `DATABASE` 错误。
/// - 连接池构建失败返回 `DATABASE` 错误。
pub fn init_db_pool(db_path: &Path) -> IcodeResult<()> {
    // 确保数据目录存在，避免 SQLite 创建文件失败
    if let Some(parent) = db_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let manager = SqliteConnectionManager::file(db_path)
        .with_init(|c| {
            // 启用外键约束（默认关闭）
            c.execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA busy_timeout = 5000;",
            )
        });
    let pool = Pool::builder()
        .max_size(8) // 单进程内 8 个连接足够，过多会占用 SQLite 锁
        .build(manager)
        .map_err(|e| IcodeError::database(format!("初始化数据库连接池失败：{e}")))?;

    let mut guard = DB_POOL.lock().map_err(|e| {
        IcodeError::internal(format!("获取数据库连接池锁失败：{e}"))
    })?;
    *guard = Some(pool);
    Ok(())
}

/// 获取全局连接池句柄
///
/// 在 Tauri Command 中获取连接的标准方式：
/// ```ignore
/// let pool = get_db_pool()?;
/// let conn = pool.get()?;
/// ```
pub fn get_db_pool() -> IcodeResult<DbPool> {
    DB_POOL
        .lock()
        .map_err(|e| IcodeError::internal(format!("获取数据库连接池锁失败：{e}")))?
        .clone()
        .ok_or_else(|| IcodeError::internal("数据库连接池尚未初始化，请先调用 init_db_pool"))
}

/// 关闭并释放全局数据库连接池
///
/// 取出池中所有连接并释放，确保 SQLite 数据库文件句柄被关闭。
/// 主要用于备份恢复等需要替换数据库文件的场景。
#[allow(dead_code)]
pub fn close_db_pool() -> IcodeResult<()> {
    let pool = DB_POOL
        .lock()
        .map_err(|e| IcodeError::internal(format!("获取数据库连接池锁失败：{e}")))?
        .take();
    if let Some(pool) = pool {
        // r2d2 Pool 的 Drop 实现会关闭内部所有连接
        drop(pool);
    }
    Ok(())
}

/// 重置数据库连接池
///
/// 先关闭旧连接池，再按指定路径重新初始化。用于恢复备份后重新加载数据库。
#[allow(dead_code)]
pub fn reset_db_pool(db_path: &Path) -> IcodeResult<()> {
    close_db_pool()?;
    init_db_pool(db_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_pool_in_memory() {
        // 测试用内存数据库，验证连接池基础功能
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(2).build(manager).unwrap();
        let conn = pool.get().unwrap();
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY)", []).unwrap();
        conn.execute("INSERT INTO t (id) VALUES (1)", []).unwrap();
    }
}
