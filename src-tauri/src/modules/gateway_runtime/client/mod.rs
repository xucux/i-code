//! # 上游供应商客户端抽象层
//!
//! 借鉴参考项目 `vscode-unify-chat-provider/src/client/` 的 `ApiProvider` 统一接口思想，
//! 将不同供应商的协议差异封装到独立的 Client 实现中。
//!
//! ## 核心设计
//!
//! - `UpstreamClient` trait：定义统一的请求执行入口。
//! - `ClientFactory`：根据 `provider.provider_type` 创建对应 Client。
//! - `UpstreamRequest` / `UpstreamResponse`：统一请求/响应封装，屏蔽协议细节。
//! - `ClientError`：Client 层错误类型，统一转换为 `IcodeError`。
//!
//! ## 当前实现
//!
//! - `OpenAiChatClient`：OpenAI Chat Completions 兼容协议（REST + SSE）。
//! - `AnthropicClient`：Anthropic Messages API（REST + SSE）。
//! - `WebSocketClient`：WebSocket 协议预留抽象（OpenAI Responses / Codex 待实现）。
//!
//! ## 扩展方式
//!
//! 1. 新建 `xxx_client.rs` 实现 `UpstreamClient`。
//! 2. 在 `ClientFactory::create` 中注册新类型。
//! 3. 在 `UpstreamClient::execute` 的 match 分支中调用新方法（如需要单独协议入口）。

use std::fmt;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::header::HeaderMap;
use serde_json::Value;

use crate::error::IcodeError;
use crate::modules::ai_gateway::types::Provider;
use crate::modules::shared::{ProviderProxyConfig, ProviderProxyType, TimeoutConfig};
// LoggerServiceHandle 已随响应转换迁移至 forwarding/response_handler，不再在此处使用

pub mod anthropic_client;
pub mod openai_chat_client;
pub mod websocket_client;

use anthropic_client::AnthropicClient;
use openai_chat_client::OpenAiChatClient;
use websocket_client::WebSocketClient;

/// 上游请求协议类型
///
/// 网关对外提供 OpenAI / Anthropic 两套兼容接口，转发时按原协议路由到对应 Client。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamProtocol {
    /// OpenAI `/v1/chat/completions`
    ChatCompletions,
    /// Anthropic `/v1/messages`
    AnthropicMessages,
}

impl fmt::Display for UpstreamProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChatCompletions => write!(f, "chat/completions"),
            Self::AnthropicMessages => write!(f, "messages"),
        }
    }
}

/// 上游请求上下文
///
/// 由 `upstream.rs` 解析并构造，包含目标供应商、真实模型 ID、认证信息等。
#[derive(Debug, Clone)]
pub struct UpstreamContext {
    /// 目标供应商（已解析为真实供应商，虚拟供应商场景下为路由目标供应商）
    pub provider: Provider,
    /// 网关暴露模型记录（可能为空，直连模式或虚拟供应商）
    pub gateway_model_id: Option<String>,
    /// 真实模型 ID（已去除 provider_slug / virtual_alias 前缀）
    pub upstream_model_id: String,
    /// 是否通过虚拟供应商路由
    #[expect(dead_code)]
    pub is_virtual: bool,
    /// 当前使用的路由索引（仅虚拟供应商有意义，保留用于后续日志扩展）
    #[allow(dead_code)]
    pub route_index: usize,
    /// 本次请求唯一标识
    pub request_id: String,
    /// 是否为流式请求
    pub is_stream: bool,
    /// 已解析的认证配置（含 Secret 明文）
    pub auth_config: Option<crate::modules::ai_gateway::types::AuthConfig>,
}

/// 上游请求封装
#[derive(Debug, Clone)]
pub struct UpstreamRequest {
    /// 请求协议
    pub protocol: UpstreamProtocol,
    /// 请求体（已替换为真实 upstream_model_id）
    pub body: Value,
    /// 流式标志
    pub is_stream: bool,
}

/// 上游响应封装
///
/// Client 层返回统一响应，由 `upstream.rs` 转换为 axum Response。
pub enum UpstreamResponse {
    /// 流式响应（SSE 或 WebSocket 透传）
    ///
    /// 直接持有 `reqwest::Response`，调用方通过 `response.bytes_stream()` 透传。
    Streaming {
        /// reqwest 响应对象（含流式 body）
        response: reqwest::Response,
        /// 上游协议类型（用于 SSE 流中 usage 事件解析）
        protocol: UpstreamProtocol,
    },
    /// 完整响应（非流式）
    Complete {
        /// 上游响应状态码
        status: reqwest::StatusCode,
        /// 上游响应头
        headers: HeaderMap,
        /// 响应体字节
        body: Vec<u8>,
    },
}

