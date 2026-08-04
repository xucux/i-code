//! # 网关日志记录层
//!
//! 使用「记录器（Recorder）+ 记录上下文（LogRecord）」的设计模式，统一处理
//! 网关入口请求、上游转发请求、调用失败的日志输出。
//!
//! ## 设计目标
//!
//! - **解耦**：业务层只需构造 `LogRecord` 并调用 `LogPipeline::record`，
//!   不再关心日志写往何处、是否截断、是否走 tauri-plugin-log。
//! - **多通道**：同一条记录同时写入自研内存 logger（UI 可见）和
//!   tauri-plugin-log（终端/文件，完整不截断），通过 `LogRecorder` trait 组合实现。
//! - **可扩展**：新增日志通道（如文件滚动、WebDAV 推送）只需实现 `LogRecorder`。
//!
//! ## 模块组成
//!
//! - [`recorder`]：`LogRecorder` trait、`LogRecord` 上下文、`LogPipeline` 组合入口
//! - [`forward_log`]：转发日志记录器（写入自研 logger 环形缓冲）
//! - [`gateway_log`]：网关入口日志记录器（同上，`LogSource::Gateway`）
//! - [`tauri_emitter`]：tauri-plugin-log 输出器（不截断）

pub mod forward_log;
pub mod gateway_log;
pub mod headers;
pub mod recorder;
pub mod tauri_emitter;

#[allow(unused_imports)]
pub use forward_log::ForwardLogRecorder;
#[allow(unused_imports)]
pub use gateway_log::GatewayLogRecorder;
#[allow(unused_imports)]
pub use recorder::{
    LogKind, LogPipeline, LogRecord, LogRecordBuilder, LogRecorder, LogStatus,
};
#[allow(unused_imports)]
pub use tauri_emitter::TauriLogEmitter;
