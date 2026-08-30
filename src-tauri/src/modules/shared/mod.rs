//! # 跨模块共享类型
//!
//! 提供被多个业务模块依赖的通用配置类型，对应前端 `src/modules/ai-gateway/types.ts`
//! 中的 `ProxyConfig` / `RetryConfig` / `TimeoutConfig`。
//!
//! ## 设计原则
//!
//! - 本模块**零业务依赖**，仅依赖 `serde` / `serde_json`。
//! - 其他业务模块（`settings` / `ai_gateway` / `cli_management` 等）可自由引用此处类型。
//! - 序列化字段统一使用 camelCase，与前端 JSON 字段名一致。
//!
//! ## 为何独立模块
//!
//! `ProxyConfig` / `RetryConfig` / `TimeoutConfig` 同时被 `settings`（全局默认值）
//! 与 `ai_gateway`（供应商级覆盖）使用。若定义在任一业务模块中，会导致反向依赖。
//! 放在 `shared` 层符合 `docs/development.md` §4 中的依赖方向：
//! `core/shared → theme/i18n/secret/db/balance/logger/backup → settings/ai-gateway/...`

use serde::{Deserialize, Serialize};

/// 全局代理配置
///
/// 对应 `docs/database.md` §5.3，用于 `app_settings.global_proxy_json`。
/// 与供应商级代理 `ProviderProxyConfig` 区分：全局代理支持「直连」「系统代理」
/// 「HTTP 代理」「SOCKS 代理」四种策略，用于应用级网络设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
    /// 代理类型
    /// - `direct`：直连，不使用代理
    /// - `system`：使用系统代理设置（读取 HTTP_PROXY / HTTPS_PROXY 环境变量）
    /// - `http`：HTTP 代理（URL 可含用户名:密码，如 `http://user:pass@host:port`）
    /// - `socks`：SOCKS5 代理（URL 可含用户名:密码，如 `socks5://user:pass@host:port`）
    #[serde(rename = "type")]
    pub proxy_type: ProxyType,
    /// 代理 URL（仅 `http` / `socks` 类型生效）
    /// 支持在 URL 中包含认证信息：`http://user:pass@host:port`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// 不走代理的主机列表（NO_PROXY 环境变量等价物）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub no_proxy: Vec<String>,
}

/// 全局代理类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyType {
    /// 直连
    Direct,
    /// 系统代理
    System,
    /// HTTP 代理
    Http,
    /// SOCKS5 代理
    Socks,
}

impl ProxyConfig {
    /// 将全局代理配置应用到 `reqwest::ClientBuilder`
    ///
    /// - `direct`：显式禁用代理
    /// - `system`：沿用 reqwest 默认行为（读取系统环境变量代理）
    /// - `http` / `socks`：构造 `reqwest::Proxy::all(url)`，URL 可含用户名:密码
    pub fn apply_to_client_builder(
        &self,
        mut builder: reqwest::ClientBuilder,
    ) -> reqwest::ClientBuilder {
        match self.proxy_type {
            ProxyType::Direct => {
                builder = builder.no_proxy();
                builder
            }
            ProxyType::System => {
                // 沿用 reqwest 默认行为（读取 HTTP_PROXY / HTTPS_PROXY 环境变量）
                builder
            }
            ProxyType::Http | ProxyType::Socks => {
                if let Some(url) = self.url.as_deref().filter(|s| !s.is_empty()) {
                    if let Ok(proxy) = reqwest::Proxy::all(url) {
                        builder = builder.proxy(proxy);
                    }
                }
                builder
            }
        }
    }

    /// 将全局代理配置应用到 `reqwest::blocking::ClientBuilder`
    ///
    /// 与 `apply_to_client_builder` 相同逻辑，用于同步阻塞客户端。
    /// 当前仅由脚本运行时的代理解析间接使用；保留供未来阻塞式客户端场景使用。
    #[allow(dead_code)]
    pub fn apply_to_blocking_client_builder(
        &self,
        mut builder: reqwest::blocking::ClientBuilder,
    ) -> reqwest::blocking::ClientBuilder {
        match self.proxy_type {
            ProxyType::Direct => {
                builder = builder.no_proxy();
                builder
            }
            ProxyType::System => builder,
            ProxyType::Http | ProxyType::Socks => {
                if let Some(url) = self.url.as_deref().filter(|s| !s.is_empty()) {
                    if let Ok(proxy) = reqwest::Proxy::all(url) {
                        builder = builder.proxy(proxy);
                    }
                }
                builder
            }
        }
    }