/// Client 层错误
#[derive(Debug)]
#[allow(dead_code)]
pub enum ClientError {
    /// 不支持的供应商类型
    UnsupportedProvider(String),
    /// 不支持的协议组合
    UnsupportedProtocol {
        provider_type: String,
        protocol: UpstreamProtocol,
    },
    /// 认证信息缺失或无效
    AuthError(String),
    /// 请求构造失败
    BuildRequestError(String),
    /// 上游 HTTP 请求失败
    RequestFailed(String),
    /// 读取响应体失败
    ReadBodyError(String),
    /// 其他错误
    Other(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProvider(t) => write!(f, "不支持的供应商类型: {}", t),
            Self::UnsupportedProtocol { provider_type, protocol } => {
                write!(f, "供应商 {} 不支持协议 {}", provider_type, protocol)
            }
            Self::AuthError(msg) => write!(f, "认证错误: {}", msg),
            Self::BuildRequestError(msg) => write!(f, "构造请求失败: {}", msg),
            Self::RequestFailed(msg) => write!(f, "上游请求失败: {}", msg),
            Self::ReadBodyError(msg) => write!(f, "读取响应体失败: {}", msg),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ClientError {}

/// 将底层错误格式化为包含完整 `source()` 链的字符串
///
/// reqwest 等库的 `Display::to_string()` 只输出顶层错误描述，底层原因（如 DNS 解析失败、
/// TLS 握手错误、连接被拒绝）需要通过 `std::error::Error::source()` 链逐层展开。
/// 此函数用于上游请求失败时在日志中保留完整诊断信息，便于区分网络层错误与应用层错误。
pub fn format_client_error(err: &dyn std::error::Error) -> String {
    let mut msg = err.to_string();
    let mut source = err.source();
    while let Some(s) = source {
        msg.push_str(&format!(" | caused by: {}", s));
        source = s.source();
    }
    msg
}

impl From<ClientError> for IcodeError {
    fn from(err: ClientError) -> Self {
        match err {
            ClientError::UnsupportedProvider(_) | ClientError::UnsupportedProtocol { .. } => {
                IcodeError::validation(err.to_string())
            }
            ClientError::AuthError(_) => IcodeError::validation(err.to_string()),
            ClientError::RequestFailed(msg) => IcodeError::internal(msg),
            ClientError::ReadBodyError(msg) => IcodeError::internal(msg),
            _ => IcodeError::internal(err.to_string()),
        }
    }
}

/// 上游客户端统一 trait
///
/// 不同供应商实现此 trait，在 `execute` 中完成请求构造、发送、响应解析。
#[async_trait::async_trait]
pub trait UpstreamClient: Send + Sync {
    /// 执行上游请求，返回统一响应
    async fn execute(
        &self,
        ctx: &UpstreamContext,
        request: UpstreamRequest,
    ) -> Result<UpstreamResponse, ClientError>;

    /// 返回客户端对应的供应商类型标识
    #[expect(dead_code)]
    fn provider_type(&self) -> &'static str;
}

/// 根据 provider_type 创建对应 Client
pub struct ClientFactory;

impl ClientFactory {
    /// 根据 provider_type 创建对应 Client 实例
    ///
    /// 需要与 `ai_gateway::types::ProviderType` 及 `docs/database.md` §5.1 保持一致。
    /// 历史遗留的短名称（如 `openai`、`openai-compatible`）继续保留以兼容旧数据。
    pub fn create(provider_type: &str) -> Result<Box<dyn UpstreamClient>, ClientError> {
        match provider_type {
            // OpenAI Chat Completions 兼容协议
            "openai" | "openai-compatible" | "openai-chat-completion"
            | "deepseek" | "moonshot-ai" | "kimi-code" | "newapi" | "siliconflow"
            | "aihubmix" | "openrouter" | "minimax" | "xai-grok-build" | "ollama"
            | "custom" | "codex" | "gemini-cli" | "antigravity" => {
                Ok(Box::new(OpenAiChatClient::new()))
            }
            // Anthropic Messages API
            "anthropic" | "claude-relay-service" => {
                Ok(Box::new(AnthropicClient::new()))
            }
            // WebSocket 协议（OpenAI Responses / Codex，当前为未实现占位，调用即失败）
            "websocket" | "openai-responses" | "openai-codex" => {
                Ok(Box::new(WebSocketClient::new(provider_type)))
            }
            _ => Err(ClientError::UnsupportedProvider(provider_type.to_string())),
        }
    }
}

/// 构造上游请求 URL
///
/// 将供应商 `base_url` 与请求路径拼接为完整 URL。
///
/// ### 行为
/// - `use_raw_base_url = true`：仅去除尾部斜杠，`base_url` 原样保留，不处理 `/v1` 路径。
/// - `use_raw_base_url = false`：去除尾部斜杠，并自动补全 `/v1` 路径：
///   - 若 `base_url` 路径中已包含 `/v1`，或请求路径以 `/v1` 开头，则不再追加。
///   - 否则在 `base_url` 末尾追加 `/v1`。
pub fn build_upstream_url(provider: &Provider, path: &str) -> Result<String, ClientError> {
    let base = if provider.use_raw_base_url {
        // 原始模式：仅去尾部斜杠，不处理 /v1
        provider.base_url.trim_end_matches('/').to_string()
    } else {
        // 自动模式：去尾部斜杠 + 自动补全 /v1
        let url = provider.base_url.trim_end_matches('/');
        if url.contains("/v1") || path.starts_with("/v1") {
            url.to_string()
        } else {
            format!("{}/v1", url)
        }
    };
    Ok(format!("{}{}", base, path))
}

/// 读取上游完整响应体（非流式）
///
/// 返回 `(status, headers, body, content_type)`
pub async fn read_response_body(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<(reqwest::StatusCode, HeaderMap, Vec<u8>, String), ClientError> {
    let status = response.status();
    let headers = response.headers().clone();
    let content_type = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    let body = response
        .bytes()
        .await
        .map_err(|e| ClientError::ReadBodyError(e.to_string()))?;
    let body = if body.len() > max_bytes {
        body.slice(..max_bytes)
    } else {
        body
    };
    Ok((status, headers, body.to_vec(), content_type))
}

/// 截断响应体字符串
pub fn truncate_body(body: &str, max_len: usize) -> String {
    if body.len() <= max_len {
        body.to_string()
    } else {
        format!("{}...（截断，原始 {} 字节）", &body[..max_len], body.len())
    }
}

// ===== 统一 HTTP 客户端构造层 =====

/// 非流式响应体最大字节数（32 MiB）
///
/// 用于防止异常上游返回超大 body 导致 OOM。流式响应由上层 SSE 透传，
/// 不受此上限约束。
pub const MAX_RESPONSE_BODY_BYTES: usize = 32 * 1024 * 1024;

/// 默认 User-Agent
const DEFAULT_USER_AGENT: &str = concat!("i-code-gateway/", env!("CARGO_PKG_VERSION"));

/// 进程级默认 HTTP 客户端
///
/// 当供应商未配置自定义 timeout / proxy 时复用此实例，避免每请求
/// `Client::new()` 导致的连接池浪费。reqwest::Client 内部自带连接池与
/// Keep-Alive，跨请求复用是性能关键。
static DEFAULT_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// 取进程级默认 HTTP 客户端
///
/// 首次调用时读取全局代理配置并应用；之后复用同一实例。
/// 若用户修改全局代理设置后需重启应用才能生效。
fn default_http_client() -> reqwest::Client {
    DEFAULT_HTTP_CLIENT
        .get_or_init(|| {
            let builder = reqwest::Client::builder().user_agent(DEFAULT_USER_AGENT);
            // 应用全局代理配置
            let builder = crate::modules::shared::apply_global_proxy(builder);
            builder.build().expect("构造默认 HTTP 客户端失败")
        })
        .clone()
}

/// 根据供应商配置选择 HTTP 客户端
///
/// - 供应商未配置 `timeout_json` / `proxy_json`：复用进程级默认客户端（推荐路径）。
/// - 否则按其配置构造独立客户端。
///
/// `is_stream` 决定是否应用响应总超时：流式请求不在 Client 层设总超时
/// （SSE 帧间隔不可控），仅设连接超时；非流式请求同时设连接与响应超时。
pub fn http_client_for(
    provider: &Provider,
    is_stream: bool,
) -> Result<reqwest::Client, ClientError> {
    let has_custom = provider.timeout_json.is_some() || provider.proxy_json.is_some();
    if !has_custom {
        return Ok(default_http_client());
    }
    build_http_client(provider, is_stream)
}

/// 按供应商配置构造带 timeout / proxy / User-Agent 的 HTTP 客户端
fn build_http_client(
    provider: &Provider,
    is_stream: bool,
) -> Result<reqwest::Client, ClientError> {
    let mut builder = reqwest::Client::builder().user_agent(DEFAULT_USER_AGENT);

    // 超时：连接超时始终生效；响应总超时仅对非流式生效
    if let Some(timeout) = parse_timeout(provider)? {
        builder = builder.connect_timeout(Duration::from_millis(timeout.connection));
        if !is_stream {
            builder = builder.timeout(Duration::from_millis(timeout.response));
        }
    }

    // 代理
    builder = apply_proxy(builder, provider)?;

    builder
        .build()
        .map_err(|e| ClientError::BuildRequestError(format!("构造 HTTP 客户端失败: {}", e)))
}

/// 解析 `provider.timeout_json` 为 `TimeoutConfig`
fn parse_timeout(provider: &Provider) -> Result<Option<TimeoutConfig>, ClientError> {
    provider
        .timeout_json
        .as_deref()
        .map(|s| {
            serde_json::from_str::<TimeoutConfig>(s)
                .map_err(|e| ClientError::BuildRequestError(format!("解析 timeout_json 失败: {}", e)))
        })
        .transpose()
}

/// 按 `provider.proxy_json` 配置代理
///
/// 供应商级代理策略：
/// - `global`：读取全局代理配置并应用；若全局代理未启用则沿用 reqwest 默认行为。
/// - `direct`：显式 `no_proxy()` 禁用代理。
/// - `socks` / `http`：构造 `reqwest::Proxy::all(url)`；reqwest 根据 URL scheme
///   自动选择 SOCKS5 或 HTTP 代理协议。
fn apply_proxy(
    builder: reqwest::ClientBuilder,
    provider: &Provider,
) -> Result<reqwest::ClientBuilder, ClientError> {
    let Some(json) = provider.proxy_json.as_deref() else {
        // 未配置：回退到全局代理
        return Ok(crate::modules::shared::apply_global_proxy(builder));
    };
    let cfg: ProviderProxyConfig = serde_json::from_str(json)
        .map_err(|e| ClientError::BuildRequestError(format!("解析 proxy_json 失败: {}", e)))?;
    match cfg.proxy_type {
        ProviderProxyType::Global => {
            // 读取全局代理配置并应用
            Ok(crate::modules::shared::apply_global_proxy(builder))
        }
        ProviderProxyType::Direct => Ok(builder.no_proxy()),
        ProviderProxyType::Socks | ProviderProxyType::Http => {
            let url = cfg
                .url
                .as_deref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ClientError::BuildRequestError("socks/http 代理缺少 url".into()))?;
            let proxy = reqwest::Proxy::all(url)
                .map_err(|e| ClientError::BuildRequestError(format!("构造代理失败: {}", e)))?;
            Ok(builder.proxy(proxy))
        }
    }
}

// ===== 响应转换 =====

/// SSE 流中提取的 usage 数据
///
/// 从流的最后一个事件中解析得到，供调用记录更新使用。
#[derive(Debug, Default, Clone)]
pub struct SseUsageData {
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cached_tokens: Option<i64>,
    pub cache_hit: Option<bool>,
}

/// SSE 流 usage 累加器
///
/// 流式透传过程中，解析每个 SSE 事件并累积 usage 数据。
/// 流结束后由调用方（upstream.rs）读取并更新调用记录。
pub type SseUsageAccumulator = Arc<std::sync::Mutex<SseUsageData>>;

/// 创建新的 SSE usage 累加器
pub fn new_sse_usage_accumulator() -> SseUsageAccumulator {
    Arc::new(std::sync::Mutex::new(SseUsageData::default()))
}

// ===== 响应转换已迁移至 forwarding/response_handler =====
// 原 build_axum_response_with_usage / build_sse_response / parse_sse_event_for_usage /
// build_json_response 已迁移到 forwarding 子模块，统一处理流式 / 非流式响应与
// usage 提取，避免 client 层承担 axum Response 构造职责。
