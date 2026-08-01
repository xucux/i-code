//! # AI Gateway 数据访问层
//!
//! 直接操作 `providers` / `model_configs` / `gateway_models` 表的 CRUD，
//! 不包含业务逻辑。Service 层通过此 Repository 读写数据，
//! 敏感字段（`$SECRET:{snowflake_id}$` 引用）的解析由 Service 层负责。
//!
//! ## 表结构概览
//!
//! - `providers`：供应商主表（database.md §4.3）
//! - `model_configs`：模型完整配置（database.md §4.6）
//! - `gateway_models`：网关暴露模型（database.md §4.9）
//!
//! ## 设计约定
//!
//! - JSON 字段（`auth_json` / `balance_provider_json` 等）在 Repository 层
//!   保持原始字符串形态，序列化/反序列化由 Service 层完成。
//! - 布尔字段在 SQLite 中存为 INTEGER 0/1，Repository 层负责转换。
//! - 时间戳使用 ISO 8601 UTC 文本（`chrono::Utc::now().to_rfc3339()`）。

use chrono::Utc;

use crate::db::{get_db_pool, DbConn};
use crate::error::{IcodeError, IcodeResult};

use super::types::{
    CreateGatewayAuthKeyInput, CreateGatewayModelInput, CreateModelConfigInput,
    CreateProviderInput, GatewayAuthKey, GatewayModel, GatewaySettings, ModelConfig, Provider,
    UpdateGatewayAuthKeyInput, UpdateGatewayModelInput, UpdateGatewaySettingsInput,
    UpdateModelConfigInput, UpdateProviderInput,
};

// ===== 通用辅助 =====

/// 获取一个数据库连接
fn get_conn() -> IcodeResult<DbConn> {
    Ok(get_db_pool()?.get()?)
}

/// 生成雪花 ID 字符串
fn new_id() -> String {
    crate::core::id::generate_id()
}

/// 当前时间 ISO 8601 字符串
fn now() -> String {
    Utc::now().to_rfc3339()
}

// ===== providers 表 =====

/// 插入供应商
///
/// `auth_json` 应为已序列化的 AuthConfig JSON 字符串，可为 None。
/// `auth_method` / `auth_expires_at` 为顶层冗余字段，由 Service 层从 AuthConfig 派生。
/// 返回新插入记录的 ID。
pub fn insert_provider(
    input: &CreateProviderInput,
    auth_json: Option<&str>,
    auth_method: Option<&str>,
    auth_expires_at: Option<&str>,
    script_variables_json: Option<&str>,
) -> IcodeResult<Provider> {
    let conn = get_conn()?;
    let id = new_id();
    let now = now();
    let is_enabled_i: i64 = if input.is_enabled { 1 } else { 0 };
    let auto_fetch_i: i64 = if input.auto_fetch_official_models { 1 } else { 0 };
    let sort_order = input.sort_order.unwrap_or(0);

    conn.execute(
        "INSERT INTO providers
            (id, slug, display_name, provider_type, base_url, use_raw_base_url,
             transport, service_tier, auth_json, auth_expires_at, auth_method,
             balance_provider_json, timeout_json, retry_json, proxy_json,
             auto_fetch_official_models, context_cache_json, well_known_template_id,
             is_enabled, sort_order, created_at, updated_at, script_variables_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, ?7, ?8, ?9,
                 ?10, ?11, ?12, ?13, ?14, NULL, NULL, ?15, ?16, ?17, ?18, ?19)",
        rusqlite::params![
            id,
            input.slug,
            input.display_name,
            input.provider_type,
            input.base_url,
            input.use_raw_base_url as i64,
            auth_json,
            auth_expires_at,
            auth_method,
            input.balance_provider_json,
            input.timeout_json,
            input.retry_json,
            input.proxy_json,
            auto_fetch_i,
            is_enabled_i,
            sort_order,
            now,
            now,
            script_variables_json,
        ],
    )?;

    // 插入后立即查询返回完整记录
    find_provider_by_id(&id)
}

/// 按 ID 查询供应商
pub fn find_provider_by_id(id: &str) -> IcodeResult<Provider> {
    let conn = get_conn()?;
    let sql = format!("{PROVIDER_SELECT_SQL} WHERE p.id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], provider_row_mapper)?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Err(IcodeError::not_found("Provider", Some(id))),
    }
}