    /// 返回脱敏后的日志描述字符串，供业务 logger 记录代理配置变更
    ///
    /// 脱敏规则：代理 URL 中的 `user:pass@` 部分替换为 `***@`，
    /// 避免将代理认证信息写入日志（符合 AGENTS.md §9.5 安全约束）。
    /// 脱敏由模块级 `redact_proxy_url` 完成（与 `host_proxied_http` 共用）。
    ///
    /// # 示例
    /// - `http://user:pass@127.0.0.1:7890` → `type=http, url=http://***@127.0.0.1:7890`
    /// - `socks5://127.0.0.1:1080` → `type=socks, url=socks5://127.0.0.1:1080`
    /// - `direct` → `type=direct`
    pub fn to_log_string(&self) -> String {
        let type_str = match self.proxy_type {
            ProxyType::Direct => "direct",
            ProxyType::System => "system",
            ProxyType::Http => "http",
            ProxyType::Socks => "socks",
        };
        match self.url.as_deref().filter(|s| !s.is_empty()) {
            Some(url) => format!("type={}, url={}", type_str, redact_proxy_url(url)),
            None => format!("type={}", type_str),
        }
    }
}

/// 供应商级代理配置
///
/// 对应 `docs/database.md` §5.3，用于 `providers.proxy_json` 字段。
/// 与全局代理 `ProxyConfig` 区分：供应商代理支持「使用全局代理」「直连」
/// 「SOCKS 代理」「HTTP 代理」四种策略。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProxyConfig {
    /// 代理类型
    /// - `global`：使用应用全局代理设置
    /// - `direct`：直连，不使用代理
    /// - `socks`：SOCKS5 代理
    /// - `http`：HTTP 代理
    #[serde(rename = "type")]
    pub proxy_type: ProviderProxyType,
    /// 代理 URL（仅 `socks` / `http` 类型生效）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// 供应商级代理类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderProxyType {
    /// 使用全局代理设置
    Global,
    /// 直连
    Direct,
    /// SOCKS5 代理
    Socks,
    /// HTTP 代理
    Http,
}

/// 超时配置（毫秒）
///
/// 对应 `docs/database.md` §5.7。
/// 与 `app_settings.network_timeout_ms`（标量）不同，此结构区分连接超时与响应超时，
/// 用于供应商级精细控制。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeoutConfig {
    /// TCP 连接超时（毫秒）
    pub connection: u64,
    /// SSE 流响应超时（毫秒），每次收到数据块后重置
    pub response: u64,
}

/// 重试策略配置
///
/// 对应 `docs/database.md` §5.8。
/// 采用指数退避 + 抖动策略，避免雪崩。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryConfig {
    /// 最大重试次数（不含首次请求）
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// 初始退避延迟（毫秒），前端展示为「重试间隔」
    #[serde(default = "default_initial_delay_ms")]
    pub initial_delay_ms: u64,
    /// 最大退避延迟上限（毫秒）
    #[serde(default = "default_max_delay_ms")]
    pub max_delay_ms: u64,
    /// 退避倍率（每次延迟 = 上次延迟 × 倍率）
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
    /// 抖动因子（0.0-1.0），添加随机性避免同步重试
    #[serde(default = "default_jitter_factor")]
    pub jitter_factor: f64,
    /// 触发重试的 HTTP 状态码列表（如 429、500、502、503、504）
    #[serde(default = "default_status_codes", skip_serializing_if = "Vec::is_empty")]
    pub status_codes: Vec<u16>,
}

fn default_max_retries() -> u32 { 8 }
fn default_initial_delay_ms() -> u64 { 2000 }
fn default_max_delay_ms() -> u64 { 8000 }
fn default_backoff_multiplier() -> f64 { 2.0 }
fn default_jitter_factor() -> f64 { 0.2 }
fn default_status_codes() -> Vec<u16> { vec![429, 500, 502, 503, 504] }

