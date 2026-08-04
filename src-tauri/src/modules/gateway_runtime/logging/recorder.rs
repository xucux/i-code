//! # LogRecorder trait 与记录上下文
//!
//! 定义统一的日志记录接口与数据载体，所有具体记录器（自研 logger、
//! tauri-plugin-log、未来滚动文件等）实现 `LogRecorder` trait。

use std::sync::Arc;

use crate::modules::gateway_runtime::service::GatewaySharedState;

use super::forward_log::ForwardLogRecorder;
use super::gateway_log::GatewayLogRecorder;
use super::tauri_emitter::TauriLogEmitter;

/// 日志通道类型
///
/// 区分是「网关入口请求」还是「上游供应商转发请求」，决定：
/// - 写入 `LogEntry.source` 字段（Gateway / ProviderApi）
/// - 读取哪一份日志开关配置（gateway_* 或 forward_*）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    /// 网关入口请求（客户端 → 本地网关）
    Gateway,
    /// 上游供应商转发请求（本地网关 → 真实供应商）
    Forward,
}

/// 日志状态分类
///
/// 用于在 tauri-plugin-log 中选择 info / warn 级别输出。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStatus {
    /// 成功（2xx）
    Success,
    /// 客户端错误（4xx）或上游错误（5xx）
    Error,
    /// 调用失败（无状态码，请求未发出/被中断）
    Failed,
}

impl LogStatus {
    /// 根据状态码推断 LogStatus
    pub fn from_status(status: Option<u16>) -> Self {
        match status {
            Some(s) if (200..300).contains(&s) => Self::Success,
            Some(_) => Self::Error,
            None => Self::Failed,
        }
    }
}

/// 日志记录上下文
///
/// 由业务层构造，包含一次请求/响应的完整信息：
/// - 请求方法、URL、请求体（原始未截断）
/// - 响应状态码、响应体（原始未截断）
/// - Token 数据、耗时、错误信息、tags
///
/// 各 `LogRecorder` 实现按各自规则截断/格式化后输出。
#[derive(Debug, Clone, Default)]
pub struct LogRecord {
    /// 日志通道：网关入口 or 上游转发
    pub kind: Option<LogKind>,
    /// HTTP 方法（"GET" / "POST" ...）
    pub method: Option<String>,
    /// 请求 URL 或路径
    pub url: Option<String>,
    /// 响应状态码
    pub status_code: Option<u16>,
    /// 耗时（毫秒）
    pub duration_ms: Option<u64>,
    /// 请求 ID
    pub request_id: Option<String>,
    /// 模型 ID（`{provider_slug}/{model_id}` 形式）
    pub model_id: Option<String>,
    /// 请求头（已去敏，JSON 字符串，如 `{"authorization":"***"}`）
    pub request_headers: Option<String>,
    /// 请求体原始字符串（未截断）
    pub request_body: Option<String>,
    /// 响应体原始字符串（未截断）
    pub response_body: Option<String>,
    /// prompt token 数
    pub prompt_tokens: Option<i64>,
    /// completion token 数
    pub completion_tokens: Option<i64>,
    /// total token 数
    pub total_tokens: Option<i64>,
    /// 缓存命中 token 数
    pub cached_tokens: Option<i64>,
    /// 错误信息（无状态码或非 2xx 时填入）
    pub error_message: Option<String>,
    /// 协议标签（sse / websocket / network ...）
    pub tags: Vec<String>,
    /// 状态分类
    pub status: Option<LogStatus>,
}

impl LogRecord {
    /// 构造 builder
    pub fn builder() -> LogRecordBuilder {
        LogRecordBuilder::default()
    }
}

#[derive(Debug, Default)]
pub struct LogRecordBuilder {
    record: LogRecord,
}

