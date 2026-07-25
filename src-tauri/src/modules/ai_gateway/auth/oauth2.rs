//! # 通用 OAuth 2.0 客户端
//!
//! 支持三种授权流程：
//! - Authorization Code + PKCE（浏览器授权）
//! - Client Credentials
//! - Device Code
//!
//! 以及 Token 刷新。

use axum::{
    extract::{Query, State},
    response::Html,
    routing::get,
    Router,
};
use rand::RngCore;
use serde::Deserialize;
use tauri::Emitter;
use tokio::sync::oneshot;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::ai_gateway::types::{OAuth2Config, Provider};
use crate::modules::shared::{ProviderProxyConfig, ProviderProxyType, TimeoutConfig};

use super::OAuth2TokenData;

/// OAuth 浏览器授权回调服务器存活超时（秒）
///
/// 与前端 `OAUTH_TIMEOUT_SECONDS` 保持一致，超时后自动释放端口，
/// 避免固定 redirect_uri 的供应商（Claude Code / OpenAI Codex / xAI）
/// 因服务器未关闭而导致再次授权时端口被占用。
const OAUTH_CALLBACK_TIMEOUT_SECONDS: u64 = 120;

/// OAuth2 标准 token 响应（snake_case）
#[derive(Debug, Clone, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    token_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_in: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
}

impl TokenResponse {
    fn into_token_data(self, now: i64) -> OAuth2TokenData {
        OAuth2TokenData {
            access_token: self.access_token,
            token_type: self.token_type,
            refresh_token: self.refresh_token,
            expires_at: self.expires_in.map(|secs| now + secs),
            scope: self.scope,
        }
    }
}

/// Device Code 授权端点响应（snake_case）
#[derive(Debug, Clone, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    #[serde(alias = "verification_url")]
    verification_uri: String,
    #[serde(skip_serializing_if = "Option::is_none", alias = "verification_url_complete")]
    verification_uri_complete: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_in: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interval: Option<i64>,
}

impl DeviceCodeResponse {
    fn into_public(self) -> super::DeviceCodeInfo {
        super::DeviceCodeInfo {
            device_code: self.device_code,
            user_code: self.user_code,
            verification_uri: self.verification_uri,
            verification_uri_complete: self.verification_uri_complete,
            expires_in: self.expires_in,
            interval: self.interval.unwrap_or(5),
        }
    }
}

/// OAuth2 客户端
#[derive(Debug, Clone)]
pub struct OAuth2Client {
    http: reqwest::Client,
}

impl OAuth2Client {
    /// 创建新的 OAuth2 客户端
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// 根据供应商配置构造 OAuth2 客户端
    ///
    /// 读取供应商的 `timeout_json` / `proxy_json`，应用连接超时、响应超时与代理设置，
    /// 使 OAuth 授权请求与后续网关转发请求走一致的网络策略。
    pub fn new_for_provider(provider: &Provider) -> IcodeResult<Self> {
        let mut builder = reqwest::Client::builder()
            .user_agent(concat!("i-code-oauth/", env!("CARGO_PKG_VERSION")));

        if let Some(json) = provider.timeout_json.as_deref() {
            let cfg: TimeoutConfig = serde_json::from_str(json)
                .map_err(|e| IcodeError::validation(format!("解析 timeout_json 失败: {}", e)))?;
            builder = builder.connect_timeout(std::time::Duration::from_millis(cfg.connection));
            builder = builder.timeout(std::time::Duration::from_millis(cfg.response));
        }

        if let Some(json) = provider.proxy_json.as_deref() {
            let cfg: ProviderProxyConfig = serde_json::from_str(json)
                .map_err(|e| IcodeError::validation(format!("解析 proxy_json 失败: {}", e)))?;
            match cfg.proxy_type {
                ProviderProxyType::Global => {
                    // 使用全局代理：沿用 reqwest 默认行为（读取系统环境变量代理）
                }
                ProviderProxyType::Direct => {
                    builder = builder.no_proxy();
                }
                ProviderProxyType::Socks | ProviderProxyType::Http => {
                    let url = cfg
                        .url
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| IcodeError::validation("代理缺少 url"))?;
                    let proxy = reqwest::Proxy::all(url)
                        .map_err(|e| IcodeError::validation(format!("构造代理失败: {}", e)))?;
                    builder = builder.proxy(proxy);
                }
            }
        }

