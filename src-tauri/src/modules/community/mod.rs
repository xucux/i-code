//! # 社区模块（后端）
//!
//! 独立第二代码库（Cloudflare Worker + D1）的客户端侧，仅消费其 REST API。
//! 设计见 `docs/proposals/community.md`。
//!
//! ## 架构
//!
//! ```text
//! commands.rs   → 参数校验、调 Service、错误转换
//! service.rs    → 门禁/身份/本地状态编排，转发 Worker REST API
//! client.rs     → reqwest 调用 Worker（复用全局代理，附加 X-User-Id / X-App-Token）
//! repository.rs → app_settings.community_json 本地状态读写
//! types.rs      → DTO（serde camelCase）
//! ```
//!
//! ## 隐私约束（§1.4 / §9）
//!
//! `user_id` 与原始设备标识禁止写入任何日志；本模块业务日志不记录用户内容
//! （帖子正文等隐私数据仅记录「发帖成功」类无内容事件）。

pub mod client;
pub mod commands;
pub mod repository;
pub mod service;
pub mod types;

pub use service::CommunityHandle;
