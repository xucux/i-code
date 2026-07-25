//! # AI Gateway 模块类型定义
//!
//! 与前端 `src/modules/ai-gateway/types.ts` 对齐。
//! 序列化字段使用 camelCase，确保前后端 JSON 字段名一致。

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// 供应商协议类型
///
/// 对应 database.md §5.1，与参考项目 ProviderType 对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderType {
    Anthropic,
    #[serde(rename = "claude-code")]
    ClaudeCode,
    #[serde(rename = "google-ai-studio")]
    GoogleAiStudio,
    #[serde(rename = "google-vertex-ai")]
    GoogleVertexAi,
    #[serde(rename = "google-antigravity")]
    GoogleAntigravity,
    #[serde(rename = "google-gemini-cli")]
    GoogleGeminiCli,
    #[serde(rename = "github-copilot")]
    GithubCopilot,
    #[serde(rename = "openai-chat-completion")]
    OpenaiChatCompletion,
    #[serde(rename = "openai-codex")]
    OpenaiCodex,
    #[serde(rename = "openai-responses")]
    OpenaiResponses,
    #[serde(rename = "xai-grok-build")]
    XaiGrokBuild,
    Ollama,
    Custom,
}

impl ProviderType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "anthropic" => Some(Self::Anthropic),
            "claude-code" => Some(Self::ClaudeCode),
            "google-ai-studio" => Some(Self::GoogleAiStudio),
            "google-vertex-ai" => Some(Self::GoogleVertexAi),
            "google-antigravity" => Some(Self::GoogleAntigravity),
            "google-gemini-cli" => Some(Self::GoogleGeminiCli),
            "github-copilot" => Some(Self::GithubCopilot),
            "openai-chat-completion" => Some(Self::OpenaiChatCompletion),
            "openai-codex" => Some(Self::OpenaiCodex),
            "openai-responses" => Some(Self::OpenaiResponses),
            "xai-grok-build" => Some(Self::XaiGrokBuild),
            "ollama" => Some(Self::Ollama),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::ClaudeCode => "claude-code",
            Self::GoogleAiStudio => "google-ai-studio",
            Self::GoogleVertexAi => "google-vertex-ai",
            Self::GoogleAntigravity => "google-antigravity",
            Self::GoogleGeminiCli => "google-gemini-cli",
            Self::GithubCopilot => "github-copilot",
            Self::OpenaiChatCompletion => "openai-chat-completion",
            Self::OpenaiCodex => "openai-codex",
            Self::OpenaiResponses => "openai-responses",
            Self::XaiGrokBuild => "xai-grok-build",
            Self::Ollama => "ollama",
            Self::Custom => "custom",
        }
    }
}

/// 传输方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportType {
    Auto,
    Sse,
    Websocket,
}

/// 模型来源标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelSource {
    /// 用户手动添加
    Manual,
    /// 从 builtin_models 选择
    Builtin,
    /// 从供应商 API 拉取
    Official,
}

/// 认证方法枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthMethod {
    None,
    ApiKey,
    Oauth2,
    #[serde(rename = "google-vertex-ai-auth")]
    GoogleVertexAiAuth,
    #[serde(rename = "antigravity-oauth")]
    AntigravityOauth,
    #[serde(rename = "google-gemini-oauth")]
    GoogleGeminiOauth,
    #[serde(rename = "openai-codex")]
    OpenaiCodexAuth,
    #[serde(rename = "claude-code")]
    ClaudeCodeAuth,
    #[serde(rename = "xai-grok-oauth")]
    XaiGrokOauth,
    #[serde(rename = "github-copilot")]
    GithubCopilotAuth,
}

/// OAuth 2.0 授权类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuth2GrantType {
    /// 授权码模式（需浏览器授权）
    AuthorizationCode,
    /// 客户端凭证模式
    ClientCredentials,
    /// 设备码模式
    DeviceCode,
}

