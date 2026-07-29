//! # 脚本系统变量注入
//!
//! 向 Rhai Scope 注入只读系统变量：api_key / provider / auth / now_ms / template / proxy。
//!
//! ## 代理变量
//!
//! 脚本中可通过以下变量获取代理配置：
//! - `proxy.provider_url`：供应商级代理 URL（若供应商配置了 socks/http 代理）
//! - `proxy.provider_type`：供应商代理策略（`"global"` / `"direct"` / `"socks"` / `"http"`）
//! - `proxy.global_url`：应用全局代理 URL（若全局代理已启用且配置了 socks/http 代理）
//! - `proxy.global_type`：全局代理策略（`"direct"` / `"system"` / `"http"` / `"socks"`）
//! - `proxy.global_enabled`：全局代理开关是否启用

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
    /// 已解密的模板变量列表
    pub script_variables: Vec<(String, String)>,
    /// 供应商代理配置（解析后的 ProviderProxyConfig）
    pub provider_proxy_type: Option<String>,
    pub provider_proxy_url: Option<String>,
    /// 全局代理配置
    pub global_proxy_enabled: bool,
    pub global_proxy_type: Option<String>,
    pub global_proxy_url: Option<String>,
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

        // 解析供应商代理配置
        let (provider_proxy_type, provider_proxy_url) = provider
            .proxy_json
            .as_deref()
            .and_then(|j| serde_json::from_str::<crate::modules::shared::ProviderProxyConfig>(j).ok())
            .map(|cfg| {
                let t = match cfg.proxy_type {
                    crate::modules::shared::ProviderProxyType::Global => "global",
                    crate::modules::shared::ProviderProxyType::Direct => "direct",
                    crate::modules::shared::ProviderProxyType::Socks => "socks",
                    crate::modules::shared::ProviderProxyType::Http => "http",
                };
                (Some(t.to_string()), cfg.url.filter(|s| !s.is_empty()))
            })
            .unwrap_or((None, None));

        // 解析全局代理配置
        let (global_proxy_enabled, global_proxy_type, global_proxy_url) = {
            let settings = crate::modules::settings::repository::find();
            match settings {
                Ok(s) => {
                    if s.global_proxy_enabled {
                        let (gt, gu) = s
                            .global_proxy_json
                            .as_deref()
                            .and_then(|j| serde_json::from_str::<crate::modules::shared::ProxyConfig>(j).ok())
                            .map(|cfg| {
                                let t = match cfg.proxy_type {
                                    crate::modules::shared::ProxyType::Direct => "direct",
                                    crate::modules::shared::ProxyType::System => "system",
                                    crate::modules::shared::ProxyType::Http => "http",
                                    crate::modules::shared::ProxyType::Socks => "socks",
                                };
                                (Some(t.to_string()), cfg.url.filter(|s| !s.is_empty()))
                            })
                            .unwrap_or((None, None));
                        (true, gt, gu)
                    } else {
                        (false, None, None)
                    }
                }
                Err(_) => (false, None, None),
            }
        };

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
            script_variables: input.script_variables.clone(),
            provider_proxy_type,
            provider_proxy_url,
            global_proxy_enabled,
            global_proxy_type,
            global_proxy_url,
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

        // 注入 proxy 变量：供应商代理与全局代理配置
        let mut proxy = Map::new();
        // 供应商代理
        if let Some(pt) = &self.provider_proxy_type {
            proxy.insert("provider_type".into(), Dynamic::from(pt.clone()));
        }
        if let Some(pu) = &self.provider_proxy_url {
            proxy.insert("provider_url".into(), Dynamic::from(pu.clone()));
        }
        // 全局代理
        proxy.insert("global_enabled".into(), Dynamic::from(self.global_proxy_enabled));
        if let Some(gt) = &self.global_proxy_type {
            proxy.insert("global_type".into(), Dynamic::from(gt.clone()));
        }
        if let Some(gu) = &self.global_proxy_url {
            proxy.insert("global_url".into(), Dynamic::from(gu.clone()));
        }
        scope.push_constant("proxy", Dynamic::from_map(proxy));

        // 注入 variables map（key → 明文 value）
        let mut vars = Map::new();
        for (k, v) in &self.script_variables {
            vars.insert(k.clone().into(), Dynamic::from(v.clone()));
        }
        scope.push_constant("variables", Dynamic::from_map(vars));

        // 注入扁平别名（每个 key 直接作为顶层常量，便于脚本直接写 cookie）
        // 保留名冲突时跳过扁平注入，仅 variables[...] 可用
        for (k, v) in &self.script_variables {
            if !is_reserved_name(k) {
                scope.push_constant(k.clone(), v.clone());
            }
        }
    }
}

/// 保留名列表（与 SCRIPT_VARIABLE_RESERVED_NAMES 对齐，运行时二次保险）
fn is_reserved_name(name: &str) -> bool {
    matches!(
        name,
        "api_key" | "now_ms" | "provider" | "auth" | "template" | "variables" | "proxy" | "pi" | "e"
    )
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