        let http = builder
            .build()
            .map_err(|e| IcodeError::internal(format!("构造 OAuth HTTP 客户端失败: {}", e)))?;
        Ok(Self { http })
    }

    /// 生成一个 OAuth state 随机字符串
    pub fn generate_state() -> String {
        generate_random_string(32)
    }

    /// 生成 Authorization Code 授权 URL
    ///
    /// 传入的 `state` 必须与回调服务器校验的 state 一致。返回 (授权 URL, code_verifier)。
    /// 调用方需要把 state 和 code_verifier 保存起来，用于后续换 token。
    pub fn build_authorization_url(
        &self,
        config: &OAuth2Config,
        state: &str,
    ) -> IcodeResult<(String, String)> {
        let auth_url = config
            .authorization_url
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 缺少 authorization_url"))?;
        let client_id = config
            .client_id
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 缺少 client_id"))?;
        let redirect_uri = config
            .redirect_uri
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 缺少 redirect_uri"))?;

        let pkce_enabled = config.pkce.unwrap_or(true);
        let code_verifier = if pkce_enabled {
            generate_code_verifier()
        } else {
            String::new()
        };
        let code_challenge = if pkce_enabled {
            Some(base64_urlencode(&sha256(&code_verifier)))
        } else {
            None
        };

        let mut url: reqwest::Url = auth_url
            .parse()
            .map_err(|e| IcodeError::validation(format!("OAuth2 授权 URL 无效: {}", e)))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("response_type", "code");
            pairs.append_pair("client_id", client_id);
            pairs.append_pair("redirect_uri", redirect_uri);
            pairs.append_pair("state", &state);
            if let Some(scope) = &config.scopes {
                if !scope.is_empty() {
                    pairs.append_pair("scope", &scope.join(" "));
                }
            }
            if let Some(challenge) = &code_challenge {
                pairs.append_pair("code_challenge", challenge);
                pairs.append_pair("code_challenge_method", "S256");
            }
        }

        Ok((url.to_string(), code_verifier))
    }

    /// 用 authorization code 换取 token
    pub async fn exchange_code(
        &self,
        config: &OAuth2Config,
        code: &str,
        code_verifier: &str,
    ) -> IcodeResult<OAuth2TokenData> {
        let token_url = config
            .token_url
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 缺少 token_url"))?;
        let client_id = config
            .client_id
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 缺少 client_id"))?;
        let redirect_uri = config
            .redirect_uri
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 缺少 redirect_uri"))?;

        let mut params = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
        ];

        let pkce_enabled = config.pkce.unwrap_or(true);
        if pkce_enabled {
            params.push(("code_verifier", code_verifier));
        }

        if let Some(secret) = config.client_secret.as_ref().filter(|s| !s.is_empty()) {
            params.push(("client_secret", secret));
        }

        self.post_token(token_url, &params).await
    }

    /// Client Credentials 流程换 token
    #[expect(dead_code)]
    pub async fn exchange_client_credentials(
        &self,
        config: &OAuth2Config,
    ) -> IcodeResult<OAuth2TokenData> {
        let token_url = config
            .token_url
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 缺少 token_url"))?;
        let client_id = config
            .client_id
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 缺少 client_id"))?;
        let client_secret = config
            .client_secret
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 client_credentials 缺少 client_secret"))?;

        let mut params = vec![
            ("grant_type", "client_credentials"),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ];

        let scope_str;
        if let Some(scope) = &config.scopes {
            if !scope.is_empty() {
                scope_str = scope.join(" ");
                params.push(("scope", &scope_str));
            }
        }

        self.post_token(token_url, &params).await
    }

    /// 发起 Device Code 流程
    ///
    /// 向设备授权端点请求设备码与用户码，返回的信息需要展示给用户，
    /// 由用户在浏览器中访问 `verification_uri` 并输入 `user_code`。
    pub async fn request_device_code(
        &self,
        config: &OAuth2Config,
    ) -> IcodeResult<super::DeviceCodeInfo> {
        let device_url = config
            .device_authorization_url
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 缺少 device_authorization_url"))?;
        let client_id = config
            .client_id
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 缺少 client_id"))?;

        let mut params = vec![("client_id", client_id)];

        let scope_str;
        if let Some(scope) = &config.scopes {
            if !scope.is_empty() {
                scope_str = scope.join(" ");
                params.push(("scope", &scope_str));
            }
        }

        let resp = self
            .http
            .post(device_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| IcodeError::gateway(Self::format_reqwest_error("请求 device code 端点失败", &e)))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| IcodeError::gateway(Self::format_reqwest_error("读取 device code 响应失败", &e)))?;

        if !status.is_success() {
            return Err(IcodeError::gateway(format!(
                "device code 端点返回错误 [{}]: {}",
                status, text
            )));
        }

        let device_resp: DeviceCodeResponse = serde_json::from_str(&text).map_err(|e| {
            IcodeError::gateway(format!("解析 device code 响应失败: {}，响应: {}", e, text))
        })?;

        Ok(device_resp.into_public())
    }

    /// 轮询 Device Code token
    ///
    /// 单次轮询：若用户尚未完成授权，多数供应商会返回 `authorization_pending`，
    /// 此时调用方应按 `interval` 等待后再次调用。
    ///
    /// 返回 `None` 表示 `authorization_pending` 或 `slow_down`，需要继续轮询。
    pub async fn poll_device_token(
        &self,
        config: &OAuth2Config,
        device_code: &str,
    ) -> IcodeResult<Option<OAuth2TokenData>> {
        let token_url = config
            .token_url
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 缺少 token_url"))?;
        let client_id = config
            .client_id
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 缺少 client_id"))?;

        let mut params = vec![
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:device_code",
            ),
            ("client_id", client_id),
            ("device_code", device_code),
        ];

        if let Some(secret) = config.client_secret.as_ref().filter(|s| !s.is_empty()) {
            params.push(("client_secret", secret));
        }

        let resp = self
            .http
            .post(token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| IcodeError::gateway(Self::format_reqwest_error("轮询 device token 端点失败", &e)))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| IcodeError::gateway(Self::format_reqwest_error("读取 device token 响应失败", &e)))?;

        if !status.is_success() {
            // 设备码流程中，pending / slow_down 也可能以 400/428 返回
            if let Ok(error_body) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(error) = error_body.get("error").and_then(|v| v.as_str()) {
                    match error {
                        "authorization_pending" | "slow_down" => return Ok(None),
                        "expired_token" => {
                            return Err(IcodeError::validation("设备码已过期，请重新发起授权"))
                        }
                        "access_denied" => {
                            return Err(IcodeError::validation("用户拒绝授权"))
                        }
                        _ => {}
                    }
                }
            }
            return Err(IcodeError::gateway(format!(
                "device token 端点返回错误 [{}]: {}",
                status, text
            )));
        }

        // 某些实现（如 GitHub）在 pending 时仍返回 200 且 body 含 error
        if let Ok(error_body) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(error) = error_body.get("error").and_then(|v| v.as_str()) {
                match error {
                    "authorization_pending" | "slow_down" => return Ok(None),
                    "expired_token" => {
                        return Err(IcodeError::validation("设备码已过期，请重新发起授权"))
                    }
                    "access_denied" => return Err(IcodeError::validation("用户拒绝授权")),
                    _ => {}
                }
            }
        }

        let token_resp: TokenResponse = serde_json::from_str(&text).map_err(|e| {
            IcodeError::gateway(format!("解析 device token 响应失败: {}，响应: {}", e, text))
        })?;

        let now = chrono::Utc::now().timestamp();
        Ok(Some(token_resp.into_token_data(now)))
    }

    /// 刷新 access_token
    pub async fn refresh_access_token(
        &self,
        config: &OAuth2Config,
        refresh_token: &str,
    ) -> IcodeResult<OAuth2TokenData> {
        let token_url = config
            .token_url
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 缺少 token_url"))?;
        let client_id = config
            .client_id
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IcodeError::validation("OAuth2 缺少 client_id"))?;

        let mut params = vec![
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
        ];

        if let Some(secret) = config.client_secret.as_ref().filter(|s| !s.is_empty()) {
            params.push(("client_secret", secret));
        }

        self.post_token(token_url, &params).await
    }

    /// 启动临时回调服务器
    ///
    /// 若传入 `redirect_uri`，则解析其中的 host/port/path 并绑定到固定地址；
    /// 否则监听 `127.0.0.1:0` 并返回动态 `redirect_uri`。
    /// 调用方生成授权 URL 并打开浏览器后，通过接收器等待回调结果。
    /// 服务器在接收器 drop 或收到结果后自动关闭。
    ///
    /// # 返回
    /// - `(redirect_uri, receiver)`：receiver 收到 `CallbackQuery`
    pub async fn start_callback_server(
        &self,
        expected_state: &str,
        redirect_uri: Option<&str>,
    ) -> IcodeResult<(String, oneshot::Receiver<IcodeResult<CallbackQuery>>)> {
        let (bind_addr, redirect_path, redirect_uri) = match redirect_uri {
            Some(uri) if !uri.is_empty() => {
                let parsed = uri
                    .parse::<reqwest::Url>()
                    .map_err(|e| IcodeError::validation(format!("OAuth2 redirect_uri 无效: {}", e)))?;
                let host = parsed
                    .host_str()
                    .ok_or_else(|| IcodeError::validation("OAuth2 redirect_uri 缺少 host"))?;
                let port = parsed
                    .port_or_known_default()
                    .ok_or_else(|| IcodeError::validation("OAuth2 redirect_uri 缺少端口"))?;
                let path = parsed.path();
                let bind_addr = format!("{}:{}", host, port);
                let redirect_uri = uri.to_string();
                (bind_addr, path.to_string(), redirect_uri)
            }
            _ => {
                let bind_addr = "127.0.0.1:0".to_string();
                let redirect_path = "/callback".to_string();
                (bind_addr, redirect_path, String::new())
            }
        };

        let listener = tokio::net::TcpListener::bind(&bind_addr)
            .await
            .map_err(|e| IcodeError::internal(format!("启动 OAuth 回调服务器失败: {}", e)))?;
        let addr = listener
            .local_addr()
            .map_err(|e| IcodeError::internal(format!("获取回调服务器地址失败: {}", e)))?;

        // 动态端口时需要根据实际绑定地址拼接 redirect_uri
        let redirect_uri = if redirect_uri.is_empty() {
            format!("http://{}/callback", addr)
        } else {
            redirect_uri
        };

        let (tx, rx) = oneshot::channel::<IcodeResult<CallbackQuery>>();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let shutdown_tx = std::sync::Arc::new(std::sync::Mutex::new(Some(shutdown_tx)));
        let state = CallbackServerState {
            sender: std::sync::Arc::new(std::sync::Mutex::new(Some(tx))),
            expected_state: expected_state.to_string(),
            shutdown_tx: shutdown_tx.clone(),
        };

        let app = Router::new()
            .route(&redirect_path, get(callback_handler))
            .with_state(state);

        // 120 秒超时后主动关闭服务器，避免固定端口长期被占用
        let timeout_shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(OAUTH_CALLBACK_TIMEOUT_SECONDS)).await;
            if let Some(tx) = timeout_shutdown_tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
        });

        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        tokio::spawn(async move {
            if let Err(e) = server.await {
                log::error!("OAuth 回调服务器异常: {}", e);
            }
        });

        Ok((redirect_uri, rx))
    }

    /// 启动回调服务器，回调成功时通过 Tauri Event 通知前端
    ///
    /// 与 `start_callback_server` 不同，此方法不使用 oneshot channel 传递回调结果，
    /// 而是在收到回调时通过 Tauri Event (`oauth-callback-result`) 发送授权码。
    /// 适用于 `gateway_provider_oauth_start` 命令（立即返回，不等待回调）。
    ///
    /// 事件 payload 结构为 `OAuthCallbackEvent { provider_id, code, state }`，
    /// 前端监听此事件后可自动调用 `gateway_provider_oauth_complete` 完成流程。
    ///
    /// # 返回
    /// - `redirect_uri`：回调服务器实际监听的 URI
    pub async fn start_callback_server_with_event(
        &self,
        app: &tauri::AppHandle,
        expected_state: &str,
        redirect_uri: Option<&str>,
        provider_id: &str,
    ) -> IcodeResult<String> {
        let (bind_addr, redirect_path, redirect_uri) = match redirect_uri {
            Some(uri) if !uri.is_empty() => {
                let parsed = uri
                    .parse::<reqwest::Url>()
                    .map_err(|e| IcodeError::validation(format!("OAuth2 redirect_uri 无效: {}", e)))?;
                let host = parsed
                    .host_str()
                    .ok_or_else(|| IcodeError::validation("OAuth2 redirect_uri 缺少 host"))?;
                let port = parsed
                    .port_or_known_default()
                    .ok_or_else(|| IcodeError::validation("OAuth2 redirect_uri 缺少端口"))?;
                let path = parsed.path();
                let bind_addr = format!("{}:{}", host, port);
                let redirect_uri = uri.to_string();
                (bind_addr, path.to_string(), redirect_uri)
            }
            _ => {
                let bind_addr = "127.0.0.1:0".to_string();
                let redirect_path = "/callback".to_string();
                (bind_addr, redirect_path, String::new())
            }
        };

        let listener = tokio::net::TcpListener::bind(&bind_addr)
            .await
            .map_err(|e| IcodeError::internal(format!("启动 OAuth 回调服务器失败: {}", e)))?;
        let addr = listener
            .local_addr()
            .map_err(|e| IcodeError::internal(format!("获取回调服务器地址失败: {}", e)))?;

        // 动态端口时需要根据实际绑定地址拼接 redirect_uri
        let redirect_uri = if redirect_uri.is_empty() {
            format!("http://{}/callback", addr)
        } else {
            redirect_uri
        };

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let shutdown_tx = std::sync::Arc::new(std::sync::Mutex::new(Some(shutdown_tx)));
        let state = EventCallbackServerState {
            app_handle: app.clone(),
            expected_state: expected_state.to_string(),
            provider_id: provider_id.to_string(),
            shutdown_tx: shutdown_tx.clone(),
        };

        let app = Router::new()
            .route(&redirect_path, get(event_callback_handler))
            .with_state(state);

        // 120 秒超时后主动关闭服务器，避免固定端口长期被占用
        let timeout_shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(OAUTH_CALLBACK_TIMEOUT_SECONDS)).await;
            if let Some(tx) = timeout_shutdown_tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
        });

        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        tokio::spawn(async move {
            if let Err(e) = server.await {
                log::error!("OAuth 回调服务器异常: {}", e);
            }
        });

        Ok(redirect_uri)
    }

    /// 统一 POST token endpoint
    async fn post_token(
        &self,
        token_url: &str,
        params: &[(&str, &str)],
    ) -> IcodeResult<OAuth2TokenData> {
        self.post_token_with_error(token_url, params).await
    }

    /// 格式化 reqwest 错误，包含底层原因链
    fn format_reqwest_error(context: &str, e: &reqwest::Error) -> String {
        let mut msg = format!("{}: {}", context, e);
        let mut source = std::error::Error::source(e);
        while let Some(s) = source {
            msg.push_str(&format!("; {}", s));
            source = s.source();
        }
        msg
    }

    /// 统一 POST token endpoint（保留原始错误上下文）
    async fn post_token_with_error(
        &self,
        token_url: &str,
        params: &[(&str, &str)],
    ) -> IcodeResult<OAuth2TokenData> {
        let resp = self
            .http
            .post(token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| IcodeError::gateway(Self::format_reqwest_error("请求 token 端点失败", &e)))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| IcodeError::gateway(Self::format_reqwest_error("读取 token 响应失败", &e)))?;

        if !status.is_success() {
            return Err(IcodeError::gateway(format!(
                "token 端点返回错误 [{}]: {}",
                status, text
            )));
        }

        let token_resp: TokenResponse = serde_json::from_str(&text).map_err(|e| {
            IcodeError::gateway(format!("解析 token 响应失败: {}，响应: {}", e, text))
        })?;

        let now = chrono::Utc::now().timestamp();
        Ok(token_resp.into_token_data(now))
    }
}

