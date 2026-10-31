//! # 上游错误判定器
//!
//! 把上游 HTTP 错误转换为"路由级故障 / 请求级错误"判定结果，供虚拟供应商
//! 故障转移使用。判定顺序（短路，先命中先返回）：
//!
//! 1. **状态码确定性判定**：401/402/403/404/405/406/408/429/5xx 直接定级；
//! 2. **标准错误码/type**：`insufficient_quota`、`model_not_found` 等；
//! 3. **消息关键词**：仅对没有标准码的 `400 / 404 / 429` 生效；
//! 4. **默认**：无法识别一律视为请求级错误（透传，不切换、不降级）。
//!
//! 关键约束：
//! - `400 / 403` 的内容安全拒绝（`content_filter`）优先判为请求级，避免 403 被
//!   误判成"权限故障"而连带降级整组路由；
//! - 上下文超限（`context_length_exceeded`）为请求级，不切换路由；
//! - 关键词只对 `message` 生效，不对 `code/type` 生效，避免
//!   `context_length_exceeded` 因含 "exceeded" 被误判为额度耗尽。

use axum::response::Response;
use serde_json::Value;

use crate::modules::virtual_provider::types::{RouteFailureKind, RoutePenalty};

/// 路由级故障判定结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteVerdict {
    /// 是否切换到下一条路由（`false` 表示请求级错误，直接透传给客户端）
    pub failover: bool,
    /// 失败类型
    pub kind: RouteFailureKind,
    /// 降级惩罚（与是否切换分离：上下文超限只切换、不惩罚）
    pub penalty: RoutePenalty,
}

impl RouteVerdict {
    /// 构造请求级判定（透传、不降级）
    pub fn client_request() -> Self {
        Self {
            failover: false,
            kind: RouteFailureKind::ClientRequest,
            penalty: RoutePenalty::None,
        }
    }

    /// 按失败类型构造判定（是否切换由 `kind.is_client_request()` 推导）
    pub fn new(kind: RouteFailureKind, penalty: RoutePenalty) -> Self {
        Self {
            failover: !kind.is_client_request(),
            kind,
            penalty,
        }
    }
}

/// 一次路由尝试的结果
///
/// `Success` 仅代表 2xx；所有 4xx/5xx 都落在 `Failure` 中，由
/// `VirtualForwarder` 依据 `verdict.failover` 决定是否继续尝试下一条路由。
pub enum AttemptOutcome {
    /// 2xx 成功响应（已构造为 axum Response）
    Success(Response),
    /// 失败（已构造为可直接返回给客户端的 axum Response）
    Failure(AttemptFailure),
}

/// 失败尝试的详细信息
pub struct AttemptFailure {
    /// 判定结果
    pub verdict: RouteVerdict,
    /// 上游 HTTP 状态码（网络层错误时为 0）
    pub status: u16,
    /// 错误摘要（用于 attempts 历史与聚合错误）
    pub message: String,
    /// 已构造好的响应（请求级错误时直接返回给客户端）
    pub response: Response,
}

/// 分类结果：判定 + 可读摘要
#[derive(Debug, Clone)]
pub struct ClassifiedError {
    pub verdict: RouteVerdict,
    /// 上游原始错误信息（已截断）
    pub message: String,
    /// 归一化后的 code/type（日志用）
    pub code: Option<String>,
}

/// 对上游响应进行分类
///
/// - `status < 400` 返回 `None`（视为成功）
/// - 其余返回判定结果；`body` 为上游原始错误体字节（流式 4xx 也应在读取后传入）
pub fn classify(status: u16, body: Option<&[u8]>) -> Option<ClassifiedError> {
    if status < 400 {
        return None;
    }

    let err = normalize(status, body);
    let verdict = decide(&err);

    Some(ClassifiedError {
        verdict,
        message: err.message.clone(),
        code: err.code.clone().or(err.typ.clone()),
    })
}

