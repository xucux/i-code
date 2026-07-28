//! # CLI 管理模块类型定义
//!
//! 与前端 `src/modules/cli-management/types.ts` 对齐，
//! 序列化字段使用 camelCase，确保前后端 JSON 字段名一致。
//!
//! 对应数据库表：
//! - `cli_profiles`：受管 CLI 配置档案
//! - `cli_providers`：CLI 与 Gateway 供应商绑定
//! - `cli_model_mappings`：CLI 内模型别名到真实模型的映射

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// 受管 CLI 类型
///
/// 对应 database.md §5.2 与 development.md §5.6.1。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CliType {
    /// Claude Code CLI
    ClaudeCode,
    /// OpenAI Codex CLI
    Codex,
    /// OpenCode CLI
    OpenCode,
    /// Google Gemini CLI
    GeminiCli,
    /// Cursor Agent 模式
    CursorAgent,
    /// 自定义 CLI
    Custom,
}

impl CliType {
    /// 从字符串解析 CLI 类型
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "claude-code" => Some(Self::ClaudeCode),
            "codex" => Some(Self::Codex),
            "opencode" => Some(Self::OpenCode),
            "gemini-cli" => Some(Self::GeminiCli),
            "cursor-agent" => Some(Self::CursorAgent),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    /// 转换为数据库字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
            Self::GeminiCli => "gemini-cli",
            Self::CursorAgent => "cursor-agent",
            Self::Custom => "custom",
        }
    }
}

/// CLI 配置档案 DTO
///
/// 对应 `cli_profiles` 表。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliProfile {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    /// CLI 类型字符串，由前端/调用方按需解析为枚举
    pub cli_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_json: Option<String>,
    pub is_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// CLI 绑定供应商 DTO
///
/// 对应 `cli_providers` 表。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliProvider {
    pub id: String,
    pub cli_profile_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    pub display_name: String,
    /// 路由模式：1=走本地网关，0=直连供应商
    pub route_mode: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance_json: Option<String>,
    pub sort_order: i32,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// CLI 模型映射 DTO
///
/// 对应 `cli_model_mappings` 表。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliModelMapping {
    pub id: String,
    pub cli_provider_id: String,
    pub cli_model_alias: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_model_id: Option<String>,
    /// 输入模式：`select`（从 Gateway 模型选择）或 `manual`（手动输入）
    pub input_mode: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建 CLI 档案输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCliProfileInput {
    pub slug: String,
    pub display_name: String,
    pub cli_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_json: Option<String>,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
}

/// 更新 CLI 档案输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCliProfileInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enabled: Option<bool>,
}

/// 创建 CLI 供应商绑定输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCliProviderInput {
    pub cli_profile_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    pub display_name: String,
    #[serde(default)]
    pub route_mode: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_json: Option<String>,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub is_default: bool,
}

/// 更新 CLI 供应商绑定输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCliProviderInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_mode: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
}

/// 创建 CLI 模型映射输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCliModelMappingInput {
    pub cli_provider_id: String,
    pub cli_model_alias: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_model_id: Option<String>,
    #[serde(default = "default_select_mode")]
    pub input_mode: String,
}

/// 更新 CLI 模型映射输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCliModelMappingInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_model_alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_mode: Option<String>,
}

/// CLI 配置文件探测结果
///
/// 只返回路径与解析状态，不向前端暴露配置文件正文。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliConfigFileInspection {
    pub cli_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_path: Option<String>,
    pub suggested_path: String,
    pub resolved_path: String,
    pub format: String,
    pub exists: bool,
    pub is_file: bool,
    pub readable: bool,
    /// `missing` / `valid` / `invalid`
    pub parse_status: String,
    /// `not-file` / `unreadable` / `invalid-syntax`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// 对应客户端 CLI 是否在 PATH 中可用
    pub client_available: bool,
}

/// CLI 配置文件内容（前端编辑回写）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliConfigFileContent {
    pub cli_type: String,
    pub resolved_path: String,
    pub format: String,
    pub content: String,
}

/// Claude CLI 应用时的单条角色映射输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyClaudeConfigMappingItem {
    pub role: String,
    pub display_name: String,
    pub actual_model: String,
    /// 前端字段名为 `supports1M`，单独指定以避免 `1m` 与 `1M` 大小写不匹配
    #[serde(rename = "supports1M")]
    pub supports_1m: bool,
}

/// Claude CLI 应用时的开关配置输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyClaudeConfigSwitches {
    pub hide_co_author: bool,
    pub agent_teams: bool,
    pub tool_search: bool,
    pub max_effort: bool,
    pub disable_autoupdater: bool,
}

/// Claude CLI 应用配置输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyClaudeConfigInput {
    pub cli_provider_id: String,
    pub mappings: Vec<ApplyClaudeConfigMappingItem>,
    pub fallback_model: String,
    pub api_key: String,
    pub switches: ApplyClaudeConfigSwitches,
}

/// Claude CLI 应用配置结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyClaudeConfigResult {
    pub cli_provider_id: String,
    pub resolved_path: String,
    pub content: String,
}

fn default_true() -> bool {
    true
}

fn default_select_mode() -> String {
    "select".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_type_roundtrip() {
        for t in [
            CliType::ClaudeCode,
            CliType::Codex,
            CliType::OpenCode,
            CliType::GeminiCli,
            CliType::CursorAgent,
            CliType::Custom,
        ] {
            let s = t.as_str();
            assert_eq!(CliType::from_str(s), Some(t));
        }
        assert_eq!(CliType::from_str("unknown"), None);
    }
}
