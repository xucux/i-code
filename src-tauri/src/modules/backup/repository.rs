//! # 备份模块数据访问层
//!
//! 直接操作 `webdav_configs` 表的 CRUD。
//! 按当前业务需求，WebDAV 密码以明文存储在该表中。

use chrono::Utc;
use rusqlite::params;

use crate::core::id::generate_id;
use crate::db::{get_db_pool, DbConn};
use crate::error::{IcodeError, IcodeResult};

use super::types::{SaveWebDavConfigInput, WebDavConfigRecord, WebDavPreset};

/// 获取一个数据库连接
fn get_conn() -> IcodeResult<DbConn> {
    Ok(get_db_pool()?.get()?)
}

/// 将 SQLite 0/1 转换为 bool
fn i64_to_bool(v: i64) -> bool {
    v != 0
}

/// 从数据库行构造 WebDavConfigRecord
fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<WebDavConfigRecord> {
    let preset_str: String = row.get("preset")?;
    let preset = WebDavPreset::from_str(&preset_str).unwrap_or(WebDavPreset::Custom);

    Ok(WebDavConfigRecord {
        id: row.get("id")?,
        name: row.get("name")?,
        url: row.get("url")?,
        username: row.get("username")?,
        password: row.get("password")?,
        remote_path: row.get("remote_path")?,
        strict_ssl: i64_to_bool(row.get::<_, i64>("strict_ssl")?),
        preset,
        sort_order: row.get::<_, i64>("sort_order")? as u32,
        is_enabled: i64_to_bool(row.get::<_, i64>("is_enabled")?),
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// 列出所有 WebDAV 配置（按 sort_order 与创建时间排序）
pub fn list_webdav_configs() -> IcodeResult<Vec<WebDavConfigRecord>> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, url, username, password, remote_path, strict_ssl,
                preset, sort_order, is_enabled, created_at, updated_at
         FROM webdav_configs
         ORDER BY sort_order ASC, created_at ASC",
    )?;
    let rows = stmt.query_map([], row_to_record)?;
    let mut records = Vec::new();
    for row in rows {
        records.push(row?);
    }
    Ok(records)
}

/// 获取单个 WebDAV 配置
pub fn get_webdav_config(id: &str) -> IcodeResult<Option<WebDavConfigRecord>> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, url, username, password, remote_path, strict_ssl,
                preset, sort_order, is_enabled, created_at, updated_at
         FROM webdav_configs WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_record(row)?))
    } else {
        Ok(None)
    }
}

/// 保存 WebDAV 配置
///
/// - `id` 为 None 时新建记录
/// - `id` 存在且数据库中存在时更新记录
/// - 返回保存后的完整记录
pub fn save_webdav_config(input: &SaveWebDavConfigInput) -> IcodeResult<WebDavConfigRecord> {
    let conn = get_conn()?;
    let now = Utc::now().to_rfc3339();

    let id = match &input.id {
        Some(id) if !id.is_empty() => id.clone(),
        _ => generate_id(),
    };

    let preset_str = input.preset.as_str();
    let remote_path = if input.remote_path.is_empty() {
        "/i-code-backups/".to_string()
    } else {
        input.remote_path.clone()
    };

    // 若传入 id 存在则尝试更新
    if let Some(existing) = get_webdav_config(&id)? {
        let _ = existing; // 仅用于确认记录存在
        conn.execute(
            "UPDATE webdav_configs
             SET name = ?1, url = ?2, username = ?3, password = ?4,
                 remote_path = ?5, strict_ssl = ?6, preset = ?7,
                 updated_at = ?8
             WHERE id = ?9",
            params![
                input.name,
                input.url,
                input.username,
                input.password,
                remote_path,
                input.strict_ssl as i64,
                preset_str,
                now,
                id,
            ],
        )?;
    } else {
        conn.execute(
            "INSERT INTO webdav_configs
             (id, name, url, username, password, remote_path, strict_ssl,
              preset, sort_order, is_enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                id,
                input.name,
                input.url,
                input.username,
                input.password,
                remote_path,
                input.strict_ssl as i64,
                preset_str,
                0i64,
                1i64,
                now,
                now,
            ],
        )?;
    }

    get_webdav_config(&id)?.ok_or_else(|| IcodeError::internal("保存 WebDAV 配置后读取失败"))
}

/// 删除 WebDAV 配置
pub fn delete_webdav_config(id: &str) -> IcodeResult<()> {
    let conn = get_conn()?;
    conn.execute("DELETE FROM webdav_configs WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webdav_preset_from_str() {
        assert_eq!(WebDavPreset::from_str("jianguoyun"), Some(WebDavPreset::Jianguoyun));
        assert_eq!(WebDavPreset::from_str("custom"), Some(WebDavPreset::Custom));
        assert_eq!(WebDavPreset::from_str("unknown"), None);
    }
}
