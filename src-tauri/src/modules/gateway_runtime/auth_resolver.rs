//! # 网关认证解析层
//!
//! 负责把 [`AuthConfig`] 转换为实际发送给上游供应商的 HTTP 头或查询参数。
//!
//! 核心抽象：
//! - `resolve_auth(auth)` → 返回结构化的认证凭证 `AuthResolution`
//! - 调用方（OpenAI/Anthropic Client）根据目标协议把凭证放到正确的 header
//!
//! 设计原则：
//! 1. 所有认证相关判断集中在此，client 不再直接 match `AuthConfig` 变体。
//! 2. OAuth token 统一按 JSON 解析，提取 `accessToken`，默认作为 `Bearer` 凭证。
//! 3. Vertex AI 等需要特殊 query 参数或附加头的场景也在此处理。
//! 4. 敏感字段已经由 `ai_gateway` Service 解密，本模块只负责格式转换。

use serde::Deserialize;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::ai_gateway::types::{AuthConfig, GoogleVertexAiAuthSubType};

/// OAuth token JSON 的最小结构
///
/// 参考项目把 OAuth token 序列化为 JSON 字符串存储，字段使用 camelCase。
/// 目前网关转发只关心 `accessToken`，未来可扩展 refresh、scope 等。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OAuthTokenData {
    access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[expect(dead_code)]
    token_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[expect(dead_code)]
    refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[expect(dead_code)]
    expires_at: Option<i64>,
}

/// 解析后的认证凭证
///
/// Client 根据目标协议决定 `Bearer` 放 `Authorization` 还是 `ApiKey` 放 `x-api-key`。
#[derive(Debug, Clone)]
pub enum AuthCredential {
    /// Bearer token，应写入 `Authorization: Bearer {token}`
    Bearer(String),
    /// 通用 API Key，由 Client 决定 header 名称
    ApiKey(String),
}

/// 认证解析结果
#[derive(Debug, Clone, Default)]
pub struct AuthResolution {
    /// 认证凭证（None 表示无认证）
    pub credential: Option<AuthCredential>,
    /// 需要追加到请求头的额外键值对
    pub extra_headers: Vec<(String, String)>,
    /// 需要追加到 URL query 的键值对
    pub query_params: Vec<(String, String)>,
}

impl AuthResolution {
    fn bearer(token: String) -> Self {
        Self {
            credential: Some(AuthCredential::Bearer(token)),
            extra_headers: vec![],
            query_params: vec![],
        }
    }

    fn api_key(key: String) -> Self {
        Self {
            credential: Some(AuthCredential::ApiKey(key)),
            extra_headers: vec![],
            query_params: vec![],
        }
    }

    fn none() -> Self {
        Self::default()
    }
}

/// 解析 `AuthConfig`，生成发送给上游供应商所需的结构化凭证
///
/// # 支持矩阵
/// - `None`：返回空
/// - `ApiKey`：`AuthCredential::ApiKey`
/// - `Oauth2 / AntigravityOauth / GoogleGeminiOauth / OpenaiCodexAuth / ClaudeCode / XaiGrokOauth / GithubCopilot`：
///   解析 token JSON → `AuthCredential::Bearer`
/// - `GoogleVertexAiAuth`：
///   - API Key 子类型：`AuthCredential::ApiKey` + query `?key={api_key}`
///   - ADC / Service Account：暂不支持，返回验证错误
///
/// # 错误
/// - token JSON 无法解析或缺少 `accessToken`
/// - 不支持的 Vertex AI 子类型
pub fn resolve_auth(auth: &AuthConfig) -> IcodeResult<AuthResolution> {
    match auth {
        AuthConfig::None => Ok(AuthResolution::none()),
        AuthConfig::ApiKey { api_key, .. } => {
            let key = api_key
                .as_ref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| IcodeError::validation("API Key 认证缺少 api_key"))?;
            Ok(AuthResolution::api_key(key.clone()))
        }
        AuthConfig::Oauth2 { token, .. }
        | AuthConfig::AntigravityOauth { token, .. }
        | AuthConfig::GoogleGeminiOauth { token, .. }
        | AuthConfig::OpenaiCodexAuth { token, .. }
        | AuthConfig::ClaudeCode { token, .. }
        | AuthConfig::GithubCopilot { token, .. } => {
            let token_str = token
                .as_ref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| IcodeError::validation("OAuth 认证缺少 token"))?;
            let access_token = extract_oauth_access_token(token_str)?;
            Ok(AuthResolution::bearer(access_token))
        }
        // xAI Grok Build OAuth：Bearer + CLI chat-proxy 身份标识头
        // 对齐 CLIProxyAPI applyXAIChatHeaders()（xai_executor.go）
        AuthConfig::XaiGrokOauth { token, .. } => {
            let token_str = token
                .as_ref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| IcodeError::validation("xAI Grok OAuth 认证缺少 token"))?;
            let access_token = extract_oauth_access_token(token_str)?;
            Ok(AuthResolution {
                credential: Some(AuthCredential::Bearer(access_token)),
                extra_headers: vec![
                    ("X-XAI-Token-Auth".to_string(), "xai-grok-cli".to_string()),
                    ("x-grok-client-version".to_string(), "0.2.93".to_string()),
                    ("User-Agent".to_string(), "xai-grok-workspace/0.2.93".to_string()),
                ],
                query_params: vec![],
            })
        }
        AuthConfig::GoogleVertexAiAuth {
            sub_type,
            api_key,
            ..
        } => match sub_type {
            GoogleVertexAiAuthSubType::ApiKey => {
                let key = api_key
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| IcodeError::validation("Vertex AI API Key 子类型缺少 api_key"))?;
                Ok(AuthResolution {
                    credential: Some(AuthCredential::ApiKey(key.clone())),
                    extra_headers: vec![],
                    query_params: vec![("key".to_string(), key.clone())],
                })
            }
            GoogleVertexAiAuthSubType::Adc => Err(IcodeError::validation(
                "Vertex AI ADC 认证尚未实现，请先使用 API Key 子类型",
            )),
            GoogleVertexAiAuthSubType::ServiceAccount => Err(IcodeError::validation(
                "Vertex AI Service Account 认证尚未实现，请先使用 API Key 子类型",
            )),
        },
    }
}

/// 从 OAuth token JSON 字符串中提取 access_token
///
/// 兼容两种字段风格：
/// - 参考项目风格：`{ accessToken, ... }`（camelCase）
/// - 标准 OAuth 风格：`{ access_token, ... }`（snake_case）
fn extract_oauth_access_token(token_str: &str) -> IcodeResult<String> {
    // 先尝试 camelCase（参考项目存储格式）
    if let Ok(data) = serde_json::from_str::<OAuthTokenData>(token_str) {
        if !data.access_token.is_empty() {
            return Ok(data.access_token);
        }
    }

    // 再尝试标准 snake_case OAuth 响应
    #[derive(Debug, Clone, Deserialize)]
    struct StandardOAuthToken {
        access_token: String,
    }
    if let Ok(data) = serde_json::from_str::<StandardOAuthToken>(token_str) {
        if !data.access_token.is_empty() {
            return Ok(data.access_token);
        }
    }

    // 兼容旧数据：直接存 access_token 字符串（长度校验防止误把 Secret 引用当 token）
    if token_str.starts_with('$') || token_str.len() < 10 {
        return Err(IcodeError::validation(
            "OAuth token 格式无效：缺少 accessToken 字段",
        ));
    }

    Ok(token_str.to_string())
}
