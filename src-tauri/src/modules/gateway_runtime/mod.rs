//! # Gateway Runtime 模块
//!
//! 本地 HTTP 网关运行时，启动 axum HTTP Server 接收 OpenAI 兼容请求
//! 并路由到真实供应商。
//!
//! ## 模块组成
//!
//! - [`types`]：`GatewayRuntimeState` / `StartGatewayInput` / `StartGatewayResult` 等 DTO
//! - [`service`]：HTTP Server 生命周期管理（start / stop / status / health）
//! - [`router`]：HTTP 路由定义与请求转发
//! - [`upstream`]：上游供应商实际请求转发（reqwest 流式 + SSE）
//! - [`client`]：上游供应商协议 Client 抽象层（OpenAI/Anthropic/WebSocket 预留）
//! - [`auth`]：认证中间件（Gateway Key 校验，内部 CLI 豁免）
//! - [`commands`]：Tauri Command 声明
//!
//! ## v0.1 实现范围
//!
//! - **生命周期管理**：start / stop / status，通过 tokio::sync::oneshot 传递 shutdown 信号
//! - **路由**：
//!   - `GET /health`：存活检查（always 200）
//!   - `GET /v1/models`：列出已暴露的模型（来自 ai_gateway）
//!   - `POST /v1/chat/completions`：转发到上游供应商（支持流式 SSE）
//!   - `POST /v1/messages`：Anthropic 兼容接口转发
//! - **认证**：从 `Authorization: Bearer {gateway_key}` 校验 Gateway Key
//!   - Gateway Key 明文从 `gateway_settings.default_api_key_secret_id` 解析
//!   - 内部 CLI 豁免（暂未实现，需通过请求头标识判断）
//! - **日志**：请求完成后写入 logger 模块的环形缓冲区
//! - **虚拟供应商故障转移**：通过 [`virtual_provider`](crate::modules::virtual_provider) 解析 fallback 路由
//! - **调用记录**：通过 [`call_records`](crate::modules::call_records) 持久化到 `model_call_logs` 表
//!
//! 以下功能待后续迭代：
//! - 模型级 extra_headers / extra_body（供应商级 extra_headers 已实现）
//! - 响应体格式转换（如 Anthropic → OpenAI）
//! - 流式响应 token 数解析
//! - 完整的拦截器链
//!
//! ## 与其他模块的关系
//!
//! - 依赖 [`ai_gateway`](crate::modules::ai_gateway) 获取供应商配置、暴露模型列表、监听地址与 Gateway Key
//! - 依赖 [`secret`](crate::modules::secret) 解析供应商认证配置中的 `$SECRET:{snowflake_id}$` 引用
//! - 依赖 [`logger`](crate::modules::logger) 记录请求日志
//! - 依赖 [`virtual_provider`](crate::modules::virtual_provider) 解析虚拟供应商故障转移路由
//! - 依赖 [`call_records`](crate::modules::call_records) 持久化调用记录

pub mod auth;
pub mod auth_resolver;
// 桥接模块 P1 阶段仅完成模块内可独立编译+测试的部分，尚未接入 forwarder/response_handler。
// P2/P3 接入前，模块内公开 API 在非测试构建中无调用方，整体标记 `allow(dead_code)` 抑制告警。
#[allow(dead_code)]
pub mod bridge;
pub mod client;
pub mod commands;
pub mod forwarding;
pub mod header_variable_resolver;
pub mod logging;
pub mod router;
pub mod service;
pub mod types;

pub use service::GatewayRuntimeHandle;
