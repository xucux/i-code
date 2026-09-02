//! # 媒体生成模块（文生图 / 文生视频）
//!
//! 承载视觉生成能力的运行时与历史记录：
//! - 图像生成客户端：按供应商协议适配器调用 `POST {base_url}/images/generations`
//! - 产物存储（asset_store）：生成成功后立即下载图片到应用数据目录
//!   （供应商返回的 URL 存在过期机制，如 SenseNova 固定 1 小时）
//! - 生成历史（media_generations 表）：prompt / 参数 / 本地产物路径 / 状态 / 耗时
//!
//! ## 隔离约束
//!
//! 本模块仅服务「视觉生成」供应商（媒体生成协议族，见
//! `ai_gateway::types::MEDIA_GENERATION_PROVIDER_TYPES`）。
//! 该类供应商不进入原网关转发逻辑与虚拟供应商逻辑，其模型不进入 `/v1/models`。
//!
//! ## 跨模块调用
//!
//! - 通过 [`crate::modules::ai_gateway::AiGatewayService`] 获取供应商、解析认证与附加头
//! - 通过 [`crate::modules::call_records::CallRecordsService`] 写入调用统计
//! - 通过自研 logger 与 tauri-plugin-log 双写运行时日志

pub mod asset_store;
pub mod commands;
pub mod repository;
pub mod service;
pub mod types;
