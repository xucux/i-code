//! # 内置供应商与模型种子数据加载
//!
//! 项目发版时随二进制打包两个 JSON 文件：
//! - `data/builtin-providers.json`
//! - `data/builtin-models.json`
//!
//! 这些文件**不导入数据库**，运行时通过 `include_str!` 嵌入二进制，
//! 由 Service 层在需要时解析并与数据库中用户自定义数据合并返回。
//!
//! ## 使用场景
//!
//! - 前端「从内置供应商添加」列表
//! - 前端「从内置模型添加」列表（可按供应商类型筛选）
//! - 创建供应商时一键填充默认 baseUrl / auth 配置
//! - 创建模型时一键填充默认 maxTokens / family 等配置
//!
//! ## 版本控制
//!
//! JSON 文件中的 `version` 字段用于未来判断是否需要刷新缓存或提示用户。
//! v0.1 仅做展示，不做版本校验逻辑。

use serde::{Deserialize, Serialize};

use crate::error::IcodeResult;

/// 内置模型能力配置
/// 对应参考项目 ModelCapabilities，以及 data/builtin-models.json 中的 `capabilities` 字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinModelCapabilities {
    /// 是否支持工具/函数调用；可能是布尔值或最大工具数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calling: Option<serde_json::Value>,
    /// 是否支持图片输入
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_input: Option<bool>,
    /// 编辑器工具提示，如 `multi-find-replace`、`apply-patch`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_tools: Option<String>,
}

/// 内置模型思考配置
/// 对应参考项目 thinking 字段，以及 data/builtin-models.json 中的 `thinking` 字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinModelThinking {
    #[serde(rename = "type")]
    #[serde(default)]
    pub thinking_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<i64>,
}

/// 内置供应商预设
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinProvider {
    /// 供应商标识符，如 `openai`
    pub id: String,
    /// 展示名称（英文）
    pub display_name: String,
    /// 中文展示名称，无中文名时为空串
    #[serde(default)]
    pub display_cn_name: String,
    /// UI 分组，如 `General` / `Local`
    pub category: String,
    /// 协议类型，如 `openai-chat-completion`
    pub provider_type: String,
    /// 该供应商支持的全部协议类型列表（含 provider_type），如 `["openai-chat-completion", "anthropic"]`
    ///
    /// 用于「从内置预设创建」时预填充协议类型与认证方式的可选范围；
    /// 未提供时回退为 `[provider_type]`。
    #[serde(default)]
    pub provider_types: Vec<String>,
    /// 默认 API Base URL
    pub base_url: String,
    /// 是否原样使用 base URL
    #[serde(default)]
    pub use_raw_base_url: bool,
    /// 供应商备注（来自内置预设，创建供应商时自动填充 remark 字段）
    #[serde(default)]
    pub remark: Option<String>,
    /// 支持的认证方法列表，如 `["api-key"]`
    pub auth_methods: Vec<String>,
    /// 默认认证配置（用户创建时可一键填充）
    pub default_auth: Option<serde_json::Value>,
    /// 是否默认开启官方模型自动拉取
    #[serde(default)]
    pub auto_fetch_official_models: bool,
    /// 排序优先级
    pub sort_order: i64,
    /// 默认附加请求头（用户创建供应商时自动填充到 provider_extra_headers 表）
    #[serde(default)]
    pub default_extra_headers: Option<std::collections::HashMap<String, String>>,
    /// 默认关联模型（用户创建供应商时自动创建 gateway_models）
    /// 对应 data/builtin-providers.json 中的 `defaultModels` 字段
    #[serde(default)]
    pub default_models: Option<Vec<BuiltinProviderDefaultModel>>,
}

/// 内置供应商默认关联模型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinProviderDefaultModel {
    /// 实际发给供应商的模型 ID（网关对外路由 `{provider_slug}/{model_id}` 的 model_id）
    pub model_id: String,
    /// 展示名称（可选，缺省时使用匹配到的内置模型展示名）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 在 data/builtin-models.json 中匹配的模型 id（可选，缺省时该条目不参与内置模型匹配）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_model_id: Option<String>,
}

/// 内置模型预设
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinModel {
    /// 模型标识符，如 `gpt-4.1`
    pub id: String,
    /// 展示名称
    pub display_name: String,
    /// 模型背景简介（参数/性能/发布日期等），仅用于前端内置模型列表悬浮展示，不落库
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// 模型族
    pub family: String,
    /// 适配的供应商类型列表
    pub provider_types: Vec<String>,
    /// 最大输入 Token 数
    pub max_input_tokens: Option<i64>,
    /// 最大输出 Token 数
    pub max_output_tokens: Option<i64>,
    /// Token 计数乘数
    #[serde(default = "crate::modules::ai_gateway::types::default_token_multiplier")]
    pub token_count_multiplier: f64,
    /// 是否支持流式输出
    pub stream: Option<bool>,
    /// 是否支持并行工具调用
    pub parallel_tool_calling: Option<bool>,
    /// 分词器选项，如 `openai`、`deepseek`、`default`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer: Option<String>,
    /// 模型能力配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<BuiltinModelCapabilities>,
    /// 思考模式配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<BuiltinModelThinking>,
}

