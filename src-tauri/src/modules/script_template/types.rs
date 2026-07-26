//! # 脚本模板模块类型定义
//!
//! 与 `docs/proposals/balance-script-templates.md` 及 `script_templates` 表对齐。
//! 序列化字段使用 camelCase，确保前后端 JSON 字段名一致。

use serde::{Deserialize, Serialize};

/// 脚本模板类型（本期仅额度监控）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScriptTemplateKind {
    /// 额度监控
    Balance,
}

impl ScriptTemplateKind {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "balance" => Some(Self::Balance),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Balance => "balance",
        }
    }
}

/// 脚本模板生命周期状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScriptTemplateStatus {
    /// 草稿
    Draft,
    /// 启用
    Active,
    /// 禁用
    Disabled,
}

impl ScriptTemplateStatus {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(Self::Draft),
            "active" => Some(Self::Active),
            "disabled" => Some(Self::Disabled),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }
}

/// 状态迁移动作
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptTemplateStatusAction {
    /// 发布/启用
    Publish,
    /// 禁用
    Disable,
    /// 重新设为草稿
    RevertToDraft,
}

impl ScriptTemplateStatusAction {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "publish" => Some(Self::Publish),
            "disable" => Some(Self::Disable),
            "revert_to_draft" => Some(Self::RevertToDraft),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::Disable => "disable",
            Self::RevertToDraft => "revert_to_draft",
        }
    }
}

/// 脚本模板 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptTemplate {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub kind: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub script_body: String,
    pub engine: String,
    pub default_timeout_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_hosts_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_test_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_test_ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_test_message: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 列表筛选参数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptTemplateListFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword: Option<String>,
}

/// 创建脚本模板输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateScriptTemplateInput {
    pub name: String,
    pub slug: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub script_body: String,
    #[serde(default = "default_timeout")]
    pub default_timeout_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_hosts_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet_id: Option<String>,
    #[serde(default)]
    pub sort_order: i64,
}

fn default_kind() -> String {
    "balance".to_string()
}

fn default_timeout() -> i64 {
    15000
}

/// 更新脚本模板输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateScriptTemplateInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_timeout_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_hosts_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<i64>,
}

/// 试运行输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptTemplateTestInput {
    pub template_id: String,
    pub provider_id: String,
    /// 可选：用未保存的编辑器正文覆盖库中版本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_body_override: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// 试运行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptTemplateTestResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<crate::modules::balance::types::BalanceSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<String>,
}

/// 引用该模板的供应商摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptTemplateRef {
    pub provider_id: String,
    pub slug: String,
    pub display_name: String,
}

/// 内置 snippet 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptSnippet {
    pub id: String,
    pub name: String,
    pub description: String,
    pub body: String,
}

/// 下拉选择项（仅 active 模板）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptTemplateSelectItem {
    pub id: String,
    pub name: String,
    pub slug: String,
}