/// OAuth 2.0 通用配置
///
/// 字段按 grantType 动态使用：
/// - authorization_code：authorization_url / token_url / client_id / client_secret / scopes / pkce / redirect_uri
/// - client_credentials：token_url / client_id / client_secret / scopes
/// - device_code：device_authorization_url / token_url / client_id / scopes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuth2Config {
    /// 授权类型
    pub grant_type: OAuth2GrantType,
    /// Token 端点 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_url: Option<String>,
    /// Token 吊销端点 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revocation_url: Option<String>,
    /// OAuth scopes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    /// 授权端点 URL（仅 authorization_code）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_url: Option<String>,
    /// 客户端 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// 客户端密钥（敏感字段，存储为 Secret 引用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    /// 是否使用 PKCE（authorization_code，默认 true）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pkce: Option<bool>,
    /// 回调 URI（authorization_code，临时 localhost 服务器自动生成）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,
    /// 设备授权端点 URL（仅 device_code）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_authorization_url: Option<String>,
}

/// Google Vertex AI 认证子类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GoogleVertexAiAuthSubType {
    /// Application Default Credentials
    Adc,
    /// Service Account JSON key file
    ServiceAccount,
    /// API Key（Express Mode）
    ApiKey,
}

/// 认证配置（多态联合类型）
///
/// 使用 `#[serde(tag = "method")]` 内部标签，与前端 AuthConfig 对齐。
/// 枚举级 `rename_all = "kebab-case"` 仅作用于 tag 值（变体名），
/// 各变体单独标注 `rename_all = "camelCase"` 以使字段名与前端对齐。
/// 敏感字段（apiKey、token、clientSecret 等）存储为 `$SECRET:{snowflake_id}$` 引用或明文。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "kebab-case")]
pub enum AuthConfig {
    /// 无认证
    None,
    /// API Key 认证
    #[serde(rename_all = "camelCase")]
    ApiKey {
        /// UI 标签
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// UI 描述
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// API Key 原文或 `$SECRET:{snowflake_id}$` 引用
        #[serde(skip_serializing_if = "Option::is_none")]
        api_key: Option<String>,
    },
    /// OAuth 2.0 通用认证
    #[serde(rename_all = "camelCase")]
    Oauth2 {
        /// UI 标签
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// UI 描述
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// 身份标识（UUID）
        #[serde(skip_serializing_if = "Option::is_none")]
        identity_id: Option<String>,
        /// OAuth token 数据（JSON 字符串或 `$SECRET:{snowflake_id}$` 引用）
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        /// OAuth token 过期时间（Unix 秒）
        #[serde(skip_serializing_if = "Option::is_none")]
        expires_at: Option<i64>,
        /// OAuth 2.0 端点与客户端配置
        #[serde(skip_serializing_if = "Option::is_none")]
        oauth: Option<OAuth2Config>,
    },
    /// Google Vertex AI 认证
    #[serde(rename = "google-vertex-ai-auth", rename_all = "camelCase")]
    GoogleVertexAiAuth {
        /// UI 标签
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// UI 描述
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// 认证子类型
        #[serde(rename = "subType")]
        sub_type: GoogleVertexAiAuthSubType,
        /// Google Cloud Project ID
        #[serde(skip_serializing_if = "Option::is_none")]
        project_id: Option<String>,
        /// Google Cloud Location/Region
        #[serde(skip_serializing_if = "Option::is_none")]
        location: Option<String>,
        /// Service Account key 文件路径
        #[serde(skip_serializing_if = "Option::is_none")]
        key_file_path: Option<String>,
        /// API Key 原文或 `$SECRET:{snowflake_id}$` 引用
        #[serde(skip_serializing_if = "Option::is_none")]
        api_key: Option<String>,
    },
    /// Google Antigravity 认证
    #[serde(rename = "antigravity-oauth", rename_all = "camelCase")]
    AntigravityOauth {
        /// UI 标签
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// UI 描述
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// 身份标识（UUID）
        #[serde(skip_serializing_if = "Option::is_none")]
        identity_id: Option<String>,
        /// OAuth token 数据（JSON 字符串或 `$SECRET:{snowflake_id}$` 引用）
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        /// OAuth token 过期时间（Unix 秒）
        #[serde(skip_serializing_if = "Option::is_none")]
        expires_at: Option<i64>,
        /// 用户提供的 project id
        #[serde(skip_serializing_if = "Option::is_none")]
        project_id: Option<String>,
        /// Cloud Code Assist managed project id
        #[serde(skip_serializing_if = "Option::is_none")]
        managed_project_id: Option<String>,
        /// 套餐类型
        #[serde(skip_serializing_if = "Option::is_none")]
        tier: Option<String>,
        /// 精确套餐标识
        #[serde(skip_serializing_if = "Option::is_none")]
        tier_id: Option<String>,
        /// 邮箱
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
    },
    /// Google Gemini CLI 认证
    #[serde(rename = "google-gemini-oauth", rename_all = "camelCase")]
    GoogleGeminiOauth {
        /// UI 标签
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// UI 描述
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// 身份标识（UUID）
        #[serde(skip_serializing_if = "Option::is_none")]
        identity_id: Option<String>,
        /// OAuth token 数据（JSON 字符串或 `$SECRET:{snowflake_id}$` 引用）
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        /// OAuth token 过期时间（Unix 秒）
        #[serde(skip_serializing_if = "Option::is_none")]
        expires_at: Option<i64>,
        /// 用户提供的 project id
        #[serde(skip_serializing_if = "Option::is_none")]
        project_id: Option<String>,
        /// OAuth 账户类型
        #[serde(skip_serializing_if = "Option::is_none")]
        oauth_type: Option<String>,
        /// Cloud Code Assist managed project id
        #[serde(skip_serializing_if = "Option::is_none")]
        managed_project_id: Option<String>,
        /// 套餐类型
        #[serde(skip_serializing_if = "Option::is_none")]
        tier: Option<String>,
        /// 精确套餐标识
        #[serde(skip_serializing_if = "Option::is_none")]
        tier_id: Option<String>,
        /// 邮箱
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
    },
    /// OpenAI Codex 认证
    #[serde(rename = "openai-codex", rename_all = "camelCase")]
    OpenaiCodexAuth {
        /// UI 标签
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// UI 描述
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// 身份标识（UUID）
        #[serde(skip_serializing_if = "Option::is_none")]
        identity_id: Option<String>,
        /// OAuth token 数据（JSON 字符串或 `$SECRET:{snowflake_id}$` 引用）
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        /// OAuth token 过期时间（Unix 秒）
        #[serde(skip_serializing_if = "Option::is_none")]
        expires_at: Option<i64>,
        /// ChatGPT organization/subscription account ID
        #[serde(skip_serializing_if = "Option::is_none")]
        account_id: Option<String>,
        /// 邮箱
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
    },
    /// Claude Code 认证
    #[serde(rename = "claude-code", rename_all = "camelCase")]
    ClaudeCode {
        /// UI 标签
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// UI 描述
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// 身份标识（UUID）
        #[serde(skip_serializing_if = "Option::is_none")]
        identity_id: Option<String>,
        /// OAuth token 数据（JSON 字符串或 `$SECRET:{snowflake_id}$` 引用）
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        /// OAuth token 过期时间（Unix 秒）
        #[serde(skip_serializing_if = "Option::is_none")]
        expires_at: Option<i64>,
        /// 邮箱
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
    },
    /// xAI Grok 认证
    #[serde(rename = "xai-grok-oauth", rename_all = "camelCase")]
    XaiGrokOauth {
        /// UI 标签
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// UI 描述
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// 身份标识（UUID）
        #[serde(skip_serializing_if = "Option::is_none")]
        identity_id: Option<String>,
        /// OAuth token 数据（JSON 字符串或 `$SECRET:{snowflake_id}$` 引用）
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        /// OAuth token 过期时间（Unix 秒）
        #[serde(skip_serializing_if = "Option::is_none")]
        expires_at: Option<i64>,
        /// 邮箱
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
    },
    /// GitHub Copilot 认证
    #[serde(rename = "github-copilot", rename_all = "camelCase")]
    GithubCopilot {
        /// UI 标签
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// UI 描述
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// 身份标识（UUID）
        #[serde(skip_serializing_if = "Option::is_none")]
        identity_id: Option<String>,
        /// OAuth token 数据（JSON 字符串或 `$SECRET:{snowflake_id}$` 引用）
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        /// OAuth token 过期时间（Unix 秒）
        #[serde(skip_serializing_if = "Option::is_none")]
        expires_at: Option<i64>,
        /// Enterprise domain（hostname，可选端口）
        #[serde(skip_serializing_if = "Option::is_none")]
        enterprise_url: Option<String>,
        /// 邮箱
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
    },
}

