//! 脚本模板市场 DTO
//!
//! 与 `docs/proposals/script-template-marketplace.md` 及公共仓 catalog/meta 对齐。
//! 序列化字段使用 camelCase。

use serde::{Deserialize, Serialize};

/// 官方公共仓（人类可读）
pub const MARKETPLACE_REPO_URL: &str = "https://github.com/xucux/i-code-script-templates";

/// raw 内容基址（默认 main 分支）
pub const MARKETPLACE_BASE_URL: &str =
    "https://raw.githubusercontent.com/xucux/i-code-script-templates/main";

/// catalog 相对路径
pub const CATALOG_PATH: &str = "catalog.json";

/// 支持从市场应用的 kind 白名单（首期仅额度）
pub const SUPPORTED_KINDS: &[&str] = &["balance"];

/// catalog / 单脚本体积上限
pub const MAX_CATALOG_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_SCRIPT_BYTES: usize = 64 * 1024;

/// 默认缓存 TTL（秒）
pub const DEFAULT_CACHE_TTL_SECS: u64 = 900;

/// 远程 catalog.json 根结构
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCatalog {
    pub schema_version: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
    #[serde(default)]
    pub items: Vec<RemoteCatalogItem>,
}

/// catalog 中的单条（列表字段，不含脚本正文）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCatalogItem {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub kind: String,
    #[serde(default = "default_engine")]
    pub engine: String,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_timeout_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_hosts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_app_version: Option<String>,
    /// 脚本依赖的变量列表（系统变量 + 供应商「扩展模板变量」）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub var_list: Option<Vec<VarDef>>,
}

fn default_engine() -> String {
    "rhai".to_string()
}

/// 变量来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VarSource {
    /// 引擎自动注入的系统变量（api_key / provider.* / auth.* / template.* / now_ms 等）
    System,
    /// 供应商「扩展模板变量」中由用户配置的键值对，脚本通过 variables["name"] 访问
    Custom,
}

/// 脚本依赖的单个变量声明
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VarDef {
    /// 变量名（如 api_key / provider.base_url / cookie / token）
    pub name: String,
    /// 变量来源
    pub source: VarSource,
    /// 是否必填
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
}

/// 返回给前端的市场列表项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceItemSummary {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub kind: String,
    pub engine: String,
    pub author: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_app_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_timeout_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_hosts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var_list: Option<Vec<VarDef>>,
}

impl From<&RemoteCatalogItem> for MarketplaceItemSummary {
    fn from(item: &RemoteCatalogItem) -> Self {
        Self {
            id: item.id.clone(),
            slug: item.slug.clone(),
            name: item.name.clone(),
            kind: item.kind.clone(),
            engine: item.engine.clone(),
            author: item.author.clone(),
            description: item.description.clone(),
            tags: item.tags.clone(),
            version: item.version.clone(),
            created_at: item.created_at.clone(),
            updated_at: item.updated_at.clone(),
            min_app_version: item.min_app_version.clone(),
            default_timeout_ms: item.default_timeout_ms,
            allowed_hosts: item.allowed_hosts.clone(),
            var_list: item.var_list.clone(),
        }
    }
}

/// 市场列表结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceListResult {
    /// 源标识（repo URL 或 base）
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    pub items: Vec<MarketplaceItemSummary>,
    pub fetched_at: String,
    pub from_cache: bool,
}

/// 列表筛选
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceListFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword: Option<String>,
    #[serde(default)]
    pub force_refresh: bool,
}

/// 市场条目详情（含脚本正文可选）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceItemDetail {
    #[serde(flatten)]
    pub summary: MarketplaceItemSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
}

/// 脚本预览结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceScriptPreview {
    pub id: String,
    pub slug: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub script_body: String,
}

/// slug 冲突策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketplaceConflictStrategy {
    Rename,
    Fail,
}

impl MarketplaceConflictStrategy {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "rename" => Some(Self::Rename),
            "fail" => Some(Self::Fail),
            _ => None,
        }
    }
}

impl Default for MarketplaceConflictStrategy {
    fn default() -> Self {
        Self::Rename
    }
}

/// 从市场应用为本地模板
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceApplyInput {
    /// catalog 项 id，如 `balance/deepseek-balance`
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug_override: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_override: Option<String>,
    /// 默认 rename
    #[serde(default)]
    pub conflict_strategy: MarketplaceConflictStrategy,
    /// 默认 false：创建为 draft
    #[serde(default)]
    pub publish_after_create: bool,
}