impl Default for RetryConfig {
    /// 默认重试策略：8 次重试，初始间隔 2s，最大 8s，倍率 2.0，抖动 0.2
    fn default() -> Self {
        Self {
            max_retries: 8,
            initial_delay_ms: 2000,
            max_delay_ms: 8000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.2,
            status_codes: vec![429, 500, 502, 503, 504],
        }
    }
}

impl RetryConfig {
    /// 从供应商 `retry_json` 解析配置；JSON 为空或解析失败时返回默认值
    pub fn from_json(json: Option<&str>) -> Self {
        let Some(json_str) = json.filter(|s| !s.is_empty()) else {
            return Self::default();
        };
        match serde_json::from_str::<Self>(json_str) {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::warn!("解析 retry_json 失败，使用默认配置: {} | raw={}", e, json_str);
                Self::default()
            }
        }
    }

    /// 计算第 `attempt` 次重试（从 1 开始）的延迟毫秒数
    ///
    /// 采用指数退避 + 抖动：
    /// - 基础延迟 = `initial_delay_ms * backoff_multiplier^(attempt-1)`
    /// - 上限截断 = `max_delay_ms`
    /// - 抖动 = `delay * (1 - jitter_factor + rand(0..1) * 2 * jitter_factor)`
    pub fn retry_delay_ms(&self, attempt: u32) -> u64 {
        if attempt == 0 {
            return 0;
        }
        let exponent = (attempt - 1) as f64;
        let base = self.initial_delay_ms as f64 * self.backoff_multiplier.powf(exponent);
        let capped = base.min(self.max_delay_ms as f64);

        // 抖动：在 [delay*(1-j), delay*(1+j)] 范围内随机
        let jitter_range = capped * self.jitter_factor;
        let jitter = if jitter_range > 0.0 {
            // 简单的伪随机：使用线程本地时间纳秒做种子
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as f64)
                .unwrap_or(0.0);
            (nanos / 1_000_000_000.0) * 2.0 * jitter_range - jitter_range
        } else {
            0.0
        };

        (capped + jitter).max(0.0) as u64
    }
}

/// 从数据库读取全局代理配置并应用到 `reqwest::ClientBuilder`
///
/// 语义：
/// - 全局代理未启用（`global_proxy_enabled = false`）：**强制直连**（`no_proxy()`），
///   不再回落到 reqwest 默认行为（读取系统环境变量代理）。
/// - 全局代理已启用：按 `ProxyConfig` 应用（`direct` 直连 / `system` 环境变量 /
///   `http` / `socks`）。
///
/// 这样供应商代理策略为 `global` 时，若全局代理未启用，会**回退到直连**而非
/// 读取系统环境变量代理，符合「全局代理开关 = 应用级网络策略总开关」的语义。
pub fn apply_global_proxy(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    let settings = match crate::modules::settings::repository::find() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("[proxy] global | read app_settings failed | err={:?} → forced direct", e);
            return builder.no_proxy();
        }
    };
    if !settings.global_proxy_enabled {
        tracing::trace!("[proxy] global | enabled=false → forced direct (no_proxy)");
        return builder.no_proxy();
    }
    let Some(json) = settings.global_proxy_json.as_deref() else {
        tracing::trace!("[proxy] global | enabled=true but json=null → forced direct (no_proxy)");
        return builder.no_proxy();
    };
    let cfg: ProxyConfig = match serde_json::from_str(json) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("[proxy] global | parse json failed | err={:?} | raw={} → forced direct", e, json);
            return builder.no_proxy();
        }
    };
    tracing::trace!("[proxy] global | enabled=true | strategy={:?} | url={:?} | no_proxy={:?}",
        cfg.proxy_type, cfg.url, cfg.no_proxy);
    cfg.apply_to_client_builder(builder)
}

