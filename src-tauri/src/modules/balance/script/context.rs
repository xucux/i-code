//! # 脚本系统变量注入
//!
//! 向 Rhai Scope 注入只读系统变量：api_key / provider / auth / now_ms / template。

use rhai::{Dynamic, Map, Scope};

use crate::modules::ai_gateway::types::Provider;
use crate::modules::script_template::types::ScriptTemplate;

use super::super::provider::BalanceRefreshInput;

/// 脚本执行上下文
#[derive(Debug, Clone)]
pub struct ScriptContext {
    pub api_key: String,
    pub provider_id: String,
    pub provider_slug: String,
    pub provider_name: String,
    pub provider_base_url: String,
    pub provider_type: String,
    pub provider_is_enabled: bool,
    pub auth_method: String,
    pub project_id: Option<String>,
    pub managed_project_id: Option<String>,
    pub account_id: Option<String>,
    pub now_ms: i64,
    pub template_id: String,
    pub template_name: String,
    pub template_kind: String,
}

impl ScriptContext {
    pub fn from_parts(
        template: &ScriptTemplate,
        provider: &Provider,
        input: &BalanceRefreshInput,
    ) -> Self {
        // 从 auth_json 粗略提取 method（不解密完整 token）
        let auth_method = provider
            .auth_json
            .as_deref()
            .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
            .and_then(|v| v.get("method").and_then(|m| m.as_str().map(|s| s.to_string())))
            .unwrap_or_else(|| "none".to_string());

        Self {
            api_key: input.api_key.clone().unwrap_or_default(),
            provider_id: provider.id.clone(),
            provider_slug: provider.slug.clone(),
            provider_name: provider.display_name.clone(),
            provider_base_url: provider.base_url.clone(),
            provider_type: provider.provider_type.clone(),
            provider_is_enabled: provider.is_enabled,
            auth_method,
            project_id: input.project_id.clone(),
            managed_project_id: input.managed_project_id.clone(),
            account_id: input.account_id.clone(),
            now_ms: now_ms(),
            template_id: template.id.clone(),
            template_name: template.name.clone(),
            template_kind: template.kind.clone(),
        }
    }

    /// 注入 Scope（常量，脚本不可改）
    pub fn inject_into_scope(&self, scope: &mut Scope<'_>) {
        scope.push_constant("api_key", self.api_key.clone());
        scope.push_constant("now_ms", self.now_ms);

        let mut provider = Map::new();
        provider.insert("id".into(), Dynamic::from(self.provider_id.clone()));
        provider.insert("slug".into(), Dynamic::from(self.provider_slug.clone()));
        provider.insert("name".into(), Dynamic::from(self.provider_name.clone()));
        provider.insert("base_url".into(), Dynamic::from(self.provider_base_url.clone()));
        provider.insert(
            "provider_type".into(),
            Dynamic::from(self.provider_type.clone()),
        );
        provider.insert("is_enabled".into(), Dynamic::from(self.provider_is_enabled));
        scope.push_constant("provider", Dynamic::from_map(provider));

        let mut auth = Map::new();
        auth.insert("method".into(), Dynamic::from(self.auth_method.clone()));
        if let Some(v) = &self.project_id {
            auth.insert("project_id".into(), Dynamic::from(v.clone()));
        }
        if let Some(v) = &self.managed_project_id {
            auth.insert("managed_project_id".into(), Dynamic::from(v.clone()));
        }
        if let Some(v) = &self.account_id {
            auth.insert("account_id".into(), Dynamic::from(v.clone()));
        }
        scope.push_constant("auth", Dynamic::from_map(auth));

        let mut template = Map::new();
        template.insert("id".into(), Dynamic::from(self.template_id.clone()));
        template.insert("name".into(), Dynamic::from(self.template_name.clone()));
        template.insert("kind".into(), Dynamic::from(self.template_kind.clone()));
        scope.push_constant("template", Dynamic::from_map(template));
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
