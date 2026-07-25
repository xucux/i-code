//! # 应用全局设置模块
//!
//! 管理应用全局设置、主题、语言、网络超时与重试、配置密钥。
//! 网关监听地址与默认 API Key 已迁移到 [`ai_gateway`](crate::modules::ai_gateway) 模块。
//!
//! ## 模块组成
//!
//! - [`types`]：`AppSettings` / `AppSettingsDto` / `UpdateSettingsInput` 等 DTO
//! - [`repository`]：`app_settings` 表的读写（单例模式）
//! - [`service`]：业务逻辑（读取/更新）
//! - [`commands`]：Tauri Command 声明
//!
//! ## 单例约束
//!
//! `app_settings` 表固定单例记录（`id = 'default'`），
//! 由 V001 迁移脚本插入。本模块所有操作都针对该行。
//!
//! ## 与其他模块的关系
//!
//! - [`secret`](crate::modules::secret) 模块根据 `store_secrets_in_keychain`
//!   字段决定存储模式（v0.1 仅支持本地 AES-GCM）。

pub mod commands;
pub mod repository;
pub mod service;
pub mod types;

pub use service::SettingsServiceHandle;
