//! # OpenAI Chat Completion 兼容协议客户端
//!
//! 处理所有使用 OpenAI `/v1/chat/completions` 端点的供应商，包括：
//! - OpenAI Chat Completion
//! - DeepSeek
//! - SiliconFlow
//! - OpenRouter
//! - xAI Grok Build（ Responses 端点未实现前兼容 fallback）
//! - Ollama
//! - 其他 OpenAI 兼容供应商
//!
//! ## 设计要点
//!
//! - 统一使用 `Authorization: Bearer {api_key}` 认证。
//! - 根据 `stream` 字段决定流式/非流式响应。
//! - 流式响应直接返回 `reqwest::Response`，由调用方透传 SSE。
//! - 非流式响应读取完整 body 后返回，便于日志记录。

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

use crate::modules::ai_gateway::types::{AuthConfig, Provider};
use crate::modules::gateway_runtime::auth_resolver::{resolve_auth, AuthCredential, AuthResolution};

use super::{
    build_upstream_url, format_client_error, http_client_for, read_response_body,
    ClientError, UpstreamClient, UpstreamContext, UpstreamProtocol, UpstreamRequest,
    UpstreamResponse, MAX_RESPONSE_BODY_BYTES,
};

/// OpenAI Chat Completion 客户端
///
/// 不再持有固定 `reqwest::Client`，改为每次 `execute` 时通过
/// [`http_client_for`] 按供应商配置选择客户端（默认复用进程级连接池，
/// 配置了 timeout/proxy 时构造独立实例）。
#[derive(Debug, Clone, Default)]
pub struct OpenAiChatClient;

impl OpenAiChatClient {
    /// 创建新的 OpenAI Chat Client
    pub fn new() -> Self {
        Self
    }

    /// 构造上游请求路径
    ///
    /// OpenAI 兼容供应商的 `base_url` 通常已包含 `/v1`（如 `https://api.openai.com/v1`），
    /// 因此此处路径不再重复版本前缀，避免生成 `/v1/v1/chat/completions`。
    fn build_path(protocol: UpstreamProtocol) -> &'static str {
        match protocol {
            UpstreamProtocol::ChatCompletions => "/chat/completions",
            UpstreamProtocol::AnthropicMessages => "/chat/completions",
        }
    }

    /// 解析认证配置
    ///
    /// 统一走 `auth_resolver`：
    /// - API Key / OAuth token 均映射为 `Authorization: Bearer {token}`
    /// - 无认证返回空
    fn resolve_auth(
        auth: Option<AuthConfig>,
    ) -> Result<AuthResolution, ClientError> {
        match auth {
            Some(config) => {
                resolve_auth(&config).map_err(|e| ClientError::AuthError(e.to_string()))
            }
            None => Ok(AuthResolution::default()),
        }
    }

    /// 构造请求头
    ///
    /// OpenAI 兼容协议统一使用 `Authorization: Bearer {token}`，
    /// 额外 headers 由 `auth_resolver` 提供（未来可扩展 `x-goog-user-project` 等）。
    fn build_headers(
        provider: &Provider,
        resolution: &AuthResolution,
    ) -> Result<reqwest::header::HeaderMap, ClientError> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/json".parse().map_err(|e| {
            ClientError::BuildRequestError(format!("构造 Content-Type 失败: {}", e))
        })?);

        if let Some(credential) = &resolution.credential {
            let token = match credential {
                AuthCredential::Bearer(token) => token.clone(),
                AuthCredential::ApiKey(key) => key.clone(),
            };
            headers.insert(
                AUTHORIZATION,
                format!("Bearer {}", token).parse().map_err(|e| {
                    ClientError::BuildRequestError(format!("构造 Authorization 失败: {}", e))
                })?,
            );
        }

        for (k, v) in &resolution.extra_headers {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(k.as_bytes())
                    .map_err(|e| ClientError::BuildRequestError(format!("非法 header 名 {}: {}", k, e)))?,
                v.parse().map_err(|e| {
                    ClientError::BuildRequestError(format!("构造 header {} 失败: {}", k, e))
                })?,
            );
        }

        // extra_headers 在最后注入，可覆盖默认头
        // TODO: provider_extra_headers / model_extra_headers 需在 UpstreamContext 中
        // 透传（当前 Provider 结构体不含该字段，仅在 ExportedProvider 中），client 层无法接入。
        let _ = provider;
        Ok(headers)
    }
}

#[async_trait::async_trait]
impl UpstreamClient for OpenAiChatClient {
    async fn execute(
        &self,
        ctx: &UpstreamContext,
        request: UpstreamRequest,
    ) -> Result<UpstreamResponse, ClientError> {
        if request.protocol != UpstreamProtocol::ChatCompletions {
            return Err(ClientError::UnsupportedProtocol {
                provider_type: ctx.provider.provider_type.clone(),
                protocol: request.protocol,
            });
        }

        let path = Self::build_path(request.protocol);
        let mut upstream_url = build_upstream_url(&ctx.provider, path)?;
        let auth_resolution = Self::resolve_auth(ctx.auth_config.clone())?;

        // 追加认证相关的 query 参数（如 Vertex AI API Key 的 ?key=...）
        if !auth_resolution.query_params.is_empty() {
            let mut url: reqwest::Url = upstream_url
                .parse()
                .map_err(|e| ClientError::BuildRequestError(format!("解析上游 URL 失败: {}", e)))?;
            for (k, v) in &auth_resolution.query_params {
                url.query_pairs_mut().append_pair(k, v);
            }
            upstream_url = url.to_string();
        }

        let headers = Self::build_headers(&ctx.provider, &auth_resolution)?;

        let client = http_client_for(&ctx.provider, request.is_stream)?;
        let upstream_resp = client
            .post(&upstream_url)
            .headers(headers)
            .json(&request.body)
            .send()
            .await
            .map_err(|e| ClientError::RequestFailed(format_client_error(&e)))?;

        if request.is_stream {
            // 流式响应：直接返回 reqwest Response，由上层透传 SSE
            Ok(UpstreamResponse::Streaming { response: upstream_resp, protocol: UpstreamProtocol::ChatCompletions })
        } else {
            // 非流式响应：读取完整 body，应用上限保护防止异常上游 OOM
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
        "openai-chat-completion"
    }
}

