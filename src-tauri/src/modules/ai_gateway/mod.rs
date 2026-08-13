//! # AI Gateway 模块
//!
//! 维护 AI Gateway 供应商、模型、认证、额度与代理配置。
//!
//! ## 模块组成
//!
//! - [`types`]：`Provider` / `GatewayModel` / `ModelConfig` / `AuthConfig` 等 DTO
//! - [`repository`]：`providers` / `model_configs` / `gateway_models` 等表的 CRUD
//! - [`service`]：供应商与模型的业务逻辑（含 Secret 引用处理）
//! - [`seed`]：内置供应商/模型种子数据加载（JSON 随发版嵌入二进制）
//! - [`commands`]：Tauri Command 声明
//!
//! ## v0.1 实现范围
//!
//! - **供应商 CRUD**：创建/读取/更新/删除/列表/启用禁用
//! - **模型配置 CRUD**：创建/读取/更新/删除
//! - **网关模型 CRUD**：创建/读取/删除/列表（含暴露过滤）
//! - **暴露模型列表**：`/v1/models` 接口数据源
//! - **Secret 引用**：保存时将敏感值交给 secret 模块加密，读取时解析引用
//!
//! 以下功能待后续迭代：
//! - 从内置供应商列表添加（依赖 data/ 下 JSON 种子数据）
//! - 从配置导入 / 导出 base64 JSON
//! - 官方模型拉取（依赖 reqwest 实时调用供应商 API，不做缓存）
//! - 内置模型别名匹配
//!
//! ## 与其他模块的关系
//!
//! - 依赖 [`secret`](crate::modules::secret) 模块加密 API Key 等敏感字段
//! - 依赖 [`shared`](crate::modules::shared) 模块的 ProxyConfig / RetryConfig
//! - 被 [`gateway_runtime`](crate::modules::gateway_runtime) 调用获取供应商配置

pub mod auth;
pub mod commands;
pub mod repository;
pub mod seed;
pub mod service;
pub mod types;

pub use service::{AiGatewayService, AiGatewayServiceHandle};
