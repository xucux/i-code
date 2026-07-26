//! # 脚本模板数据访问层
//!
//! 直接操作 `script_templates` 表。

use chrono::Utc;

use crate::core::id::generate_id;
use crate::db::{get_db_pool, DbConn};
use crate::error::{IcodeError, IcodeResult};

use super::types::{
    CreateScriptTemplateInput, ScriptTemplate, ScriptTemplateListFilter, ScriptTemplateRef,
    ScriptTemplateSelectItem, UpdateScriptTemplateInput,
};

fn get_conn() -> IcodeResult<DbConn> {
    Ok(get_db_pool()?.get()?)
}

fn new_id() -> String {
    generate_id()
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

const SELECT_SQL: &str = "SELECT id, name, slug, kind, status, description, script_body, engine,
    default_timeout_ms, allowed_hosts_json, snippet_id, last_test_at, last_test_ok,
    last_test_message, sort_order, created_at, updated_at FROM script_templates";

fn row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScriptTemplate> {
    let last_test_ok_i: Option<i64> = row.get(12)?;
    Ok(ScriptTemplate {
        id: row.get(0)?,
        name: row.get(1)?,
        slug: row.get(2)?,
        kind: row.get(3)?,
        status: row.get(4)?,
        description: row.get(5)?,
        script_body: row.get(6)?,
        engine: row.get(7)?,
        default_timeout_ms: row.get(8)?,
        allowed_hosts_json: row.get(9)?,
        snippet_id: row.get(10)?,
        last_test_at: row.get(11)?,
        last_test_ok: last_test_ok_i.map(|v| v != 0),
        last_test_message: row.get(13)?,
        sort_order: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

/// 插入脚本模板（默认 draft）
pub fn insert(input: &CreateScriptTemplateInput) -> IcodeResult<ScriptTemplate> {
    let conn = get_conn()?;
    let id = new_id();
    let now = now();

    conn.execute(
        "INSERT INTO script_templates
            (id, name, slug, kind, status, description, script_body, engine,
             default_timeout_ms, allowed_hosts_json, snippet_id, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'draft', ?5, ?6, 'rhai', ?7, ?8, ?9, ?10, ?11, ?12)",
        rusqlite::params![
            id,
            input.name,
            input.slug,
            input.kind,
            input.description,
            input.script_body,
            input.default_timeout_ms.max(1000),
            input.allowed_hosts_json,
            input.snippet_id,
            input.sort_order,
            now,
            now,
        ],
    )?;

    find_by_id(&id)
}

/// 按 ID 查找
pub fn find_by_id(id: &str) -> IcodeResult<ScriptTemplate> {
    let conn = get_conn()?;
    let sql = format!("{SELECT_SQL} WHERE id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], row_mapper)?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Err(IcodeError::not_found("ScriptTemplate", Some(id))),
    }
}

/// 按 slug 查找
pub fn find_by_slug(slug: &str) -> IcodeResult<Option<ScriptTemplate>> {
    let conn = get_conn()?;
    let sql = format!("{SELECT_SQL} WHERE slug = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([slug], row_mapper)?;
    if let Some(row) = rows.next() {
        Ok(Some(row?))
    } else {
        Ok(None)
    }
}

/// 列表筛选
pub fn list(filter: &ScriptTemplateListFilter) -> IcodeResult<Vec<ScriptTemplate>> {
    let conn = get_conn()?;
    let mut sql = SELECT_SQL.to_string();
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut idx = 1usize;

    if let Some(kind) = &filter.kind {
        conditions.push(format!("kind = ?{idx}"));
        params.push(Box::new(kind.clone()));
        idx += 1;
    }
    if let Some(status) = &filter.status {
        conditions.push(format!("status = ?{idx}"));
        params.push(Box::new(status.clone()));
        idx += 1;
    }
    if let Some(keyword) = &filter.keyword {
        let kw = format!("%{}%", keyword.trim());
        if !keyword.trim().is_empty() {
            conditions.push(format!(
                "(name LIKE ?{idx} OR slug LIKE ?{idx} OR IFNULL(description,'') LIKE ?{idx})"
            ));
            params.push(Box::new(kw));
            let _ = idx;
        }
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(" ORDER BY sort_order ASC, updated_at DESC");

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), row_mapper)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 列出启用中的额度脚本模板（下拉用）
pub fn list_active_for_select(kind: &str) -> IcodeResult<Vec<ScriptTemplateSelectItem>> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, slug FROM script_templates
         WHERE kind = ?1 AND status = 'active'
         ORDER BY sort_order ASC, name ASC",
    )?;
    let rows = stmt.query_map([kind], |row| {
        Ok(ScriptTemplateSelectItem {
            id: row.get(0)?,
            name: row.get(1)?,
            slug: row.get(2)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 更新模板元数据与正文
pub fn update(id: &str, input: &UpdateScriptTemplateInput) -> IcodeResult<ScriptTemplate> {
    let conn = get_conn()?;
    let now = now();

    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut idx = 1usize;

    if let Some(v) = &input.name {
        sets.push(format!("name = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &input.slug {
        sets.push(format!("slug = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &input.description {
        sets.push(format!("description = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &input.script_body {
        sets.push(format!("script_body = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.default_timeout_ms {
        sets.push(format!("default_timeout_ms = ?{idx}"));
        params.push(Box::new(v.max(1000)));
        idx += 1;
    }
    if let Some(v) = &input.allowed_hosts_json {
        sets.push(format!("allowed_hosts_json = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.sort_order {
        sets.push(format!("sort_order = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }

    if sets.is_empty() {
        return find_by_id(id);
    }

    sets.push(format!("updated_at = ?{idx}"));
    params.push(Box::new(now));
    idx += 1;
    params.push(Box::new(id.to_string()));

    let sql = format!(
        "UPDATE script_templates SET {} WHERE id = ?{}",
        sets.join(", "),
        idx
    );
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, param_refs.as_slice())?;
    if affected == 0 {
        return Err(IcodeError::not_found("ScriptTemplate", Some(id)));
    }
    find_by_id(id)
}

/// 更新状态
pub fn update_status(id: &str, status: &str) -> IcodeResult<ScriptTemplate> {
    let conn = get_conn()?;
    let now = now();
    let affected = conn.execute(
        "UPDATE script_templates SET status = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![status, now, id],
    )?;
    if affected == 0 {
        return Err(IcodeError::not_found("ScriptTemplate", Some(id)));
    }
    find_by_id(id)
}

/// 更新试运行结果摘要
pub fn update_last_test(
    id: &str,
    ok: bool,
    message: &str,
) -> IcodeResult<()> {
    let conn = get_conn()?;
    let now = now();
    let affected = conn.execute(
        "UPDATE script_templates
         SET last_test_at = ?1, last_test_ok = ?2, last_test_message = ?3, updated_at = ?4
         WHERE id = ?5",
        rusqlite::params![now, if ok { 1i64 } else { 0i64 }, message, now, id],
    )?;
    if affected == 0 {
        return Err(IcodeError::not_found("ScriptTemplate", Some(id)));
    }
    Ok(())
}

/// 删除模板
pub fn delete(id: &str) -> IcodeResult<()> {
    let conn = get_conn()?;
    let affected = conn.execute("DELETE FROM script_templates WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(IcodeError::not_found("ScriptTemplate", Some(id)));
    }
    Ok(())
}

/// 查询引用该模板的供应商（JSON 模糊匹配，MVP）
pub fn list_refs(template_id: &str) -> IcodeResult<Vec<ScriptTemplateRef>> {
    let conn = get_conn()?;
    // balance_provider_json 中存 camelCase：scriptTemplateId
    let pattern = format!("%{template_id}%");
    let mut stmt = conn.prepare(
        "SELECT id, slug, display_name, balance_provider_json
         FROM providers
         WHERE balance_provider_json IS NOT NULL
           AND balance_provider_json LIKE '%\"method\":\"script\"%'
           AND balance_provider_json LIKE ?1",
    )?;
    let rows = stmt.query_map([pattern], |row| {
        let id: String = row.get(0)?;
        let slug: String = row.get(1)?;
        let display_name: String = row.get(2)?;
        let json: String = row.get(3)?;
        Ok((id, slug, display_name, json))
    })?;

    let mut result = Vec::new();
    for row in rows {
        let (id, slug, display_name, json) = row?;
        // 二次校验：解析 JSON 确认 scriptTemplateId 精确匹配
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json) {
            let tid = val
                .get("scriptTemplateId")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if tid == template_id {
                result.push(ScriptTemplateRef {
                    provider_id: id,
                    slug,
                    display_name,
                });
            }
        }
    }
    Ok(result)
}
