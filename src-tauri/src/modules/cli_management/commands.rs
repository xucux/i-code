//! # CLI 管理模块 Tauri Commands
//!
//! 所有 Command 只做参数校验、调用 Service、捕获错误并转换为 `IcodeError`。
//! 前后端字段采用 camelCase 序列化。

use tauri::State;

use crate::error::IcodeResult;

use super::service::CliManagementServiceHandle;
use super::types::{
    ApplyClaudeConfigInput, ApplyClaudeConfigResult, CliConfigFileContent, CliConfigFileInspection,
    CliModelMapping, CliProfile, CliProvider, CreateCliModelMappingInput, CreateCliProfileInput,
    CreateCliProviderInput, UpdateCliModelMappingInput, UpdateCliProfileInput, UpdateCliProviderInput,
};

// ===== CLI 档案 Commands =====

/// 列出所有 CLI 档案
#[tauri::command]
pub fn cli_profile_list(handle: State<CliManagementServiceHandle>) -> IcodeResult<Vec<CliProfile>> {
    handle.service().list_profiles()
}

/// 幂等创建并返回内置 CLI 档案
#[tauri::command]
pub fn cli_profile_ensure_defaults(
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<Vec<CliProfile>> {
    handle.service().ensure_default_profiles()
}

/// 探测 CLI 配置文件并验证语法，不返回文件内容
#[tauri::command]
pub fn cli_config_inspect(
    cli_type: String,
    configured_path: Option<String>,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<CliConfigFileInspection> {
    handle
        .service()
        .inspect_config_file(&cli_type, configured_path.as_deref())
}

/// 读取 CLI 配置文件内容（不含 Secret 正文，仅返回文本）
#[tauri::command]
pub fn cli_config_read(
    cli_type: String,
    configured_path: Option<String>,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<CliConfigFileContent> {
    handle
        .service()
        .read_config_file(&cli_type, configured_path.as_deref())
}

/// 将前端编辑后的内容写回 CLI 配置文件
#[tauri::command]
pub fn cli_config_save(
    cli_type: String,
    configured_path: Option<String>,
    content: String,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<CliConfigFileContent> {
    handle
        .service()
        .save_config_file(&cli_type, configured_path.as_deref(), &content)
}

/// 检测客户端 CLI 是否在 PATH 中可用
#[tauri::command]
pub fn cli_client_check(cli_type: String) -> IcodeResult<bool> {
    Ok(crate::modules::cli_management::service::is_client_available(&cli_type))
}

/// 获取 CLI 档案详情
#[tauri::command]
pub fn cli_profile_get(
    id: String,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<CliProfile> {
    handle.service().get_profile(&id)
}

/// 创建 CLI 档案
#[tauri::command]
pub fn cli_profile_create(
    input: CreateCliProfileInput,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<CliProfile> {
    handle.service().create_profile(input)
}

/// 更新 CLI 档案
#[tauri::command]
pub fn cli_profile_update(
    id: String,
    input: UpdateCliProfileInput,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<CliProfile> {
    handle.service().update_profile(&id, input)
}

/// 删除 CLI 档案
#[tauri::command]
pub fn cli_profile_delete(
    id: String,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<()> {
    handle.service().delete_profile(&id)
}

// ===== CLI 供应商绑定 Commands =====

/// 列出某 CLI 档案绑定的供应商
#[tauri::command]
pub fn cli_provider_list(
    cli_profile_id: String,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<Vec<CliProvider>> {
    handle.service().list_providers(&cli_profile_id)
}

/// 获取 CLI 供应商绑定详情
#[tauri::command]
pub fn cli_provider_get(
    id: String,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<CliProvider> {
    handle.service().get_provider(&id)
}

/// 创建 CLI 供应商绑定
#[tauri::command]
pub fn cli_provider_create(
    input: CreateCliProviderInput,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<CliProvider> {
    handle.service().create_provider(input)
}

/// 更新 CLI 供应商绑定
#[tauri::command]
pub fn cli_provider_update(
    id: String,
    input: UpdateCliProviderInput,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<CliProvider> {
    handle.service().update_provider(&id, input)
}

/// 删除 CLI 供应商绑定
#[tauri::command]
pub fn cli_provider_delete(
    id: String,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<()> {
    handle.service().delete_provider(&id)
}

// ===== CLI 模型映射 Commands =====

/// 列出某 CLI 供应商下的模型映射
#[tauri::command]
pub fn cli_model_mapping_list(
    cli_provider_id: String,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<Vec<CliModelMapping>> {
    handle.service().list_model_mappings(&cli_provider_id)
}

/// 创建 CLI 模型映射
#[tauri::command]
pub fn cli_model_mapping_create(
    input: CreateCliModelMappingInput,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<CliModelMapping> {
    handle.service().create_model_mapping(input)
}

/// 更新 CLI 模型映射
#[tauri::command]
pub fn cli_model_mapping_update(
    id: String,
    input: UpdateCliModelMappingInput,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<CliModelMapping> {
    handle.service().update_model_mapping(&id, input)
}

/// 删除 CLI 模型映射
#[tauri::command]
pub fn cli_model_mapping_delete(
    id: String,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<()> {
    handle.service().delete_model_mapping(&id)
}

/// 应用 Claude CLI 配置到实际配置文件
///
/// 根据传入的映射、开关、API Key 生成 Claude Code settings.json，
/// 写入 cli_profiles.config_file_path 或默认候选路径。
#[tauri::command]
pub fn cli_apply_claude_config(
    input: ApplyClaudeConfigInput,
    handle: State<CliManagementServiceHandle>,
) -> IcodeResult<ApplyClaudeConfigResult> {
    handle.service().apply_claude_config(input)
}