impl Default for OAuth2Client {
    fn default() -> Self {
        Self::new()
    }
}

/// 回调 URL 查询参数
#[derive(Debug, Clone, Deserialize)]
pub struct CallbackQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(rename = "error_description", skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
}

/// 回调服务器共享状态
///
/// 保存一次性结果发送器与期望的 state，用于把浏览器回调传回主流程并防止 CSRF。
/// 同时持有服务器关闭信号发送端，收到回调或超时时主动释放端口。
#[derive(Clone)]
struct CallbackServerState {
    /// 一次性发送器，用于把回调结果传回主流程
    sender: std::sync::Arc<std::sync::Mutex<Option<oneshot::Sender<IcodeResult<CallbackQuery>>>>>,
    /// 期望的 state 值，用于防止 CSRF
    expected_state: String,
    /// 服务器关闭信号发送端，收到回调或超时时触发 graceful shutdown
    shutdown_tx: std::sync::Arc<std::sync::Mutex<Option<oneshot::Sender<()>>>>,
}

/// OAuth 回调处理器
///
/// 浏览器授权后重定向到 `http://127.0.0.1:{port}/callback?code=...&state=...`。
/// 校验 state 后把结果通过 oneshot 发送给等待方，并返回成功页面。
async fn callback_handler(
    State(state): State<CallbackServerState>,
    Query(query): Query<CallbackQuery>,
) -> Html<&'static str> {
    let result = if let Some(ref error) = query.error {
        let msg = query
            .error_description
            .clone()
            .unwrap_or_else(|| error.clone());
        Err(IcodeError::gateway(format!(
            "OAuth 授权失败: {} ({})",
            error, msg
        )))
    } else if query.state.as_deref() != Some(state.expected_state.as_str()) {
        Err(IcodeError::validation("OAuth state 不匹配"))
    } else if query.code.is_none() {
        Err(IcodeError::validation("OAuth 回调缺少 code 参数"))
    } else {
        Ok(query)
    };

    if let Some(sender) = state.sender.lock().unwrap().take() {
        let _ = sender.send(result);
    }

    // 回调已处理，触发服务器 graceful shutdown 以尽快释放端口
    if let Some(tx) = state.shutdown_tx.lock().unwrap().take() {
        let _ = tx.send(());
    }

    Html(CALLBACK_SUCCESS_HTML)
}

