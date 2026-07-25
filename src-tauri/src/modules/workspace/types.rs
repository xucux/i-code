//! # 工作区模块类型定义
//!
//! 与前端 `src/modules/workspace/types.ts` 对齐。
//! 序列化字段使用 camelCase，确保前后端 JSON 字段名一致。
//!
//! 对应数据库表：
//! - `workspaces`：工作区主表
//! - `workspace_cli_configs`：工作区 × CLI 配置头
//! - `workspace_prompts`：工作区 Prompts
//! - `workspace_mcp_servers`：工作区 MCP 配置
//! - `workspace_skills`：工作区 Skill 配置

use serde::{Deserialize, Serialize};

use crate::modules::cli_management::types::CliProfile;

/// 工作区 DTO
///
/// 对应 `workspaces` 表。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub root_path: String,
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_applied_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 工作区 CLI 配置头 DTO
///
/// 对应 `workspace_cli_configs` 表。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCliConfig {
    pub id: String,
    pub workspace_id: String,
    pub cli_profile_id: String,
    pub is_applied: bool,
    pub pending_apply: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 工作区 Prompt DTO
///
/// 对应 `workspace_prompts` 表。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePrompt {
    pub id: String,
    pub workspace_cli_config_id: String,
    pub name: String,
    pub content: String,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// 工作区 MCP Server DTO
///
/// 对应 `workspace_mcp_servers` 表。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMcpServer {
    pub id: String,
    pub workspace_cli_config_id: String,
    pub name: String,
    /// 传输方式：`stdio` / `sse` / `http`
    pub transport: String,
    /// MCP 完整配置 JSON 字符串
    pub config_json: String,
    pub is_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 工作区 Skill DTO
///
/// 对应 `workspace_skills` 表。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSkill {
    pub id: String,
    pub workspace_cli_config_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub is_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建工作区输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceInput {
    pub slug: String,
    pub display_name: String,
    pub root_path: String,
    #[serde(default = "default_false")]
    pub is_active: bool,
}

/// 更新工作区输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkspaceInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_path: Option<String>,
}

/// 创建 Prompt 输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspacePromptInput {
    pub workspace_cli_config_id: String,
    pub name: String,
    pub content: String,
    #[serde(default)]
    pub sort_order: i32,
}

/// 更新 Prompt 输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkspacePromptInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<i32>,
}

/// 创建 MCP Server 输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceMcpServerInput {
    pub workspace_cli_config_id: String,
    pub name: String,
    pub transport: String,
    pub config_json: String,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
}

/// 更新 MCP Server 输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkspaceMcpServerInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enabled: Option<bool>,
}

/// 创建 Skill 输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceSkillInput {
    pub workspace_cli_config_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
}

/// 更新 Skill 输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkspaceSkillInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enabled: Option<bool>,
}

/// 应用工作区结果
///
/// 由 `workspace_apply` 返回，告知前端哪些 CLI 配置已写入、哪些失败。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyWorkspaceResult {
    pub workspace_id: String,
    pub applied_count: usize,
    pub failed_count: usize,
    pub details: Vec<ApplyCliResult>,
}

/// 单个 CLI 的应用结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyCliResult {
    pub cli_profile_id: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 工作区 CLI 配置聚合项
///
/// 包含一个 CLI 配置头及其关联的 CLI 档案、Prompts / MCP / Skills。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCliConfigAggregate {
    pub config: WorkspaceCliConfig,
    pub profile: CliProfile,
    pub prompts: Vec<WorkspacePrompt>,
    pub mcp_servers: Vec<WorkspaceMcpServer>,
    pub skills: Vec<WorkspaceSkill>,
}

/// 工作区聚合数据
///
/// 一次请求返回工作区下所有 CLI 配置及其子配置，减少前端请求数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceAggregate {
    pub workspace: Workspace,
    pub cli_configs: Vec<WorkspaceCliConfigAggregate>,
}

/// 工作区预览结果
///
/// 返回指定 CLI 配置头将要写入 CLI 实际配置文件的内容文本。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePreviewResult {
    pub workspace_cli_config_id: String,
    pub cli_profile_id: String,
    pub cli_type: String,
    pub content: String,
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}
