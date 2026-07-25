//! # 全局配置键值访问层
//!
//! 提供 `global_configs` 表的通用读取接口。
//! 该表用于存储不适合放入单例表的零散全局配置，例如 OAuth 预设凭据。
//!
//! ## 键名约定
//!
//! - `group` 用于分类（如 `oauth`）。
//! - `key` 在 group 内唯一，使用 `snake_case`。
//! - 写入时由调用方保证 id / group / key 的规范性；本模块只负责读取。

use crate::db::schema::table;
use crate::db::DbConn;
use crate::error::{IcodeError, IcodeResult};

/// 读取单个全局配置值
///
/// # 参数
/// - `conn`：数据库连接
/// - `group`：配置分组
/// - `key`：配置键
///
/// # 返回
/// - `Ok(Some(value))`：找到配置
/// - `Ok(None)`：配置不存在
/// - `Err(DATABASE)`：查询失败
pub fn get_global_config(conn: &DbConn, group: &str, key: &str) -> IcodeResult<Option<String>> {
    let result = conn.query_row(
        &format!(
            "SELECT value FROM {table} WHERE \"group\" = ?1 AND key = ?2",
            table = table::GLOBAL_CONFIGS
        ),
        rusqlite::params![group, key],
        |row| row.get(0),
    );

    match result {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(IcodeError::database(format!(
            "读取 global_configs [{}/{}] 失败：{e}",
            group, key
        ))),
    }
}