impl LogRecordBuilder {
    pub fn kind(mut self, kind: LogKind) -> Self {
        self.record.kind = Some(kind);
        self
    }
    pub fn method(mut self, method: &str) -> Self {
        self.record.method = Some(method.to_string());
        self
    }
    pub fn url(mut self, url: &str) -> Self {
        self.record.url = Some(url.to_string());
        self
    }
    pub fn status_code(mut self, code: u16) -> Self {
        self.record.status_code = Some(code);
        self
    }
    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.record.duration_ms = Some(ms);
        self
    }
    pub fn request_id(mut self, id: &str) -> Self {
        self.record.request_id = Some(id.to_string());
        self
    }
    pub fn model_id(mut self, id: &str) -> Self {
        self.record.model_id = Some(id.to_string());
        self
    }
    pub fn request_headers(mut self, headers: &str) -> Self {
        self.record.request_headers = Some(headers.to_string());
        self
    }
    pub fn request_body(mut self, body: &str) -> Self {
        self.record.request_body = Some(body.to_string());
        self
    }
    pub fn response_body(mut self, body: &str) -> Self {
        self.record.response_body = Some(body.to_string());
        self
    }
    pub fn prompt_tokens(mut self, v: i64) -> Self {
        self.record.prompt_tokens = Some(v);
        self
    }
    pub fn completion_tokens(mut self, v: i64) -> Self {
        self.record.completion_tokens = Some(v);
        self
    }
    pub fn total_tokens(mut self, v: i64) -> Self {
        self.record.total_tokens = Some(v);
        self
    }
    pub fn cached_tokens(mut self, v: i64) -> Self {
        self.record.cached_tokens = Some(v);
        self
    }
    pub fn error_message(mut self, msg: &str) -> Self {
        self.record.error_message = Some(msg.to_string());
        self
    }
    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.record.tags = tags;
        self
    }
    pub fn status(mut self, status: LogStatus) -> Self {
        self.record.status = Some(status);
        self
    }
    pub fn build(mut self) -> LogRecord {
        if self.record.status.is_none() {
            self.record.status = Some(LogStatus::from_status(self.record.status_code));
        }
        self.record
    }
}

/// 日志记录器 trait
///
/// 每个具体记录器（自研 logger、tauri-plugin-log）实现此 trait，
/// 在 `record` 中按自身规则决定是否输出、如何截断、如何格式化。
pub trait LogRecorder: Send + Sync {
    /// 记录一条日志
    fn record(&self, shared: &GatewaySharedState, entry: &LogRecord);
}

/// 日志管道：组合多个记录器统一输出
///
/// 业务层通过 `LogPipeline::record` 将一条 `LogRecord` 同时推给所有
/// 记录器，避免散落调用。默认组合 `ForwardLogRecorder` + `TauriLogEmitter`。
pub struct LogPipeline {
    recorders: Vec<Arc<dyn LogRecorder>>,
}

impl LogPipeline {
    /// 创建空管道
    pub fn new() -> Self {
        Self { recorders: Vec::new() }
    }

    /// 添加记录器
    pub fn with(mut self, recorder: Arc<dyn LogRecorder>) -> Self {
        self.recorders.push(recorder);
        self
    }

    /// 构造默认管道：转发日志记录器 + tauri-plugin-log 输出器
    pub fn default_forward() -> Self {
        Self::new()
            .with(Arc::new(ForwardLogRecorder))
            .with(Arc::new(TauriLogEmitter))
    }

    /// 构造默认管道：网关入口日志记录器 + tauri-plugin-log 输出器
    pub fn default_gateway() -> Self {
        Self::new()
            .with(Arc::new(GatewayLogRecorder))
            .with(Arc::new(TauriLogEmitter))
    }

    /// 推送一条记录到所有记录器
    pub fn record(&self, shared: &GatewaySharedState, entry: &LogRecord) {
        for r in &self.recorders {
            r.record(shared, entry);
        }
    }
}

impl Default for LogPipeline {
    fn default() -> Self {
        Self::new()
    }
}