impl AuthConfig {
    /// 获取认证方法
    pub fn method(&self) -> AuthMethod {
        match self {
            Self::None => AuthMethod::None,
            Self::ApiKey { .. } => AuthMethod::ApiKey,
            Self::Oauth2 { .. } => AuthMethod::Oauth2,
            Self::GoogleVertexAiAuth { .. } => AuthMethod::GoogleVertexAiAuth,
            Self::AntigravityOauth { .. } => AuthMethod::AntigravityOauth,
            Self::GoogleGeminiOauth { .. } => AuthMethod::GoogleGeminiOauth,
            Self::OpenaiCodexAuth { .. } => AuthMethod::OpenaiCodexAuth,
            Self::ClaudeCode { .. } => AuthMethod::ClaudeCodeAuth,
            Self::XaiGrokOauth { .. } => AuthMethod::XaiGrokOauth,
            Self::GithubCopilot { .. } => AuthMethod::GithubCopilotAuth,
        }
    }

    /// 获取 OAuth token 字符串（若该认证方法支持）
    ///
    /// 用于 token 刷新等场景，从任意 OAuth 变体中提取 `token` 字段。
    pub fn token(&self) -> Option<String> {
        match self {
            Self::Oauth2 { token, .. }
            | Self::AntigravityOauth { token, .. }
            | Self::GoogleGeminiOauth { token, .. }
            | Self::OpenaiCodexAuth { token, .. }
            | Self::ClaudeCode { token, .. }
            | Self::XaiGrokOauth { token, .. }
            | Self::GithubCopilot { token, .. } => token.clone(),
            _ => None,
        }
    }