/// 网络层错误（无 HTTP 响应）的判定
///
/// 超时与连接失败都视为路由级故障，使用短冷却；网络错误消息含超时关键字时归为
/// [`RouteFailureKind::Timeout`]，便于 UI 展示。
pub fn classify_transport_error(message: &str) -> RouteVerdict {
    let lower = message.to_lowercase();
    let is_timeout = TIMEOUT_MARKERS.iter().any(|m| lower.contains(m));
    let kind = if is_timeout {
        RouteFailureKind::Timeout
    } else {
        RouteFailureKind::UpstreamUnavailable
    };
    RouteVerdict::new(kind, RoutePenalty::Short)
}

// ===== 内部实现 =====

/// 归一化后的错误信息
struct NormalizedError {
    status: u16,
    /// 小写化的 `error.code` / `error.status`
    code: Option<String>,
    /// 小写化的 `error.type`
    typ: Option<String>,
    /// 小写化的 `error.message`（无 JSON 时为原始 body 截断）
    message: String,
}

impl NormalizedError {
    /// code / type 是否命中给定标记
    fn code_hits(&self, markers: &[&str]) -> bool {
        [self.code.as_deref(), self.typ.as_deref()]
            .into_iter()
            .flatten()
            .any(|v| markers.iter().any(|m| v.contains(m)))
    }

    /// message 是否命中给定标记
    fn message_hits(&self, markers: &[&str]) -> bool {
        markers.iter().any(|m| self.message.contains(m))
    }
}

/// 从状态码与响应体提取归一化错误信息
fn normalize(status: u16, body: Option<&[u8]>) -> NormalizedError {
    let text = body
        .map(|b| String::from_utf8_lossy(b).to_string())
        .unwrap_or_default();

    let mut message = truncate(text.trim(), 400);
    let mut code: Option<String> = None;
    let mut typ: Option<String> = None;

    if let Ok(v) = serde_json::from_str::<Value>(&text) {
        // OpenAI / 通用：{ "error": { message, type, code } }
        if let Some(err) = v.get("error") {
            if let Some(s) = err.get("message").and_then(|x| x.as_str()) {
                message = truncate(s.trim(), 400);
            }
            // Gemini：error.code 为数字，error.status 为字符串（如 RESOURCE_EXHAUSTED）
            if let Some(s) = err.get("code") {
                if let Some(s) = s.as_str() {
                    code = Some(s.to_string());
                }
            }
            if let Some(s) = err.get("status").and_then(|x| x.as_str()) {
                code = Some(s.to_string());
            }
            if let Some(s) = err.get("type").and_then(|x| x.as_str()) {
                typ = Some(s.to_string());
            }
        } else if let Some(s) = v.get("message").and_then(|x| x.as_str()) {
            // 部分中转站：{ "message": "...", "code": "..." }
            message = truncate(s.trim(), 400);
            if let Some(c) = v.get("code").and_then(|x| x.as_str()) {
                code = Some(c.to_string());
            }
        }
    }

    NormalizedError {
        status,
        code: code.map(|s| s.to_lowercase()),
        typ: typ.map(|s| s.to_lowercase()),
        message: message.to_lowercase(),
    }
}

/// 截断到指定字符数（按字符边界，避免切断 UTF-8）
fn truncate(s: &str, limit: usize) -> String {
    if s.chars().count() <= limit {
        return s.to_string();
    }
    s.chars().take(limit).collect()
}