/// 从数据库读取全局代理配置并应用到 `reqwest::blocking::ClientBuilder`
///
/// 与 `apply_global_proxy` 相同逻辑，用于同步阻塞客户端（如 Rhai 脚本 HTTP 调用）。
/// 当前不再被脚本运行时直接调用（脚本改用 `proxied_http` 模块或 `http::set_proxy`），
/// 保留供未来阻塞式客户端场景使用。
#[allow(dead_code)]
pub fn apply_global_proxy_blocking(builder: reqwest::blocking::ClientBuilder) -> reqwest::blocking::ClientBuilder {
    let settings = match crate::modules::settings::repository::find() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("[proxy] global(blocking) | read app_settings failed | err={:?} → forced direct", e);
            return builder.no_proxy();
        }
    };
    if !settings.global_proxy_enabled {
        tracing::trace!("[proxy] global(blocking) | enabled=false → forced direct (no_proxy)");
        return builder.no_proxy();
    }
    let Some(json) = settings.global_proxy_json.as_deref() else {
        tracing::trace!("[proxy] global(blocking) | enabled=true but json=null → forced direct (no_proxy)");
        return builder.no_proxy();
    };
    let cfg: ProxyConfig = match serde_json::from_str(json) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("[proxy] global(blocking) | parse json failed | err={:?} | raw={} → forced direct", e, json);
            return builder.no_proxy();
        }
    };
    tracing::trace!("[proxy] global(blocking) | enabled=true | strategy={:?} | url={:?} | no_proxy={:?}",
        cfg.proxy_type, cfg.url, cfg.no_proxy);
    cfg.apply_to_blocking_client_builder(builder)
}

/// 从数据库读取全局代理配置
///
/// 返回 `Some(ProxyConfig)` 表示全局代理已启用且配置有效。
#[allow(dead_code)]
pub fn read_global_proxy() -> Option<ProxyConfig> {
    let settings = crate::modules::settings::repository::find().ok()?;
    if !settings.global_proxy_enabled {
        return None;
    }
    let json = settings.global_proxy_json.as_deref()?;
    serde_json::from_str(json).ok()
}

/// 将供应商级代理配置（`providers.proxy_json`）应用到 `reqwest::ClientBuilder`
///
/// 供应商代理策略：
/// - `None`（未配置）或 `global`：应用全局代理；若全局代理未启用则**强制直连**
///   （见 [`apply_global_proxy`]）。
/// - `direct`：显式 `no_proxy()`。
/// - `socks` / `http`：构造 `reqwest::Proxy::all(url)`。
///
/// 抽出到 `shared` 层，供 `ai_gateway`（模型拉取 / OAuth）与 `gateway_runtime`
/// （网关转发）共用，保证两条网络路径策略一致。
pub fn apply_provider_proxy(
    builder: reqwest::ClientBuilder,
    provider_proxy_json: Option<&str>,
) -> Result<reqwest::ClientBuilder, String> {
    let Some(json) = provider_proxy_json else {
        tracing::trace!("[proxy] provider | json=null → delegate to global");
        return Ok(apply_global_proxy(builder));
    };
    let cfg: ProviderProxyConfig = serde_json::from_str(json).map_err(|e| {
        tracing::error!("[proxy] provider | parse json failed | err={:?} | raw={}", e, json);
        format!("解析 proxy_json 失败: {}", e)
    })?;
    match cfg.proxy_type {
        ProviderProxyType::Global => {
            tracing::trace!("[proxy] provider | strategy=global → delegate to global");
            Ok(apply_global_proxy(builder))
        }
        ProviderProxyType::Direct => {
            tracing::trace!("[proxy] provider | strategy=direct → no_proxy");
            Ok(builder.no_proxy())
        }
        ProviderProxyType::Socks | ProviderProxyType::Http => {
            let url = cfg
                .url
                .as_deref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    tracing::error!("[proxy] provider | strategy={:?} | missing url", cfg.proxy_type);
                    "socks/http 代理缺少 url".to_string()
                })?;
            tracing::trace!("[proxy] provider | strategy={:?} | url={}", cfg.proxy_type, redact_proxy_url(url));
            let proxy = reqwest::Proxy::all(url).map_err(|e| {
                tracing::error!("[proxy] provider | strategy={:?} | build proxy failed | url={} | err={:?}",
                    cfg.proxy_type, redact_proxy_url(url), e);
                format!("构造代理失败: {}", e)
            })?;
            Ok(builder.proxy(proxy))
        }
    }
}

