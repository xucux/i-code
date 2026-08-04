//! # 日志控制台模块类型定义
//!
//! 与前端 `src/modules/logger/types.ts` 对齐。

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// 日志时间格式：yyyy-MM-dd HH:mm:ss.SSS
pub const LOG_TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.3f";

/// 日志级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    #[serde(rename = "DEBUG")]
    Debug,
    #[serde(rename = "INFO")]
    Info,
    #[serde(rename = "WARN")]
    Warn,
    #[serde(rename = "ERROR")]
    Error,
}

impl LogLevel {
    /// 从字符串解析
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "DEBUG" => Some(Self::Debug),
            "INFO" => Some(Self::Info),
            "WARN" => Some(Self::Warn),
            "ERROR" => Some(Self::Error),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }

    /// 级别数值，用于阈值过滤（DEBUG=0 < INFO=1 < WARN=2 < ERROR=3）
    pub fn level_value(&self) -> u8 {
        match self {
            Self::Debug => 0,
            Self::Info => 1,
            Self::Warn => 2,
            Self::Error => 3,
        }
    }
}

/// 日志来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogSource {
    /// 本地网关请求转发
    #[serde(rename = "gateway")]
    Gateway,
    /// 上游供应商 API 调用
    #[serde(rename = "provider-api")]
    ProviderApi,
    /// 系统级日志（启动、停止、错误）
    #[serde(rename = "system")]
    System,
}

impl LogSource {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "gateway" => Some(Self::Gateway),
            "provider-api" => Some(Self::ProviderApi),
            "system" => Some(Self::System),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gateway => "gateway",
            Self::ProviderApi => "provider-api",
            Self::System => "system",
        }
    }
}

/// 单条日志记录
///
/// 由 `gateway-runtime` 的响应拦截器在请求完成后异步写入。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    /// UUID
    pub id: String,
    /// 日志时间戳（格式：yyyy-MM-dd HH:mm:ss.SSS）
    pub timestamp: String,
    pub level: LogLevel,
    pub source: LogSource,
    /// HTTP 方法，如 `POST`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// 请求 URL（已脱敏，去除 query 中的敏感参数）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// HTTP 状态码
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    /// 请求耗时（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// 提示 Token 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u64>,
    /// 补全 Token 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u64>,
    /// 总 Token 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
    /// 缓存命中 Token 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u64>,
    /// 错误信息（如有）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// 网关生成的唯一请求追踪 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// 请求使用的模型 ID（网关暴露的 model 字段，含 provider_slug 前缀）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// 请求头（已去敏，JSON 字符串，如 `{"authorization":"***"}`）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_headers: Option<String>,
    /// 请求体内容（转发详细日志开启时记录）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<String>,
    /// 响应体内容（转发详细日志开启时记录）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body: Option<String>,
    /// 协议/场景标签，如 `sse`、`websocket`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// 源文件名（如 `upstream.rs`）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// 源文件行号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<u32>,
}

/// 日志过滤参数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFilter {
    /// 按级别过滤（OR 关系）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<LogLevel>,
    /// 按来源过滤（OR 关系）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<LogSource>,
    /// 按状态码过滤（OR 关系）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status_codes: Vec<u16>,
    /// 关键词模糊匹配（URL、errorMessage）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword: Option<String>,
    /// 时间范围
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_range: Option<TimeRange>,
    /// 请求 ID 精确匹配
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// 时间范围
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeRange {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

/// 日志导出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogExportFormat {
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "csv")]
    Csv,
}

/// 日志导出结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogExportResult {
    /// 导出文件路径（保存到应用临时目录）
    pub file_path: String,
    /// 导出的日志条数
    pub count: usize,
    /// 导出格式
    pub format: LogExportFormat,
}

/// 日志滚动记录配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRollingConfig {
    /// 内存缓冲队列大小（条数），默认 5000
    pub buffer_size: usize,
    /// 是否启用本地日志文件持久化
    pub enable_file_persistence: bool,
    /// 单个日志文件大小上限（MB），默认 10
    pub max_file_size_mb: u64,
    /// 保留的日志文件数量，默认 7
    pub max_file_count: u32,
    /// 日志文件保留天数，默认 30
    pub max_retention_days: u32,
    /// 日志级别阈值（低于此级别不写入文件）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_log_level: Option<LogLevel>,
}

