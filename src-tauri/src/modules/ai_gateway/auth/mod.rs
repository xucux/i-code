//! # AI Gateway 认证辅助模块
//!
//! 负责 OAuth 2.0 授权流程、Token 刷新以及各供应商 OAuth 配置。
//!
//! 模块结构：
//! - [`oauth2`]：通用 OAuth2 客户端、临时回调服务器、Device Code 轮询
//! - [`providers`]：各供应商授权端点、scope、PKCE 等常量配置
//!
//! 设计约束：
//! - 所有敏感字段（access_token、refresh_token、client_secret）仅在 Rust 后端处理。
//! - 授权完成后返回的 token 数据以 JSON 字符串形式交给 Service 层加密存储。
//! - 浏览器授权使用临时 `127.0.0.1:0` 回调服务器，避免依赖 deep-link 插件。

pub mod oauth2;
pub mod providers;

use serde::{Deserialize, Serialize};

/// 浏览器授权流程启动结果
///
/// `gateway_provider_oauth_start` 命令返回给前端的数据，
/// 包含授权 URL 以及 PKCE code_verifier（用于后续手动换 token）。
/// 前端需打开授权 URL 让用户在浏览器中登录，
/// 若回调自动完成则整个授权流程走通；
/// 若浏览器不自动回调（显示授权码让用户复制），则前端需让用户手动输入授权码
/// 并调用 `gateway_provider_oauth_complete` 完成流程。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthStartResult {
    /// 浏览器授权 URL，前端需调用 `open()` 打开
    pub authorization_url: String,
    /// PKCE code_verifier，用于后续换取 token
    /// 此字段为临时敏感数据，不持久化存储
    pub code_verifier: String,
    /// OAuth state，用于验证回调合法性
    pub state: String,
    /// 回调服务器实际监听的 redirect_uri
    pub redirect_uri: String,
}

/// OAuth 2.0 授权结果
///
/// 浏览器授权或 Device Code 流程完成后返回给调用方的数据结构。
/// 前端不需要知道 access_token 明文，但后端需要把它序列化后加密保存。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationResult {
    /// 授权成功后的 token 数据
    pub token: OAuth2TokenData,
    /// 供应商返回的额外账户信息（如 email、account_id、project_id 等）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_info: Option<serde_json::Value>,
}

/// Device Code 授权初始响应
///
/// 设备码流程第一步返回给用户的信息，用于在浏览器中完成授权。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeInfo {
    /// 设备码，用于后续轮询 token
    pub device_code: String,
    /// 用户码，需在授权页面输入
    pub user_code: String,
    /// 授权页面 URL
    pub verification_uri: String,
    /// 带用户码的完整授权页面 URL（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_uri_complete: Option<String>,
    /// 设备码过期时间（秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<i64>,
    /// 建议轮询间隔（秒）
    pub interval: i64,
}

/// OAuth 2.0 Token 数据
///
/// 与参考项目对齐，使用 camelCase 序列化。
/// Service 层会把整个结构序列化为 JSON 字符串，再加密存储为 `$SECRET:{snowflake_id}$`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuth2TokenData {
    /// 访问令牌
    pub access_token: String,
    /// Token 类型，通常为 "Bearer"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    /// 刷新令牌
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// 过期时间戳（Unix 秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    /// 授权 scope
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

impl OAuth2TokenData {
    /// 判断是否已过期（预留 60 秒缓冲）
    #[expect(dead_code)]
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(exp) => {
                let now = chrono::Utc::now().timestamp();
                now >= exp - 60
            }
            None => false,
        }
    }
}
