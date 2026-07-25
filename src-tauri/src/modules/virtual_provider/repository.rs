//! # 虚拟供应商数据访问层
//!
//! 直接操作 `virtual_providers` / `virtual_models` / `virtual_model_routes` 三表。

use chrono::Utc;

use crate::core::id::generate_id;
use crate::db::{get_db_pool, DbConn};
use crate::error::{IcodeError, IcodeResult};

use super::types::{
    CreateVirtualModelInput, CreateVirtualModelRouteInput, CreateVirtualProviderInput,
    ExposedVirtualModel, SaveVirtualModelInput, UpdateVirtualModelInput,
    UpdateVirtualModelRouteInput, UpdateVirtualProviderInput, VirtualModel, VirtualModelRoute,
    VirtualProvider, VirtualProviderStrategy,
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

// ===== virtual_providers =====

pub fn insert_provider(input: &CreateVirtualProviderInput) -> IcodeResult<VirtualProvider> {
    let conn = get_conn()?;
    let id = new_id();
    let now = now();
    let is_enabled_i: i64 = if input.is_enabled { 1 } else { 0 };
    let strategy = input
        .strategy
        .clone()
        .unwrap_or_else(|| VirtualProviderStrategy::Fallback.as_str().to_string());
    let max_retries = input.max_retries.max(0);
    let retry_interval_ms = input.retry_interval_ms.max(0);

    conn.execute(
        "INSERT INTO virtual_providers
            (id, name, alias, display_name, is_enabled, strategy, max_retries,
             retry_interval_ms, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            id,
            input.name,
            input.alias,
            input.display_name,
            is_enabled_i,
            strategy,
            max_retries,
            retry_interval_ms,
            now,
            now,
        ],
    )?;

    find_provider_by_id(&id)
}

pub fn find_provider_by_id(id: &str) -> IcodeResult<VirtualProvider> {
    let conn = get_conn()?;
    let sql = format!("{PROVIDER_SELECT_SQL} WHERE id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], provider_row_mapper)?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Err(IcodeError::not_found("VirtualProvider", Some(id))),
    }
}

pub fn find_provider_by_alias(alias: &str) -> IcodeResult<Option<VirtualProvider>> {
    let conn = get_conn()?;
    let sql = format!("{PROVIDER_SELECT_SQL} WHERE alias = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([alias], provider_row_mapper)?;
    if let Some(row) = rows.next() {
        Ok(Some(row?))
    } else {
        Ok(None)
    }
}

pub fn list_providers() -> IcodeResult<Vec<VirtualProvider>> {
    let conn = get_conn()?;
    let sql = format!("{PROVIDER_SELECT_SQL} ORDER BY created_at ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], provider_row_mapper)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn update_provider(id: &str, input: &UpdateVirtualProviderInput) -> IcodeResult<VirtualProvider> {
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
    if let Some(v) = &input.alias {
        sets.push(format!("alias = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &input.display_name {
        sets.push(format!("display_name = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.is_enabled {
        sets.push(format!("is_enabled = ?{idx}"));
        params.push(Box::new(v as i64));
        idx += 1;
    }
    if let Some(v) = &input.strategy {
        sets.push(format!("strategy = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.max_retries {
        sets.push(format!("max_retries = ?{idx}"));
        params.push(Box::new(v.max(0)));
        idx += 1;
    }
    if let Some(v) = input.retry_interval_ms {
        sets.push(format!("retry_interval_ms = ?{idx}"));
        params.push(Box::new(v.max(0)));
        idx += 1;
    }

    if sets.is_empty() {
        return find_provider_by_id(id);
    }

    sets.push(format!("updated_at = ?{idx}"));
    params.push(Box::new(now));

    let sql = format!(
        "UPDATE virtual_providers SET {} WHERE id = ?{}",
        sets.join(", "),
        idx + 1
    );
    params.push(Box::new(id.to_string()));

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, params_ref.as_slice())?;
    if affected == 0 {
        return Err(IcodeError::not_found("VirtualProvider", Some(id)));
    }

    find_provider_by_id(id)
}

pub fn delete_provider(id: &str) -> IcodeResult<()> {
    let conn = get_conn()?;
    let affected = conn.execute("DELETE FROM virtual_providers WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(IcodeError::not_found("VirtualProvider", Some(id)));
    }
    Ok(())
}

/// 保存虚拟模型及其子级路由（事务）
///
/// - `id` 存在时更新虚拟模型（并校验属于指定供应商），否则新建；
/// - 保存完成后删除该虚拟模型的全部已有路由，按提交顺序重新写入 `routes`。
pub fn save_model(input: &SaveVirtualModelInput) -> IcodeResult<VirtualModel> {
    let conn = get_conn()?;
    let tx = conn.unchecked_transaction()?;
    let now = now();

    let virtual_model_id = if let Some(id) = &input.id {
        tx.execute(
            "UPDATE virtual_models
             SET model_id = ?1, display_name = ?2, is_enabled = ?3, updated_at = ?4
             WHERE id = ?5 AND virtual_provider_id = ?6",
            rusqlite::params![
                input.model_id,
                input.display_name,
                input.is_enabled as i64,
                now,
                id,
                input.virtual_provider_id,
            ],
        )?;
        id.clone()
    } else {
        let vm_id = new_id();
        tx.execute(
            "INSERT INTO virtual_models
                (id, virtual_provider_id, model_id, display_name, is_enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                vm_id,
                input.virtual_provider_id,
                input.model_id,
                input.display_name,
                input.is_enabled as i64,
                now,
                now,
            ],
        )?;
        vm_id
    };

    // 删除该虚拟模型下的全部已有路由，随后重新写入
    tx.execute(
        "DELETE FROM virtual_model_routes WHERE virtual_model_id = ?1",
        [&virtual_model_id],
    )?;

    for route in &input.routes {
        let route_id = new_id();
        let enabled_i: i64 = if route.enabled { 1 } else { 0 };
        let is_healthy_i: i64 = if route.is_healthy { 1 } else { 0 };
        tx.execute(
            "INSERT INTO virtual_model_routes
                (id, virtual_model_id, target_provider_id, target_model_id, priority,
                 enabled, max_retries, retry_interval_ms, timeout_ms, is_healthy, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                route_id,
                virtual_model_id,
                route.target_provider_id,
                route.target_model_id,
                route.priority,
                enabled_i,
                route.max_retries.max(0),
                route.retry_interval_ms.max(0),
                None::<i64>,
                is_healthy_i,
                now,
                now,
            ],
        )?;
    }

    tx.commit()?;
    find_model_by_id(&virtual_model_id)
}

const PROVIDER_SELECT_SQL: &str = "SELECT id, name, alias, display_name, is_enabled,
    strategy, max_retries, retry_interval_ms, created_at, updated_at FROM virtual_providers";

fn provider_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<VirtualProvider> {
    let is_enabled: i64 = row.get(4)?;
    Ok(VirtualProvider {
        id: row.get(0)?,
        name: row.get(1)?,
        alias: row.get(2)?,
        display_name: row.get(3)?,
        is_enabled: is_enabled != 0,
        strategy: row.get(5)?,
        max_retries: row.get(6)?,
        retry_interval_ms: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

// ===== virtual_models =====

pub fn insert_model(input: &CreateVirtualModelInput) -> IcodeResult<VirtualModel> {
    let conn = get_conn()?;
    let id = new_id();
    let now = now();
    let is_enabled_i: i64 = if input.is_enabled { 1 } else { 0 };

    conn.execute(
        "INSERT INTO virtual_models
            (id, virtual_provider_id, model_id, display_name, is_enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            id,
            input.virtual_provider_id,
            input.model_id,
            input.display_name,
            is_enabled_i,
            now,
            now,
        ],
    )?;

    find_model_by_id(&id)
}

pub fn find_model_by_id(id: &str) -> IcodeResult<VirtualModel> {
    let conn = get_conn()?;
    let sql = format!("{MODEL_SELECT_SQL} WHERE id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], model_row_mapper)?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Err(IcodeError::not_found("VirtualModel", Some(id))),
    }
}

pub fn find_model_by_provider_and_model_id(
    virtual_provider_id: &str,
    model_id: &str,
) -> IcodeResult<Option<VirtualModel>> {
    let conn = get_conn()?;
    let sql = format!(
        "{MODEL_SELECT_SQL} WHERE virtual_provider_id = ?1 AND model_id = ?2"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([virtual_provider_id, model_id], model_row_mapper)?;
    if let Some(row) = rows.next() {
        Ok(Some(row?))
    } else {
        Ok(None)
    }
}

pub fn list_models_by_provider(virtual_provider_id: &str) -> IcodeResult<Vec<VirtualModel>> {
    let conn = get_conn()?;
    let sql = format!(
        "{MODEL_SELECT_SQL} WHERE virtual_provider_id = ?1 ORDER BY created_at ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([virtual_provider_id], model_row_mapper)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 列出所有已启用的虚拟供应商下已启用的虚拟模型
///
/// 用于 `/v1/models` 接口数据源，返回结果可直接构造 `{virtual_alias}/{model_id}` 对外 ID。
pub fn list_exposed_virtual_models() -> IcodeResult<Vec<ExposedVirtualModel>> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare(
        "SELECT vm.id, vm.virtual_provider_id, vp.alias, vm.model_id, vm.display_name
         FROM virtual_models vm
         INNER JOIN virtual_providers vp ON vp.id = vm.virtual_provider_id
         WHERE vp.is_enabled = 1 AND vm.is_enabled = 1
         ORDER BY vp.alias ASC, vm.model_id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let model_id: String = row.get(3)?;
        let alias: String = row.get(2)?;
        let display_name: Option<String> = row.get(4)?;
        Ok(ExposedVirtualModel {
            id: format!("{}/{}", alias, model_id),
            virtual_provider_id: row.get(1)?,
            alias: alias.clone(),
            model_id: model_id.clone(),
            display_name: display_name.unwrap_or_else(|| model_id.clone()),
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn update_model(id: &str, input: &UpdateVirtualModelInput) -> IcodeResult<VirtualModel> {
    let conn = get_conn()?;
    let now = now();

    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut idx = 1usize;

    if let Some(v) = &input.model_id {
        sets.push(format!("model_id = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &input.display_name {
        sets.push(format!("display_name = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.is_enabled {
        sets.push(format!("is_enabled = ?{idx}"));
        params.push(Box::new(v as i64));
        idx += 1;
    }

    if sets.is_empty() {
        return find_model_by_id(id);
    }

    sets.push(format!("updated_at = ?{idx}"));
    params.push(Box::new(now));

    let sql = format!(
        "UPDATE virtual_models SET {} WHERE id = ?{}",
        sets.join(", "),
        idx + 1
    );
    params.push(Box::new(id.to_string()));

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, params_ref.as_slice())?;
    if affected == 0 {
        return Err(IcodeError::not_found("VirtualModel", Some(id)));
    }

    find_model_by_id(id)
}

pub fn delete_model(id: &str) -> IcodeResult<()> {
    let conn = get_conn()?;
    let affected = conn.execute("DELETE FROM virtual_models WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(IcodeError::not_found("VirtualModel", Some(id)));
    }
    Ok(())
}

const MODEL_SELECT_SQL: &str = "SELECT id, virtual_provider_id, model_id, display_name,
    is_enabled, created_at, updated_at FROM virtual_models";

fn model_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<VirtualModel> {
    let is_enabled: i64 = row.get(4)?;
    Ok(VirtualModel {
        id: row.get(0)?,
        virtual_provider_id: row.get(1)?,
        model_id: row.get(2)?,
        display_name: row.get(3)?,
        is_enabled: is_enabled != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

// ===== virtual_model_routes =====

pub fn insert_route(input: &CreateVirtualModelRouteInput) -> IcodeResult<VirtualModelRoute> {
    let conn = get_conn()?;
    let id = new_id();
    let now = now();
    let enabled_i: i64 = if input.enabled { 1 } else { 0 };
    let is_healthy_i: i64 = 1;

    conn.execute(
        "INSERT INTO virtual_model_routes
            (id, virtual_model_id, target_provider_id, target_model_id, priority,
             enabled, max_retries, retry_interval_ms, timeout_ms, is_healthy, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        rusqlite::params![
            id,
            input.virtual_model_id,
            input.target_provider_id,
            input.target_model_id,
            input.priority,
            enabled_i,
            input.max_retries.max(0),
            input.retry_interval_ms.max(0),
            input.timeout_ms,
            is_healthy_i,
            now,
            now,
        ],
    )?;

    find_route_by_id(&id)
}

pub fn find_route_by_id(id: &str) -> IcodeResult<VirtualModelRoute> {
    let conn = get_conn()?;
    let sql = format!("{ROUTE_SELECT_SQL} WHERE virtual_model_routes.id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], route_row_mapper)?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Err(IcodeError::not_found("VirtualModelRoute", Some(id))),
    }
}

pub fn list_routes_by_virtual_model(virtual_model_id: &str) -> IcodeResult<Vec<VirtualModelRoute>> {
    let conn = get_conn()?;
    let sql = format!(
        "{ROUTE_SELECT_SQL} WHERE virtual_model_id = ?1 AND enabled = 1
         ORDER BY priority ASC, created_at ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([virtual_model_id], route_row_mapper)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 列出指定虚拟供应商下所有已启用的路由
/// 列出指定虚拟供应商下所有已启用路由
///
/// 通过 `virtual_models` 表关联查询，返回该供应商全部虚拟模型的路由。
pub fn list_routes_by_provider(virtual_provider_id: &str) -> IcodeResult<Vec<VirtualModelRoute>> {
    let conn = get_conn()?;
    let sql = format!(
        "{ROUTE_SELECT_SQL}
         INNER JOIN virtual_models vm ON virtual_model_routes.virtual_model_id = vm.id
         WHERE vm.virtual_provider_id = ?1 AND virtual_model_routes.enabled = 1
         ORDER BY vm.created_at ASC, virtual_model_routes.priority ASC, virtual_model_routes.created_at ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([virtual_provider_id], route_row_mapper)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn update_route(id: &str, input: &UpdateVirtualModelRouteInput) -> IcodeResult<VirtualModelRoute> {
    let conn = get_conn()?;
    let now = now();

    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut idx = 1usize;

    if let Some(v) = &input.target_provider_id {
        sets.push(format!("target_provider_id = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &input.target_model_id {
        sets.push(format!("target_model_id = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.priority {
        sets.push(format!("priority = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.enabled {
        sets.push(format!("enabled = ?{idx}"));
        params.push(Box::new(v as i64));
        idx += 1;
    }
    if let Some(v) = input.max_retries {
        sets.push(format!("max_retries = ?{idx}"));
        params.push(Box::new(v.max(0)));
        idx += 1;
    }
    if let Some(v) = input.retry_interval_ms {
        sets.push(format!("retry_interval_ms = ?{idx}"));
        params.push(Box::new(v.max(0)));
        idx += 1;
    }
    if let Some(v) = input.timeout_ms {
        sets.push(format!("timeout_ms = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }

    if sets.is_empty() {
        return find_route_by_id(id);
    }

    sets.push(format!("updated_at = ?{idx}"));
    params.push(Box::new(now));

    let sql = format!(
        "UPDATE virtual_model_routes SET {} WHERE id = ?{}",
        sets.join(", "),
        idx + 1
    );
    params.push(Box::new(id.to_string()));

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, params_ref.as_slice())?;
    if affected == 0 {
        return Err(IcodeError::not_found("VirtualModelRoute", Some(id)));
    }

    find_route_by_id(id)
}

pub fn delete_route(id: &str) -> IcodeResult<()> {
    let conn = get_conn()?;
    let affected = conn.execute("DELETE FROM virtual_model_routes WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(IcodeError::not_found("VirtualModelRoute", Some(id)));
    }
    Ok(())
}

const ROUTE_SELECT_SQL: &str = "SELECT
    virtual_model_routes.id, virtual_model_routes.virtual_model_id, virtual_model_routes.target_provider_id,
    virtual_model_routes.target_model_id, virtual_model_routes.priority, virtual_model_routes.enabled,
    virtual_model_routes.max_retries, virtual_model_routes.retry_interval_ms, virtual_model_routes.timeout_ms,
    virtual_model_routes.is_healthy, virtual_model_routes.last_healthy_at, virtual_model_routes.extra_headers_json,
    virtual_model_routes.extra_body_json, virtual_model_routes.created_at, virtual_model_routes.updated_at
FROM virtual_model_routes";

fn route_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<VirtualModelRoute> {
    let enabled: i64 = row.get(5)?;
    let is_healthy: i64 = row.get(9)?;
    Ok(VirtualModelRoute {
        id: row.get(0)?,
        virtual_model_id: row.get(1)?,
        target_provider_id: row.get(2)?,
        target_model_id: row.get(3)?,
        priority: row.get(4)?,
        enabled: enabled != 0,
        max_retries: row.get(6)?,
        retry_interval_ms: row.get(7)?,
        timeout_ms: row.get(8)?,
        is_healthy: is_healthy != 0,
        last_healthy_at: row.get(10)?,
        extra_headers_json: row.get(11)?,
        extra_body_json: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

/// 将指定路由标记为不健康（网关层重试耗尽后降级）
pub fn mark_route_unhealthy(id: &str) -> IcodeResult<()> {
    let conn = get_conn()?;
    let now = now();
    conn.execute(
        "UPDATE virtual_model_routes SET is_healthy = 0, last_healthy_at = NULL, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    )?;
    Ok(())
}
