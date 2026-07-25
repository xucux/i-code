//! # CLI 管理模块仓储层
//!
//! 直接操作 `cli_profiles`、`cli_providers`、`cli_model_mappings` 三张表，
//! 负责数据库行与 DTO 之间的映射。复杂业务逻辑不放在本层。

use rusqlite::{params, OptionalExtension};

use crate::db::{get_db_pool, DbConn};
use crate::error::IcodeResult;

use super::types::{
    CliModelMapping, CliProfile, CliProvider, CreateCliModelMappingInput,
    CreateCliProfileInput, CreateCliProviderInput, UpdateCliModelMappingInput,
    UpdateCliProfileInput, UpdateCliProviderInput,
};

/// CLI 管理仓储
///
/// 封装所有与 CLI 相关的数据库访问。所有方法返回 `IcodeResult<T>`，
/// 数据库错误会通过 `From<rusqlite::Error>` 自动转换为 `IcodeError`。
pub struct CliManagementRepository;

impl CliManagementRepository {
    /// 创建仓储实例
    pub fn new() -> Self {
        Self
    }

    // ===== cli_profiles =====

    /// 列出所有 CLI 档案，按 `display_name` 字母顺序排序
    pub fn list_profiles(&self) -> IcodeResult<Vec<CliProfile>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, slug, display_name, cli_type, config_file_path, proxy_json, is_enabled, created_at, updated_at
             FROM cli_profiles
             ORDER BY display_name ASC",
        )?;
        let rows = stmt.query_map([], map_profile_row)?;
        collect_rows(rows)
    }

    /// 根据 ID 获取 CLI 档案
    pub fn get_profile(&self, id: &str) -> IcodeResult<Option<CliProfile>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, slug, display_name, cli_type, config_file_path, proxy_json, is_enabled, created_at, updated_at
             FROM cli_profiles WHERE id = ?1",
        )?;
        stmt.query_row([id], map_profile_row).optional().map_err(Into::into)
    }

    /// 根据 slug 获取 CLI 档案（用于唯一性校验）
    pub fn get_profile_by_slug(&self, slug: &str) -> IcodeResult<Option<CliProfile>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, slug, display_name, cli_type, config_file_path, proxy_json, is_enabled, created_at, updated_at
             FROM cli_profiles WHERE slug = ?1",
        )?;
        stmt.query_row([slug], map_profile_row).optional().map_err(Into::into)
    }

    /// 创建 CLI 档案
    pub fn create_profile(&self, id: &str, input: &CreateCliProfileInput, now: &str) -> IcodeResult<CliProfile> {
        let conn = get_conn()?;
        conn.execute(
            "INSERT INTO cli_profiles (id, slug, display_name, cli_type, config_file_path, proxy_json, is_enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                input.slug,
                input.display_name,
                input.cli_type,
                input.config_file_path,
                input.proxy_json,
                input.is_enabled as i32,
                now,
                now,
            ],
        )?;
        Ok(CliProfile {
            id: id.to_string(),
            slug: input.slug.clone(),
            display_name: input.display_name.clone(),
            cli_type: input.cli_type.clone(),
            config_file_path: input.config_file_path.clone(),
            proxy_json: input.proxy_json.clone(),
            is_enabled: input.is_enabled,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
    }

    /// 更新 CLI 档案
    pub fn update_profile(&self, id: &str, input: &UpdateCliProfileInput, now: &str) -> IcodeResult<CliProfile> {
        let conn = get_conn()?;
        // 使用动态 SET 子句，仅更新传入的字段
        let mut sets = Vec::new();
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(slug) = &input.slug {
            sets.push("slug = ?".to_string());
            args.push(Box::new(slug.clone()));
        }
        if let Some(display_name) = &input.display_name {
            sets.push("display_name = ?".to_string());
            args.push(Box::new(display_name.clone()));
        }
        if let Some(cli_type) = &input.cli_type {
            sets.push("cli_type = ?".to_string());
            args.push(Box::new(cli_type.clone()));
        }
        if let Some(config_file_path) = &input.config_file_path {
            sets.push("config_file_path = ?".to_string());
            args.push(Box::new(config_file_path.clone()));
        }
        if let Some(proxy_json) = &input.proxy_json {
            sets.push("proxy_json = ?".to_string());
            args.push(Box::new(proxy_json.clone()));
        }
        if let Some(is_enabled) = input.is_enabled {
            sets.push("is_enabled = ?".to_string());
            args.push(Box::new(is_enabled as i32));
        }
        sets.push("updated_at = ?".to_string());
        args.push(Box::new(now.to_string()));

        if sets.is_empty() {
            return self.get_profile(id)?.ok_or_else(|| crate::error::IcodeError::not_found("CLI 档案", Some(id)));
        }

        let sql = format!("UPDATE cli_profiles SET {} WHERE id = ?", sets.join(", "));
        args.push(Box::new(id.to_string()));
        // 将 Box<dyn ToSql> 转换为 Vec<&dyn ToSql> 执行
        let arg_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|a| a.as_ref()).collect();
        conn.execute(&sql, arg_refs.as_slice())?;
        self.get_profile(id)?.ok_or_else(|| crate::error::IcodeError::not_found("CLI 档案", Some(id)))
    }

    /// 删除 CLI 档案（级联删除 cli_providers 与 cli_model_mappings）
    pub fn delete_profile(&self, id: &str) -> IcodeResult<()> {
        let conn = get_conn()?;
        conn.execute("DELETE FROM cli_profiles WHERE id = ?1", [id])?;
        Ok(())
    }

    /// 获取所有 CLI 档案 ID（供 workspace 模块初始化配置头使用）
    pub fn list_profile_ids(&self) -> IcodeResult<Vec<String>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare("SELECT id FROM cli_profiles")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        collect_rows(rows)
    }

    // ===== cli_providers =====

    /// 列出某 CLI 档案绑定的所有供应商
    pub fn list_providers(&self, cli_profile_id: &str) -> IcodeResult<Vec<CliProvider>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, cli_profile_id, provider_id, display_name, route_mode, gateway_base_url, direct_base_url,
                    auth_json, balance_json, sort_order, is_default, created_at, updated_at
             FROM cli_providers
             WHERE cli_profile_id = ?1
             ORDER BY sort_order ASC, created_at ASC",
        )?;
        let rows = stmt.query_map([cli_profile_id], map_provider_row)?;
        collect_rows(rows)
    }

    /// 根据 ID 获取 CLI 供应商绑定
    pub fn get_provider(&self, id: &str) -> IcodeResult<Option<CliProvider>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, cli_profile_id, provider_id, display_name, route_mode, gateway_base_url, direct_base_url,
                    auth_json, balance_json, sort_order, is_default, created_at, updated_at
             FROM cli_providers WHERE id = ?1",
        )?;
        stmt.query_row([id], map_provider_row).optional().map_err(Into::into)
    }

    /// 创建 CLI 供应商绑定
    pub fn create_provider(&self, id: &str, input: &CreateCliProviderInput, now: &str) -> IcodeResult<CliProvider> {
        let conn = get_conn()?;
        conn.execute(
            "INSERT INTO cli_providers (id, cli_profile_id, provider_id, display_name, route_mode, gateway_base_url,
                                       direct_base_url, auth_json, balance_json, sort_order, is_default, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10, ?11, ?12)",
            params![
                id,
                input.cli_profile_id,
                input.provider_id,
                input.display_name,
                input.route_mode,
                input.gateway_base_url,
                input.direct_base_url,
                input.auth_json,
                input.sort_order,
                input.is_default as i32,
                now,
                now,
            ],
        )?;
        Ok(CliProvider {
            id: id.to_string(),
            cli_profile_id: input.cli_profile_id.clone(),
            provider_id: input.provider_id.clone(),
            display_name: input.display_name.clone(),
            route_mode: input.route_mode,
            gateway_base_url: input.gateway_base_url.clone(),
            direct_base_url: input.direct_base_url.clone(),
            auth_json: input.auth_json.clone(),
            balance_json: None,
            sort_order: input.sort_order,
            is_default: input.is_default,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
    }

    /// 更新 CLI 供应商绑定
    pub fn update_provider(&self, id: &str, input: &UpdateCliProviderInput, now: &str) -> IcodeResult<CliProvider> {
        let conn = get_conn()?;
        let mut sets = Vec::new();
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(provider_id) = &input.provider_id {
            sets.push("provider_id = ?".to_string());
            args.push(Box::new(provider_id.clone()));
        }
        if let Some(display_name) = &input.display_name {
            sets.push("display_name = ?".to_string());
            args.push(Box::new(display_name.clone()));
        }
        if let Some(route_mode) = input.route_mode {
            sets.push("route_mode = ?".to_string());
            args.push(Box::new(route_mode));
        }
        if let Some(gateway_base_url) = &input.gateway_base_url {
            sets.push("gateway_base_url = ?".to_string());
            args.push(Box::new(gateway_base_url.clone()));
        }
        if let Some(direct_base_url) = &input.direct_base_url {
            sets.push("direct_base_url = ?".to_string());
            args.push(Box::new(direct_base_url.clone()));
        }
        if let Some(auth_json) = &input.auth_json {
            sets.push("auth_json = ?".to_string());
            args.push(Box::new(auth_json.clone()));
        }
        if let Some(sort_order) = input.sort_order {
            sets.push("sort_order = ?".to_string());
            args.push(Box::new(sort_order));
        }
        if let Some(is_default) = input.is_default {
            sets.push("is_default = ?".to_string());
            args.push(Box::new(is_default as i32));
        }
        sets.push("updated_at = ?".to_string());
        args.push(Box::new(now.to_string()));

        if sets.is_empty() {
            return self.get_provider(id)?.ok_or_else(|| crate::error::IcodeError::not_found("CLI 供应商绑定", Some(id)));
        }

        let sql = format!("UPDATE cli_providers SET {} WHERE id = ?", sets.join(", "));
        args.push(Box::new(id.to_string()));
        let arg_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|a| a.as_ref()).collect();
        conn.execute(&sql, arg_refs.as_slice())?;
        self.get_provider(id)?.ok_or_else(|| crate::error::IcodeError::not_found("CLI 供应商绑定", Some(id)))
    }

    /// 删除 CLI 供应商绑定（级联删除 cli_model_mappings）
    pub fn delete_provider(&self, id: &str) -> IcodeResult<()> {
        let conn = get_conn()?;
        conn.execute("DELETE FROM cli_providers WHERE id = ?1", [id])?;
        Ok(())
    }

    // ===== cli_model_mappings =====

    /// 列出某 CLI 供应商下的所有模型映射
    pub fn list_model_mappings(&self, cli_provider_id: &str) -> IcodeResult<Vec<CliModelMapping>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, cli_provider_id, cli_model_alias, gateway_model_id, raw_model_id, input_mode, created_at, updated_at
             FROM cli_model_mappings
             WHERE cli_provider_id = ?1
             ORDER BY cli_model_alias ASC",
        )?;
        let rows = stmt.query_map([cli_provider_id], map_mapping_row)?;
        collect_rows(rows)
    }

    /// 根据 ID 获取模型映射
    pub fn get_model_mapping(&self, id: &str) -> IcodeResult<Option<CliModelMapping>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, cli_provider_id, cli_model_alias, gateway_model_id, raw_model_id, input_mode, created_at, updated_at
             FROM cli_model_mappings WHERE id = ?1",
        )?;
        stmt.query_row([id], map_mapping_row).optional().map_err(Into::into)
    }

    /// 创建模型映射
    pub fn create_model_mapping(
        &self,
        id: &str,
        input: &CreateCliModelMappingInput,
        now: &str,
    ) -> IcodeResult<CliModelMapping> {
        let conn = get_conn()?;
        conn.execute(
            "INSERT INTO cli_model_mappings (id, cli_provider_id, cli_model_alias, gateway_model_id, raw_model_id, input_mode, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                input.cli_provider_id,
                input.cli_model_alias,
                input.gateway_model_id,
                input.raw_model_id,
                input.input_mode,
                now,
                now,
            ],
        )?;
        Ok(CliModelMapping {
            id: id.to_string(),
            cli_provider_id: input.cli_provider_id.clone(),
            cli_model_alias: input.cli_model_alias.clone(),
            gateway_model_id: input.gateway_model_id.clone(),
            raw_model_id: input.raw_model_id.clone(),
            input_mode: input.input_mode.clone(),
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
    }

    /// 更新模型映射
    pub fn update_model_mapping(
        &self,
        id: &str,
        input: &UpdateCliModelMappingInput,
        now: &str,
    ) -> IcodeResult<CliModelMapping> {
        let conn = get_conn()?;
        let mut sets = Vec::new();
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(cli_model_alias) = &input.cli_model_alias {
            sets.push("cli_model_alias = ?".to_string());
            args.push(Box::new(cli_model_alias.clone()));
        }
        if let Some(gateway_model_id) = &input.gateway_model_id {
            sets.push("gateway_model_id = ?".to_string());
            args.push(Box::new(gateway_model_id.clone()));
        }
        if let Some(raw_model_id) = &input.raw_model_id {
            sets.push("raw_model_id = ?".to_string());
            args.push(Box::new(raw_model_id.clone()));
        }
        if let Some(input_mode) = &input.input_mode {
            sets.push("input_mode = ?".to_string());
            args.push(Box::new(input_mode.clone()));
        }
        sets.push("updated_at = ?".to_string());
        args.push(Box::new(now.to_string()));

        if sets.is_empty() {
            return self.get_model_mapping(id)?.ok_or_else(|| crate::error::IcodeError::not_found("CLI 模型映射", Some(id)));
        }

        let sql = format!("UPDATE cli_model_mappings SET {} WHERE id = ?", sets.join(", "));
        args.push(Box::new(id.to_string()));
        let arg_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|a| a.as_ref()).collect();
        conn.execute(&sql, arg_refs.as_slice())?;
        self.get_model_mapping(id)?.ok_or_else(|| crate::error::IcodeError::not_found("CLI 模型映射", Some(id)))
    }

    /// 删除模型映射
    pub fn delete_model_mapping(&self, id: &str) -> IcodeResult<()> {
        let conn = get_conn()?;
        conn.execute("DELETE FROM cli_model_mappings WHERE id = ?1", [id])?;
        Ok(())
    }
}