    /// 创建无认证配置
    pub fn none() -> Self {
        Self::None
    }

    /// UI 标签
    pub fn label(&self) -> Option<&String> {
        match self {
            Self::ApiKey { label, .. }
            | Self::Oauth2 { label, .. }
            | Self::GoogleVertexAiAuth { label, .. }
            | Self::AntigravityOauth { label, .. }
            | Self::GoogleGeminiOauth { label, .. }
            | Self::OpenaiCodexAuth { label, .. }
            | Self::ClaudeCode { label, .. }
            | Self::XaiGrokOauth { label, .. }
            | Self::GithubCopilot { label, .. } => label.as_ref(),
            Self::None => None,
        }
    }

    /// UI 描述
    pub fn description(&self) -> Option<&String> {
        match self {
            Self::ApiKey { description, .. }
            | Self::Oauth2 { description, .. }
            | Self::GoogleVertexAiAuth { description, .. }
            | Self::AntigravityOauth { description, .. }
            | Self::GoogleGeminiOauth { description, .. }
            | Self::OpenaiCodexAuth { description, .. }
            | Self::ClaudeCode { description, .. }
            | Self::XaiGrokOauth { description, .. }
            | Self::GithubCopilot { description, .. } => description.as_ref(),
            Self::None => None,
        }
    }

    /// 身份标识 UUID
    pub fn identity_id(&self) -> Option<&String> {
        match self {
            Self::Oauth2 { identity_id, .. }
            | Self::AntigravityOauth { identity_id, .. }
            | Self::GoogleGeminiOauth { identity_id, .. }
            | Self::OpenaiCodexAuth { identity_id, .. }
            | Self::ClaudeCode { identity_id, .. }
            | Self::XaiGrokOauth { identity_id, .. }
            | Self::GithubCopilot { identity_id, .. } => identity_id.as_ref(),
            _ => None,
        }
    }

    /// OAuth 2.0 端点配置（仅 `Oauth2` 变体）
    pub fn oauth_config(&self) -> Option<&OAuth2Config> {
        match self {
            Self::Oauth2 { oauth, .. } => oauth.as_ref(),
            _ => None,
        }
    }

    /// Google Cloud Project ID
    pub fn project_id(&self) -> Option<&String> {
        match self {
            Self::GoogleVertexAiAuth { project_id, .. }
            | Self::AntigravityOauth { project_id, .. }
            | Self::GoogleGeminiOauth { project_id, .. } => project_id.as_ref(),
            _ => None,
        }
    }

    /// Google Gemini OAuth 账户类型
    pub fn oauth_type(&self) -> Option<&String> {
        match self {
            Self::GoogleGeminiOauth { oauth_type, .. } => oauth_type.as_ref(),
            _ => None,
        }
    }

