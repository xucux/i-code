//! # 工作区模块
//!
//! 管理工作区（Workspace）以及每个工作区下的 CLI 子配置：
//! Prompts、MCP Servers、Skills。
//! 同时提供「应用」能力，将工作区配置写入 CLI 实际配置文件。
//!
//! ## 模块组成
//!
//! - [`types`]：`Workspace` / `WorkspaceCliConfig` / `WorkspacePrompt` /
//!   `WorkspaceMcpServer` / `WorkspaceSkill` 等 DTO
//! - [`repository`]：`workspaces` / `workspace_cli_configs` / `workspace_prompts` /
//!   `workspace_mcp_servers` / `workspace_skills` 表的 CRUD
//! - [`service`]：工作区、子配置、应用编排的业务逻辑与校验
//! - [`commands`]：Tauri Command 声明
//!
//! ## v0.1 实现范围
//!
//! - 工作区 CRUD 与切换
//! - Prompts / MCP Servers / Skills CRUD
//! - 创建 workspace 时自动为所有 CLI 档案生成 `workspace_cli_configs`
//! - 子配置变更时自动将对应 `workspace_cli_configs.pending_apply` 置 1
//! - `workspace_apply` 基础实现：生成统一 JSON 配置并写入 `cli_profiles.config_file_path`
//!
//! ## 与其他模块的关系
//!
//! - 依赖 [`cli_management`](crate::modules::cli_management) 获取 CLI 档案列表与路径
//! - 应用配置时会直接写入文件系统

pub mod commands;
pub mod repository;
pub mod service;
pub mod types;

pub use service::WorkspaceServiceHandle;