/// 主判定逻辑
fn decide(err: &NormalizedError) -> RouteVerdict {
    // 5xx 无条件视为上游不可用（错误体可能是 HTML / 空，不参与识别）
    if err.status >= 500 {
        return RouteVerdict::new(
            RouteFailureKind::UpstreamUnavailable,
            RoutePenalty::Short,
        );
    }

    // Step 0：内容安全拒绝优先判定为请求级（400 / 403）
    if matches!(err.status, 400 | 403) && err.message_hits(CONTENT_FILTER_MARKERS) {
        return RouteVerdict::client_request();
    }

    match err.status {
        401 => RouteVerdict::new(RouteFailureKind::AuthFailure, RoutePenalty::Long),
        402 => RouteVerdict::new(RouteFailureKind::QuotaExhausted, RoutePenalty::Long),
        403 => RouteVerdict::new(RouteFailureKind::AuthFailure, RoutePenalty::Long),
        404 => {
            if err.message_hits(ENDPOINT_MISCONFIG_MARKERS) {
                RouteVerdict::new(
                    RouteFailureKind::ProviderMisconfigured,
                    RoutePenalty::Long,
                )
            } else {
                RouteVerdict::new(RouteFailureKind::ModelUnavailable, RoutePenalty::Long)
            }
        }
        405 | 406 => RouteVerdict::new(
            RouteFailureKind::ProviderMisconfigured,
            RoutePenalty::Long,
        ),
        408 => RouteVerdict::new(RouteFailureKind::Timeout, RoutePenalty::Short),
        429 => {
            // 部分供应商用 429 表达额度耗尽，优先按额度判定（长冷却）
            if err.code_hits(QUOTA_CODES) || err.message_hits(QUOTA_MESSAGE_MARKERS) {
                RouteVerdict::new(RouteFailureKind::QuotaExhausted, RoutePenalty::Long)
            } else {
                RouteVerdict::new(RouteFailureKind::RateLimited, RoutePenalty::Short)
            }
        }
        400 => decide_bad_request(err),
        413 | 414 | 415 | 422 => RouteVerdict::client_request(),
        // 其余 4xx（409/423/425/426/428 等）默认透传
        _ => RouteVerdict::client_request(),
    }
}

/// 400 的二级判定：标准 code → 关键词 → 默认透传
fn decide_bad_request(err: &NormalizedError) -> RouteVerdict {
    // 1. 标准错误码 / type（指向路由本身）
    if err.code_hits(QUOTA_CODES) {
        return RouteVerdict::new(RouteFailureKind::QuotaExhausted, RoutePenalty::Long);
    }
    if err.code_hits(MODEL_CODES) {
        return RouteVerdict::new(RouteFailureKind::ModelUnavailable, RoutePenalty::Long);
    }
    if err.code_hits(AUTH_CODES) {
        return RouteVerdict::new(RouteFailureKind::AuthFailure, RoutePenalty::Long);
    }
    if err.code_hits(RATE_CODES) {
        return RouteVerdict::new(RouteFailureKind::RateLimited, RoutePenalty::Short);
    }
    if err.code_hits(SERVER_CODES) {
        return RouteVerdict::new(
            RouteFailureKind::UpstreamUnavailable,
            RoutePenalty::Short,
        );
    }
    // 明确的请求级 code（含 context_length_exceeded / invalid_request_error）直接透传
    if err.code_hits(REQUEST_CODES) {
        return RouteVerdict::client_request();
    }

    // 2. 无标准 code 时按消息关键词（仅针对 message）
    if err.message_hits(QUOTA_MESSAGE_MARKERS) {
        return RouteVerdict::new(RouteFailureKind::QuotaExhausted, RoutePenalty::Long);
    }
    if err.message_hits(MODEL_MESSAGE_MARKERS) {
        return RouteVerdict::new(RouteFailureKind::ModelUnavailable, RoutePenalty::Long);
    }
    if err.message_hits(RATE_MESSAGE_MARKERS) {
        return RouteVerdict::new(RouteFailureKind::RateLimited, RoutePenalty::Short);
    }

    // 3. 默认：请求级错误（透传，不切换、不惩罚）
    RouteVerdict::client_request()
}

// ===== 标记表 =====

/// 内容安全 / 审核拒绝（400 / 403 优先判为请求级）
const CONTENT_FILTER_MARKERS: &[&str] = &[
    "content_filter",
    "content_policy",
    "content policy",
    "moderation",
    "safety",
    "sensitive",
    "policy violation",
    "flagged",
    "内容安全",
    "敏感",
    "违规",
    "违反政策",
    "审核",
];

