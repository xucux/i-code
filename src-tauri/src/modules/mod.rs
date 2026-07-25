//! # 后端业务模块入口
//!
//! 按 DDD-like 模块分离，每个业务域有独立的 `commands / service / repository / types` 子层。
//! 模块组成与前端 `src/modules/` 一一对应。
//!
//! ## 模块列表
//!
//! - [`shared`]：跨模块共享的通用配置类型（ProxyConfig / RetryConfig / TimeoutConfig）
//! - [`secret`]：敏感数据加密与密钥链
//! - [`settings`]：应用全局设置、主题、语言、网关监听地址
//! - [`balance`]：额度监控与余额查询
//! - [`logger`]：日志控制台
//! - [`backup`]：数据库压缩备份与恢复
//! - [`ai_gateway`]：供应商、模型、认证、额度、代理
//! - [`cli_management`]：CLI 档案、CLI 供应商绑定、模型映射
//! - [`workspace`]：工作区、Prompts、MCP、Skill
//! - [`gateway_runtime`]：本地 HTTP 网关生命周期与请求路由
//! - [`virtual_provider`]：虚拟供应商与模型故障转移
//! - [`call_records`]：模型调用记录与统计
//!
//! ## 跨模块调用规则
//!
//! - 后端 Service 层可调用其他模块的 Service，但禁止直接访问其他模块的 Repository。
//! - 后端 Repository 层禁止调用 Service 或发送事件。
//! - 后端 Repository 直接操作 SQLite；前端通过 Tauri Commands 与后端通信。

pub mod ai_gateway;
pub mod balance;
pub mod backup;
pub mod call_records;
pub mod chat;
pub mod cli_management;
pub mod gateway_runtime;
pub mod logger;
pub mod secret;
pub mod settings;
pub mod shared;
pub mod tokenizer;
pub mod virtual_provider;
pub mod workspace;