impl Default for CliManagementRepository {
    fn default() -> Self {
        Self::new()
    }
}

// ===== 私有辅助函数 =====

fn get_conn() -> IcodeResult<DbConn> {
    Ok(get_db_pool()?.get()?)
}

fn map_profile_row(row: &rusqlite::Row) -> rusqlite::Result<CliProfile> {
    Ok(CliProfile {
        id: row.get("id")?,
        slug: row.get("slug")?,
        display_name: row.get("display_name")?,
        cli_type: row.get("cli_type")?,
        config_file_path: row.get("config_file_path")?,
        proxy_json: row.get("proxy_json")?,
        is_enabled: row.get::<_, i32>("is_enabled")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_provider_row(row: &rusqlite::Row) -> rusqlite::Result<CliProvider> {
    Ok(CliProvider {
        id: row.get("id")?,
        cli_profile_id: row.get("cli_profile_id")?,
        provider_id: row.get("provider_id")?,
        display_name: row.get("display_name")?,
        route_mode: row.get("route_mode")?,
        gateway_base_url: row.get("gateway_base_url")?,
        direct_base_url: row.get("direct_base_url")?,
        auth_json: row.get("auth_json")?,
        balance_json: row.get("balance_json")?,
        sort_order: row.get("sort_order")?,
        is_default: row.get::<_, i32>("is_default")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_mapping_row(row: &rusqlite::Row) -> rusqlite::Result<CliModelMapping> {
    Ok(CliModelMapping {
        id: row.get("id")?,
        cli_provider_id: row.get("cli_provider_id")?,
        cli_model_alias: row.get("cli_model_alias")?,
        gateway_model_id: row.get("gateway_model_id")?,
        raw_model_id: row.get("raw_model_id")?,
        input_mode: row.get("input_mode")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn collect_rows<T>(rows: impl Iterator<Item = rusqlite::Result<T>>) -> IcodeResult<Vec<T>> {
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}
