//! # 数据库迁移机制
//!
//! 基于 `schema_migrations` 表实现版本化迁移，灵感来自 `refinery` 与 `sqlx`。
//!
//! ## 迁移文件命名
//!
//! 文件位于 `src-tauri/src/db/migrations/`，命名格式：`V{version}__{description}.sql`
//! - `version`：正整数，零填充 3 位（如 `V001`、`V002`），便于排序。
//! - `description`：简短描述，下划线分隔。
//! - 例：`V001__init.sql`、`V002__add_virtual_providers.sql`
//!
//! ## 执行流程
//!
//! 1. 创建 `schema_migrations` 表（若不存在）。
//! 2. 读取已应用的迁移版本集合。
//! 3. 遍历内置迁移列表，跳过已应用的，按版本号顺序执行未应用的。
//! 4. 每个迁移在事务中执行：成功则写入版本记录并提交，失败则回滚并返回错误。
//!
//! ## 基线重置模式
//!
//! 当 [`BUILTIN_MIGRATIONS`] 仅包含基线版本（V001）时，启用基线重置模式：
//! - 在执行迁移前清空 `schema_migrations` 表中的所有记录。
//! - 这样可让 V001 在已有数据库上重新应用，配合 V001 中使用 `CREATE TABLE IF NOT EXISTS`
//!   与 `INSERT OR IGNORE` 实现幂等重跑。
//!
//! ## 内置迁移
//!
//! 当前所有迁移通过 `include_str!` 宏在编译时嵌入二进制，
//! 避免依赖运行时文件系统路径。新增迁移时在 [`BUILTIN_MIGRATIONS`] 中追加。/// 当存在 V002+ 迁移时，自动退出基线重置模式，转为增量迁移。
use crate::error::{IcodeError, IcodeResult};

use super::connection::{get_db_pool, DbConn};
use super::schema::table;

/// 初始迁移 v1：由 main.sql 聚合生成的完整表结构 + 索引 + 默认数据（基线重置版）
///
/// 该迁移为当前唯一基线版本，包含全部表结构、索引与默认数据。
/// 所有 `CREATE TABLE` 使用 `IF NOT EXISTS`，便于在已有数据库上重跑。
const V001__INIT: &str = include_str!("./migrations/V001__init.sql");

/// 供应商扩展模板变量：providers 表新增 script_variables_json 列
const V002__PROVIDER_SCRIPT_VARIABLES: &str =
    include_str!("./migrations/V002__provider_script_variables.sql");

/// 内置迁移列表：(版本号, 描述, SQL 内容)
///
/// 增量迁移模式：V001 为基线，V002+ 为增量变更。
/// 新增迁移在此数组中追加，并同步更新 `SCHEMA_VERSION`。
const BUILTIN_MIGRATIONS: &[(u32, &str, &str)] = &[
    (1, "init", V001__INIT),
    (2, "provider_script_variables", V002__PROVIDER_SCRIPT_VARIABLES),
];

/// 执行所有未应用的迁移
///
/// 应用启动时（在 Tauri `setup` 中）调用一次，确保数据库 schema 最新。
///
/// # 错误
/// - `DATABASE`：迁移文件 SQL 语法错误或冲突。
/// - `INTERNAL`：连接池未初始化。
pub fn run_migrations() -> IcodeResult<()> {
    let pool = get_db_pool()?;
    let mut conn = pool.get()?;
    run_migrations_with_conn(&mut conn)
}

/// 在指定连接上执行迁移
///
/// 主要供测试代码使用：传入内存数据库连接，验证迁移正确性。
pub fn run_migrations_with_conn(conn: &mut DbConn) -> IcodeResult<()> {
    // 1. 确保 schema_migrations 表存在
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {table} (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
        table = table::SCHEMA_MIGRATIONS
    )).map_err(|e| IcodeError::database(format!("创建 schema_migrations 表失败：{e}")))?;

    // 2. 基线重置模式：当仅保留基线版本 V001 时，清空 schema_migrations，
    //    使 V001 能够在已有数据库上重新应用（依赖 IF NOT EXISTS / INSERT OR IGNORE 幂等性）。
    if is_baseline_only() {
        log::info!("检测到仅基线版本 V001，清空 schema_migrations 以触发基线重跑");
        conn.execute(
            &format!("DELETE FROM {table}", table = table::SCHEMA_MIGRATIONS),
            [],
        ).map_err(|e| IcodeError::database(format!("清空 schema_migrations 失败：{e}")))?;
    }

    // 3. 查询已应用的最大版本号
    let current_version: u32 = conn
        .query_row(
            &format!("SELECT COALESCE(MAX(version), 0) FROM {table}", table = table::SCHEMA_MIGRATIONS),
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // 4. 按版本号顺序执行所有大于 current_version 的迁移
    for (version, desc, sql) in BUILTIN_MIGRATIONS.iter() {
        if *version <= current_version {
            continue;
        }

        log::info!("应用数据库迁移 V{version}__{desc}");

        // 在事务中执行，失败自动回滚
        let tx = conn.transaction()?;
        tx.execute_batch(sql).map_err(|e| {
            IcodeError::database(format!(
                "迁移 V{version}__{desc} 执行失败：{e}"
            ))
        })?;

        // 写入版本记录
        let now = chrono::Utc::now().to_rfc3339();
        tx.execute(
            &format!(
                "INSERT INTO {table} (version, applied_at) VALUES (?1, ?2)",
                table = table::SCHEMA_MIGRATIONS
            ),
            rusqlite::params![version, now],
        )?;

        tx.commit()?;
        log::info!("迁移 V{version}__{desc} 应用成功");
    }

    Ok(())
}

