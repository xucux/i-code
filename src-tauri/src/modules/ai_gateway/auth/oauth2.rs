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
use crate::modules::logger::Log;
use crate::modules::shared::TimeoutConfig;

use super::OAuth2TokenData;
use super::callback_registry::{global_registry, CallbackServerEntry};

/// OAuth 浏览器授权回调服务器存活超时（秒）
///
/// 与前端 `OAUTH_TIMEOUT_SECONDS` 保持一致，超时后自动释放端口，
/// 避免固定 redirect_uri 的供应商（Claude Code / OpenAI Codex / xAI）
/// 因服务器未关闭而导致再次授权时端口被占用。
const OAUTH_CALLBACK_TIMEOUT_SECONDS: u64 = 120;

/// 将 TCP 绑定错误转换为 IcodeError
///
/// 端口被占用时返回 `CONFLICT` 错误并在 details 中附带 `port` 与 `reason: "port_in_use"`，
/// 前端据此弹出分平台清理进程提示；其他错误返回 `INTERNAL`。
fn bind_error_to_icode(e: std::io::Error, port_hint: Option<u16>) -> IcodeError {
    if e.kind() == std::io::ErrorKind::AddrInUse {
        let port_str = port_hint
            .map(|p| p.to_string())
            .unwrap_or_else(|| "未知".to_string());
        let msg = format!("OAuth 回调端口 {} 已被占用", port_str);
        log::error!("OAuth 回调端口占用: port={:?}", port_hint);
        Log::error(&format!(
            "OAuth 回调端口 {} 已被占用，请清理占用进程后重试",
            port_str
        ));
        IcodeError::conflict(msg)
            .with_details(serde_json::json!({ "port": port_hint, "reason": "port_in_use" }))
    } else {
        log::error!("启动 OAuth 回调服务器失败: {}", e);
        Log::error(&format!("启动 OAuth 回调服务器失败: {}", e));
        IcodeError::internal(format!("启动 OAuth 回调服务器失败: {}", e))
    }
}

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
        let is_renewable = self.refresh_token.is_some();
        OAuth2TokenData {
            access_token: self.access_token,
            token_type: self.token_type,
            refresh_token: self.refresh_token,
            expires_at: self.expires_in.map(|secs| now + secs),
            scope: self.scope,
            is_renewable: Some(is_renewable),
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
        use crate::modules::shared::apply_provider_proxy;

        let mut builder = reqwest::Client::builder()
            .user_agent(concat!("i-code-oauth/", env!("CARGO_PKG_VERSION")));

        if let Some(json) = provider.timeout_json.as_deref() {
            let cfg: TimeoutConfig = serde_json::from_str(json)
                .map_err(|e| IcodeError::validation(format!("解析 timeout_json 失败: {}", e)))?;
            builder = builder.connect_timeout(std::time::Duration::from_millis(cfg.connection));
            builder = builder.timeout(std::time::Duration::from_millis(cfg.response));
        }

        // 供应商级代理（含 global 回退到全局代理 / 直连），与网关转发策略一致
        builder = apply_provider_proxy(builder, provider.proxy_json.as_deref())
            .map_err(|e| IcodeError::validation(format!("构造 OAuth HTTP 客户端失败: {}", e)))?;

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

        log::info!("请求 device code 端点: url={}, client_id={}", device_url, client_id);
        Log::info(&format!("[OAuth] 请求 device code | url={}", device_url));

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
            log::error!("device code 端点返回错误 [{}]: {}", status, text);
            Log::error(&format!("[OAuth] device code 端点返回错误 [{}]", status));
            return Err(IcodeError::gateway(format!(
                "device code 端点返回错误 [{}]: {}",
                status, text
            )));
        }

        log::debug!("device code 原始响应: {}", text);
        let device_resp = Self::parse_device_code_response(&text)?;

        log::info!(
            "device code 请求成功: verification_uri={}, expires_in={:?}, interval={:?}",
            device_resp.verification_uri,
            device_resp.expires_in,
            device_resp.interval
        );
        Log::info(&format!(
            "[OAuth] device code 请求成功 | verification_uri={}",
            device_resp.verification_uri
        ));

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

        log::info!("轮询 device token: url={}", token_url);
        Log::info(&format!("[OAuth] 轮询 device token | url={}", token_url));

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

        log::debug!("device token 原始响应: status={}, body={}", status, text);

        if !status.is_success() {
            // 设备码流程中，pending / slow_down 也可能以 400/428 返回
            if let Some(error) = Self::extract_device_token_error(&text) {
                match error.as_str() {
                    "authorization_pending" | "slow_down" => {
                        log::debug!("device token 等待用户授权: {}", error);
                        return Ok(None);
                    }
                    "expired_token" => {
                        log::warn!("device token 已过期");
                        Log::warn("[OAuth] device token 已过期，请重新发起授权");
                        return Err(IcodeError::validation("设备码已过期，请重新发起授权"))
                    }
                    "access_denied" => {
                        log::warn!("device token 用户拒绝授权");
                        Log::warn("[OAuth] 用户拒绝授权");
                        return Err(IcodeError::validation("用户拒绝授权"))
                    }
                    _ => {}
                }
            }
            log::error!("device token 端点返回错误 [{}]: {}", status, text);
            Log::error(&format!("[OAuth] device token 端点返回错误 [{}]", status));
            return Err(IcodeError::gateway(format!(
                "device token 端点返回错误 [{}]: {}",
                status, text
            )));
        }

        // 某些实现（如 GitHub）在 pending 时仍返回 200 且 body 含 error
        if let Some(error) = Self::extract_device_token_error(&text) {
            match error.as_str() {
                "authorization_pending" | "slow_down" => {
                    log::debug!("device token 等待用户授权: {}", error);
                    return Ok(None);
                }
                "expired_token" => {
                    log::warn!("device token 已过期");
                    Log::warn("[OAuth] device token 已过期，请重新发起授权");
                    return Err(IcodeError::validation("设备码已过期，请重新发起授权"))
                }
                "access_denied" => {
                    log::warn!("device token 用户拒绝授权");
                    Log::warn("[OAuth] 用户拒绝授权");
                    return Err(IcodeError::validation("用户拒绝授权"))
                }
                _ => {}
            }
        }

        let token_resp = Self::parse_token_response(&text)?;

        log::info!(
            "device token 轮询成功: token_type={:?}, scope={:?}, expires_in={:?}",
            token_resp.token_type,
            token_resp.scope,
            token_resp.expires_in
        );
        Log::info("[OAuth] device token 轮询成功");

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
        provider_id: &str,
        provider_name: &str,
    ) -> IcodeResult<(String, oneshot::Receiver<IcodeResult<CallbackQuery>>)> {
        let (bind_addr, redirect_path, redirect_uri, fixed_port) = match redirect_uri {
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
                (bind_addr, path.to_string(), redirect_uri, Some(port))
            }
            _ => {
                let bind_addr = "127.0.0.1:0".to_string();
                let redirect_path = "/callback".to_string();
                (bind_addr, redirect_path, String::new(), None)
            }
        };

        log::info!(
            "启动 OAuth 回调服务器: bind_addr={}, provider={}",
            bind_addr,
            provider_name
        );

        let listener = tokio::net::TcpListener::bind(&bind_addr)
            .await
            .map_err(|e| bind_error_to_icode(e, fixed_port))?;
        let addr = listener
            .local_addr()
            .map_err(|e| IcodeError::internal(format!("获取回调服务器地址失败: {}", e)))?;

        // 动态端口时需要根据实际绑定地址拼接 redirect_uri
        let is_fixed_port = fixed_port.is_some();
        let redirect_uri = if redirect_uri.is_empty() {
            format!("http://{}/callback", addr)
        } else {
            redirect_uri
        };
        let port = addr.port();

        log::info!(
            "OAuth 回调服务器已启动: port={}, redirect_uri={}, provider={}",
            port,
            redirect_uri,
            provider_name
        );
        Log::info(&format!(
            "OAuth 回调服务器已启动 (端口 {}, 供应商 {})",
            port, provider_name
        ));

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

        // 注册到全局回调服务器注册表
        let entry_id = uuid::Uuid::new_v4().to_string();
        global_registry().register(CallbackServerEntry {
            id: entry_id.clone(),
            provider_id: provider_id.to_string(),
            provider_name: provider_name.to_string(),
            port,
            redirect_uri: redirect_uri.clone(),
            is_fixed_port,
            started_at: chrono::Utc::now().timestamp(),
            shutdown_tx: shutdown_tx.clone(),
        });

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
        let registry_id = entry_id.clone();
        tokio::spawn(async move {
            if let Err(e) = server.await {
                log::error!("OAuth 回调服务器异常: {}", e);
            }
            // 服务器结束后从注册表移除
            global_registry().unregister(&registry_id);
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
        provider_name: &str,
    ) -> IcodeResult<String> {
        let (bind_addr, redirect_path, redirect_uri, fixed_port) = match redirect_uri {
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
                (bind_addr, path.to_string(), redirect_uri, Some(port))
            }
            _ => {
                let bind_addr = "127.0.0.1:0".to_string();
                let redirect_path = "/callback".to_string();
                (bind_addr, redirect_path, String::new(), None)
            }
        };

        log::info!(
            "启动 OAuth 回调服务器(事件模式): bind_addr={}, provider={}",
            bind_addr,
            provider_name
        );

        let listener = tokio::net::TcpListener::bind(&bind_addr)
            .await
            .map_err(|e| bind_error_to_icode(e, fixed_port))?;
        let addr = listener
            .local_addr()
            .map_err(|e| IcodeError::internal(format!("获取回调服务器地址失败: {}", e)))?;

        // 动态端口时需要根据实际绑定地址拼接 redirect_uri
        let is_fixed_port = fixed_port.is_some();
        let redirect_uri = if redirect_uri.is_empty() {
            format!("http://{}/callback", addr)
        } else {
            redirect_uri
        };
        let port = addr.port();

        log::info!(
            "OAuth 回调服务器已启动(事件模式): port={}, redirect_uri={}, provider={}",
            port,
            redirect_uri,
            provider_name
        );
        Log::info(&format!(
            "OAuth 回调服务器已启动 (端口 {}, 供应商 {})",
            port, provider_name
        ));

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

        // 注册到全局回调服务器注册表
        let entry_id = uuid::Uuid::new_v4().to_string();
        global_registry().register(CallbackServerEntry {
            id: entry_id.clone(),
            provider_id: provider_id.to_string(),
            provider_name: provider_name.to_string(),
            port,
            redirect_uri: redirect_uri.clone(),
            is_fixed_port,
            started_at: chrono::Utc::now().timestamp(),
            shutdown_tx: shutdown_tx.clone(),
        });

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
        let registry_id = entry_id.clone();
        tokio::spawn(async move {
            if let Err(e) = server.await {
                log::error!("OAuth 回调服务器异常: {}", e);
            }
            // 服务器结束后从注册表移除
            global_registry().unregister(&registry_id);
        });

        Ok(redirect_uri)
    }

    /// 统一解析 device code 响应，支持 JSON 与 `application/x-www-form-urlencoded`
    ///
    /// GitHub 的 device code 端点按 OAuth 2.0 标准返回 form-urlencoded，
    /// 其他供应商（Google 等）返回 JSON，因此需要同时兼容两种格式。
    fn parse_device_code_response(text: &str) -> IcodeResult<DeviceCodeResponse> {
        match serde_json::from_str::<DeviceCodeResponse>(text) {
            Ok(resp) => Ok(resp),
            Err(json_err) => {
                log::debug!(
                    "device code 响应不是 JSON，尝试 form-urlencoded 解析: {}",
                    json_err
                );
                Self::parse_url_encoded_device_code_response(text)
            }
        }
    }

    /// 解析 URL 编码的 device code 响应
    fn parse_url_encoded_device_code_response(text: &str) -> IcodeResult<DeviceCodeResponse> {
        let mut device_code: Option<String> = None;
        let mut user_code: Option<String> = None;
        let mut verification_uri: Option<String> = None;
        let mut verification_uri_complete: Option<String> = None;
        let mut expires_in: Option<i64> = None;
        let mut interval: Option<i64> = None;

        for (key, value) in url::form_urlencoded::parse(text.as_bytes()) {
            match key.as_ref() {
                "device_code" => device_code = Some(value.into_owned()),
                "user_code" => user_code = Some(value.into_owned()),
                "verification_uri" | "verification_url" => {
                    verification_uri = Some(value.into_owned())
                }
                "verification_uri_complete" | "verification_url_complete" => {
                    verification_uri_complete = Some(value.into_owned())
                }
                "expires_in" => expires_in = value.parse().ok(),
                "interval" => interval = value.parse().ok(),
                _ => {}
            }
        }

        let device_code = device_code.ok_or_else(|| {
            IcodeError::gateway("解析 device code 响应失败: form-urlencoded 中缺少 device_code")
        })?;
        let user_code = user_code.ok_or_else(|| {
            IcodeError::gateway("解析 device code 响应失败: form-urlencoded 中缺少 user_code")
        })?;
        let verification_uri = verification_uri.ok_or_else(|| {
            IcodeError::gateway(
                "解析 device code 响应失败: form-urlencoded 中缺少 verification_uri",
            )
        })?;

        Ok(DeviceCodeResponse {
            device_code,
            user_code,
            verification_uri,
            verification_uri_complete,
            expires_in,
            interval,
        })
    }

    /// 统一解析 token 响应，支持 JSON 与 `application/x-www-form-urlencoded`
    ///
    /// GitHub 的 access token 端点按 OAuth 2.0 标准返回 form-urlencoded，
    /// 其他供应商（Google、OpenAI 等）返回 JSON，因此需要同时兼容两种格式。
    fn parse_token_response(text: &str) -> IcodeResult<TokenResponse> {
        match serde_json::from_str::<TokenResponse>(text) {
            Ok(resp) => Ok(resp),
            Err(json_err) => {
                log::debug!(
                    "token 响应不是 JSON，尝试 form-urlencoded 解析: {}",
                    json_err
                );
                Self::parse_url_encoded_token_response(text)
            }
        }
    }

    /// 解析 URL 编码的 token 响应
    fn parse_url_encoded_token_response(text: &str) -> IcodeResult<TokenResponse> {
        let mut access_token: Option<String> = None;
        let mut token_type: Option<String> = None;
        let mut refresh_token: Option<String> = None;
        let mut expires_in: Option<i64> = None;
        let mut scope: Option<String> = None;

        for (key, value) in url::form_urlencoded::parse(text.as_bytes()) {
            match key.as_ref() {
                "access_token" => access_token = Some(value.into_owned()),
                "token_type" => token_type = Some(value.into_owned()),
                "refresh_token" => refresh_token = Some(value.into_owned()),
                "expires_in" => expires_in = value.parse().ok(),
                "scope" => scope = Some(value.into_owned()),
                _ => {}
            }
        }

        let access_token = access_token.ok_or_else(|| {
            IcodeError::gateway("解析 token 响应失败: form-urlencoded 中缺少 access_token")
        })?;

        Ok(TokenResponse {
            access_token,
            token_type,
            refresh_token,
            expires_in,
            scope,
        })
    }

    /// 从 device token 响应中提取 OAuth 错误码
    ///
    /// 兼容 JSON 与 form-urlencoded 两种格式，用于识别 `authorization_pending` /
    /// `slow_down` / `expired_token` / `access_denied` 等设备码流程特有情境。
    fn extract_device_token_error(text: &str) -> Option<String> {
        // 先尝试 JSON
        if let Ok(body) = serde_json::from_str::<serde_json::Value>(text) {
            if let Some(error) = body.get("error").and_then(|v| v.as_str()) {
                return Some(error.to_string());
            }
        }
        // 再尝试 form-urlencoded
        for (key, value) in url::form_urlencoded::parse(text.as_bytes()) {
            if key == "error" {
                return Some(value.into_owned());
            }
        }
        None
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

        let token_resp = Self::parse_token_response(&text)?;

        log::info!(
            "token 换取成功: token_type={:?}, scope={:?}, expires_in={:?}",
            token_resp.token_type,
            token_resp.scope,
            token_resp.expires_in
        );
        Log::info("[OAuth] token 换取成功");

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
    log::info!("收到 OAuth 回调: state={:?}", query.state);

    let result = if let Some(ref error) = query.error {
        let msg = query
            .error_description
            .clone()
            .unwrap_or_else(|| error.clone());
        log::error!("OAuth 授权回调返回错误: {} ({})", error, msg);
        Log::error(&format!("OAuth 授权失败: {} ({})", error, msg));
        Err(IcodeError::gateway(format!(
            "OAuth 授权失败: {} ({})",
            error, msg
        )))
    } else if query.state.as_deref() != Some(state.expected_state.as_str()) {
        log::warn!(
            "OAuth 回调 state 不匹配: 期望={}, 实际={}",
            state.expected_state,
            query.state.as_deref().unwrap_or("")
        );
        Err(IcodeError::validation("OAuth state 不匹配"))
    } else if query.code.is_none() {
        log::warn!("OAuth 回调缺少 code 参数");
        Err(IcodeError::validation("OAuth 回调缺少 code 参数"))
    } else {
        log::info!("OAuth 回调成功，已收到授权码");
        Log::info("OAuth 授权回调成功，已收到授权码");
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
    log::info!(
        "收到 OAuth 回调(事件模式): provider_id={}, state={:?}",
        state.provider_id,
        query.state
    );

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

    if let Some(ref error) = query.error {
        let msg = query.error_description.as_deref().unwrap_or("");
        log::error!("OAuth 授权回调返回错误: {} ({})", error, msg);
        Log::error(&format!("OAuth 授权失败: {} ({})", error, msg));
    } else if query.code.is_some() {
        log::info!("OAuth 回调成功(事件模式): provider_id={}", state.provider_id);
        Log::info(&format!(
            "OAuth 授权回调成功 (供应商 {})",
            state.provider_id
        ));
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
