//! # 额度查询 Provider trait
//!
//! 每个供应商实现 `BalanceProvider` trait，提供 `refresh()` 方法。
//! service.rs 通过注册表分发到对应实现，替代原先的 match 14 臂。

use std::future::Future;
use std::pin::Pin;

use crate::error::IcodeResult;

use super::types::BalanceSnapshot;

/// 额度查询 Provider 输入参数
///
/// 由 service 层构造，从 provider 表 + secret 解析后填入
#[derive(Debug, Clone, Default)]
pub struct BalanceRefreshInput {
    /// 已解析的 API Key / Access Token 明文（从 `$SECRET` 引用解密后获得）
    pub api_key: Option<String>,
    /// 供应商 API 基础 URL
    pub base_url: Option<String>,
    /// New API 方法特有参数
    pub newapi_config: Option<super::types::NewApiConfig>,
    /// Claude Relay Service 方法特有参数
    pub claude_relay_config: Option<super::types::ClaudeRelayServiceConfig>,
    /// AIHubMix 可选 APP-Code 请求头
    pub app_code: Option<String>,
    /// Code Assist project id（gemini-cli / antigravity）
    pub project_id: Option<String>,
    /// Managed project id（优先于 project_id）
    pub managed_project_id: Option<String>,
    /// Codex ChatGPT Account Id
    pub account_id: Option<String>,
    /// 供应商元信息（脚本模板注入用）
    pub provider_id: Option<String>,
    pub provider_slug: Option<String>,
    pub provider_name: Option<String>,
    pub provider_type: Option<String>,
    pub provider_is_enabled: Option<bool>,
    /// 认证方法摘要（脚本 auth.method）
    pub auth_method: Option<String>,
    /// 已解密的模板变量（key → 明文 value）
    pub script_variables: Vec<(String, String)>,
}

/// 异步查询结果类型（boxed future，支持 dyn trait）
type BalanceRefreshFuture<'a> = Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>>;

/// 额度查询 Provider trait
///
/// 各供应商实现此 trait，在 `refresh()` 中调用对应的 HTTP API 并解析响应。
pub trait BalanceProvider: Send + Sync {
    /// 该 Provider 对应的 BalanceConfig method
    #[expect(dead_code)]
    fn method(&self) -> &'static str;

    /// 查询额度快照
    fn refresh<'a>(&'a self, input: &'a BalanceRefreshInput) -> BalanceRefreshFuture<'a>;
}

/// 构造额度查询 HTTP 客户端（复用应用全局代理配置）
///
/// balance provider 属于面向供应商的网络路径，须走 `shared::apply_global_proxy`
/// （对齐 `docs/proxy.md`：禁止直接 `reqwest::Client::new()`）。否则在需要代理
/// 才能访问供应商接口的环境（如某些海外额度端点）会连接失败。
/// 全局代理未启用时按规范**强制直连**（不读取系统环境变量代理）。
pub fn build_balance_http_client() -> IcodeResult<reqwest::Client> {
    use crate::modules::shared::apply_global_proxy;
    let builder = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30));
    apply_global_proxy(builder)
        .build()
        .map_err(|e| {
            crate::error::IcodeError::internal(format!(
                "构造额度查询 HTTP 客户端失败: {}",
                e
            ))
        })
}
