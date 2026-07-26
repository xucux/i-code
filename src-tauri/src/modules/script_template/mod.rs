//! # 脚本模板模块
//!
//! 管理额度监控等可复用 Rhai 脚本模板：CRUD、状态机、试运行。
//!
//! ## 架构
//!
//! ```text
//! commands.rs  → 参数校验、调 Service
//! service.rs   → 业务逻辑与试运行编排
//! repository.rs → script_templates 表访问
//! types.rs     → DTO / 领域类型
//! ```

pub mod commands;
pub mod repository;
pub mod service;
pub mod types;

pub use service::ScriptTemplateHandle;
