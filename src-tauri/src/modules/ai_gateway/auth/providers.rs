//! # 各供应商 OAuth 配置
//!
//! 集中维护各供应商的 OAuth 端点、默认 scope、client_id 等常量。
//!
//! ⚠️ 注意事项：
//! - 以下 client_id 来自参考项目 `vscode-unify-chat-provider`，属于第三方公开客户端标识。
//! - 生产环境使用时应注册自己的 OAuth 应用并替换这些值。
//! - 部分供应商（Claude Code、OpenAI Codex、xAI）的 OAuth 应用只接受固定 redirect_uri，
//!   因此必须为它们配置固定端口；Google 类供应商允许动态 localhost 端口。

use crate::modules::ai_gateway::types::{AuthMethod, OAuth2Config, OAuth2GrantType};

/// 供应商 OAuth 预设
///
/// 用于前端未填写完整 OAuth 端点时自动填充默认值。
#[derive(Debug, Clone)]
pub struct OAuthProviderPreset {
    /// 认证方法标识
    pub method: AuthMethod,
    /// OAuth 授权类型
    pub grant_type: OAuth2GrantType,
    /// 授权端点 URL
    pub authorization_url: &'static str,
    /// Token 端点 URL
    pub token_url: &'static str,
    /// 设备授权端点 URL（device_code 流程）
    pub device_authorization_url: &'static str,
    /// 默认 scopes（空格拼接）
    pub default_scopes: &'static str,
    /// 是否启用 PKCE
    pub pkce: bool,
    /// 预设 client_id
    pub client_id: &'static str,
    /// 预设 client_secret（可为空）
    pub client_secret: &'static str,
    /// 固定回调地址（空字符串表示使用动态本地端口）
    pub redirect_uri: &'static str,
}

impl OAuthProviderPreset {
    /// 将预设合并到用户提供的 `OAuth2Config` 中
    ///
    /// 仅填充用户未填写的字段，保留用户显式覆盖的值。
    pub fn apply(&self, config: &mut OAuth2Config) {
        if config.grant_type != self.grant_type {
            config.grant_type = self.grant_type;
        }
        if config.authorization_url.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
            config.authorization_url = Some(self.authorization_url.to_string());
        }
        if config.token_url.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
            config.token_url = Some(self.token_url.to_string());
        }
        if config.device_authorization_url.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
            config.device_authorization_url = Some(self.device_authorization_url.to_string());
        }
        if config.scopes.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
            config.scopes = Some(self.default_scopes.split(' ').map(String::from).collect());
        }
        if config.pkce.is_none() {
            config.pkce = Some(self.pkce);
        }
        if config.client_id.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
            config.client_id = Some(self.client_id.to_string());
        }
        if config.client_secret.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
            if !self.client_secret.is_empty() {
                config.client_secret = Some(self.client_secret.to_string());
            }
        }
        if config.redirect_uri.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
            if !self.redirect_uri.is_empty() {
                config.redirect_uri = Some(self.redirect_uri.to_string());
            }
        }
    }
}

/// 根据认证方法获取 OAuth 预设
///
/// 返回 `None` 表示该方法不需要或不支持 OAuth 预设。
pub fn get_oauth_preset(method: AuthMethod) -> Option<OAuthProviderPreset> {
    match method {
        AuthMethod::Oauth2 => Some(OAuthProviderPreset {
            method: AuthMethod::Oauth2,
            grant_type: OAuth2GrantType::AuthorizationCode,
            authorization_url: "",
            token_url: "",
            device_authorization_url: "",
            default_scopes: "",
            pkce: true,
            client_id: "",
            client_secret: "",
            redirect_uri: "",
        }),
        AuthMethod::AntigravityOauth => Some(OAuthProviderPreset {
            method: AuthMethod::AntigravityOauth,
            grant_type: OAuth2GrantType::AuthorizationCode,
            authorization_url: "https://accounts.google.com/o/oauth2/v2/auth",
            token_url: "https://oauth2.googleapis.com/token",
            device_authorization_url: "",
            default_scopes: "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile https://www.googleapis.com/auth/cclog https://www.googleapis.com/auth/experimentsandconfigs",
            pkce: true,
            client_id: "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com",
            client_secret: "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",
            redirect_uri: "",
        }),
        AuthMethod::GoogleGeminiOauth => Some(OAuthProviderPreset {
            method: AuthMethod::GoogleGeminiOauth,
            grant_type: OAuth2GrantType::AuthorizationCode,
            authorization_url: "https://accounts.google.com/o/oauth2/v2/auth",
            token_url: "https://oauth2.googleapis.com/token",
            device_authorization_url: "",
            default_scopes: "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile",
            pkce: true,
            client_id: "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com",
            client_secret: "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",
            redirect_uri: "",
        }),
        AuthMethod::OpenaiCodexAuth => Some(OAuthProviderPreset {
            method: AuthMethod::OpenaiCodexAuth,
            grant_type: OAuth2GrantType::AuthorizationCode,
            authorization_url: "https://auth.openai.com/authorize",
            token_url: "https://auth.openai.com/token",
            device_authorization_url: "",
            default_scopes: "openid email profile offline_access",
            pkce: true,
            client_id: "app_EMoamEEZ73f0CkXaXp7hrann",
            client_secret: "",
            redirect_uri: "http://localhost:1455/auth/callback",
        }),
        AuthMethod::ClaudeCodeAuth => Some(OAuthProviderPreset {
            method: AuthMethod::ClaudeCodeAuth,
            grant_type: OAuth2GrantType::AuthorizationCode,
            authorization_url: "https://claude.ai/oauth/authorize",
            token_url: "https://platform.claude.com/v1/oauth/token",
            device_authorization_url: "",
            default_scopes: "org:create_api_key user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload",
            pkce: true,
            client_id: "9d1c250a-e61b-44d9-88ed-5944d1962f5e",
            client_secret: "",
            redirect_uri: "http://localhost:54545/callback",
        }),
        AuthMethod::XaiGrokOauth => Some(OAuthProviderPreset {
            method: AuthMethod::XaiGrokOauth,
            grant_type: OAuth2GrantType::AuthorizationCode,
            authorization_url: "https://auth.x.ai/oauth2/authorize",
            token_url: "https://auth.x.ai/oauth2/token",
            device_authorization_url: "",
            default_scopes: "openid profile email offline_access grok-cli:access api:access",
            pkce: true,
            client_id: "b1a00492-073a-47ea-816f-4c329264a828",
            client_secret: "",
            redirect_uri: "http://127.0.0.1:56121/callback",
        }),
        AuthMethod::GithubCopilotAuth => Some(OAuthProviderPreset {
            method: AuthMethod::GithubCopilotAuth,
            grant_type: OAuth2GrantType::DeviceCode,
            authorization_url: "",
            token_url: "https://github.com/login/oauth/access_token",
            device_authorization_url: "https://github.com/login/device/code",
            default_scopes: "read:user",
            pkce: false,
            client_id: "Ov23li8tweQw6odWQebz",
            client_secret: "",
            redirect_uri: "",
        }),
        _ => None,
    }
}

/// 判断认证方法是否需要 OAuth 浏览器授权
pub fn is_oauth_method(method: AuthMethod) -> bool {
    matches!(
        method,
        AuthMethod::Oauth2
            | AuthMethod::AntigravityOauth
            | AuthMethod::GoogleGeminiOauth
            | AuthMethod::OpenaiCodexAuth
            | AuthMethod::ClaudeCodeAuth
            | AuthMethod::XaiGrokOauth
            | AuthMethod::GithubCopilotAuth
    )
}