/// 回调服务器事件通知状态
///
/// 与 `CallbackServerState` 不同，此状态在收到回调时不使用 oneshot channel，
/// 而是通过 Tauri Event 将授权码通知给前端。
/// 同时持有服务器关闭信号发送端，收到回调或超时时主动释放端口。
#[derive(Clone)]
struct EventCallbackServerState {
    /// Tauri AppHandle，用于发送事件到前端
    app_handle: tauri::AppHandle,
    /// 期望的 state 值，用于防止 CSRF
    expected_state: String,
    /// 供应商 ID，用于事件 payload
    provider_id: String,
    /// 服务器关闭信号发送端，收到回调或超时时触发 graceful shutdown
    shutdown_tx: std::sync::Arc<std::sync::Mutex<Option<oneshot::Sender<()>>>>,
}

/// OAuth 回调事件 payload
///
/// 回调服务器收到浏览器重定向后，通过 Tauri Event `oauth-callback-result`
/// 发送此结构到前端。前端监听此事件后可自动调用 `gateway_provider_oauth_complete`
/// 完成授权流程。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthCallbackEvent {
    /// 供应商 ID
    pub provider_id: String,
    /// 授权码
    pub code: Option<String>,
    /// OAuth state
    pub state: Option<String>,
    /// 错误码（授权失败时）
    pub error: Option<String>,
    /// 错误描述
    pub error_description: Option<String>,
}

