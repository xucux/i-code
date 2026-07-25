//! # 聊天模块
//!
//! 应用内聊天：会话管理、附件/图片、经本地网关的 SSE/HTTP 对话。
//!
//! ## 界面职责（前端对应）
//!
//! - 侧栏「聊天」→ 路由 `/chat`
//! - 左：会话列表；右上：消息气泡（思考过程 + token）；右下：输入与中断
//! - 顶栏：当前会话累计 token
//!
//! ## 逻辑概览
//!
//! 1. 会话/消息以 JSONL 落在程序目录 `chat/`
//! 2. 发送走 `POST {gateway}/v1/chat/completions`，头带 `inner-cli-api`
//! 3. 附件：文件全文进 content；图片 base64 进 `image_url`；名称写入会话
//! 4. 流式结果 `emit`：`chat:stream-chunk` / `done` / `error`；中断用 oneshot
//! 5. 助手 `thinking` 从 `reasoning_content` / `reasoning` / `thinking` 等字段解析
//!
//! ## 模块组成
//!
//! - [`types`]：会话 / 消息 / 附件 / 流式事件 DTO
//! - [`repository`]：`chat/` 下 JSONL 读写
//! - [`service`]：CRUD、组包、网关请求、中断、事件
//! - [`commands`]：Tauri Command 声明
//!
//! ## 存储
//!
//! ```text
//! {exe_dir}/chat/
//!   sessions.jsonl
//!   messages/{session_id}.jsonl
//! ```

pub mod commands;
pub mod repository;
pub mod service;
pub mod types;

pub use service::ChatServiceHandle;
