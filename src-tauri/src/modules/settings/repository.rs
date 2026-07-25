//! # 应用设置数据访问层
//!
//! 直接操作 `app_settings` 表的 CRUD。
//! 由于 `app_settings` 是单例表（`id = 'default'`），Repository 提供
//! 读取与更新两个核心方法，不提供插入/删除。
//!
//! ## 表结构
//!
//! ```sql
//! CREATE TABLE app_settings (
//!   id TEXT PRIMARY KEY DEFAULT 'default',
//!   theme TEXT NOT NULL DEFAULT 'dark',
//!   locale TEXT NOT NULL DEFAULT 'zh-CN',
//!   global_proxy_json TEXT,
//!   network_timeout_ms INTEGER DEFAULT 120000,
//!   network_retry_json TEXT,
//!   store_secrets_in_keychain INTEGER NOT NULL DEFAULT 1,
//!   config_key TEXT,
//!   created_at TEXT NOT NULL,
//!   updated_at TEXT NOT NULL
//! );
//! ```
//!
//! 默认行由 `V001__init.sql` 迁移脚本插入，应用启动后必然存在。
//! 网关监听地址、端口、默认 API Key 等已迁移到 `gateway_settings` 表。

use chrono::Utc;

use crate::db::{get_db_pool, DbConn};
use crate::error::{IcodeError, IcodeResult};

/// app_settings 表的完整行
///
/// JSON 字段保留原始字符串，由 Service 层负责解析为强类型对象。
#[derive(Debug, Clone)]
pub struct AppSettingsRow {
    pub theme: String,
    pub locale: String,
    pub global_proxy_json: Option<String>,
    pub network_timeout_ms: Option<i64>,
    pub network_retry_json: Option<String>,
    pub store_secrets_in_keychain: bool,
    /// 全局代理开关（关闭=直连，开启=所有请求走代理）
    pub global_proxy_enabled: bool,
    /// 配置密钥（AES 密钥），用于加密部分落库数据
    pub config_key: Option<String>,
    /// 标题栏信息展示配置 JSON
    pub titlebar_info_json: Option<String>,
    /// 备份设置 JSON
    pub backup_settings_json: Option<String>,
    /// 开机自启开关
    pub auto_start_enabled: bool,
    /// 网关上次关闭时的运行状态
    pub gateway_last_running: bool,
    /// 全局日志级别
    pub log_level: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 获取一个数据库连接
fn get_conn() -> IcodeResult<DbConn> {
    Ok(get_db_pool()?.get()?)
}

/// 读取应用设置单例行
///
/// 应用启动时由 V001 迁移保证默认行存在。
/// 若行不存在（异常情况），返回 INTERNAL 错误。
pub fn find() -> IcodeResult<AppSettingsRow> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare(
        "SELECT theme, locale, global_proxy_json,
                network_timeout_ms, network_retry_json,
                store_secrets_in_keychain, global_proxy_enabled, config_key,
                titlebar_info_json, backup_settings_json,
                auto_start_enabled, gateway_last_running, log_level,
                created_at, updated_at
         FROM app_settings WHERE id = 'default'",
    )?;
    let mut rows = stmt.query([])?;
    if let Some(row) = rows.next()? {
        let store_secrets_in_keychain: i64 = row.get(5)?;
        let global_proxy_enabled: i64 = row.get(6)?;
        let auto_start_enabled: i64 = row.get(10)?;
        let gateway_last_running: i64 = row.get(11)?;
        Ok(AppSettingsRow {
            theme: row.get(0)?,
            locale: row.get(1)?,
            global_proxy_json: row.get(2)?,
            network_timeout_ms: row.get(3)?,
            network_retry_json: row.get(4)?,
            // SQLite INTEGER 0/1 转 bool
            store_secrets_in_keychain: store_secrets_in_keychain != 0,
            global_proxy_enabled: global_proxy_enabled != 0,
            config_key: row.get(7)?,
            titlebar_info_json: row.get(8)?,
            backup_settings_json: row.get(9)?,
            auto_start_enabled: auto_start_enabled != 0,
            gateway_last_running: gateway_last_running != 0,
            log_level: row.get(12)?,
            created_at: row.get(13)?,
            updated_at: row.get(14)?,
        })
    } else {
        Err(IcodeError::internal(
            "app_settings 单例行不存在，请检查 V001 迁移是否已执行",
        ))
    }
}