/// 额度耗尽标准码
const QUOTA_CODES: &[&str] = &[
    "insufficient_quota",
    "quota_exceeded",
    "billing_hard_limit_reached",
    "billing_not_active",
    "resource_exhausted",
    "no_quota",
];

/// 额度耗尽消息关键词
const QUOTA_MESSAGE_MARKERS: &[&str] = &[
    "quota",
    "insufficient",
    "balance",
    "credit",
    "billing",
    "payment",
    "spending limit",
    "usage limit",
    "额度",
    "余额",
    "欠费",
    "充值",
    "配额",
    "计费",
];

/// 模型不可用标准码
const MODEL_CODES: &[&str] = &[
    "model_not_found",
    "model_not_available",
    "not_found_error",
    "unsupported_model",
    "no_such_model",
];

/// 模型不可用消息关键词
const MODEL_MESSAGE_MARKERS: &[&str] = &[
    "model_not_found",
    "no such model",
    "does not exist",
    "invalid model",
    "unsupported model",
    "not a valid model",
    "model is not available",
    "no access to",
    "模型不存在",
    "无效的模型",
    "未开通",
    "无权访问",
    "不支持的模型",
];

/// 凭证 / 权限标准码
const AUTH_CODES: &[&str] = &[
    "invalid_api_key",
    "authentication_error",
    "permission_denied",
    "permission_error",
    "account_deactivated",
    "unauthenticated",
    "forbidden",
];

/// 限流标准码
const RATE_CODES: &[&str] = &[
    "rate_limit_exceeded",
    "rate_limit_error",
    "too_many_requests",
    "throttled",
];

/// 限流消息关键词
const RATE_MESSAGE_MARKERS: &[&str] = &[
    "rate limit",
    "too many requests",
    "throttl",
    "overloaded",
    "capacity",
    "concurrency",
    "限流",
    "请求过于频繁",
    "负载已饱和",
    "并发",
];

/// 上游内部故障标准码（400 场景下也按短冷却切换）
const SERVER_CODES: &[&str] = &[
    "server_error",
    "api_error",
    "overloaded_error",
    "unavailable",
    "deadline_exceeded",
];

/// 明确的请求级标准码
const REQUEST_CODES: &[&str] = &[
    "context_length_exceeded",
    "string_above_max_length",
    "invalid_request_error",
    "invalid_argument",
    "failed_precondition",
    "request_too_large",
    "out_of_range",
    "invalid_value",
    "missing_parameter",
    "unknown_parameter",
];

/// 404 时指向接口/路径配置错误的标记
const ENDPOINT_MISCONFIG_MARKERS: &[&str] = &[
    "endpoint",
    "path",
    "route",
    "接口不存在",
    "路径",
];