/// 事件通知版回调处理器
///
/// 浏览器授权后重定向到 `http://127.0.0.1:{port}/callback?code=...&state=...`。
/// 校验 state 后把结果通过 Tauri Event 发送给前端，并返回成功页面。
async fn event_callback_handler(
    State(state): State<EventCallbackServerState>,
    Query(query): Query<CallbackQuery>,
) -> Html<&'static str> {
    let event = OAuthCallbackEvent {
        provider_id: state.provider_id.clone(),
        code: query.code.clone(),
        state: query.state.clone(),
        error: query.error.clone(),
        error_description: query.error_description.clone(),
    };

    // 校验 state：仅当 state 匹配时才把 code 传给前端
    if query.state.as_deref() != Some(state.expected_state.as_str()) {
        log::warn!("OAuth 回调 state 不匹配: 期望={}, 实际={}", state.expected_state, query.state.as_deref().unwrap_or(""));
    }

    // 发送事件到前端
    if let Err(e) = state.app_handle.emit("oauth-callback-result", &event) {
        log::error!("发送 OAuth 回调事件失败: {}", e);
    }

    // 回调已处理，触发服务器 graceful shutdown 以尽快释放端口
    if let Some(tx) = state.shutdown_tx.lock().unwrap().take() {
        let _ = tx.send(());
    }

    Html(CALLBACK_SUCCESS_HTML)
}

/// 授权成功后的回调页面
const CALLBACK_SUCCESS_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>授权成功</title>
    <style>
        body { font-family: system-ui, sans-serif; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; background: #f5f5f5; }
        .card { background: white; padding: 2rem; border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.1); text-align: center; }
        h1 { margin: 0 0 0.5rem; color: #16a34a; }
        p { color: #666; margin: 0; }
    </style>
</head>
<body>
    <div class="card">
        <h1>授权成功</h1>
        <p>请回到 i-code 应用继续操作。</p>
    </div>
</body>
</html>"#;

/// 生成 PKCE code_verifier
fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64_urlencode(&bytes)
}

/// 对字节序列做 base64url 编码（无填充）
fn base64_urlencode(input: &[u8]) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    URL_SAFE_NO_PAD.encode(input)
}

/// 计算 SHA-256
fn sha256(input: &str) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher.finalize().to_vec()
}

/// 生成指定长度的随机字符串
fn generate_random_string(len: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = (rng.next_u32() as usize) % CHARSET.len();
            CHARSET[idx] as char
        })
        .collect()
}