/// 更新字段的辅助类型
///
/// 每个字段为 `Some(value)` 时表示需要更新，`None` 表示跳过。
/// JSON 字段使用 `Option<String>` 保留原始序列化文本，
/// 由 Service 层在调用前完成对象到 JSON 的序列化。
#[derive(Debug, Default, Clone)]
pub struct UpdateSettingsFields {
    pub theme: Option<String>,
    pub locale: Option<String>,
    pub global_proxy_json: Option<Option<String>>,
    pub network_timeout_ms: Option<i64>,
    pub network_retry_json: Option<Option<String>>,
    pub store_secrets_in_keychain: Option<bool>,
    /// 全局代理开关（关闭=直连，开启=所有请求走代理）
    pub global_proxy_enabled: Option<bool>,
    /// 配置密钥（AES 密钥）；`Some(None)` 表示重置/清空
    pub config_key: Option<Option<String>>,
    /// 标题栏信息展示配置 JSON
    pub titlebar_info_json: Option<String>,
    /// 备份设置 JSON
    pub backup_settings_json: Option<String>,
    /// 开机自启开关
    pub auto_start_enabled: Option<bool>,
    /// 网关上次关闭时的运行状态
    pub gateway_last_running: Option<bool>,
    /// 全局日志级别
    pub log_level: Option<String>,
}

/// 更新应用设置单例行
///
/// 仅更新 `fields` 中为 `Some` 的字段，其余保持原值。
/// `updated_at` 自动刷新为当前时间。
pub fn update(fields: &UpdateSettingsFields) -> IcodeResult<()> {
    let conn = get_conn()?;
    let now = Utc::now().to_rfc3339();

    // 动态构造 SET 子句，避免覆盖未传字段
    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    // 字段索引从 1 开始（?1, ?2, ...）
    let mut idx = 1usize;

    if let Some(v) = &fields.theme {
        sets.push(format!("theme = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &fields.locale {
        sets.push(format!("locale = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    // Option<Option<String>>：外层 Some 表示需要更新，内层 None 表示置空
    if let Some(v) = &fields.config_key {
        sets.push(format!("config_key = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &fields.global_proxy_json {
        sets.push(format!("global_proxy_json = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = fields.network_timeout_ms {
        sets.push(format!("network_timeout_ms = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = &fields.network_retry_json {
        sets.push(format!("network_retry_json = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = fields.store_secrets_in_keychain {
        sets.push(format!("store_secrets_in_keychain = ?{idx}"));
        // SQLite 没有 bool，存为 INTEGER 0/1
        params.push(Box::new(if v { 1i64 } else { 0i64 }));
        idx += 1;
    }
    if let Some(v) = fields.global_proxy_enabled {
        sets.push(format!("global_proxy_enabled = ?{idx}"));
        // SQLite 没有 bool，存为 INTEGER 0/1
        params.push(Box::new(if v { 1i64 } else { 0i64 }));
        idx += 1;
    }
    if let Some(v) = &fields.titlebar_info_json {
        sets.push(format!("titlebar_info_json = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = &fields.backup_settings_json {
        sets.push(format!("backup_settings_json = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(v) = fields.auto_start_enabled {
        sets.push(format!("auto_start_enabled = ?{idx}"));
        // SQLite 没有 bool，存为 INTEGER 0/1
        params.push(Box::new(if v { 1i64 } else { 0i64 }));
        idx += 1;
    }
    if let Some(v) = fields.gateway_last_running {
        sets.push(format!("gateway_last_running = ?{idx}"));
        // SQLite 没有 bool，存为 INTEGER 0/1
        params.push(Box::new(if v { 1i64 } else { 0i64 }));
        idx += 1;
    }
    if let Some(v) = &fields.log_level {
        sets.push(format!("log_level = ?{idx}"));
        params.push(Box::new(v.clone()));
        idx += 1;
    }

    // 至少更新 updated_at
    sets.push(format!("updated_at = ?{idx}"));
    params.push(Box::new(now));

    let sql = format!(
        "UPDATE app_settings SET {} WHERE id = 'default'",
        sets.join(", ")
    );

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, params_ref.as_slice())?;
    if affected == 0 {
        return Err(IcodeError::internal(
            "app_settings 单例行不存在，无法更新；请检查 V001 迁移是否已执行",
        ));
    }
    Ok(())
}
