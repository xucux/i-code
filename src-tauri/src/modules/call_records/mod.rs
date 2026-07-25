//! # 调用记录模块
//!
//! 持久化模型调用日志到 `model_call_logs` 表，供后续统计与审计使用。
//!
//! ## 模块组成
//!
//! - [`types`]：`ModelCallLog` / `CreateModelCallLogInput` / `UpdateModelCallLogInput` / `RouteMode`
//! - [`repository`]：`model_call_logs` 表的 CRUD
//! - [`service`]：调用记录的写入与查询业务逻辑
//! - [`commands`]：Tauri Command 声明
//!
//! ## 集成点
//!
//! `gateway_runtime/upstream.rs` 在转发请求前后调用 `CallRecordsService`：
//! - 请求开始前：`start_call` 写入初始记录
//! - 请求结束后：`finish_call_with_duration` 补充状态码、错误信息、耗时
//!
//! v0.1 仅记录请求级元信息，流式响应的 token 数留空。

pub mod commands;
pub mod repository;
pub mod service;
pub mod types;

pub use service::CallRecordsHandle;