impl Default for LogRollingConfig {
    fn default() -> Self {
        Self {
            buffer_size: 5_000,
            enable_file_persistence: false,
            max_file_size_mb: 10,
            max_file_count: 7,
            max_retention_days: 30,
            file_log_level: Some(LogLevel::Info),
        }
    }
}

/// 转发详细日志配置
///
/// 控制网关转发时是否记录请求/响应体到日志缓冲区。
/// 存储在 `GatewaySharedState` 中，运行时可通过 Command 动态修改。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardLogConfig {
    /// 是否记录转发请求体（包含 model、messages 等）
    pub enable_request_log: bool,
    /// 是否记录转发响应体（包含 choices、usage 等）
    pub enable_response_log: bool,
    /// 单条请求/响应体最大记录长度（字符数），超出截断并追加 `...[truncated]`
    pub max_body_length: usize,
}

impl Default for ForwardLogConfig {
    fn default() -> Self {
        Self {
            enable_request_log: false,
            enable_response_log: false,
            max_body_length: 4096,
        }
    }
}

/// 前后端 Command 交互日志配置
///
/// 控制 Tauri Command 调用时是否记录请求/响应到系统日志。
/// 存储在 `GatewaySharedState` 中，运行时可通过 Command 动态修改。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandLogConfig {
    /// 是否记录 Command 调用到系统日志
    pub enable_command_log: bool,
    /// 是否记录 Command 请求参数
    pub enable_command_request_log: bool,
    /// 是否记录 Command 响应数据
    pub enable_command_response_log: bool,
    /// 单条请求/响应最大记录长度（字符数），超出截断
    pub max_body_length: usize,
}

impl Default for CommandLogConfig {
    fn default() -> Self {
        Self {
            enable_command_log: true,
            enable_command_request_log: false,
            enable_command_response_log: false,
            max_body_length: 4096,
        }
    }
}

/// 直连网关请求日志配置
///
/// 控制外部客户端直接请求本地网关时，是否记录请求/响应体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayLogConfig {
    /// 是否记录直连网关请求体
    pub enable_gateway_request_log: bool,
    /// 是否记录直连网关响应体
    pub enable_gateway_response_log: bool,
    /// 单条请求/响应体最大记录长度（字符数），超出截断
    pub max_body_length: usize,
}

impl Default for GatewayLogConfig {
    fn default() -> Self {
        Self {
            enable_gateway_request_log: false,
            enable_gateway_response_log: false,
            max_body_length: 4096,
        }
    }
}

/// 统一日志配置
///
/// 合并 ForwardLogConfig / CommandLogConfig / LogRollingConfig 为单一配置对象。
/// 持久化到 `log_settings` 数据库表，启动时从 DB 加载，运行时可通过 Command 更新。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSettings {
    // === 基础设置 ===
    /// 内存缓冲队列大小（条数），默认 5000
    pub buffer_size: usize,
    /// 日志文件目录（空字符串使用默认目录）
    pub log_dir: String,
    /// 日志文件保留天数，默认 30
    pub max_retention_days: u32,
    /// 是否启用文件持久化
    pub enable_file_persistence: bool,
    /// 单个日志文件大小上限（MB），默认 10
    pub max_file_size_mb: u64,
    /// 保留的日志文件数量，默认 7
    pub max_file_count: u32,
    /// 文件写入级别阈值
    pub file_log_level: Option<LogLevel>,

    // === 转发详细日志 ===
    /// 是否记录转发请求体
    pub enable_request_log: bool,
    /// 是否记录转发响应体
    pub enable_response_log: bool,
    /// 转发日志最大记录长度
    pub forward_max_body_length: usize,

    // === 直连网关请求日志 ===
    /// 是否记录直连网关请求体
    pub enable_gateway_request_log: bool,
    /// 是否记录直连网关响应体
    pub enable_gateway_response_log: bool,
    /// 直连网关日志最大记录长度
    pub gateway_max_body_length: usize,

    // === Command 交互日志 ===
    /// 是否记录 Command 调用
    pub enable_command_log: bool,
    /// 是否记录 Command 请求参数
    pub enable_command_request_log: bool,
    /// 是否记录 Command 响应数据
    pub enable_command_response_log: bool,
    /// Command 日志最大记录长度
    pub command_max_body_length: usize,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            buffer_size: 5_000,
            log_dir: String::new(),
            max_retention_days: 30,
            enable_file_persistence: false,
            max_file_size_mb: 10,
            max_file_count: 7,
            file_log_level: Some(LogLevel::Info),
            enable_request_log: false,
            enable_response_log: false,
            forward_max_body_length: 4096,
            enable_gateway_request_log: false,
            enable_gateway_response_log: false,
            gateway_max_body_length: 4096,
            enable_command_log: true,
            enable_command_request_log: false,
            enable_command_response_log: false,
            command_max_body_length: 4096,
        }
    }
}

