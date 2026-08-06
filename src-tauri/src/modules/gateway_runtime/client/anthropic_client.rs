//! # Anthropic Messages API 客户端
//!
//! 处理 Anthropic 官方 Messages API 以及 Claude Code 等 Anthropic 兼容协议。
//!
//! ## 设计要点
//!
//! - 使用 `x-api-key` Header 认证，并固定 `anthropic-version: 2023-06-01`。
//! - 支持流式 SSE 与非流式 JSON 响应。
//! - 请求体按 Anthropic Messages 格式直接透传，v0.2 不做 OpenAI ↔ Anthropic 转换。
//! - 流式响应直接返回 `reqwest::Response`，由上层透传 SSE。

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

use crate::modules::ai_gateway::types::{AuthConfig, Provider};
use crate::modules::gateway_runtime::auth_resolver::{
    resolve_auth, AuthCredential, AuthResolution,
};

use super::{
    build_upstream_url, format_client_error, http_client_for, read_response_body, ClientError,
    UpstreamClient, UpstreamContext, UpstreamProtocol, UpstreamRequest, UpstreamResponse,
    MAX_RESPONSE_BODY_BYTES,
};

/// Anthropic Messages 客户端
///
/// 不再持有固定 `reqwest::Client`，改为每次 `execute` 时通过
/// [`http_client_for`] 按供应商配置选择客户端。
#[derive(Debug, Clone, Default)]
pub struct AnthropicClient;

/// 默认 `anthropic-version` 头
const DEFAULT_ANTHROPIC_VERSION: &str = "2023-06-01";

impl AnthropicClient {
    /// 创建新的 Anthropic Client
    pub fn new() -> Self {
        Self
    }

    /// 解析认证并提取 Anthropic 所需凭证
    ///
    /// 统一走 `auth_resolver`：
    /// - API Key 直接作为 `x-api-key`
    /// - Bearer token 不适用于官方 Anthropic 协议，返回错误
    ///   （若需走中转网关的 Bearer 认证，可通过 `extra_headers`
    ///   显式注入 `Authorization` 头覆盖默认行为）
    fn resolve_auth_credential(
        auth: Option<AuthConfig>,
    ) -> Result<AuthResolution, ClientError> {
        match auth {
            Some(config) => {
                let resolution = resolve_auth(&config)
                    .map_err(|e| ClientError::AuthError(e.to_string()))?;
                match &resolution.credential {
                    Some(AuthCredential::ApiKey(_)) => Ok(resolution),
                    Some(AuthCredential::Bearer(_)) => Err(ClientError::AuthError(format!(
                        "Anthropic 官方协议不支持 Bearer token，请使用 API Key 或 Claude Code 认证；\
                         若通过中转网关调用请用 extra_headers 显式注入 Authorization，实际方法: {:?}",
                        config.method()
                    ))),
                    None => Ok(resolution),
                }
            }
            None => Ok(AuthResolution::default()),
        }
    }

    /// 构造请求头
    ///
    /// 顺序：默认 Content-Type → 默认 `x-api-key` + `Authorization`（来自 credential）→
    /// 默认 `anthropic-version` → `resolution.extra_headers` 覆盖。
    /// 这样 `extra_headers` 可覆盖 version、注入 `anthropic-beta`、
    /// 或为代理网关提供自定义 header。
    ///
    /// **同一凭证双写**：配置 `ApiKey` 时，除写入 `x-api-key` 外，同步写入
    /// `Authorization: Bearer {key}`，兼容需要双重认证的中转网关（如小米 token-plan 等）。
    /// 官方 Anthropic API 只认 `x-api-key`，多出的 `Authorization` 头会被忽略，无副作用。
    fn build_headers(
        _provider: &Provider,
        resolution: &AuthResolution,
        extra_headers: &[(String, String)],
    ) -> Result<reqwest::header::HeaderMap, ClientError> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/json".parse().map_err(|e| {
            ClientError::BuildRequestError(format!("构造 Content-Type 失败: {}", e))
        })?);

        if let Some(AuthCredential::ApiKey(key)) = &resolution.credential {
            headers.insert(
                "x-api-key",
                key.parse().map_err(|e| {
                    ClientError::BuildRequestError(format!("构造 x-api-key 失败: {}", e))
                })?,
            );
            // 同一凭证双写 Authorization: Bearer {key}，兼容需要双重认证的中转网关
            headers.insert(
                AUTHORIZATION,
                format!("Bearer {}", key).parse().map_err(|e| {
                    ClientError::BuildRequestError(format!("构造 Authorization 失败: {}", e))
                })?,
            );
        }

        headers.insert(
            "anthropic-version",
            DEFAULT_ANTHROPIC_VERSION.parse().map_err(|e| {
                ClientError::BuildRequestError(format!("构造 anthropic-version 失败: {}", e))
            })?,
        );

        // extra_headers 在最后注入，可覆盖 x-api-key / Authorization / anthropic-version，
        // 也可注入 anthropic-beta（prompt caching、computer use 等）。
        for (k, v) in &resolution.extra_headers {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(k.as_bytes())
                    .map_err(|e| ClientError::BuildRequestError(format!("非法 header 名 {}: {}", k, e)))?,
                v.parse().map_err(|e| {
                    ClientError::BuildRequestError(format!("构造 header {} 失败: {}", k, e))
                })?,
            );
        }

        // 供应商级 extra_headers 在最后注入，可覆盖默认头
        for (k, v) in extra_headers {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(k.as_bytes())
                    .map_err(|e| ClientError::BuildRequestError(format!("非法 extra header 名 {}: {}", k, e)))?,
                v.parse().map_err(|e| {
                    ClientError::BuildRequestError(format!("构造 extra header {} 失败: {}", k, e))
                })?,
            );
        }

        Ok(headers)
    }
}

#[async_trait::async_trait]
impl UpstreamClient for AnthropicClient {
    async fn execute(
        &self,
        ctx: &mut UpstreamContext,
        request: UpstreamRequest,
    ) -> Result<UpstreamResponse, ClientError> {
        if request.protocol != UpstreamProtocol::AnthropicMessages {
            return Err(ClientError::UnsupportedProtocol {
                provider_type: ctx.provider.provider_type.clone(),
                protocol: request.protocol,
            });
        }

        let upstream_url = build_upstream_url(&ctx.provider, "/v1/messages")?;
        let auth_resolution = Self::resolve_auth_credential(ctx.auth_config.clone())?;
        let headers = Self::build_headers(&ctx.provider, &auth_resolution, &ctx.extra_headers)?;
        // 捕获上游请求头去敏快照，供 provider-api 日志展示真实发出的请求头
        ctx.request_headers_json =
            crate::modules::gateway_runtime::logging::headers::request_headers_to_json(&headers);

        let client = http_client_for(&ctx.provider, request.is_stream)?;
        let upstream_resp = client
            .post(&upstream_url)
            .headers(headers)
            .json(&request.body)
            .send()
            .await
            .map_err(|e| ClientError::RequestFailed(format_client_error(&e)))?;

        if request.is_stream {
            Ok(UpstreamResponse::Streaming { response: upstream_resp, protocol: UpstreamProtocol::AnthropicMessages })
        } else {
            let (status, headers, body, _) =
                read_response_body(upstream_resp, MAX_RESPONSE_BODY_BYTES).await?;
            Ok(UpstreamResponse::Complete {
                status,
                headers,
                body,
            })
        }
    }

    fn provider_type(&self) -> &'static str {
        "anthropic"
    }
}
