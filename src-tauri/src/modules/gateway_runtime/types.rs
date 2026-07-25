//! # Gateway Runtime 模块类型定义
//!
//! 与前端 `src/modules/gateway-runtime/types.ts` 对齐。
//! 序列化字段使用 camelCase，确保前后端 JSON 字段名一致。
//!
//! 运行时状态仅在内存中维护，不持久化到数据库。

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// 网关运行时状态
///
/// 存储在 Tauri State 中，前端通过 `gateway_status` 命令读取
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRuntimeState {
    /// 是否正在运行
    pub is_running: bool,
    /// 绑定的监听地址，如 `127.0.0.1`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_host: Option<String>,
    /// 绑定的监听端口，如 `54321`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_port: Option<u16>,
    /// 启动时间戳（ISO 8601）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// 最近一次错误信息（如启动失败、端口占用等）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// 已处理的请求总数（统计计数）
    pub total_requests: u64,
    /// 当前活跃请求数（并发计数）
    pub active_requests: u64,
}

/// 启动网关的输入参数
///
/// 为空时使用 `gateway_settings` 中的配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartGatewayInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

/// 启动网关的结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartGatewayResult {
    pub success: bool,
    pub host: String,
    pub port: u16,
    /// 启动失败时的错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 健康检查结果
///
/// 对应 `GET /health` 与 `GET /readyz` 接口
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResult {
    /// 是否存活（HTTP Server 是否响应）
    pub alive: bool,
    /// 是否就绪（数据库与上游供应商是否可达）
    pub ready: bool,
    /// 数据库连接是否正常
    pub database_ok: bool,
    /// 检查时间戳（毫秒）
    pub checked_at: i64,
}

/// 网关请求来源类型
///
/// 用于认证豁免判断
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GatewayRequestSource {
    /// 内部 CLI（可豁免 API Key 校验）
    InternalCli,
    /// 外部客户端（必须校验 Gateway Key）
    External,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_default() {
        let state = GatewayRuntimeState::default();
        assert!(!state.is_running);
        assert_eq!(state.total_requests, 0);
        assert_eq!(state.active_requests, 0);
    }

    #[test]
    fn test_start_input_default() {
        let input = StartGatewayInput::default();
        assert!(input.host.is_none());
        assert!(input.port.is_none());
    }

    #[test]
    fn test_start_input_serde() {
        let input = StartGatewayInput {
            host: Some("0.0.0.0".to_string()),
            port: Some(9000),
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"host\":\"0.0.0.0\""));
        assert!(json.contains("\"port\":9000"));

        let parsed: StartGatewayInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.host.as_deref(), Some("0.0.0.0"));
        assert_eq!(parsed.port, Some(9000));
    }

    #[test]
    fn test_state_serde_skip_none() {
        let state = GatewayRuntimeState::default();
        let json = serde_json::to_string(&state).unwrap();
        // None 字段应被 skip
        assert!(!json.contains("boundHost"));
        assert!(!json.contains("boundPort"));
        assert!(!json.contains("startedAt"));
        assert!(!json.contains("lastError"));
        // 但 false / 0 应保留
        assert!(json.contains("\"isRunning\":false"));
        assert!(json.contains("\"totalRequests\":0"));
    }
}