impl LogSettings {
    /// 从 LogSettings 提取 ForwardLogConfig（供 upstream.rs 使用）
    pub fn to_forward_config(&self) -> ForwardLogConfig {
        ForwardLogConfig {
            enable_request_log: self.enable_request_log,
            enable_response_log: self.enable_response_log,
            max_body_length: self.forward_max_body_length,
        }
    }

    /// 从 LogSettings 提取 GatewayLogConfig（供 router.rs 使用）
    pub fn to_gateway_config(&self) -> GatewayLogConfig {
        GatewayLogConfig {
            enable_gateway_request_log: self.enable_gateway_request_log,
            enable_gateway_response_log: self.enable_gateway_response_log,
            max_body_length: self.gateway_max_body_length,
        }
    }

    /// 从 LogSettings 提取 CommandLogConfig
    pub fn to_command_config(&self) -> CommandLogConfig {
        CommandLogConfig {
            enable_command_log: self.enable_command_log,
            enable_command_request_log: self.enable_command_request_log,
            enable_command_response_log: self.enable_command_response_log,
            max_body_length: self.command_max_body_length,
        }
    }

    /// 从 LogSettings 提取 LogRollingConfig（供 LoggerService 使用）
    pub fn to_rolling_config(&self) -> LogRollingConfig {
        LogRollingConfig {
            buffer_size: self.buffer_size,
            enable_file_persistence: self.enable_file_persistence,
            max_file_size_mb: self.max_file_size_mb,
            max_file_count: self.max_file_count,
            max_retention_days: self.max_retention_days,
            file_log_level: self.file_log_level.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_roundtrip() {
        for level in [LogLevel::Debug, LogLevel::Info, LogLevel::Warn, LogLevel::Error] {
            let s = level.as_str();
            assert_eq!(LogLevel::from_str(s), Some(level));
        }
        assert_eq!(LogLevel::from_str("unknown"), None);
    }

    #[test]
    fn test_log_level_order() {
        assert!(LogLevel::Debug.level_value() < LogLevel::Info.level_value());
        assert!(LogLevel::Info.level_value() < LogLevel::Warn.level_value());
        assert!(LogLevel::Warn.level_value() < LogLevel::Error.level_value());
    }

    #[test]
    fn test_log_source_roundtrip() {
        for source in [LogSource::Gateway, LogSource::ProviderApi, LogSource::System] {
            let s = source.as_str();
            assert_eq!(LogSource::from_str(s), Some(source));
        }
    }

    #[test]
    fn test_log_entry_serde() {
        let entry = LogEntry {
            id: "test-id".to_string(),
            timestamp: "2026-07-15T00:00:00Z".to_string(),
            level: LogLevel::Info,
            source: LogSource::Gateway,
            method: Some("POST".to_string()),
            url: Some("https://api.example.com/v1/chat".to_string()),
            status_code: Some(200),
            duration_ms: Some(150),
            prompt_tokens: Some(100),
            completion_tokens: Some(50),
            total_tokens: Some(150),
            cached_tokens: None,
            error_message: None,
            request_id: Some("req-123".to_string()),
            model_id: Some("openai/gpt-4.1".to_string()),
            request_headers: None,
            request_body: None,
            response_body: None,
            tags: vec!["sse".to_string()],
            file_name: None,
            line_number: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        // camelCase 字段
        assert!(json.contains("\"statusCode\":200"));
        assert!(json.contains("\"durationMs\":150"));
        assert!(json.contains("\"promptTokens\":100"));
        assert!(json.contains("\"requestId\":\"req-123\""));
        // skip_serializing_if 字段不应出现
        assert!(!json.contains("cachedTokens"));
    }
}