/// 超时关键字（网络层错误分类）
const TIMEOUT_MARKERS: &[&str] = &[
    "timed out",
    "timeout",
    "deadline",
    "超时",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(status: u16, body: &str) -> RouteVerdict {
        classify(status, Some(body.as_bytes()))
            .expect("status >= 400 应返回判定")
            .verdict
    }

    #[test]
    fn test_2xx_is_success() {
        assert!(classify(200, None).is_none());
    }

    #[test]
    fn test_402_is_quota_with_long_cooldown() {
        let v = verdict(402, "");
        assert!(v.failover);
        assert_eq!(v.kind, RouteFailureKind::QuotaExhausted);
        assert_eq!(v.penalty, RoutePenalty::Long);
    }

    #[test]
    fn test_400_insufficient_quota_code() {
        let body = r#"{"error":{"message":"You exceeded your current quota","type":"invalid_request_error","code":"insufficient_quota"}}"#;
        let v = verdict(400, body);
        assert_eq!(v.kind, RouteFailureKind::QuotaExhausted);
        assert!(v.failover);
        assert_eq!(v.penalty, RoutePenalty::Long);
    }

    #[test]
    fn test_400_context_length_is_client_request() {
        let body = r#"{"error":{"message":"This model's maximum context length is 8192 tokens","type":"invalid_request_error","code":"context_length_exceeded"}}"#;
        let v = verdict(400, body);
        assert!(!v.failover);
        assert_eq!(v.kind, RouteFailureKind::ClientRequest);
        assert_eq!(v.penalty, RoutePenalty::None);
    }

    #[test]
    fn test_403_content_filter_is_client_request() {
        let body = r#"{"error":{"message":"Your request was rejected by the safety system","type":"content_filter"}}"#;
        let v = verdict(403, body);
        assert!(!v.failover);
        assert_eq!(v.kind, RouteFailureKind::ClientRequest);
    }

    #[test]
    fn test_403_plain_is_auth_failure() {
        let body = r#"{"error":{"message":"Forbidden"}}"#;
        let v = verdict(403, body);
        assert!(v.failover);
        assert_eq!(v.kind, RouteFailureKind::AuthFailure);
        assert_eq!(v.penalty, RoutePenalty::Long);
    }

    #[test]
    fn test_400_unknown_is_client_request() {
        let body = r#"{"error":{"message":"messages: field required"}}"#;
        let v = verdict(400, body);
        assert!(!v.failover);
        assert_eq!(v.kind, RouteFailureKind::ClientRequest);
    }

    #[test]
    fn test_400_chinese_quota_keyword() {
        let body = r#"{"error":{"message":"当前分组额度已用完，请充值后重试"}}"#;
        let v = verdict(400, body);
        assert_eq!(v.kind, RouteFailureKind::QuotaExhausted);
        assert!(v.failover);
    }

    #[test]
    fn test_429_quota_style_message_upgrades_to_long() {
        let body = r#"{"error":{"message":"You exceeded your current quota, please check your plan"}}"#;
        let v = verdict(429, body);
        assert_eq!(v.kind, RouteFailureKind::QuotaExhausted);
        assert_eq!(v.penalty, RoutePenalty::Long);
    }

    #[test]
    fn test_429_plain_is_rate_limited() {
        let body = r#"{"error":{"message":"Rate limit reached for requests"}}"#;
        let v = verdict(429, body);
        assert_eq!(v.kind, RouteFailureKind::RateLimited);
        assert_eq!(v.penalty, RoutePenalty::Short);
    }

    #[test]
    fn test_404_is_model_unavailable() {
        let v = verdict(404, r#"{"error":{"message":"model not found"}}"#);
        assert_eq!(v.kind, RouteFailureKind::ModelUnavailable);
        assert!(v.failover);
    }

    #[test]
    fn test_404_endpoint_is_provider_misconfigured() {
        let v = verdict(404, r#"{"message":"endpoint not found"}"#);
        assert_eq!(v.kind, RouteFailureKind::ProviderMisconfigured);
    }

    #[test]
    fn test_5xx_is_upstream_unavailable() {
        for status in [500u16, 502, 503, 504, 529] {
            let v = verdict(status, "<html>bad gateway</html>");
            assert_eq!(v.kind, RouteFailureKind::UpstreamUnavailable);
            assert!(v.failover);
            assert_eq!(v.penalty, RoutePenalty::Short);
        }
    }

    #[test]
    fn test_gemini_resource_exhausted() {
        let body = r#"{"error":{"code":429,"message":"Quota exceeded","status":"RESOURCE_EXHAUSTED"}}"#;
        let v = verdict(429, body);
        assert_eq!(v.kind, RouteFailureKind::QuotaExhausted);
    }

    #[test]
    fn test_anthropic_not_found_error() {
        let body = r#"{"type":"error","error":{"type":"not_found_error","message":"model: claude-x"}}"#;
        let v = verdict(404, body);
        assert_eq!(v.kind, RouteFailureKind::ModelUnavailable);
    }

    #[test]
    fn test_transport_timeout() {
        let v = classify_transport_error("operation timed out");
        assert_eq!(v.kind, RouteFailureKind::Timeout);
        assert!(v.failover);
    }
}
