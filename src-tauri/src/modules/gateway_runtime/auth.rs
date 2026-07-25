//! # 认证中间件
//!
//! 校验请求中的 Gateway API Key，并支持内部 CLI 通过专用请求头豁免。
//!
//! ## 认证流程
//!
//! 1. 优先检查 `inner-cli-api` 请求头：
//!    - 值与 `GatewaySharedState.inner_cli_api_key` 一致 → 直接放行
//!    - 头存在但值不一致 → 返回 403 Forbidden
//! 2. 未携带 `inner-cli-api` 头时，从 `Authorization: Bearer {key}` 或 `X-API-Key: {key}` 提取 key
//! 3. 优先在 `gateway_auth_keys` 中按明文 key 反查：
//!    - 命中启用且未过期的记录 → 放行，并更新 `last_used_at`
//! 4. 未命中时回退到 `gateway_settings.default_api_key_secret_id`：
//!    - 若值为 `$SECRET:{snowflake_id}$` 或雪花 ID → 解析 Secret 后比较
//!    - 若值为明文 → 直接比较（兼容用户直接填写明文 key 的场景）
//!    - 未配置 → 开放模式（豁免所有请求）
//!
//! ## 豁免规则
//!
//! - `/health` 与 `/readyz` 路径不需要认证
//! - 携带正确 `inner-cli-api` 请求头的内部 CLI 请求豁免
//! - 未配置 `default_api_key_secret_id` 且 `gateway_auth_keys` 为空时豁免所有请求（开放模式）
//!
//! ## 存储约定
//!
//! - `gateway_auth_keys.api_key_secret_id`：按业务需要保存**明文 key**，便于请求进来时直接反查。
//! - `gateway_settings.default_api_key_secret_id`：保留旧数据兼容性，可存 Secret 引用或明文。

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;

use crate::error::IcodeError;
use crate::modules::ai_gateway::AiGatewayServiceHandle;
use crate::modules::secret::SecretServiceHandle;

/// 豁免路径列表（不需要认证）
const EXEMPT_PATHS: &[&str] = &["/health", "/readyz"];

/// 认证中间件附加到请求扩展中的 API Key 信息
///
/// 值为请求实际使用的 Gateway API Key 明文（命中 `gateway_auth_keys` 时）
/// 或默认 Gateway Key 的明文；内部 CLI 豁免 / 开放模式下为 `None`。
/// 后续 handler / 上游转发层通过此扩展将 key 写入调用记录。
#[derive(Clone, Debug)]
pub struct RequestApiKey(pub Option<String>);

/// 认证中间件
///
/// 在 axum 路由层通过 `axum::middleware::from_fn_with_state` 注册。
/// 共享状态为认证所需的 Service Handle 集合。
/// 校验成功后将实际使用的 API Key 明文附加到请求扩展，供下游记录调用统计。
pub async fn auth_middleware(
    state: axum::extract::State<AuthState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path();

    // 健康检查路径豁免（不需要认证，也不记录 API Key）
    if EXEMPT_PATHS.contains(&path) {
        return Ok(next.run(request).await);
    }

    let api_key = authenticate_request(&state, &request)?;
    request.extensions_mut().insert(RequestApiKey(api_key));
    Ok(next.run(request).await)
}

