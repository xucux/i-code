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
}

/// 异步查询结果类型（boxed future，支持 dyn trait）
type BalanceRefreshFuture<'a> = Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>>;

/// 额度查询 Provider trait
///
/// 各供应商实现此 trait，在 `refresh()` 中调用对应的 HTTP API 并解析响应。
pub trait BalanceProvider: Send + Sync {
    /// 该 Provider 对应的 BalanceConfig method
    fn method(&self) -> &'static str;

    /// 查询额度快照
    fn refresh<'a>(&'a self, input: &'a BalanceRefreshInput) -> BalanceRefreshFuture<'a>;
}