/// 按 slug 查询供应商（用于唯一性校验）
pub fn find_provider_by_slug(slug: &str) -> IcodeResult<Option<Provider>> {
    let conn = get_conn()?;
    let sql = format!("{PROVIDER_SELECT_SQL} WHERE p.slug = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([slug], provider_row_mapper)?;
    if let Some(row) = rows.next() {
        Ok(Some(row?))
    } else {
        Ok(None)
    }
}

/// 列出所有供应商，按 sort_order、created_at 排序
pub fn list_providers() -> IcodeResult<Vec<Provider>> {
    let conn = get_conn()?;
    let sql = format!("{PROVIDER_SELECT_SQL} ORDER BY p.sort_order ASC, p.created_at ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], provider_row_mapper)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 列出所有已启用的供应商（网关转发时使用）
pub fn list_enabled_providers() -> IcodeResult<Vec<Provider>> {
    let conn = get_conn()?;
    let sql = format!(
        "{PROVIDER_SELECT_SQL} WHERE p.is_enabled = 1 ORDER BY p.sort_order ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], provider_row_mapper)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 更新供应商
///
/// 仅更新 `input` 中为 `Some` 的字段；`auth` 字段为 `Some(None)` 表示置空，
/// `Some(Some(json))` 表示更新。
/// `auth_method` / `auth_expires_at` 同样使用 `Option<Option<&str>>` 语义。
pub fn update_provider(
    id: &str,
    input: &UpdateProviderInput,
    auth_json: Option<Option<&str>>,
    auth_method: Option<Option<&str>>,
    auth_expires_at: Option<Option<&str>>,
    script_variables_json: Option<Option<&str>>,
) -> IcodeResult<Provider> {
    let conn = get_conn()?;
    let now = now();

    // 动态构造 SET 子句
    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut idx = 1usize;

    if let Some(v) = &input.display_name {
        sets.push(format!("display_name = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &input.base_url {
        sets.push(format!("base_url = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.use_raw_base_url {
        sets.push(format!("use_raw_base_url = ?{idx}"));
        params.push(Box::new(v as i64));
        idx += 1;
    }
    // auth_json: Option<Option<&str>> —— 外层 Some 表示需要更新，内层 None 表示置空
    if let Some(v) = auth_json {
        sets.push(format!("auth_json = ?{idx}"));
        params.push(Box::new(v.map(|s| s.to_string())));
        idx += 1;
    }
    // auth_expires_at: Option<Option<&str>>
    if let Some(v) = auth_expires_at {
        sets.push(format!("auth_expires_at = ?{idx}"));
        params.push(Box::new(v.map(|s| s.to_string())));
        idx += 1;
    }
    // auth_method: Option<Option<&str>>
    if let Some(v) = auth_method {
        sets.push(format!("auth_method = ?{idx}"));
        params.push(Box::new(v.map(|s| s.to_string())));
        idx += 1;
    }
    if let Some(v) = input.auto_fetch_official_models {
        sets.push(format!("auto_fetch_official_models = ?{idx}"));
        params.push(Box::new(v as i64));
        idx += 1;
    }
    if let Some(v) = input.is_enabled {
        sets.push(format!("is_enabled = ?{idx}"));
        params.push(Box::new(v as i64));
        idx += 1;
    }
    if let Some(v) = input.sort_order {
        sets.push(format!("sort_order = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    // balance_provider_json
    if let Some(ref v) = input.balance_provider_json {
        sets.push(format!("balance_provider_json = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    // timeout_json
    if let Some(ref v) = input.timeout_json {
        sets.push(format!("timeout_json = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    // retry_json
    if let Some(ref v) = input.retry_json {
        sets.push(format!("retry_json = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    // proxy_json
    if let Some(ref v) = input.proxy_json {
        sets.push(format!("proxy_json = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    // script_variables_json: Option<Option<&str>> —— 外层 Some 表示需要更新，内层 None 表示置空
    if let Some(v) = script_variables_json {
        sets.push(format!("script_variables_json = ?{idx}"));
        params.push(Box::new(v.map(|s| s.to_string())));
        idx += 1;
    }

    if sets.is_empty() {
        // 没有字段更新，直接返回当前记录
        return find_provider_by_id(id);
    }

    // 追加 updated_at
    sets.push(format!("updated_at = ?{idx}"));
    params.push(Box::new(now));

    let sql = format!("UPDATE providers SET {} WHERE id = ?{}", sets.join(", "), idx + 1);
    params.push(Box::new(id.to_string()));

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, params_ref.as_slice())?;
    if affected == 0 {
        return Err(IcodeError::not_found("Provider", Some(id)));
    }

    find_provider_by_id(id)
}

/// 删除供应商
///
/// 外键级联会自动删除关联的 `provider_extra_headers`、`provider_extra_body`、
/// `gateway_models`、`official_model_cache` 记录。
pub fn delete_provider(id: &str) -> IcodeResult<()> {
    let conn = get_conn()?;
    let affected = conn.execute("DELETE FROM providers WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(IcodeError::not_found("Provider", Some(id)));
    }
    Ok(())
}

// ===== model_configs 表 =====

/// 插入模型配置
pub fn insert_model_config(input: &CreateModelConfigInput) -> IcodeResult<ModelConfig> {
    let conn = get_conn()?;
    let id = new_id();
    let now = now();

    conn.execute(
        "INSERT INTO model_configs
            (id, name, family, max_input_tokens, max_output_tokens, tokenizer,
             token_count_multiplier, price_per_1m_tokens, stream, temperature, top_k, top_p,
             frequency_penalty, presence_penalty, parallel_tool_calling, service_tier,
             verbosity, capabilities_json, thinking_json, multi_agent_json,
             web_search_json, memory_tool, preset_templates_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, ?11, NULL, NULL, ?12,
                 NULL, NULL, ?13, ?14, NULL, NULL, NULL, NULL, ?15, ?16)",
        rusqlite::params![
            id,
            input.name,
            input.family,
            input.max_input_tokens,
            input.max_output_tokens,
            input.tokenizer,
            input.token_count_multiplier,
            input.price_per_1m_tokens,
            input.stream.map(|b| b as i64),
            input.temperature,
            input.top_p,
            input.parallel_tool_calling.map(|b| b as i64),
            input.capabilities_json,
            input.thinking_json,
            now,
            now,
        ],
    )?;

    find_model_config_by_id(&id)
}

/// 按 ID 查询模型配置
pub fn find_model_config_by_id(id: &str) -> IcodeResult<ModelConfig> {
    let conn = get_conn()?;
    let sql = format!("{MODEL_CONFIG_SELECT_SQL} WHERE m.id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], model_config_row_mapper)?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Err(IcodeError::not_found("ModelConfig", Some(id))),
    }
}

/// 列出所有模型配置
pub fn list_model_configs() -> IcodeResult<Vec<ModelConfig>> {
    let conn = get_conn()?;
    let sql = format!("{MODEL_CONFIG_SELECT_SQL} ORDER BY m.created_at ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], model_config_row_mapper)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 更新模型配置
///
/// 仅更新传入的非 None 字段，动态构建 SET 子句。
pub fn update_model_config(id: &str, input: &UpdateModelConfigInput) -> IcodeResult<ModelConfig> {
    let conn = get_conn()?;
    let now = now();

    // 动态构造 SET 子句
    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut idx = 1usize;

    if let Some(v) = &input.name {
        sets.push(format!("name = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &input.family {
        sets.push(format!("family = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.max_input_tokens {
        sets.push(format!("max_input_tokens = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.max_output_tokens {
        sets.push(format!("max_output_tokens = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = &input.tokenizer {
        sets.push(format!("tokenizer = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.token_count_multiplier {
        sets.push("token_count_multiplier = ?{idx}".replace("{idx}", &idx.to_string()));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.price_per_1m_tokens {
        sets.push(format!("price_per_1m_tokens = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.stream {
        sets.push(format!("stream = ?{idx}"));
        params.push(Box::new(v as i64));
        idx += 1;
    }
    if let Some(v) = input.temperature {
        sets.push(format!("temperature = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.top_k {
        sets.push(format!("top_k = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.top_p {
        sets.push(format!("top_p = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.frequency_penalty {
        sets.push(format!("frequency_penalty = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.presence_penalty {
        sets.push(format!("presence_penalty = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.parallel_tool_calling {
        sets.push(format!("parallel_tool_calling = ?{idx}"));
        params.push(Box::new(v as i64));
        idx += 1;
    }
    if let Some(v) = &input.service_tier {
        sets.push(format!("service_tier = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &input.verbosity {
        sets.push(format!("verbosity = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &input.capabilities_json {
        sets.push(format!("capabilities_json = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &input.thinking_json {
        sets.push(format!("thinking_json = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &input.multi_agent_json {
        sets.push(format!("multi_agent_json = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &input.web_search_json {
        sets.push(format!("web_search_json = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.memory_tool {
        sets.push(format!("memory_tool = ?{idx}"));
        params.push(Box::new(v as i64));
        idx += 1;
    }
    if let Some(v) = &input.preset_templates_json {
        sets.push(format!("preset_templates_json = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }

    if sets.is_empty() {
        // 没有字段更新，直接返回当前记录
        return find_model_config_by_id(id);
    }

    // 追加 updated_at
    sets.push(format!("updated_at = ?{idx}"));
    params.push(Box::new(now));

    let sql = format!(
        "UPDATE model_configs SET {} WHERE id = ?{}",
        sets.join(", "),
        idx + 1
    );
    params.push(Box::new(id.to_string()));

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, params_ref.as_slice())?;
    if affected == 0 {
        return Err(IcodeError::not_found("ModelConfig", Some(id)));
    }

    find_model_config_by_id(id)
}

/// 删除模型配置
///
/// 外键级联会自动删除关联的 `model_config_extra_headers`、
/// `model_config_extra_body`、`gateway_models` 记录。
pub fn delete_model_config(id: &str) -> IcodeResult<()> {
    let conn = get_conn()?;
    let affected = conn.execute("DELETE FROM model_configs WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(IcodeError::not_found("ModelConfig", Some(id)));
    }
    Ok(())
}

// ===== gateway_models 表 =====

/// 插入网关模型
pub fn insert_gateway_model(input: &CreateGatewayModelInput) -> IcodeResult<GatewayModel> {
    let conn = get_conn()?;
    let id = new_id();
    let now = now();
    let is_exposed_i: i64 = if input.is_exposed { 1 } else { 0 };

    conn.execute(
        "INSERT INTO gateway_models
            (id, provider_id, model_config_id, model_id, display_name, family,
             source, is_exposed, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            id,
            input.provider_id,
            input.model_config_id,
            input.model_id,
            input.display_name,
            input.family,
            input.source,
            is_exposed_i,
            now,
            now,
        ],
    )?;

    find_gateway_model_by_id(&id)
}

/// 按 ID 查询网关模型
pub fn find_gateway_model_by_id(id: &str) -> IcodeResult<GatewayModel> {
    let conn = get_conn()?;
    let sql = format!("{GATEWAY_MODEL_SELECT_SQL} WHERE g.id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], gateway_model_row_mapper)?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Err(IcodeError::not_found("GatewayModel", Some(id))),
    }
}

/// 按 provider_id + model_id 查询网关模型（唯一性校验用）
pub fn find_gateway_model_by_provider_and_model(
    provider_id: &str,
    model_id: &str,
) -> IcodeResult<Option<GatewayModel>> {
    let conn = get_conn()?;
    let sql = format!(
        "{GATEWAY_MODEL_SELECT_SQL} WHERE g.provider_id = ?1 AND g.model_id = ?2"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([provider_id, model_id], gateway_model_row_mapper)?;
    if let Some(row) = rows.next() {
        Ok(Some(row?))
    } else {
        Ok(None)
    }
}

/// 列出所有网关模型
pub fn list_gateway_models() -> IcodeResult<Vec<GatewayModel>> {
    let conn = get_conn()?;
    let sql = format!("{GATEWAY_MODEL_SELECT_SQL} ORDER BY g.created_at ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], gateway_model_row_mapper)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 列出指定供应商下的所有网关模型
pub fn list_gateway_models_by_provider(provider_id: &str) -> IcodeResult<Vec<GatewayModel>> {
    let conn = get_conn()?;
    let sql = format!(
        "{GATEWAY_MODEL_SELECT_SQL} WHERE g.provider_id = ?1 ORDER BY g.created_at ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([provider_id], gateway_model_row_mapper)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 列出所有已暴露的网关模型（用于 `/v1/models` 接口）
///
/// 关联查询 `providers` 表，仅返回供应商已启用且模型已暴露的记录。
pub fn list_exposed_gateway_models() -> IcodeResult<Vec<ExposedGatewayModelRow>> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare(
        "SELECT g.id, g.provider_id, g.model_config_id, g.model_id,
                g.display_name, g.family, p.slug AS provider_slug, p.display_name AS provider_display_name
         FROM gateway_models g
         INNER JOIN providers p ON p.id = g.provider_id
         WHERE g.is_exposed = 1 AND p.is_enabled = 1
         ORDER BY p.sort_order ASC, g.model_id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ExposedGatewayModelRow {
            id: row.get(0)?,
            provider_id: row.get(1)?,
            model_config_id: row.get(2)?,
            model_id: row.get(3)?,
            display_name: row.get(4)?,
            family: row.get(5)?,
            provider_slug: row.get(6)?,
            provider_display_name: row.get(7)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 列出所有网关模型（包含未暴露/隐藏的模型）
///
/// 用于虚拟供应商选择子级真实模型等内部管理场景，不限制 `is_exposed`，
/// 但仍要求供应商已启用。
pub fn list_all_gateway_models() -> IcodeResult<Vec<ExposedGatewayModelRow>> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare(
        "SELECT g.id, g.provider_id, g.model_config_id, g.model_id,
                g.display_name, g.family, p.slug AS provider_slug, p.display_name AS provider_display_name
         FROM gateway_models g
         INNER JOIN providers p ON p.id = g.provider_id
         WHERE p.is_enabled = 1
         ORDER BY p.sort_order ASC, g.model_id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ExposedGatewayModelRow {
            id: row.get(0)?,
            provider_id: row.get(1)?,
            model_config_id: row.get(2)?,
            model_id: row.get(3)?,
            display_name: row.get(4)?,
            family: row.get(5)?,
            provider_slug: row.get(6)?,
            provider_display_name: row.get(7)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 删除网关模型
pub fn delete_gateway_model(id: &str) -> IcodeResult<()> {
    let conn = get_conn()?;
    let affected = conn.execute("DELETE FROM gateway_models WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(IcodeError::not_found("GatewayModel", Some(id)));
    }
    Ok(())
}

/// 更新网关模型
pub fn update_gateway_model(
    id: &str,
    input: &UpdateGatewayModelInput,
) -> IcodeResult<GatewayModel> {
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
    if let Some(v) = &input.family {
        sets.push(format!("family = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.is_exposed {
        sets.push(format!("is_exposed = ?{idx}"));
        params.push(Box::new(v as i64));
        idx += 1;
    }

    if sets.is_empty() {
        return find_gateway_model_by_id(id);
    }

    sets.push(format!("updated_at = ?{idx}"));
    params.push(Box::new(now));

    let sql = format!("UPDATE gateway_models SET {} WHERE id = ?{}", sets.join(", "), idx + 1);
    params.push(Box::new(id.to_string()));

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, params_ref.as_slice())?;
    if affected == 0 {
        return Err(IcodeError::not_found("GatewayModel", Some(id)));
    }

    find_gateway_model_by_id(id)
}

// ===== 行映射器与 SQL 片段 =====

/// providers 表 SELECT 字段列表
const PROVIDER_SELECT_SQL: &str = "SELECT p.id, p.slug, p.display_name, p.provider_type,
    p.base_url, p.use_raw_base_url, p.transport, p.service_tier, p.auth_json,
    p.auth_expires_at, p.auth_method,
    p.balance_provider_json, p.timeout_json, p.retry_json, p.proxy_json,
    p.auto_fetch_official_models, p.context_cache_json, p.well_known_template_id,
    p.is_enabled, p.sort_order, p.created_at, p.updated_at,
    p.script_variables_json
    FROM providers p";

/// providers 表行映射器：将数据库行转换为 Provider DTO
fn provider_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<Provider> {
    // 列顺序见 PROVIDER_SELECT_SQL
    let use_raw: i64 = row.get(5)?;
    let auto_fetch: i64 = row.get(15)?;
    let is_enabled: i64 = row.get(18)?;
    Ok(Provider {
        id: row.get(0)?,
        slug: row.get(1)?,
        display_name: row.get(2)?,
        provider_type: row.get(3)?,
        base_url: row.get(4)?,
        use_raw_base_url: use_raw != 0,
        transport: row.get(6)?,
        service_tier: row.get(7)?,
        auth_json: row.get(8)?,
        auth_expires_at: row.get(9)?,
        auth_method: row.get(10)?,
        balance_provider_json: row.get(11)?,
        timeout_json: row.get(12)?,
        retry_json: row.get(13)?,
        proxy_json: row.get(14)?,
        auto_fetch_official_models: auto_fetch != 0,
        context_cache_json: row.get(16)?,
        well_known_template_id: row.get(17)?,
        is_enabled: is_enabled != 0,
        sort_order: row.get(19)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
        script_variables_json: row.get(22)?,
    })
}

/// model_configs 表 SELECT 字段列表
const MODEL_CONFIG_SELECT_SQL: &str = "SELECT m.id, m.name, m.family, m.max_input_tokens,
    m.max_output_tokens, m.tokenizer, m.token_count_multiplier, m.price_per_1m_tokens, m.stream,
    m.temperature, m.top_k, m.top_p, m.frequency_penalty, m.presence_penalty, m.parallel_tool_calling,
    m.service_tier, m.verbosity, m.capabilities_json, m.thinking_json, m.multi_agent_json,
    m.web_search_json, m.memory_tool, m.preset_templates_json, m.created_at, m.updated_at
    FROM model_configs m";

/// model_configs 表行映射器
fn model_config_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<ModelConfig> {
    let stream_i: Option<i64> = row.get(8)?;
    let parallel_i: Option<i64> = row.get(14)?;
    let memory_i: Option<i64> = row.get(21)?;
    Ok(ModelConfig {
        id: row.get(0)?,
        name: row.get(1)?,
        family: row.get(2)?,
        max_input_tokens: row.get(3)?,
        max_output_tokens: row.get(4)?,
        tokenizer: row.get(5)?,
        token_count_multiplier: row.get(6)?,
        price_per_1m_tokens: row.get(7)?,
        stream: stream_i.map(|v| v != 0),
        temperature: row.get(9)?,
        top_k: row.get(10)?,
        top_p: row.get(11)?,
        frequency_penalty: row.get(12)?,
        presence_penalty: row.get(13)?,
        parallel_tool_calling: parallel_i.map(|v| v != 0),
        service_tier: row.get(15)?,
        verbosity: row.get(16)?,
        capabilities_json: row.get(17)?,
        thinking_json: row.get(18)?,
        multi_agent_json: row.get(19)?,
        web_search_json: row.get(20)?,
        memory_tool: memory_i.map(|v| v != 0),
        preset_templates_json: row.get(22)?,
        created_at: row.get(23)?,
        updated_at: row.get(24)?,
    })
}

/// gateway_models 表 SELECT 字段列表
const GATEWAY_MODEL_SELECT_SQL: &str = "SELECT g.id, g.provider_id, g.model_config_id,
    g.model_id, g.display_name, g.family, g.source, g.is_exposed, g.created_at, g.updated_at
    FROM gateway_models g";

/// gateway_models 表行映射器
fn gateway_model_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<GatewayModel> {
    let is_exposed: i64 = row.get(7)?;
    Ok(GatewayModel {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        model_config_id: row.get(2)?,
        model_id: row.get(3)?,
        display_name: row.get(4)?,
        family: row.get(5)?,
        source: row.get(6)?,
        is_exposed: is_exposed != 0,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

// ===== 暴露模型查询结果 =====

/// 暴露模型查询结果行（关联 providers 表）
///
/// 用于 Service 层构造 `ExposedModel` 列表，作为 `/v1/models` 接口数据源。
/// `model_config_id` 与 `provider_display_name` 预留用于后续展示扩展。
#[allow(dead_code)]
pub struct ExposedGatewayModelRow {
    pub id: String,
    pub provider_id: String,
    pub model_config_id: String,
    pub model_id: String,
    pub display_name: Option<String>,
    pub family: Option<String>,
    pub provider_slug: String,
    pub provider_display_name: String,
}

// ===== gateway_settings 表 =====

/// 读取网关设置单例行
pub fn find_gateway_settings() -> IcodeResult<GatewaySettings> {
    let conn = get_conn()?;
    let sql = format!("{GATEWAY_SETTINGS_SELECT_SQL} WHERE gs.id = 'default'");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([], gateway_settings_row_mapper)?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Err(IcodeError::not_found("GatewaySettings", Some("default"))),
    }
}

/// 更新网关设置单例行
///
/// 仅更新 `input` 中为 `Some` 的字段；`default_api_key_secret_id` 为 `Some(None)` 表示置空。
pub fn update_gateway_settings(input: &UpdateGatewaySettingsInput) -> IcodeResult<GatewaySettings> {
    let conn = get_conn()?;
    let now = now();

    // 动态构造 SET 子句
    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut idx = 1usize;

    if let Some(ref v) = input.gateway_host {
        sets.push(format!("gateway_host = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.gateway_port {
        sets.push(format!("gateway_port = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    // default_api_key_secret_id: Option<Option<String>> —— 外层 Some 表示需要更新，内层 None 表示置空
    if let Some(ref v) = input.default_api_key_secret_id {
        sets.push(format!("default_api_key_secret_id = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.is_enabled {
        sets.push(format!("is_enabled = ?{idx}"));
        params.push(Box::new(v as i64));
        idx += 1;
    }
    if let Some(v) = input.auth_enabled {
        sets.push(format!("auth_enabled = ?{idx}"));
        params.push(Box::new(v as i64));
        idx += 1;
    }

    if sets.is_empty() {
        // 没有字段更新，直接返回当前记录
        return find_gateway_settings();
    }

    // 追加 updated_at
    sets.push(format!("updated_at = ?{idx}"));
    params.push(Box::new(now));

    let sql = format!(
        "UPDATE gateway_settings SET {} WHERE id = 'default'",
        sets.join(", ")
    );

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, params_ref.as_slice())?;

    find_gateway_settings()
}

// ===== gateway_auth_keys 表 =====

/// 按 API Key 值查询网关认证 API Key
///
/// 用于网关认证时根据请求头中的 key 反查对应记录。
/// 注意：`api_key_secret_id` 列按当前业务约定存储明文 key。
pub fn find_gateway_auth_key_by_api_key(api_key: &str) -> IcodeResult<Option<GatewayAuthKey>> {
    let conn = get_conn()?;
    let sql = format!("{GATEWAY_AUTH_KEY_SELECT_SQL} WHERE gak.api_key_secret_id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([api_key], gateway_auth_key_row_mapper)?;
    if let Some(row) = rows.next() {
        Ok(Some(row?))
    } else {
        Ok(None)
    }
}

/// 更新网关认证 API Key 的最后使用时间
pub fn touch_gateway_auth_key_last_used(id: &str) -> IcodeResult<()> {
    let conn = get_conn()?;
    conn.execute(
        "UPDATE gateway_auth_keys SET last_used_at = ?1 WHERE id = ?2",
        rusqlite::params![now(), id],
    )?;
    Ok(())
}

/// 插入网关认证 API Key
pub fn insert_gateway_auth_key(input: &CreateGatewayAuthKeyInput) -> IcodeResult<GatewayAuthKey> {
    let conn = get_conn()?;
    let id = new_id();
    let now = now();
    let is_enabled_i: i64 = if input.is_enabled { 1 } else { 0 };
    let sort_order = input.sort_order.unwrap_or(0);

    conn.execute(
        "INSERT INTO gateway_auth_keys
            (id, name, description, api_key_secret_id, is_enabled, expires_at,
             sort_order, last_used_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9)",
        rusqlite::params![
            id,
            input.name,
            input.description,
            input.api_key_secret_id,
            is_enabled_i,
            input.expires_at,
            sort_order,
            now,
            now,
        ],
    )?;

    find_gateway_auth_key(&id)
}

/// 按 ID 查询网关认证 API Key
pub fn find_gateway_auth_key(id: &str) -> IcodeResult<GatewayAuthKey> {
    let conn = get_conn()?;
    let sql = format!("{GATEWAY_AUTH_KEY_SELECT_SQL} WHERE gak.id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], gateway_auth_key_row_mapper)?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Err(IcodeError::not_found("GatewayAuthKey", Some(id))),
    }
}

/// 列出所有网关认证 API Key，按 sort_order、created_at 排序
pub fn list_gateway_auth_keys() -> IcodeResult<Vec<GatewayAuthKey>> {
    let conn = get_conn()?;
    let sql = format!(
        "{GATEWAY_AUTH_KEY_SELECT_SQL} ORDER BY gak.sort_order ASC, gak.created_at ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], gateway_auth_key_row_mapper)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 更新网关认证 API Key
///
/// 仅更新 `input` 中为 `Some` 的字段；`description` / `api_key_secret_id` / `expires_at`
/// 为 `Some(None)` 表示置空。
pub fn update_gateway_auth_key(
    id: &str,
    input: &UpdateGatewayAuthKeyInput,
) -> IcodeResult<GatewayAuthKey> {
    let conn = get_conn()?;
    let now = now();

    // 动态构造 SET 子句
    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut idx = 1usize;

    if let Some(ref v) = input.name {
        sets.push(format!("name = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    // description: Option<Option<String>> —— 外层 Some 表示需要更新，内层 None 表示置空
    if let Some(ref v) = input.description {
        sets.push(format!("description = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    // api_key_secret_id: Option<Option<String>>
    if let Some(ref v) = input.api_key_secret_id {
        sets.push(format!("api_key_secret_id = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.is_enabled {
        sets.push(format!("is_enabled = ?{idx}"));
        params.push(Box::new(v as i64));
        idx += 1;
    }
    // expires_at: Option<Option<String>>
    if let Some(ref v) = input.expires_at {
        sets.push(format!("expires_at = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = input.sort_order {
        sets.push(format!("sort_order = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }

    if sets.is_empty() {
        // 没有字段更新，直接返回当前记录
        return find_gateway_auth_key(id);
    }

    // 追加 updated_at
    sets.push(format!("updated_at = ?{idx}"));
    params.push(Box::new(now));

    let sql = format!(
        "UPDATE gateway_auth_keys SET {} WHERE id = ?{}",
        sets.join(", "),
        idx + 1
    );
    params.push(Box::new(id.to_string()));

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, params_ref.as_slice())?;
    if affected == 0 {
        return Err(IcodeError::not_found("GatewayAuthKey", Some(id)));
    }

    find_gateway_auth_key(id)
}

/// 删除网关认证 API Key
pub fn delete_gateway_auth_key(id: &str) -> IcodeResult<()> {
    let conn = get_conn()?;
    let affected = conn.execute("DELETE FROM gateway_auth_keys WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(IcodeError::not_found("GatewayAuthKey", Some(id)));
    }
    Ok(())
}

/// 查询供应商级附加请求头
///
/// 返回 `(key, value)` 列表，按 `sort_order` 排序。
/// value 可能包含 `$SECRET:{snowflake_id}$` 引用，由 Service 层负责解密。
pub fn list_provider_extra_headers(provider_id: &str) -> IcodeResult<Vec<(String, String)>> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare(
        "SELECT key, value FROM provider_extra_headers WHERE provider_id = ?1 ORDER BY sort_order ASC",
    )?;
    let rows = stmt.query_map([provider_id], |row| {
        let key: String = row.get(0)?;
        let value: String = row.get(1)?;
        Ok((key, value))
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 插入供应商级附加请求头
pub fn insert_provider_extra_header(
    provider_id: &str,
    key: &str,
    value: &str,
    sort_order: i64,
    now: &str,
) -> IcodeResult<()> {
    let conn = get_conn()?;
    conn.execute(
        "INSERT OR REPLACE INTO provider_extra_headers (provider_id, key, value, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        rusqlite::params![provider_id, key, value, sort_order, now],
    )?;
    Ok(())
}

// ===== gateway_settings / gateway_auth_keys 行映射器与 SQL 片段 =====

/// gateway_settings 表 SELECT 字段列表
const GATEWAY_SETTINGS_SELECT_SQL: &str = "SELECT gs.id, gs.gateway_host, gs.gateway_port,
    gs.default_api_key_secret_id, gs.is_enabled, gs.auth_enabled, gs.created_at, gs.updated_at
    FROM gateway_settings gs";

/// gateway_settings 表行映射器
fn gateway_settings_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<GatewaySettings> {
    let is_enabled: i64 = row.get(4)?;
    let auth_enabled: i64 = row.get(5)?;
    Ok(GatewaySettings {
        id: row.get(0)?,
        gateway_host: row.get(1)?,
        gateway_port: row.get(2)?,
        default_api_key_secret_id: row.get(3)?,
        is_enabled: is_enabled != 0,
        auth_enabled: auth_enabled != 0,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

/// gateway_auth_keys 表 SELECT 字段列表
const GATEWAY_AUTH_KEY_SELECT_SQL: &str = "SELECT gak.id, gak.name, gak.description,
    gak.api_key_secret_id, gak.is_enabled, gak.expires_at, gak.sort_order,
    gak.last_used_at, gak.created_at, gak.updated_at
    FROM gateway_auth_keys gak";

/// gateway_auth_keys 表行映射器
fn gateway_auth_key_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<GatewayAuthKey> {
    let is_enabled: i64 = row.get(4)?;
    Ok(GatewayAuthKey {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        api_key_secret_id: row.get(3)?,
        is_enabled: is_enabled != 0,
        expires_at: row.get(5)?,
        sort_order: row.get(6)?,
        last_used_at: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}