    /// Cloud Code Assist managed project id
    pub fn managed_project_id(&self) -> Option<&String> {
        match self {
            Self::AntigravityOauth { managed_project_id, .. }
            | Self::GoogleGeminiOauth { managed_project_id, .. } => managed_project_id.as_ref(),
            _ => None,
        }
    }

    /// 套餐类型
    pub fn tier(&self) -> Option<&String> {
        match self {
            Self::AntigravityOauth { tier, .. }
            | Self::GoogleGeminiOauth { tier, .. } => tier.as_ref(),
            _ => None,
        }
    }

    /// 精确套餐标识
    pub fn tier_id(&self) -> Option<&String> {
        match self {
            Self::AntigravityOauth { tier_id, .. }
            | Self::GoogleGeminiOauth { tier_id, .. } => tier_id.as_ref(),
            _ => None,
        }
    }

    /// ChatGPT organization/subscription account ID
    pub fn account_id(&self) -> Option<&String> {
        match self {
            Self::OpenaiCodexAuth { account_id, .. } => account_id.as_ref(),
            _ => None,
        }
    }

    /// GitHub Enterprise domain
    pub fn enterprise_url(&self) -> Option<&String> {
        match self {
            Self::GithubCopilot { enterprise_url, .. } => enterprise_url.as_ref(),
            _ => None,
        }
    }

    /// 邮箱
    pub fn email(&self) -> Option<&String> {
        match self {
            Self::AntigravityOauth { email, .. }
            | Self::GoogleGeminiOauth { email, .. }
            | Self::OpenaiCodexAuth { email, .. }
            | Self::ClaudeCode { email, .. }
            | Self::XaiGrokOauth { email, .. }
            | Self::GithubCopilot { email, .. } => email.as_ref(),
            _ => None,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self::None
    }
}

/// AI Gateway 供应商 DTO
///
/// 与前端 `Provider` 对齐。
/// JSON 字段在 DTO 中保持为字符串，由 Service 层负责序列化/反序列化。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub provider_type: String,
    pub base_url: String,
    pub use_raw_base_url: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    /// 认证配置 JSON（AuthConfig 序列化），密钥以 `$SECRET:{snowflake_id}$` 引用
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_json: Option<String>,
    /// 额度监控配置 JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance_provider_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_json: Option<String>,
    pub auto_fetch_official_models: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_cache_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub well_known_template_id: Option<String>,
    pub is_enabled: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 网关暴露的模型 DTO
///
/// 对外路由 ID：`{provider.slug}/{gateway_model.model_id}`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayModel {
    pub id: String,
    pub provider_id: String,
    pub model_config_id: String,
    /// 真实模型 ID，如 `gpt-4.1`
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    pub source: String,
    pub is_exposed: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 模型完整配置 DTO
///
/// 标量字段拆为列，复杂嵌套对象以 JSON 列存储
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_input_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer: Option<String>,
    pub token_count_multiplier: f64,
    /// 每百万 token 单价（元 / 1M tokens）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_per_1m_tokens: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calling: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi_agent_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_tool: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset_templates_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 供应商附加请求头
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderExtraHeader {
    pub provider_id: String,
    pub key: String,
    /// 值支持 `$SECRET:{snowflake_id}$` 引用
    pub value: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 供应商附加请求体参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderExtraBody {
    pub provider_id: String,
    pub key: String,
    pub value_json: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建供应商的输入参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProviderInput {
    pub slug: String,
    pub display_name: String,
    pub provider_type: String,
    pub base_url: String,
    #[serde(default)]
    pub use_raw_base_url: bool,
    /// 认证配置（强类型），Service 层序列化为 JSON 存储
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
    #[serde(default)]
    pub auto_fetch_official_models: bool,
    #[serde(default)]
    pub is_enabled: bool,
    #[serde(default)]
    pub sort_order: Option<i64>,
    /// 额度监控配置 JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance_provider_json: Option<String>,
    /// 供应商级超时配置 JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_json: Option<String>,
    /// 供应商级重试配置 JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_json: Option<String>,
    /// 供应商级代理配置 JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_json: Option<String>,
}

/// 更新供应商的输入参数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProviderInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_raw_base_url: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_fetch_official_models: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<i64>,
    /// 额度监控配置 JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance_provider_json: Option<String>,
    /// 供应商级超时配置 JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_json: Option<String>,
    /// 供应商级重试配置 JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_json: Option<String>,
    /// 供应商级代理配置 JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_json: Option<String>,
}

/// 创建模型配置的输入参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateModelConfigInput {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_input_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer: Option<String>,
    #[serde(default = "default_token_multiplier")]
    pub token_count_multiplier: f64,
    /// 每百万 token 单价（元 / 1M tokens）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_per_1m_tokens: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calling: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_json: Option<String>,
}

/// 更新模型配置的输入参数
///
/// 仅更新传入的非 None 字段。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateModelConfigInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_input_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count_multiplier: Option<f64>,
    /// 每百万 token 单价（元 / 1M tokens）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_per_1m_tokens: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calling: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi_agent_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_tool: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset_templates_json: Option<String>,
}

