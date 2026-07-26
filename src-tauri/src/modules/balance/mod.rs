//! # 额度监控模块
//!
//! 提供供应商额度查询能力，支持多种 BalanceMethod。
//!
//! ## 架构
//!
//! ```text
//! commands.rs  → 参数校验、调 Service
//! service.rs   → 业务入口，分发到 providers
//! provider.rs  → BalanceProvider trait 定义
//! providers/   → 各供应商实现（deepseek、openrouter、siliconflow 等）
//! repository.rs → 额度快照持久化（provider_balance_snapshots 表）
//! types.rs     → DTO / 领域类型
//! ```

pub mod commands;
pub mod provider;
pub mod providers;
pub mod repository;
/// 自定义 Rhai 脚本运行时（额度监控）
pub mod script;
pub mod service;
pub mod types;

pub use service::BalanceServiceHandle;
