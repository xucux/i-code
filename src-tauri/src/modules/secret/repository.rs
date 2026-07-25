//! # 敏感凭据数据访问层
//!
//! 直接操作 `secrets` 表的 CRUD，不包含业务逻辑。
//! Service 层通过此 Repository 读写数据，加密/解密由 Service 层负责。
//!
//! ## 表结构
//!
//! ```sql
//! CREATE TABLE secrets (
//!   id TEXT PRIMARY KEY,
//!   kind TEXT NOT NULL,
//!   encrypted_value BLOB NOT NULL,
//!   label TEXT,
//!   created_at TEXT NOT NULL,
//!   updated_at TEXT NOT NULL
//! );
//! ```

use chrono::Utc;

use crate::db::{get_db_pool, DbConn};
use crate::error::{IcodeError, IcodeResult};

use super::types::{SecretKind, SecretMask};

/// Secret 行的完整字段（包含密文）
///
/// 仅在后端 Service 层使用，禁止序列化后返回前端。
#[derive(Debug, Clone)]
pub struct SecretRow {
    pub id: String,
    pub kind: String,
    pub encrypted_value: Vec<u8>,
    pub label: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 获取一个数据库连接
fn get_conn() -> IcodeResult<DbConn> {
    Ok(get_db_pool()?.get()?)
}

/// 按 ID 查询单条 Secret
pub fn find_by_id(id: &str) -> IcodeResult<Option<SecretRow>> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, kind, encrypted_value, label, created_at, updated_at
         FROM secrets WHERE id = ?1",
    )?;
    let mut rows = stmt.query([id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(SecretRow {
            id: row.get(0)?,
            kind: row.get(1)?,
            encrypted_value: row.get(2)?,
            label: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        }))
    } else {
        Ok(None)
    }
}

/// 列出所有 Secret（仅返回掩码视图，不包含密文）
pub fn list_all() -> IcodeResult<Vec<SecretMask>> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, kind, label, created_at, updated_at
         FROM secrets ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let kind_str: String = row.get(1)?;
        let kind = SecretKind::from_str(&kind_str).unwrap_or(SecretKind::ApiKey);
        Ok(SecretMask {
            id: row.get(0)?,
            kind,
            label: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 插入新 Secret
///
/// `encrypted_value` 为已加密的二进制密文（nonce + ciphertext_with_tag）
pub fn insert(
    id: &str,
    kind: SecretKind,
    encrypted_value: &[u8],
    label: Option<&str>,
) -> IcodeResult<()> {
    let conn = get_conn()?;
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO secrets (id, kind, encrypted_value, label, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            id,
            kind.as_str(),
            encrypted_value,
            label,
            now,
            now,
        ],
    )?;
    Ok(())
}

/// 更新已有 Secret 的密文与标签
pub fn update(
    id: &str,
    encrypted_value: &[u8],
    label: Option<&str>,
) -> IcodeResult<()> {
    let conn = get_conn()?;
    let now = Utc::now().to_rfc3339();
    let affected = conn.execute(
        "UPDATE secrets
         SET encrypted_value = ?2, label = ?3, updated_at = ?4
         WHERE id = ?1",
        rusqlite::params![id, encrypted_value, label, now],
    )?;
    if affected == 0 {
        return Err(IcodeError::not_found("Secret", Some(id)));
    }
    Ok(())
}

/// 删除指定 Secret
///
/// 不存在时返回 Ok(())（幂等删除）
pub fn delete(id: &str) -> IcodeResult<()> {
    let conn = get_conn()?;
    conn.execute("DELETE FROM secrets WHERE id = ?1", [id])?;
    Ok(())
}

/// 查询所有 Secret ID（用于清理孤立 Secret 时扫描引用）
pub fn list_all_ids() -> IcodeResult<Vec<String>> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare("SELECT id FROM secrets")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 删除多个 Secret（批量）
pub fn delete_batch(ids: &[String]) -> IcodeResult<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    let conn = get_conn()?;
    let mut affected = 0;
    // SQLite 参数数量上限约为 999，分批处理
    for chunk in ids.chunks(500) {
        let placeholders: Vec<String> = (0..chunk.len())
            .map(|i| format!("?{}", i + 1))
            .collect();
        let sql = format!("DELETE FROM secrets WHERE id IN ({})", placeholders.join(", "));
        let params: Vec<&dyn rusqlite::ToSql> = chunk
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();
        affected += conn.execute(&sql, params.as_slice())?;
    }
    Ok(affected)
}
