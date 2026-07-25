//! # 工作区模块 Tauri Commands
//!
//! 所有 Command 只做参数校验、调用 Service、捕获错误并转换为 `IcodeError`。
//! 前后端字段采用 camelCase 序列化。

use tauri::{AppHandle, Emitter, State};

use crate::error::IcodeResult;

use super::service::WorkspaceServiceHandle;
use super::types::{
    ApplyCliResult, ApplyWorkspaceResult, CreateWorkspaceInput, CreateWorkspaceMcpServerInput,
    CreateWorkspacePromptInput, CreateWorkspaceSkillInput, UpdateWorkspaceInput,
    UpdateWorkspaceMcpServerInput, UpdateWorkspacePromptInput, UpdateWorkspaceSkillInput,
    Workspace, WorkspaceAggregate, WorkspaceCliConfig, WorkspaceMcpServer, WorkspacePreviewResult,
    WorkspacePrompt, WorkspaceSkill,
};

// ===== 工作区 Commands =====

/// 列出所有工作区
#[tauri::command]
pub fn workspace_list(handle: State<WorkspaceServiceHandle>) -> IcodeResult<Vec<Workspace>> {
    handle.service().list_workspaces()
}

/// 获取工作区详情
#[tauri::command]
pub fn workspace_get(
    id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<Workspace> {
    handle.service().get_workspace(&id)
}

/// 获取当前激活的工作区
#[tauri::command]
pub fn workspace_get_active(
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<Option<Workspace>> {
    handle.service().get_active_workspace()
}

/// 创建工作区
#[tauri::command]
pub fn workspace_create(
    input: CreateWorkspaceInput,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<Workspace> {
    handle.service().create_workspace(input)
}

/// 更新工作区
#[tauri::command]
pub fn workspace_update(
    id: String,
    input: UpdateWorkspaceInput,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<Workspace> {
    handle.service().update_workspace(&id, input)
}

/// 删除工作区
#[tauri::command]
pub fn workspace_delete(
    id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<()> {
    handle.service().delete_workspace(&id)
}

/// 切换激活工作区
#[tauri::command]
pub fn workspace_switch(
    id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<Workspace> {
    handle.service().switch_workspace(&id)
}

// ===== 工作区 CLI 配置头 Commands =====

/// 列出某工作区下的 CLI 配置头
#[tauri::command]
pub fn workspace_cli_config_list(
    workspace_id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<Vec<WorkspaceCliConfig>> {
    handle.service().list_cli_configs(&workspace_id)
}

// ===== Prompt Commands =====

/// 列出某 CLI 配置头下的 Prompts
#[tauri::command]
pub fn workspace_prompt_list(
    workspace_cli_config_id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<Vec<WorkspacePrompt>> {
    handle.service().list_prompts(&workspace_cli_config_id)
}

/// 获取 Prompt 详情
#[tauri::command]
pub fn workspace_prompt_get(
    id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<WorkspacePrompt> {
    handle.service().get_prompt(&id)
}

/// 创建 Prompt
#[tauri::command]
pub fn workspace_prompt_create(
    input: CreateWorkspacePromptInput,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<WorkspacePrompt> {
    handle.service().create_prompt(input)
}

/// 更新 Prompt
#[tauri::command]
pub fn workspace_prompt_update(
    id: String,
    input: UpdateWorkspacePromptInput,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<WorkspacePrompt> {
    handle.service().update_prompt(&id, input)
}

/// 删除 Prompt
#[tauri::command]
pub fn workspace_prompt_delete(
    id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<()> {
    handle.service().delete_prompt(&id)
}

// ===== MCP Server Commands =====

/// 列出某 CLI 配置头下的 MCP Servers
#[tauri::command]
pub fn workspace_mcp_server_list(
    workspace_cli_config_id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<Vec<WorkspaceMcpServer>> {
    handle.service().list_mcp_servers(&workspace_cli_config_id)
}

/// 获取 MCP Server 详情
#[tauri::command]
pub fn workspace_mcp_server_get(
    id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<WorkspaceMcpServer> {
    handle.service().get_mcp_server(&id)
}

/// 创建 MCP Server
#[tauri::command]
pub fn workspace_mcp_server_create(
    input: CreateWorkspaceMcpServerInput,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<WorkspaceMcpServer> {
    handle.service().create_mcp_server(input)
}

/// 更新 MCP Server
#[tauri::command]
pub fn workspace_mcp_server_update(
    id: String,
    input: UpdateWorkspaceMcpServerInput,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<WorkspaceMcpServer> {
    handle.service().update_mcp_server(&id, input)
}

/// 删除 MCP Server
#[tauri::command]
pub fn workspace_mcp_server_delete(
    id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<()> {
    handle.service().delete_mcp_server(&id)
}

// ===== Skill Commands =====

/// 列出某 CLI 配置头下的 Skills
#[tauri::command]
pub fn workspace_skill_list(
    workspace_cli_config_id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<Vec<WorkspaceSkill>> {
    handle.service().list_skills(&workspace_cli_config_id)
}

/// 获取 Skill 详情
#[tauri::command]
pub fn workspace_skill_get(
    id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<WorkspaceSkill> {
    handle.service().get_skill(&id)
}

/// 创建 Skill
#[tauri::command]
pub fn workspace_skill_create(
    input: CreateWorkspaceSkillInput,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<WorkspaceSkill> {
    handle.service().create_skill(input)
}

/// 更新 Skill
#[tauri::command]
pub fn workspace_skill_update(
    id: String,
    input: UpdateWorkspaceSkillInput,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<WorkspaceSkill> {
    handle.service().update_skill(&id, input)
}

/// 删除 Skill
#[tauri::command]
pub fn workspace_skill_delete(
    id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<()> {
    handle.service().delete_skill(&id)
}

// ===== 应用工作区 Command =====

/// 将当前工作区配置应用到 CLI 实际配置文件
///
/// 应用成功后广播 `workspace:applied` 事件，payload 为应用结果，
/// 方便 CLI 配置列表等监听方实时刷新状态。
#[tauri::command]
pub fn workspace_apply(
    app_handle: AppHandle,
    id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<ApplyWorkspaceResult> {
    let result = handle.service().apply_workspace(&id)?;
    let _ = app_handle.emit("workspace:applied", &result);
    Ok(result)
}

/// 聚合查询工作区下所有 CLI 配置及其子配置
///
/// 一次请求返回工作区、CLI 配置头、CLI 档案、Prompts、MCP、Skills 的完整关联数据。
#[tauri::command]
pub fn workspace_aggregate(
    id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<WorkspaceAggregate> {
    handle.service().aggregate_workspace(&id)
}

/// 预览单个 CLI 配置头将要生成的配置文件内容
///
/// 只读生成，不写入文件系统。
#[tauri::command]
pub fn workspace_preview(
    workspace_cli_config_id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<WorkspacePreviewResult> {
    handle.service().preview_cli_config(&workspace_cli_config_id)
}

/// 应用单个 CLI 配置头
///
/// 仅将指定 CLI 配置头的配置写入对应 CLI 配置文件，并标记为已应用。
/// 成功后广播 `workspace:cli-config-applied` 事件。
#[tauri::command]
pub fn workspace_apply_cli_config(
    app_handle: AppHandle,
    workspace_cli_config_id: String,
    handle: State<WorkspaceServiceHandle>,
) -> IcodeResult<ApplyCliResult> {
    let result = handle.service().apply_cli_config(&workspace_cli_config_id)?;
    let _ = app_handle.emit("workspace:cli-config-applied", &result);
    Ok(result)
}