pub fn default_token_multiplier() -> f64 {
    1.0
}

/// 创建网关模型的输入参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGatewayModelInput {
    pub provider_id: String,
    pub model_config_id: String,
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(default)]
    pub is_exposed: bool,
    /// 模型来源：manual | builtin | official
    #[serde(default = "default_model_source")]
    pub source: String,
}

fn default_model_source() -> String {
    "manual".to_string()
}

/// 更新网关模型的输入参数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGatewayModelInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_exposed: Option<bool>,
}

/// 暴露模型列表项
///
/// 用于 `/v1/models` 接口，合并 Provider 与 GatewayModel 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExposedModel {
    /// 对外路由 ID：`{provider_slug}/{model_id}`
    pub id: String,
    pub provider_slug: String,
    pub model_id: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    pub provider_id: String,
    pub gateway_model_id: String,
}

// ===== 网关设置 =====

/// 网关设置 DTO（单例行）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySettings {
    pub id: String,
    pub gateway_host: String,
    pub gateway_port: i64,
    /// 默认 Gateway API Key
    ///
    /// 兼容两种形态：
    /// - Secret 引用：裸雪花 ID 或 `$SECRET:{snowflake_id}$`
    /// - 明文：用户直接填写的 key 值
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_api_key_secret_id: Option<String>,
    pub is_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Gateway 监听地址
///
/// 由 `AiGatewayService::get_gateway_listen_address()` 返回，
/// 用于 `gateway-runtime` 启动 HTTP Server 时绑定。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayListenAddress {
    pub host: String,
    pub port: u16,
}

// ===== 供应商导出/导入 =====

/// 供应商导出数据包
///
/// 包含供应商完整信息及其下所有网关模型与模型配置，
/// 序列化为 JSON 后使用 base64 编码便于分享。
/// 敏感字段（auth_json 中的 apiKey / token 等）行为由 `include_secrets` 控制：
/// - 带密钥：将 `$SECRET:{snowflake_id}$` 引用解析为明文导出
/// - 不带密钥：清空敏感字段值，仅保留认证方法与结构
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderExportData {
    pub version: String,
    pub exported_at: String,
    pub provider: ExportedProvider,
    pub models: Vec<ExportedModel>,
}

/// 导出的供应商信息
///
/// 与 `Provider` 字段对齐，但去除运行时 ID、时间戳等本地标识，
/// 便于导入时重新生成。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedProvider {
    pub slug: String,
    pub display_name: String,
    pub provider_type: String,
    pub base_url: String,
    pub use_raw_base_url: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    /// 认证配置 JSON；带密钥时为明文，不带密钥时敏感值为空
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance_provider_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_json: Option<String>,
    pub auto_fetch_official_models: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_cache_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub well_known_template_id: Option<String>,
    pub is_enabled: bool,
    pub sort_order: i64,
    /// 供应商级附加请求头（key → value）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_headers: Option<std::collections::HashMap<String, String>>,
    /// 供应商级附加请求体参数（key → value）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_body: Option<std::collections::HashMap<String, serde_json::Value>>,
}

