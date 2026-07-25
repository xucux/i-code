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

/// 代理配置
///
/// 对应 `docs/database.md` §5.3，用于 `app_settings.global_proxy_json`
/// 与 `providers.proxy_json` 字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
    /// 代理类型
    /// - `direct`：直连，不使用代理
    /// - `custom`：自定义代理 URL
    /// - `system`：使用系统代理设置
    /// - `vscode`：使用 VSCode 代理设置（仅 CLI 集成场景）
    #[serde(rename = "type")]
    pub proxy_type: ProxyType,
    /// 自定义代理 URL（仅 `custom` 类型生效）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// 代理认证凭据或 `$SECRET:{snowflake_id}$` 引用
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization: Option<String>,
    /// 是否严格校验 SSL 证书（默认 true）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict_ssl: Option<bool>,
    /// 不走代理的主机列表（NO_PROXY 环境变量等价物）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub no_proxy: Vec<String>,
}

/// 代理类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyType {
    /// 直连
    Direct,
    /// 自定义代理
    Custom,
    /// 系统代理
    System,
    /// VSCode 代理
    Vscode,
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
    pub max_retries: u32,
    /// 初始退避延迟（毫秒）
    pub initial_delay_ms: u64,
    /// 最大退避延迟上限（毫秒）
    pub max_delay_ms: u64,
    /// 退避倍率（每次延迟 = 上次延迟 × 倍率）
    pub backoff_multiplier: f64,
    /// 抖动因子（0.0-1.0），添加随机性避免同步重试
    pub jitter_factor: f64,
    /// 触发重试的 HTTP 状态码列表（如 429、500、502、503、504）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status_codes: Vec<u16>,
}

impl Default for RetryConfig {
    /// 默认重试策略：3 次重试，初始 500ms，最大 8s，倍率 2.0，抖动 0.2
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 500,
            max_delay_ms: 8000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.2,
            status_codes: vec![429, 500, 502, 503, 504],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_config_serde() {
        let config = ProxyConfig {
            proxy_type: ProxyType::Custom,
            url: Some("http://127.0.0.1:7890".to_string()),
            authorization: Some("$SECRET:abc-123$".to_string()),
            strict_ssl: Some(false),
            no_proxy: vec!["localhost".to_string(), "127.0.0.1".to_string()],
        };
        let json = serde_json::to_string(&config).unwrap();
        // 字段名应为 camelCase / type
        assert!(json.contains("\"type\":\"custom\""));
        assert!(json.contains("\"strictSsl\""));
        assert!(json.contains("\"noProxy\""));

        // 反序列化往返
        let back: ProxyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.proxy_type, ProxyType::Custom);
        assert_eq!(back.no_proxy.len(), 2);
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.status_codes.len(), 5);
        assert!(config.status_codes.contains(&429));
    }

    #[test]
    fn test_timeout_config_serde() {
        let config = TimeoutConfig {
            connection: 5000,
            response: 120000,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"connection\":5000"));
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
            serde_json::to_string(&ProxyType::Custom).unwrap(),
            "\"custom\""
        );
        // 反序列化
        let t: ProxyType = serde_json::from_str("\"system\"").unwrap();
        assert_eq!(t, ProxyType::System);
    }
}