/// 内置供应商列表包装
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinProvidersManifest {
    pub version: String,
    pub description: String,
    pub providers: Vec<BuiltinProvider>,
}

/// 内置模型列表包装
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinModelsManifest {
    pub version: String,
    pub description: String,
    pub models: Vec<BuiltinModel>,
}

/// 加载内置供应商预设（编译时嵌入二进制）
///
/// 使用 `include_str!` 确保 JSON 随应用一起分发，不依赖运行时文件系统。
pub fn load_builtin_providers() -> IcodeResult<Vec<BuiltinProvider>> {
    const JSON: &str = include_str!("../../../data/builtin-providers.json");
    let manifest: BuiltinProvidersManifest = serde_json::from_str(JSON)?;
    Ok(manifest.providers)
}

/// 加载内置模型预设（编译时嵌入二进制）
pub fn load_builtin_models() -> IcodeResult<Vec<BuiltinModel>> {
    const JSON: &str = include_str!("../../../data/builtin-models.json");
    let manifest: BuiltinModelsManifest = serde_json::from_str(JSON)?;
    Ok(manifest.models)
}

/// 按 ID 查找内置供应商
#[allow(dead_code)]
pub fn find_builtin_provider(id: &str) -> IcodeResult<Option<BuiltinProvider>> {
    let providers = load_builtin_providers()?;
    Ok(providers.into_iter().find(|p| p.id == id))
}

/// 按 ID 查找内置模型
#[allow(dead_code)]
pub fn find_builtin_model(id: &str) -> IcodeResult<Option<BuiltinModel>> {
    let models = load_builtin_models()?;
    Ok(models.into_iter().find(|m| m.id == id))
}

/// 按供应商类型筛选内置模型
pub fn filter_builtin_models_by_provider_type(provider_type: &str) -> IcodeResult<Vec<BuiltinModel>> {
    let models = load_builtin_models()?;
    Ok(models
        .into_iter()
        .filter(|m| m.provider_types.iter().any(|pt| pt == provider_type))
        .collect())
}

/// 在已加载的内置模型中按 id 匹配，返回首个匹配项
///
/// 匹配优先级（与前端 `findBuiltinByModelId` 保持一致）：
/// 1. 精确匹配
/// 2. 目标 id 以内置模型 id 开头（如 `gpt-4o-2024-01-01` 匹配 `gpt-4o`）
/// 3. 内置模型 id 以目标 id 开头（优先选最短，如 `mimo-v2.5` 匹配 `mimo-v2.5-pro`）
/// 4. 双向包含，优先选最短
pub fn find_builtin_model_in(models: &[BuiltinModel], id: &str) -> Option<BuiltinModel> {
    let lower = id.to_lowercase();
    if let Some(exact) = models.iter().find(|m| m.id.to_lowercase() == lower) {
        return Some(exact.clone());
    }
    if let Some(prefix) = models
        .iter()
        .find(|m| lower.starts_with(&m.id.to_lowercase()))
    {
        return Some(prefix.clone());
    }
    let mut suffix_matches: Vec<&BuiltinModel> = models
        .iter()
        .filter(|m| m.id.to_lowercase().starts_with(&lower))
        .collect();
    suffix_matches.sort_by_key(|m| m.id.len());
    if let Some(first) = suffix_matches.first() {
        return Some((*first).clone());
    }
    let mut contains_matches: Vec<&BuiltinModel> = models
        .iter()
        .filter(|m| {
            let bl = m.id.to_lowercase();
            lower.contains(&bl) || bl.contains(&lower)
        })
        .collect();
    contains_matches.sort_by_key(|m| m.id.len());
    contains_matches.first().map(|m| (*m).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_builtin_providers() {
        let providers = load_builtin_providers().unwrap();
        assert!(!providers.is_empty());
        assert!(providers.iter().any(|p| p.id == "openai"));
    }

    #[test]
    fn test_load_builtin_models() {
        let models = load_builtin_models().unwrap();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "gpt-4.1"));
    }

    #[test]
    fn test_find_builtin_provider() {
        let provider = find_builtin_provider("openai").unwrap();
        assert!(provider.is_some());
        // 参考项目将 OpenAI 主供应商类型定义为 openai-responses
        assert_eq!(provider.unwrap().provider_type, "openai-responses");
    }

    #[test]
    fn test_filter_builtin_models_by_provider_type() {
        let models = filter_builtin_models_by_provider_type("anthropic").unwrap();
        assert!(!models.is_empty());
        // 参考项目中 Anthropic 主供应商类型为 anthropic，包含 claude-sonnet-5 等
        assert!(models.iter().any(|m| m.id == "claude-sonnet-5"));
    }
}