/// 脱敏代理 URL 中的认证信息（`user:pass@host` → `<redacted>@host`）
///
/// 代理 URL 常含明文凭据，写入 tauri-plugin-log 前需脱敏。
/// 仅处理 `scheme://userinfo@host` 形态；无 userinfo 则原样返回。
pub fn redact_proxy_url(url: &str) -> String {
    // 找 scheme://
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let after_scheme = &url[scheme_end + 3..];
    // userinfo 在第一个 '/' 之前、且含 '@'
    let host_start_in_after = after_scheme.find('/').unwrap_or(after_scheme.len());
    let authority = &after_scheme[..host_start_in_after];
    let Some(at) = authority.rfind('@') else {
        return url.to_string();
    };
    let host = &authority[at + 1..];
    format!("{}://<redacted>@{}", &url[..scheme_end], host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_config_serde() {
        let config = ProxyConfig {
            proxy_type: ProxyType::Http,
            url: Some("http://user:pass@127.0.0.1:7890".to_string()),
            no_proxy: vec!["localhost".to_string(), "127.0.0.1".to_string()],
        };
        let json = serde_json::to_string(&config).unwrap();
        // 字段名应为 camelCase / type
        assert!(json.contains("\"type\":\"http\""));
        assert!(json.contains("\"noProxy\""));
        assert!(json.contains("127.0.0.1:7890"));

        // 反序列化往返
        let back: ProxyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.proxy_type, ProxyType::Http);
        assert_eq!(back.no_proxy.len(), 2);
        assert!(back.url.unwrap().contains("user:pass@127.0.0.1:7890"));

        // SOCKS 代理
        let socks = ProxyConfig {
            proxy_type: ProxyType::Socks,
            url: Some("socks5://127.0.0.1:1080".to_string()),
            no_proxy: vec![],
        };
        let json = serde_json::to_string(&socks).unwrap();
        assert!(json.contains("\"type\":\"socks\""));
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 8);
        assert_eq!(config.initial_delay_ms, 2000);
        assert_eq!(config.status_codes.len(), 5);
        assert!(config.status_codes.contains(&429));
        assert!(config.status_codes.contains(&503));
    }

    #[test]
    fn test_retry_config_from_json() {
        // 空 JSON → 默认值
        let cfg = RetryConfig::from_json(None);
        assert_eq!(cfg.max_retries, 8);

        let cfg = RetryConfig::from_json(Some(""));
        assert_eq!(cfg.max_retries, 8);

        // 部分字段
        let cfg = RetryConfig::from_json(Some(r#"{"maxRetries":5,"initialDelayMs":3000}"#));
        assert_eq!(cfg.max_retries, 5);
        assert_eq!(cfg.initial_delay_ms, 3000);
        // 未指定字段用 serde default
        assert!(!cfg.status_codes.is_empty());

        // 非法 JSON → 默认值
        let cfg = RetryConfig::from_json(Some("{invalid}"));
        assert_eq!(cfg.max_retries, 8);
    }

    #[test]
    fn test_retry_delay_ms() {
        let cfg = RetryConfig {
            max_retries: 3,
            initial_delay_ms: 2000,
            max_delay_ms: 8000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.0, // 关闭抖动便于断言
            status_codes: vec![],
        };
        assert_eq!(cfg.retry_delay_ms(0), 0);
        assert_eq!(cfg.retry_delay_ms(1), 2000);
        assert_eq!(cfg.retry_delay_ms(2), 4000);
        assert_eq!(cfg.retry_delay_ms(3), 8000);
        assert_eq!(cfg.retry_delay_ms(4), 8000); // 被 max_delay_ms 截断
    }

    #[test]
    fn test_timeout_config_serde() {
        let config = TimeoutConfig {
            connection: 25000,
            response: 120000,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"connection\":25000"));
        assert!(json.contains("\"response\":120000"));
    }

    #[test]
    fn test_proxy_type_serde() {
        // 验证 lowercase 序列化
        assert_eq!(
            serde_json::to_string(&ProxyType::Direct).unwrap(),
            "\"direct\""
        );
        assert_eq!(
            serde_json::to_string(&ProxyType::Http).unwrap(),
            "\"http\""
        );
        assert_eq!(
            serde_json::to_string(&ProxyType::Socks).unwrap(),
            "\"socks\""
        );
        // 反序列化
        let t: ProxyType = serde_json::from_str("\"system\"").unwrap();
        assert_eq!(t, ProxyType::System);
    }
}
