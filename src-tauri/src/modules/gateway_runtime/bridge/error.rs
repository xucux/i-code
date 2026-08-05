//! # 桥接错误类型
//!
//! 桥接模块内部的错误封装，用于请求体 / 响应体转换失败时携带诊断信息。
//!
//! ## 设计约束（§7.6）
//!
//! - 流式状态机的解析错误用 `BridgeError` 包装但**不向上传播**，
//!   失败事件原样透传，仅 `tracing::warn!` 记录。
//! - 非流式转换失败由调用方决定回退策略（P2 接入时实现）。
//!
//! P1 阶段仅用于单元测试与请求体转换路径，不参与流式管线。

use std::fmt;

/// 桥接转换错误
#[derive(Debug, Clone)]
pub enum BridgeError {
    /// 字段缺失或类型不匹配
    ///
    /// `field` 为 JSON 路径（如 `messages[0].content`），`reason` 为具体原因。
    InvalidField {
        field: String,
        reason: String,
    },
    /// JSON 解析或序列化失败
    JsonParse(String),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidField { field, reason } => {
                write!(f, "桥接字段非法: {} ({})", field, reason)
            }
            Self::JsonParse(msg) => write!(f, "桥接 JSON 解析失败: {}", msg),
        }
    }
}

impl std::error::Error for BridgeError {}

impl BridgeError {
    /// 构造字段非法错误
    pub fn invalid_field(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidField {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// 构造 JSON 解析错误
    pub fn json_parse(msg: impl Into<String>) -> Self {
        Self::JsonParse(msg.into())
    }
}
