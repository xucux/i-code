//! # i-code 统一错误类型
//!
//! 所有后端模块的 Command / Service / Repository 层均返回 `IcodeResult<T>`，
//! 错误统一封装为 [`IcodeError`]，通过 Tauri Command 抛出时由前端 `use-command` hook 接收。
//!
//! ## 错误码规范
//!
//! | code            | 说明                          | HTTP 等价 |
//! |-----------------|-------------------------------|-----------|
//! | `VALIDATION`    | 表单/参数校验失败             | 400       |
//! | `UNAUTHORIZED`  | 未认证或会话过期               | 401       |
//! | `FORBIDDEN`     | 无权限访问                    | 403       |
//! | `NOT_FOUND`     | 资源不存在                    | 404       |
//! | `CONFLICT`      | 唯一约束冲突（如 slug 重复）  | 409       |
//! | `GATEWAY`       | 网关请求转发异常              | 502       |
//! | `DATABASE`      | 数据库操作失败                | 500       |
//! | `INTERNAL`      | 后端内部错误（不暴露堆栈）    | 500       |
//! | `UNKNOWN`       | 未分类错误                    | 500       |

#![allow(dead_code)]

use serde::Serialize;

/// 后端通用 Result 别名
pub type IcodeResult<T> = Result<T, IcodeError>;

/// 错误码枚举，与前端 `src/core/errors.ts` 的 `ErrorCode` 保持一致
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// 未分类的未知错误
    Unknown,
    /// 表单/参数校验失败
    Validation,
    /// 资源不存在
    NotFound,
    /// 认证失败或会话过期
    Unauthorized,
    /// 权限不足
    Forbidden,
    /// 唯一约束冲突
    Conflict,
    /// 网关请求转发异常
    Gateway,
    /// 数据库操作失败
    Database,
    /// 后端内部错误
    Internal,
}

impl ErrorCode {
    /// 转换为字符串字面量，便于序列化与日志输出
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::Validation => "VALIDATION",
            Self::NotFound => "NOT_FOUND",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Forbidden => "FORBIDDEN",
            Self::Conflict => "CONFLICT",
            Self::Gateway => "GATEWAY",
            Self::Database => "DATABASE",
            Self::Internal => "INTERNAL",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 业务错误基类
///
/// 所有抛出到前端的错误都应转换为此类型。
/// Repository/Service 层可直接返回具体子类型，由 `From` impl 自动转换。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IcodeError {
    /// 错误码，前端按 code 做 Toast / 表单提示
    pub code: String,
    /// 用户可见的错误消息（已本地化或为通用描述）
    pub message: String,
    /// 附加详情，例如校验失败的字段列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl IcodeError {
    /// 构造一个新错误
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    /// 通过 ErrorCode 枚举构造
    pub fn from_code(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::new(code.as_str(), message)
    }

    /// 附加详情，链式调用
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// 参数校验失败快捷构造
    pub fn validation(message: impl Into<String>) -> Self {
        Self::from_code(ErrorCode::Validation, message)
    }

    /// 资源不存在快捷构造
    pub fn not_found(resource: &str, id: Option<&str>) -> Self {
        let message = match id {
            Some(id) => format!("{resource}({id}) 不存在"),
            None => format!("{resource} 不存在"),
        };
        Self::from_code(ErrorCode::NotFound, message)
    }

    /// 认证失败快捷构造
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::from_code(ErrorCode::Unauthorized, message)
    }

    /// 权限不足快捷构造
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::from_code(ErrorCode::Forbidden, message)
    }

    /// 唯一约束冲突快捷构造
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::from_code(ErrorCode::Conflict, message)
    }

    /// 网关异常快捷构造
    pub fn gateway(message: impl Into<String>) -> Self {
        Self::from_code(ErrorCode::Gateway, message)
    }

    /// 数据库错误快捷构造
    pub fn database(message: impl Into<String>) -> Self {
        Self::from_code(ErrorCode::Database, message)
    }

    /// 内部错误快捷构造（不暴露堆栈给前端）
    pub fn internal(message: impl Into<String>) -> Self {
        Self::from_code(ErrorCode::Internal, message)
    }

    /// 功能尚未实现快捷构造
    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::from_code(ErrorCode::Internal, message)
    }
}

impl std::fmt::Display for IcodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for IcodeError {}

// ===== 各类第三方错误到 IcodeError 的自动转换 =====

/// rusqlite 错误转换：
/// - 唯一约束冲突 → `CONFLICT`
/// - 外键约束失败 → `VALIDATION`
/// - 其他 → `DATABASE`
impl From<rusqlite::Error> for IcodeError {
    fn from(err: rusqlite::Error) -> Self {
        match &err {
            rusqlite::Error::SqliteFailure(ffi_err, msg) => {
                // SQLite 错误码常量见 https://www.sqlite.org/rescode.html
                let code = ffi_err.extended_code;
                // SQLITE_CONSTRAINT_UNIQUE = 2067
                // SQLITE_CONSTRAINT_PRIMARYKEY = 1555
                if code == 2067 || code == 1555 {
                    let detail = msg.clone().unwrap_or_default();
                    return Self::conflict(format!("数据唯一约束冲突：{detail}"));
                }
                // SQLITE_CONSTRAINT_FOREIGNKEY = 787
                if code == 787 {
                    let detail = msg.clone().unwrap_or_default();
                    return Self::validation(format!("外键约束失败：{detail}"));
                }
                Self::database(err.to_string())
            }
            _ => Self::database(err.to_string()),
        }
    }
}

/// r2d2 连接池错误转换
impl From<r2d2::Error> for IcodeError {
    fn from(err: r2d2::Error) -> Self {
        Self::database(format!("数据库连接池错误：{err}"))
    }
}

/// serde_json 错误转换：JSON 字段反序列化失败 → `VALIDATION`
impl From<serde_json::Error> for IcodeError {
    fn from(err: serde_json::Error) -> Self {
        Self::validation(format!("JSON 解析失败：{err}"))
    }
}

/// reqwest HTTP 客户端错误转换
impl From<reqwest::Error> for IcodeError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::gateway(format!("上游请求超时：{err}"))
        } else if err.is_connect() {
            Self::gateway(format!("上游连接失败：{err}"))
        } else {
            Self::gateway(format!("HTTP 客户端错误：{err}"))
        }
    }
}

/// std::io 错误转换
impl From<std::io::Error> for IcodeError {
    fn from(err: std::io::Error) -> Self {
        Self::internal(format!("IO 错误：{err}"))
    }
}

/// tokio task 错误转换
impl From<tokio::task::JoinError> for IcodeError {
    fn from(err: tokio::task::JoinError) -> Self {
        Self::internal(format!("异步任务错误：{err}"))
    }
}

/// zip 压缩/解压错误转换
impl From<zip::result::ZipError> for IcodeError {
    fn from(err: zip::result::ZipError) -> Self {
        Self::internal(format!("ZIP 操作失败：{err}"))
    }
}

// 说明：此处不提供 `impl<E: std::error::Error> From<E> for IcodeError` 的泛型兜底转换，
// 因为它会与上方针对具体类型的 `From` 实现冲突（孤儿规则）。
// 业务层遇到未知错误类型时，应通过 `IcodeError::internal(e.to_string())` 显式包装。
