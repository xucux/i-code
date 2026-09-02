//! # 媒体生成历史仓库层
//!
//! 仅负责 `media_generations` 表的 SQL 读写，禁止调用 Service 或发送事件。
//! 跨表写操作（生成记录）为单表插入，无需事务。

use rusqlite::params;

use crate::db::connection::get_db_pool;
use crate::db::schema::table;
use crate::error::{IcodeError, IcodeResult};
use crate::modules::media_generation::types::MediaGeneration;

/// 获取一个数据库连接
fn get_conn() -> IcodeResult<crate::db::connection::DbConn> {
    Ok(get_db_pool()?.get()?)
}

/// `media_generations` 表 SELECT 字段列表
const MEDIA_GENERATION_SELECT_SQL: &str = "SELECT id, provider_id, provider_slug, model_id,
    prompt, params_json, status, asset_paths_json, source_urls_json, error_message,
    duration_ms, created_at
    FROM media_generations";

/// 行映射器：将数据库行转换为 [`MediaGeneration`] DTO
fn media_generation_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<MediaGeneration> {
    // 列顺序见 MEDIA_GENERATION_SELECT_SQL
    let params_json: Option<String> = row.get(5)?;
    let asset_paths_json: Option<String> = row.get(7)?;
    let source_urls_json: Option<String> = row.get(8)?;
    Ok(MediaGeneration {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        provider_slug: row.get(2)?,
        model_id: row.get(3)?,
        prompt: row.get(4)?,
        params: params_json.and_then(|s| serde_json::from_str(&s).ok()),
        status: row.get(6)?,
        asset_paths: asset_paths_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        source_urls: source_urls_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        error_message: row.get(9)?,
        duration_ms: row.get(10)?,
        created_at: row.get(11)?,
    })
}

/// 插入图像生成历史记录
pub fn insert_generation(record: &MediaGeneration) -> IcodeResult<()> {
    let conn = get_conn()?;
    conn.execute(
        &format!(
            "INSERT INTO {t} (id, provider_id, provider_slug, model_id, prompt, params_json,
                status, asset_paths_json, source_urls_json, error_message, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            t = table::MEDIA_GENERATIONS
        ),
        params![
            record.id,
            record.provider_id,
            record.provider_slug,
            record.model_id,
            record.prompt,
            record.params.as_ref().map(|v| v.to_string()),
            record.status,
            serde_json::to_string(&record.asset_paths).ok(),
            serde_json::to_string(&record.source_urls).ok(),
            record.error_message,
            record.duration_ms,
            record.created_at,
        ],
    )?;
    Ok(())
}

/// 按 ID 查询生成历史记录
pub fn find_generation_by_id(id: &str) -> IcodeResult<MediaGeneration> {
    let conn = get_conn()?;
    let sql = format!(
        "{MEDIA_GENERATION_SELECT_SQL} WHERE id = ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], media_generation_row_mapper)?;
    match rows.next() {
        Some(row) => row.map_err(IcodeError::from),
        None => Err(IcodeError::not_found("MediaGeneration", Some(id))),
    }
}

/// 按创建时间倒序列出生成历史
///
/// `limit` 限制返回条数（None 时默认 200），供画廊页分页前的 MVP 场景使用。
pub fn list_generations(limit: Option<i64>) -> IcodeResult<Vec<MediaGeneration>> {
    let conn = get_conn()?;
    let limit = limit.unwrap_or(200).clamp(1, 1000);
    let sql = format!(
        "{MEDIA_GENERATION_SELECT_SQL} ORDER BY created_at DESC LIMIT ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([limit], media_generation_row_mapper)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 删除生成历史记录
///
/// 返回被删除的记录（供 Service 层清理产物文件）。
pub fn delete_generation(id: &str) -> IcodeResult<MediaGeneration> {
    let record = find_generation_by_id(id)?;
    let conn = get_conn()?;
    let affected = conn.execute(
        &format!("DELETE FROM {} WHERE id = ?1", table::MEDIA_GENERATIONS),
        [id],
    )?;
    if affected == 0 {
        return Err(IcodeError::not_found("MediaGeneration", Some(id)));
    }
    Ok(record)
}
