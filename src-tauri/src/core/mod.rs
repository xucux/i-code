//! # 后端核心工具
//!
//! 放置零业务依赖、被各模块共享的基础能力。

pub mod atomic_filter;
pub mod id;
pub mod size_aware_appender;
pub mod trace_id;
pub mod trace_id_layer;
