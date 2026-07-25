//! # Gemini CLI 额度查询实现
//!
//! 基于 `code_assist_quota` 共享模块，调用 Code Assist `retrieveUserQuota`。
//!
//! Project ID 解析顺序：managed_project_id → project_id（无默认值，缺失则报错）。

use std::future::Future;
use std::pin::Pin;

use crate::error::IcodeResult;

use super::super::provider::{BalanceProvider, BalanceRefreshInput};
use super::super::types::BalanceSnapshot;
use super::code_assist_quota::{refresh_code_assist_quota, CodeAssistQuotaOptions};

/// Gemini CLI Code Assist 端点 fallback 列表
const GEMINI_CLI_ENDPOINT_FALLBACKS: &[&str] = &[
    "https://cloudcode-pa.googleapis.com",
    "https://daily-cloudcode-pa.sandbox.googleapis.com",
    "https://autopush-cloudcode-pa.sandbox.googleapis.com",
    "https://daily-cloudcode-pa.googleapis.com",
];

/// Gemini CLI 固定请求头
const GEMINI_CLI_REQUEST_HEADERS: &[(&str, &str)] = &[
    ("User-Agent", "GeminiCLI/0.1.5 (Windows; AMD64)"),
    ("X-Goog-Api-Client", "gl-node/22.18.0"),
    (
        "Client-Metadata",
        "ideType=IDE_UNSPECIFIED,platform=PLATFORM_UNSPECIFIED,pluginType=GEMINI",
    ),
];

/// 解析 Gemini CLI project id（无默认值）
fn resolve_gemini_cli_project_id(input: &BalanceRefreshInput) -> Option<String> {
    if let Some(managed) = input.managed_project_id.as_deref() {
        let trimmed = managed.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(pid) = input.project_id.as_deref() {
        let trimmed = pid.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

pub struct GeminiCliBalanceProvider;

impl BalanceProvider for GeminiCliBalanceProvider {
    fn method(&self) -> &'static str {
        "gemini-cli"
    }

    fn refresh<'a>(
        &'a self,
        input: &'a BalanceRefreshInput,
    ) -> Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>> {
        Box::pin(async move {
            let options = CodeAssistQuotaOptions {
                provider_name: "Gemini CLI",
                endpoint_fallbacks: GEMINI_CLI_ENDPOINT_FALLBACKS,
                request_headers: GEMINI_CLI_REQUEST_HEADERS,
                resolve_project_id: resolve_gemini_cli_project_id,
            };
            refresh_code_assist_quota(input, &options).await
        })
    }
}
