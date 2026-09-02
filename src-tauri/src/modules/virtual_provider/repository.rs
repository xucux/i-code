//! # 虚拟供应商数据访问层
//!
//! 直接操作 `virtual_providers` / `virtual_models` / `virtual_model_routes` 三表。

use chrono::Utc;

use crate::core::id::generate_id;
use crate::db::{get_db_pool, DbConn};
use crate::error::{IcodeError, IcodeResult};

use super::types::{
    CreateVirtualModelInput, CreateVirtualModelRouteInput, CreateVirtualProviderInput,
    ExposedVirtualModel, RouteAttemptStats, SaveVirtualModelInput, UpdateVirtualModelInput,
    UpdateVirtualModelRouteInput, UpdateVirtualProviderInput, VirtualModel, VirtualModelRoute,
    VirtualProvider, VirtualProviderStrategy, VirtualRouteAttempt,
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
        // 路由级 extra_headers / extra_body 序列化为 JSON 字符串存储
        let extra_headers_json: Option<String> = route
            .extra_headers
            .as_ref()
            .map(|v| v.to_string());
        let extra_body_json: Option<String> = route
            .extra_body
            .as_ref()
            .map(|v| v.to_string());
        // weight 默认 1，写入时取 max(0) 防止负值
        let weight = route.weight.max(0);
        tx.execute(
            "INSERT INTO virtual_model_routes
                (id, virtual_model_id, target_provider_id, target_model_id, priority,
                 enabled, max_retries, retry_interval_ms, timeout_ms, is_healthy,
                 extra_headers_json, extra_body_json, weight, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                route_id,
                virtual_model_id,
                route.target_provider_id,
                route.target_model_id,
                route.priority,
                enabled_i,
                route.max_retries.max(0),
                route.retry_interval_ms.max(0),
                route.timeout_ms,
                is_healthy_i,
                extra_headers_json,
                extra_body_json,
                weight,
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
/// 隔离约束：排除挂载了「视觉生成供应商」路由的虚拟模型（媒体生成协议族不进入 `/v1/models`）。
pub fn list_exposed_virtual_models() -> IcodeResult<Vec<ExposedVirtualModel>> {
    let conn = get_conn()?;
    // 动态构造媒体生成协议族的 IN 占位符
    let media_placeholders = crate::modules::ai_gateway::types::MEDIA_GENERATION_PROVIDER_TYPES
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT vm.id, vm.virtual_provider_id, vp.alias, vm.model_id, vm.display_name
         FROM virtual_models vm
         INNER JOIN virtual_providers vp ON vp.id = vm.virtual_provider_id
         WHERE vp.is_enabled = 1 AND vm.is_enabled = 1
           AND NOT EXISTS (
               SELECT 1 FROM virtual_model_routes r
               INNER JOIN providers p ON p.id = r.target_provider_id
               WHERE r.virtual_model_id = vm.id
                 AND p.provider_type IN ({media_placeholders})
           )
         ORDER BY vp.alias ASC, vm.model_id ASC",
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(
            crate::modules::ai_gateway::types::MEDIA_GENERATION_PROVIDER_TYPES.iter(),
        ),
        |row| {
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
    virtual_model_routes.extra_body_json,
    virtual_model_routes.consecutive_failures, virtual_model_routes.last_error_text,
    virtual_model_routes.last_check_duration_ms, virtual_model_routes.last_check_at,
    virtual_model_routes.weight,
    virtual_model_routes.created_at, virtual_model_routes.updated_at
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
        consecutive_failures: row.get(13)?,
        last_error_text: row.get(14)?,
        last_check_duration_ms: row.get(15)?,
        last_check_at: row.get(16)?,
        weight: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
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

/// 探活成功：置健康，重置连续失败计数，更新 last_healthy_at / last_check_at / last_check_duration_ms
///
/// 不会清空 last_error_text，保留最近一次失败原因供 UI 展示。
pub fn mark_route_healthy(id: &str, check_duration_ms: u64) -> IcodeResult<()> {
    let conn = get_conn()?;
    let now = now();
    conn.execute(
        "UPDATE virtual_model_routes
         SET is_healthy = 1,
             last_healthy_at = ?1,
             last_check_at = ?1,
             last_check_duration_ms = ?2,
             consecutive_failures = 0,
             updated_at = ?1
         WHERE id = ?3",
        rusqlite::params![now, check_duration_ms as i64, id],
    )?;
    Ok(())
}

/// 探活失败：递增 consecutive_failures，记录 last_error_text / last_check_duration_ms / last_check_at
///
/// 当 consecutive_failures 达到恢复阈值（由 Service 层判定）时，将 is_healthy 置 0。
pub fn mark_route_check_failed(
    id: &str,
    error_text: &str,
    check_duration_ms: u64,
    degrade: bool,
) -> IcodeResult<()> {
    let conn = get_conn()?;
    let now = now();
    if degrade {
        // 失败次数已达阈值，置不健康
        conn.execute(
            "UPDATE virtual_model_routes
             SET is_healthy = 0,
                 consecutive_failures = consecutive_failures + 1,
                 last_error_text = ?1,
                 last_check_duration_ms = ?2,
                 last_check_at = ?3,
                 updated_at = ?3
             WHERE id = ?4",
            rusqlite::params![error_text, check_duration_ms as i64, now, id],
        )?;
    } else {
        // 仅记录失败次数与原因
        conn.execute(
            "UPDATE virtual_model_routes
             SET consecutive_failures = consecutive_failures + 1,
                 last_error_text = ?1,
                 last_check_duration_ms = ?2,
                 last_check_at = ?3,
                 updated_at = ?3
             WHERE id = ?4",
            rusqlite::params![error_text, check_duration_ms as i64, now, id],
        )?;
    }
    Ok(())
}

/// 列出待探活路由
///
/// 返回所有启用且（is_healthy=0 OR consecutive_failures>0）的路由，
/// 即已降级或最近失败过的路由，调度器需要周期性探活以恢复健康。
pub fn list_routes_for_health_check() -> IcodeResult<Vec<VirtualModelRoute>> {
    let conn = get_conn()?;
    let sql = format!(
        "{ROUTE_SELECT_SQL}
         WHERE enabled = 1 AND (is_healthy = 0 OR consecutive_failures > 0)
         ORDER BY (last_check_at IS NULL) DESC, last_check_at ASC, created_at ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], route_row_mapper)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 列出所有启用路由（不论健康状态）
///
/// 调度器在探活开关刚开启时使用，对所有启用路由做一次初始探活。
#[allow(dead_code)]
pub fn list_all_enabled_routes() -> IcodeResult<Vec<VirtualModelRoute>> {
    let conn = get_conn()?;
    let sql = format!(
        "{ROUTE_SELECT_SQL}
         WHERE enabled = 1
         ORDER BY created_at ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], route_row_mapper)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

// ===== virtual_route_attempts =====

/// 写入一条路由尝试历史
///
/// 由 `VirtualForwarder` 在每条路由尝试结束后通过 `tauri::async_runtime::spawn` 异步调用。
pub fn insert_route_attempt(
    virtual_route_id: &str,
    virtual_provider_id: &str,
    request_id: &str,
    attempt_index: usize,
    success: bool,
    status_code: Option<u16>,
    error_message: Option<&str>,
    duration_ms: u64,
) -> IcodeResult<()> {
    let conn = get_conn()?;
    let id = new_id();
    let now = now();
    let success_i: i64 = if success { 1 } else { 0 };
    conn.execute(
        "INSERT INTO virtual_route_attempts
            (id, virtual_route_id, virtual_provider_id, request_id, attempt_index,
             success, status_code, error_message, duration_ms, attempted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            id,
            virtual_route_id,
            virtual_provider_id,
            request_id,
            attempt_index as i64,
            success_i,
            status_code.map(|s| s as i64),
            error_message,
            duration_ms as i64,
            now,
        ],
    )?;
    Ok(())
}

/// 查询指定路由的最近 N 次尝试
pub fn list_recent_attempts_by_route(
    route_id: &str,
    limit: u32,
) -> IcodeResult<Vec<VirtualRouteAttempt>> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, virtual_route_id, virtual_provider_id, request_id, attempt_index,
                success, status_code, error_message, duration_ms, attempted_at
         FROM virtual_route_attempts
         WHERE virtual_route_id = ?1
         ORDER BY attempted_at DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![route_id, limit], |row| {
        let success: i64 = row.get(5)?;
        Ok(VirtualRouteAttempt {
            id: row.get(0)?,
            virtual_route_id: row.get(1)?,
            virtual_provider_id: row.get(2)?,
            request_id: row.get(3)?,
            attempt_index: row.get(4)?,
            success: success != 0,
            status_code: row.get(6)?,
            error_message: row.get(7)?,
            duration_ms: row.get(8)?,
            attempted_at: row.get(9)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 查询指定供应商下所有路由的尝试统计
///
/// 返回每条路由的总数 / 成功数 / 失败数 / 成功率 / 平均耗时 / 最近失败原因 / 最近尝试时间。
pub fn list_route_attempt_stats_by_provider(
    virtual_provider_id: &str,
) -> IcodeResult<Vec<RouteAttemptStats>> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare(
        "SELECT
            virtual_route_id,
            COUNT(*) AS total,
            SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) AS success_count,
            SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) AS failure_count,
            CASE WHEN COUNT(*) > 0
                 THEN CAST(SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) * 100 / COUNT(*) AS INTEGER)
                 ELSE 0 END AS success_rate,
            CASE WHEN COUNT(*) > 0
                 THEN CAST(AVG(duration_ms) AS INTEGER)
                 ELSE 0 END AS avg_duration_ms,
            (SELECT error_message FROM virtual_route_attempts a2
             WHERE a2.virtual_route_id = virtual_route_attempts.virtual_route_id
               AND a2.success = 0
             ORDER BY a2.attempted_at DESC LIMIT 1) AS last_error,
            MAX(attempted_at) AS last_attempted_at
         FROM virtual_route_attempts
         WHERE virtual_provider_id = ?1
         GROUP BY virtual_route_id
         ORDER BY last_attempted_at DESC",
    )?;
    let rows = stmt.query_map([virtual_provider_id], |row| {
        Ok(RouteAttemptStats {
            virtual_route_id: row.get(0)?,
            total: row.get(1)?,
            success_count: row.get(2)?,
            failure_count: row.get(3)?,
            success_rate: row.get(4)?,
            avg_duration_ms: row.get(5)?,
            last_error: row.get(6)?,
            last_attempted_at: row.get(7)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 清理 N 天前的路由尝试历史
///
/// 由调度器每天 0 点调用，默认保留 30 天。
pub fn cleanup_old_attempts(days_to_keep: u32) -> IcodeResult<u64> {
    let conn = get_conn()?;
    let cutoff = (Utc::now() - chrono::Duration::days(days_to_keep as i64)).to_rfc3339();
    let affected = conn.execute(
        "DELETE FROM virtual_route_attempts WHERE attempted_at < ?1",
        [&cutoff],
    )?;
    Ok(affected as u64)
}

/// 统计 CLI 模型映射中 gateway_model_id 以指定前缀（`{alias}/`）开头的记录数
///
/// 用于 alias 变更影响检查：修改虚拟供应商 alias 后，
/// 所有使用 `{old_alias}/` 前缀的 CLI 模型映射将失效。
pub fn count_cli_model_mappings_by_alias_prefix(alias: &str) -> IcodeResult<i64> {
    let conn = get_conn()?;
    let prefix = format!("{alias}/");
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cli_model_mappings WHERE gateway_model_id LIKE ?1",
        [format!("{prefix}%")],
        |row| row.get(0),
    )?;
    Ok(count)
}
