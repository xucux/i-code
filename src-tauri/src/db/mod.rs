//! # 数据库基础设施模块
//!
//! 提供 SQLite 连接池、迁移机制与表结构定义。
//!
//! ## 模块组成
//!
//! - [`connection`]：r2d2 连接池单例管理，提供全局 [`DbPool`] 句柄。
//! - [`migrations`]：基于 `schema_migrations` 表的版本化迁移执行器。
//! - [`schema`]：表名常量与 schema 版本号。
//!
//! ## 数据库文件位置
//!
//! 通过 Tauri 的 `path().app_config_dir()` 获取应用数据目录，
//! 由 `tauri.conf.json` 中的 `identifier`（`com.icode.app`）决定：
//!
//! - macOS：`~/Library/Application Support/com.icode.app/i-code.db`
//! - Windows：`%APPDATA%/com.icode.app/i-code.db`
//! - Linux：`~/.config/com.icode.app/i-code.db`
//!
//! ## 迁移机制
//!
//! 迁移文件位于 `src-tauri/src/db/migrations/`，命名格式：`V{version}__{description}.sql`。
//! 应用启动时自动执行未应用的迁移，按版本号顺序执行。

pub mod connection;
pub mod global_config;
pub mod migrations;
pub mod schema;

pub use connection::{close_db_pool, DbConn, init_db_pool, get_db_pool, reset_db_pool};
pub use global_config::get_global_config;
pub use migrations::{run_migrations, run_migrations_with_conn};
pub use schema::SCHEMA_VERSION;
