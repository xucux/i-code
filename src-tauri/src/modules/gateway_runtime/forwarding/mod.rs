//! # 转发编排层
//!
//! 位于路由层（`router.rs`）与协议客户端层（`client/`）之间，负责：
//!
//! 1. 解析模型 ID 与路由（真实供应商 vs 虚拟供应商故障转移）
//! 2. 编排请求转发：构造客户端请求、执行、统一处理响应
//! 3. 流式 / 非流式响应的统一分支处理
//! 4. usage 数据提取（流式 SSE 事件拦截 + 非流式 body 解析）
//! 5. 调用记录读写、日志输出
//!
//! ## 模块组成
//!
//! - [`context`]：`ForwardContext` / `ResolvedRoute` / `ForwardRequest`
//! - [`route_resolver`]：解析 `model_id`，返回真实或虚拟路由
//! - [`forwarder`]：`Forwarder` trait + `ForwardPipeline` 统一入口
//! - [`direct_forwarder`]：真实供应商转发器
//! - [`virtual_forwarder`]：虚拟供应商转发器（健康度排序逐条降级重试）
//! - [`response_handler`]：流式 / 非流式响应统一处理
//! - [`usage_extractor`]：usage 解析（非流式 body + 流式 SSE 事件）
//! - [`util`]：模型 ID 解析、token 估算、错误响应构造等工具
//! - [`call_log`]：调用记录读写封装

pub mod call_log;
pub mod context;
pub mod forwarder;
pub mod response_handler;
pub mod route_resolver;
pub mod usage_extractor;
pub mod util;
pub mod virtual_forwarder;

pub use context::{ForwardContext, ForwardRequest};
pub use forwarder::{DirectForwarder, Forwarder, ForwardPipeline};
pub use route_resolver::{ResolvedRoute, ResolvedRouteKind};
pub use usage_extractor::parse_usage_from_response_body;
pub use util::{estimate_prompt_tokens, parse_model_id, upstream_error_response};