/// 导出的模型项
///
/// 一对 `gateway_models` 记录与其关联的 `model_configs` 记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedModel {
    pub gateway_model: ExportedGatewayModel,
    pub model_config: ExportedModelConfig,
    /// 模型级附加请求头
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_headers: Option<std::collections::HashMap<String, String>>,
    /// 模型级附加请求体参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_body: Option<std::collections::HashMap<String, serde_json::Value>>,
}

/// 导出的网关模型（去除本地标识字段）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedGatewayModel {
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    pub source: String,
    pub is_exposed: bool,
}

/// 导出的模型配置（去除本地标识字段）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedModelConfig {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_input_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer: Option<String>,
    pub token_count_multiplier: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_per_1m_tokens: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calling: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi_agent_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_tool: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset_templates_json: Option<String>,
}

/// 导出供应商的输入参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProviderInput {
    pub provider_id: String,
    /// 是否包含明文密钥；false 时敏感字段会被清空
    pub include_secrets: bool,
}

/// 导入供应商的输入参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProviderInput {
    /// base64 编码的 JSON 导出数据
    pub data: String,
    /// 当 slug 冲突时的处理策略：
    /// - `auto_rename`：自动在 slug 后追加 `-imported` 或数字后缀
    /// - `fail`：返回 CONFLICT 错误（默认）
    #[serde(default = "default_import_conflict_strategy")]
    pub conflict_strategy: String,
}

fn default_import_conflict_strategy() -> String {
    "auto_rename".to_string()
}

/// Device Code 轮询结果
///
/// 单次轮询可能得到 `pending`（需继续等待）或 `success`（已更新供应商 token）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodePollResult {
    /// 轮询状态
    pub status: DeviceCodePollStatus,
    /// 授权成功并更新后的供应商对象
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<Provider>,
}

/// Device Code 轮询状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceCodePollStatus {
    /// 用户尚未完成授权，需继续轮询
    Pending,
    /// 授权成功，token 已写入供应商
    Success,
}

/// 更新网关设置的输入参数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGatewaySettingsInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_port: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_api_key_secret_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enabled: Option<bool>,
}

// ===== 网关认证 API Key =====

/// 网关认证 API Key DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAuthKey {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// API Key 值
    ///
    /// 按当前业务约定保存**明文 key**，便于网关认证时直接反查。
    /// 字段名保留 `api_key_secret_id` 仅为了兼容已有 schema。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_secret_id: Option<String>,
    pub is_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub sort_order: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建网关认证 API Key 的输入参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGatewayAuthKeyInput {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// API Key 明文值
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_secret_id: Option<String>,
    #[serde(default)]
    pub is_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i64>,
}

/// 更新网关认证 API Key 的输入参数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGatewayAuthKeyInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Option<String>>,
    /// API Key 明文值；传 `Some(None)` 表示清空，传 `None` 表示不修改
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_secret_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_type_roundtrip() {
        for pt in [
            ProviderType::Anthropic,
            ProviderType::ClaudeCode,
            ProviderType::GoogleAiStudio,
            ProviderType::GoogleVertexAi,
            ProviderType::GoogleAntigravity,
            ProviderType::GoogleGeminiCli,
            ProviderType::GithubCopilot,
            ProviderType::OpenaiChatCompletion,
            ProviderType::OpenaiCodex,
            ProviderType::OpenaiResponses,
            ProviderType::XaiGrokBuild,
            ProviderType::Ollama,
            ProviderType::Custom,
        ] {
            let s = pt.as_str();
            assert_eq!(ProviderType::from_str(s), Some(pt));
        }
        assert_eq!(ProviderType::from_str("unknown"), None);
    }

    #[test]
    fn test_auth_config_serde() {
        // none
        let auth = AuthConfig::None;
        let json = serde_json::to_string(&auth).unwrap();
        assert_eq!(json, "{\"method\":\"none\"}");

        // api-key
        let auth = AuthConfig::ApiKey {
            api_key: Some("$SECRET:abc-123$".to_string()),
        };
        let json = serde_json::to_string(&auth).unwrap();
        assert!(json.contains("\"method\":\"api-key\""));
        assert!(json.contains("\"apiKey\":\"$SECRET:abc-123$\""));
    }

    #[test]
    fn test_auth_config_default() {
        let auth = AuthConfig::default();
        assert_eq!(auth.method(), AuthMethod::None);
    }
}
