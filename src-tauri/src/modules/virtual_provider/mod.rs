//! # 虚拟供应商模块
//!
//! 提供虚拟供应商（Virtual Provider）与虚拟模型路由（Virtual Model Route）管理。
//!
//! ## 核心概念
//!
//! - **VirtualProvider**：一个逻辑供应商标识，对外暴露为 `{virtual_alias}/{model_id}`。
//! - **VirtualModel**：虚拟供应商下的模型 ID。
//! - **VirtualModelRoute**：将虚拟模型映射到真实供应商的某条路由，含优先级、重试、超时等配置。
//! - **Strategy**：路由策略
//!   - `on_all`：同时请求所有可用路由（v0.1 未实现）
//!   - `fallback`：按优先级顺序尝试，失败则切换下一条（默认）
//!   - `load_balance`：按权重轮询（v0.1 未实现）
//!
//! ## 与 gateway_runtime 的集成
//!
//! `gateway_runtime/upstream.rs` 在解析模型 ID 时，若找不到真实供应商，
//! 会尝试到 virtual_provider 模块查找；找到后按 strategy 选择目标 provider/model 进行转发。
//!
//! ## v0.1 实现范围
//!
//! - VirtualProvider / VirtualModel / VirtualModelRoute 的 CRUD
//! - `fallback` 策略：按 `priority` 升序尝试，第一条成功的路由被使用
//! - `on_all` 与 `load_balance` 标记为未实现，直接报错

pub mod commands;
pub mod repository;
pub mod service;
pub mod types;

pub use service::VirtualProviderHandle;