/// 执行认证并返回请求使用的 API Key 明文
///
/// 返回值：
/// - `Ok(Some(key))`：命中 `gateway_auth_keys` 或默认 Gateway Key
/// - `Ok(None)`：内部 CLI 豁免或开放模式
/// - `Err(...)`：认证失败
fn authenticate_request(
    state: &AuthState,
    request: &Request,
) -> Result<Option<String>, StatusCode> {
    // 优先检查内部 CLI 豁免头
    if let Some(header_value) = request.headers().get("inner-cli-api") {
        if let Ok(value) = header_value.to_str() {
            if constant_time_eq(value.as_bytes(), state.inner_cli_api_key.as_bytes()) {
                return Ok(None);
            }
        }
        // 头存在但值不正确 → 明确拒绝，不再回退到 Gateway Key 校验
        return Err(StatusCode::FORBIDDEN);
    }

    // 从请求中提取客户端提供的 key
    let client_key = match extract_bearer_key(request) {
        Some(k) => k,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    // 1) 优先按 per-key 明文反查
    match state
        .ai_gateway_handle
        .service()
        .find_gateway_auth_key_by_api_key(&client_key)
    {
        Ok(Some(key)) if key.is_enabled && !is_expired(key.expires_at.as_deref()) => {
            // 异步更新最后使用时间，失败不影响本次请求
            let _ = state
                .ai_gateway_handle
                .service()
                .touch_gateway_auth_key_last_used(&key.id);
            return Ok(key.api_key_secret_id.clone());
        }
        Ok(_) => {}
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    // 2) 回退到默认 Gateway Key
    let settings = match state.ai_gateway_handle.service().get_gateway_settings() {
        Ok(s) => s,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let default_key = match settings.default_api_key_secret_id {
        Some(id) if !id.is_empty() => id,
        // 未配置默认 Gateway Key → 开放模式（豁免所有请求）
        _ => return Ok(None),
    };

    let expected_key = resolve_default_key(&default_key, &state.secret_handle)?;

    // 常量时间比较，避免时序攻击
    if !constant_time_eq(client_key.as_bytes(), expected_key.as_bytes()) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(Some(expected_key))
}

/// 认证中间件所需的共享状态
#[derive(Clone)]
pub struct AuthState {
    pub ai_gateway_handle: AiGatewayServiceHandle,
    pub secret_handle: SecretServiceHandle,
    /// 内部 CLI 请求豁免认证用的全局密钥
    pub inner_cli_api_key: String,
}

impl AuthState {
    pub fn new(
        ai_gateway_handle: AiGatewayServiceHandle,
        secret_handle: SecretServiceHandle,
        inner_cli_api_key: String,
    ) -> Self {
        Self {
            ai_gateway_handle,
            secret_handle,
            inner_cli_api_key,
        }
    }
}

/// 从请求头提取 Bearer Token 或 X-API-Key
///
/// 支持两种格式：
/// - `Authorization: Bearer {key}`
/// - `X-API-Key: {key}`
fn extract_bearer_key(request: &Request) -> Option<String> {
    // 优先 Authorization: Bearer xxx
    if let Some(auth) = request.headers().get("authorization") {
        if let Ok(s) = auth.to_str() {
            if let Some(token) = s.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }
    // 备选 X-API-Key
    if let Some(key) = request.headers().get("x-api-key") {
        if let Ok(s) = key.to_str() {
            return Some(s.to_string());
        }
    }
    None
}

/// 解析默认 Gateway Key
///
/// 兼容两种存储形态：
/// - `$SECRET:{snowflake_id}$` 或裸雪花 ID → 从 Secret 模块读取明文
/// - 其他字符串 → 视为明文直接返回
fn resolve_default_key(
    value: &str,
    secret_service: &SecretServiceHandle,
) -> Result<String, StatusCode> {
    let maybe_id = if let Some(id) = value.strip_prefix("$SECRET:").and_then(|s| s.strip_suffix('$')) {
        Some(id)
    } else if is_snowflake_id(value) {
        Some(value)
    } else {
        None
    };

    if let Some(id) = maybe_id {
        secret_service
            .service()
            .read_secret(id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    } else {
        Ok(value.to_string())
    }
}

/// 判断字符串是否为雪花 ID（用于识别 Secret ID 引用）
fn is_snowflake_id(s: &str) -> bool {
    crate::core::id::is_snowflake_id(s)
}

/// 判断 API Key 是否已过期
///
/// `expires_at` 为 ISO 8601 字符串，与当前 UTC 时间比较。
fn is_expired(expires_at: Option<&str>) -> bool {
    let Some(expires_at) = expires_at else {
        return false;
    };
    match chrono::DateTime::parse_from_rfc3339(expires_at) {
        Ok(dt) => dt <= chrono::Utc::now(),
        Err(_) => false,
    }
}

/// 常量时间比较，避免时序攻击
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// 将 IcodeError 转换为 StatusCode
///
/// 用于 axum handler 中将业务错误转换为 HTTP 状态码
pub fn error_to_status_code(err: &IcodeError) -> StatusCode {
    match err.code.as_str() {
        "VALIDATION" => StatusCode::BAD_REQUEST,
        "UNAUTHORIZED" => StatusCode::UNAUTHORIZED,
        "FORBIDDEN" => StatusCode::FORBIDDEN,
        "NOT_FOUND" => StatusCode::NOT_FOUND,
        "CONFLICT" => StatusCode::CONFLICT,
        "GATEWAY" => StatusCode::BAD_GATEWAY,
        "DATABASE" => StatusCode::INTERNAL_SERVER_ERROR,
        "INTERNAL" => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(!constant_time_eq(b"", b"a"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn test_is_snowflake_id() {
        assert!(is_snowflake_id("1234567890123456789"));
        assert!(!is_snowflake_id("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!is_snowflake_id("sk-icode-abc123"));
        assert!(!is_snowflake_id(""));
    }

    #[test]
    fn test_is_expired() {
        let future = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        let past = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        assert!(!is_expired(None));
        assert!(!is_expired(Some(&future)));
        assert!(is_expired(Some(&past)));
        assert!(!is_expired(Some("invalid")));
    }
}
