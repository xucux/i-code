//! # 日志控制台模块
//!
//! 记录 HTTP 请求/响应日志（网关转发与供应商 API 调用），供运维诊断。
//!
//! ## 模块组成
//!
//! - [`types`]：`LogLevel` / `LogSource` / `LogEntry` / `LogFilter` 等 DTO
//! - [`repository`]：内存环形缓冲区（Ring Buffer）读写
//! - [`service`]：日志写入、查询、导出、清理
//! - [`commands`]：Tauri Command 声明
//!
//! ## 设计要点
//!
//! - **内存为主存储**：默认保留最近 10000 条，FIFO 淘汰
//! - **异步非阻塞写入**：通过 `Mutex<VecDeque>` 保护，写入开销极低
//! - **实时推送**：新日志通过 Tauri Event `log:new-entry` 推送到前端
//! - **可选文件持久化**：v0.1 暂未实现，待后续迭代按天滚动写入文件
//!
//! ## 与审计日志的区别
//!
//! logger 聚焦**运行时诊断**：URL（已脱敏）、状态码、耗时、Token 用量、错误信息。
//! 不记录用户身份与敏感请求体内容。

pub mod commands;
pub mod logging;
pub mod repository;
pub mod service;
pub mod types;

#[allow(unused_imports)]
pub use logging::{Log, set_global_logger_handle};
pub use service::LoggerServiceHandle;