/// 判断当前是否仅保留基线版本（V001）
///
/// 当 [`BUILTIN_MIGRATIONS`] 仅包含一个迁移时，认定为基线模式。
/// 该模式会在执行迁移前清空 `schema_migrations` 表，使基线 V001 可以重跑。
fn is_baseline_only() -> bool {
    BUILTIN_MIGRATIONS.len() == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use r2d2::Pool;
    use r2d2_sqlite::SqliteConnectionManager;

    #[test]
    fn test_run_migrations_in_memory() {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).unwrap();
        let mut conn = pool.get().unwrap();

        // 首次执行迁移应成功
        run_migrations_with_conn(&mut conn).expect("首次迁移应成功");

        // 验证 schema_migrations 表存在且版本为当前最新迁移版本
        let version: i64 = conn
            .query_row(
                "SELECT MAX(version) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, crate::db::schema::SCHEMA_VERSION as i64);

        // 验证关键业务表已创建
        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN (
                    'app_settings', 'global_configs', 'providers', 'gateway_models', 'secrets',
                    'cli_profiles', 'workspaces', 'model_call_logs',
                    'gateway_settings', 'gateway_auth_keys', 'log_settings',
                    'virtual_providers', 'virtual_models', 'model_call_stats_daily',
                    'model_call_stats_hourly', 'webdav_configs', 'script_templates'
                )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 17);

        // 验证聚合后的关键列已存在
        let check_column = |table: &str, column: &str| {
            let columns: Vec<String> = conn
                .prepare(&format!("PRAGMA table_info({table})"))
                .unwrap()
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();
            assert!(
                columns.contains(&column.to_string()),
                "V001 应包含 {table}.{column} 列"
            );
        };
        check_column("log_settings", "enable_gateway_request_log");
        check_column("app_settings", "titlebar_info_json");
        check_column("app_settings", "backup_settings_json");
        check_column("app_settings", "config_key");
        check_column("model_configs", "price_per_1m_tokens");
        check_column("model_call_logs", "api_key_secret_id");
        check_column("webdav_configs", "password");
        check_column("providers", "script_variables_json");

        // 二次执行迁移应为 no-op，不报错
        run_migrations_with_conn(&mut conn).expect("二次迁移应为 no-op");
    }

    #[test]
    fn test_app_settings_default_data() {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations_with_conn(&mut conn).unwrap();

        // 验证默认 app_settings 已插入
        let theme: String = conn
            .query_row(
                "SELECT theme FROM app_settings WHERE id = 'default'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(theme, "dark");

        // 网关默认设置已初始化
        let port: i64 = conn
            .query_row(
                "SELECT gateway_port FROM gateway_settings WHERE id = 'default'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(port, 54321);

        // config_key 列已存在，默认值为 NULL
        let config_key: Option<String> = conn
            .query_row(
                "SELECT config_key FROM app_settings WHERE id = 'default'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(config_key.is_none());
    }

    #[test]
    fn test_global_configs_oauth_credentials() {
        use crate::db::schema::table;
        use r2d2::Pool;
        use r2d2_sqlite::SqliteConnectionManager;

        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations_with_conn(&mut conn).unwrap();

        // 验证 global_configs 表存在
        let exists: i64 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = '{}'",
                    table::GLOBAL_CONFIGS
                ),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1);

        // 验证 Antigravity / Google Gemini OAuth 凭据已写入
        let antigravity_id: String = conn
            .query_row(
                &format!(
                    "SELECT value FROM {table} WHERE \"group\" = 'oauth' AND key = 'antigravity_client_id'",
                    table = table::GLOBAL_CONFIGS
                ),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!antigravity_id.is_empty());

        let gemini_secret: String = conn
            .query_row(
                &format!(
                    "SELECT value FROM {table} WHERE \"group\" = 'oauth' AND key = 'google_gemini_client_secret'",
                    table = table::GLOBAL_CONFIGS
                ),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!gemini_secret.is_empty());
    }
}
