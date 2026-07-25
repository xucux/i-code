//! # CLI 管理模块
//!
//! 管理受管 CLI（Claude Code、Codex、Gemini CLI 等）的配置档案、
//! 与 Gateway 供应商的绑定关系，以及 CLI 内模型别名到真实模型的映射。
//!
//! ## 模块组成
//!
//! - [`types`]：`CliProfile` / `CliProvider` / `CliModelMapping` 等 DTO
//! - [`repository`]：`cli_profiles` / `cli_providers` / `cli_model_mappings` 表的 CRUD
//! - [`service`]：CLI 档案、绑定、映射的业务逻辑与校验
//! - [`commands`]：Tauri Command 声明
//!
//! ## v0.1 实现范围
//!
//! - CLI 档案 CRUD
//! - CLI 供应商绑定 CRUD（含路由模式校验）
//! - CLI 模型映射 CRUD（含输入模式校验）
//! - 向 workspace 模块暴露 `list_profile_ids` 以初始化工作区配置头
//!
//! ## 与其他模块的关系
//!
//! - 依赖 [`ai_gateway`](crate::modules::ai_gateway) 校验 `provider_id` 存在性
//! - 被 [`workspace`](crate::modules::workspace) 调用以维护 `workspace_cli_configs`

pub mod commands;
pub mod repository;
pub mod service;
pub mod types;

pub use service::CliManagementServiceHandle;
