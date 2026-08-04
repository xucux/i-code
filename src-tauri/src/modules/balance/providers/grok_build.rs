//! # Grok Build（xAI 订阅）额度查询实现
//!
//! 调用 Grok CLI 的内部 chat-proxy billing 端点：
//! - 周额度：`GET {base}/billing?format=credits`
//! - 月额度：`GET {base}/billing`
//!
//! 认证分支（按 `input.auth_method` 区分）：
//! - `xai-grok-oauth`（OAuth 账号，免费档 / SuperGrok）→ 走 `cli-chat-proxy.grok.com/v1/billing`
//! - 其余（API Key）→ 回退 `api.x.ai` 健康探测（`/v1/me` + `/v1/chat/completions`），
//!   仅提供可用性状态，无法给出精确剩余额度
//!
//! 请求头对齐 `gateway_runtime/auth_resolver.rs` 的 xAI Grok OAuth 解析
//! （`x-grok-client-version: 0.2.93`）。
//!
//! `{ val }` 金额字段单位为**分**（1 美元 = 100），输出统一转换为美元**字符串**传输，
//! 避免浮点精度问题（对齐 AGENTS.md §6.2）。

use std::future::Future;
use std::pin::Pin;

use crate::error::{IcodeError, IcodeResult};

use super::super::provider::{BalanceProvider, BalanceRefreshInput};
use super::super::types::{
    BalanceDirection, BalanceMetric, BalanceMetricType, BalancePeriod, BalanceSnapshot,
    BalanceStatusValue,
};

/// xAI OAuth 认证方法标识
const XAI_OAUTH_METHOD: &str = "xai-grok-oauth";
/// Grok CLI 内部 chat-proxy 默认 base_url
const DEFAULT_CHAT_PROXY_BASE_URL: &str = "https://cli-chat-proxy.grok.com/v1";
/// xAI 官方 REST API 默认 base_url（API Key 健康探测回退）
const DEFAULT_API_X_AI_BASE_URL: &str = "https://api.x.ai/v1";
/// Grok CLI 版本（对齐 `auth_resolver.rs`）
const GROK_CLI_VERSION: &str = "0.2.93";
/// chat-proxy 身份标识值（对齐 CLIProxyAPI applyXAIChatHeaders）
const XAI_TOKEN_AUTH_VALUE: &str = "xai-grok-cli";
const XAI_USER_AGENT: &str = "xai-grok-workspace/0.2.93";

/// 判断 JSON 值是否为对象
fn is_record(value: &serde_json::Value) -> bool {
    value.is_object()
}

/// 从 JSON 对象中取字符串字段
fn pick_string(record: &serde_json::Value, key: &str) -> Option<String> {
    record.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// 从 JSON 对象中取数值字段（支持 number / string）
fn pick_number(record: &serde_json::Value, key: &str) -> Option<f64> {
    let value = record.get(key)?;
    if let Some(n) = value.as_f64() {
        if n.is_finite() {
            return Some(n);
        }
    }
    if let Some(s) = value.as_str() {
        if let Ok(n) = s.parse::<f64>() {
            if n.is_finite() {
                return Some(n);
            }
        }
    }
    None
}

/// 从 JSON 对象中取布尔字段
fn pick_bool(record: &serde_json::Value, key: &str) -> Option<bool> {
    record.get(key).and_then(|v| v.as_bool())
}

/// 取 `{ "val": <number> }` 结构中的数值（金额字段单位：分）
fn pick_val_amount(record: &serde_json::Value, key: &str) -> Option<f64> {
    let holder = record.get(key)?;
    if is_record(holder) {
        return pick_number(holder, "val");
    }
    pick_number(record, key)
}

/// 分 → 美元字符串（保留 2 位小数）
fn cents_to_dollars(cents: f64) -> String {
    if !cents.is_finite() {
        return "0.00".to_string();
    }
    format!("{:.2}", cents / 100.0)
}

/// 限制百分比在 [0, 100] 范围内
fn clamp_percent(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    value.max(0.0).min(100.0)
}

/// 获取当前毫秒时间戳
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 解析 ISO 8601 / RFC3339 时间字符串为毫秒时间戳（按 UTC 解释，忽略小数与偏移）
fn parse_iso_to_millis(value: &str) -> Option<i64> {
    let s = value.trim();
    if s.len() < 19 {
        return None;
    }
    let bytes = s.as_bytes();
    let year: i32 = s.get(0..4)?.parse().ok()?;
    if bytes.get(4)? != &b'-' {
        return None;
    }
    let month: u32 = s.get(5..7)?.parse().ok()?;
    if bytes.get(7)? != &b'-' {
        return None;
    }
    let day: u32 = s.get(8..10)?.parse().ok()?;
    let sep = bytes.get(10)?;
    if sep != &b'T' && sep != &b' ' {
        return None;
    }
    let hour: u32 = s.get(11..13)?.parse().ok()?;
    if bytes.get(13)? != &b':' {
        return None;
    }
    let minute: u32 = s.get(14..16)?.parse().ok()?;
    if bytes.get(16)? != &b':' {
        return None;
    }
    let second: u32 = s.get(17..19)?.parse().ok()?;

    let days = days_from_civil(year, month, day)?;
    Some((days * 86400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64) * 1000)
}

/// 公元 1970-01-01 到指定日期的天数（Howard Hinnant 算法）
fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if month == 0 || month > 12 || day == 0 || day > 31 {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * if month > 2 { month - 3 } else { month + 9 } + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era as i64 * 146097 + doe as i64 - 719468)
}

/// 构造状态类指标快捷方式
fn status_metric(id: &str, value: BalanceStatusValue, message: Option<&str>) -> BalanceMetric {
    BalanceMetric::status(id, value, message)
}

/// 从 billing 响应中提取 `config` 对象
///
/// 兼容两种包裹形态：
/// - `{ "config": {...} }`
/// - `{ "body": "{...json string...}" }`（body 为 JSON 字符串，再取 config 或直接为 config）
fn resolve_billing_config(value: &serde_json::Value) -> Option<serde_json::Value> {
    if !is_record(value) {
        return None;
    }
    if let Some(config) = value.get("config").filter(|v| is_record(v)) {
        return Some(config.clone());
    }
    if let Some(body) = value.get("body") {
        if let Some(s) = body.as_str() {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                if let Some(config) = parsed.get("config").filter(|v| is_record(v)) {
                    return Some(config.clone());
                }
                if is_record(&parsed) {
                    return Some(parsed);
                }
            }
        } else if is_record(body) {
            return Some(body.clone());
        }
    }
    None
}

/// 解析错误响应体（OpenAI 风格 `{ error: { code, message } }` 或直接 `{ code, message }`）
fn parse_error(text: &str) -> Option<ParsedError> {
    let normalized = text.trim();
    if normalized.is_empty() {
        return None;
    }
    let parsed: serde_json::Value = serde_json::from_str(normalized).ok()?;
    let error_node = if is_record(&parsed) {
        parsed
            .get("error")
            .filter(|v| is_record(v))
            .unwrap_or(&parsed)
    } else {
        return None;
    };
    let code = pick_string(error_node, "code");
    let message = pick_string(error_node, "message");
    if code.is_none() && message.is_none() {
        return None;
    }
    Some(ParsedError { code, message })
}

/// 解析后的错误信息
struct ParsedError {
    code: Option<String>,
    message: Option<String>,
}

/// 判断 billing 响应是否为套餐耗尽（429 + free-usage-exhausted）
fn is_free_usage_exhausted(status: u16, body: &str) -> bool {
    if status != 429 {
        return false;
    }
    let Some(parsed) = parse_error(body) else {
        return false;
    };
    parsed
        .code
        .as_deref()
        .is_some_and(|c| c.contains("free-usage-exhausted"))
        || parsed
            .message
            .as_deref()
            .is_some_and(|m| m.contains("Usage resets over a rolling"))
}

/// 生成套餐耗尽快照
fn exhausted_snapshot() -> BalanceSnapshot {
    let mut status = BalanceMetric::status(
        "status",
        BalanceStatusValue::Exhausted,
        Some("免费额度已用尽，等待下一个计费周期重置"),
    );
    status.primary = Some(true);
    BalanceSnapshot {
        updated_at: now_ms(),
        items: vec![status],
    }
}

/// 将周额度百分比指标转换为 BalanceMetric
fn percent_metric(
    id: &str,
    percent: f64,
    basis: BalanceDirection,
    period: BalancePeriod,
    label: &str,
    scope: Option<&str>,
) -> BalanceMetric {
    let mut metric = BalanceMetric::percent(id, clamp_percent(percent), Some(basis))
        .with_period(period)
        .with_label(label);
    if let Some(scope) = scope {
        metric = metric.with_scope(scope);
    }
    metric
}

/// 解析 billing config → 额度指标列表
fn build_billing_items(config: &serde_json::Value) -> Vec<BalanceMetric> {
    let mut items: Vec<BalanceMetric> = Vec::new();
    let mut has_primary = false;

    // 1) 周信用额度用量 %（免费档核心指标）
    if let Some(percent) = pick_number(config, "creditUsagePercent") {
        let mut metric = percent_metric(
            "credit_percent",
            percent,
            BalanceDirection::Used,
            BalancePeriod::Week,
            "本周用量",
            None,
        );
        metric.primary = Some(true);
        has_primary = true;
        items.push(metric);
    }

    // 2) 当前周窗口（重置时刻 = end）
    if let Some(current_period) = config.get("currentPeriod").filter(|v| is_record(v)) {
        if let Some(end) = pick_string(current_period, "end") {
            if !end.is_empty() {
                let millis = parse_iso_to_millis(&end);
                items.push(
                    BalanceMetric::time("period_week", "resetAt", end, millis)
                        .with_period(BalancePeriod::Week)
                        .with_label("本周重置时间"),
                );
            }
        }
    }

    // 3) 按产品用量（GrokBuild / GrokChat 等）
    if let Some(product_usage) = config.get("productUsage").and_then(|v| v.as_array()) {
        for entry in product_usage {
            let Some(product) = pick_string(entry, "product") else { continue };
            let Some(percent) = pick_number(entry, "usagePercent") else { continue };
            items.push(
                percent_metric(
                    &format!("product_{}", product),
                    percent,
                    BalanceDirection::Used,
                    BalancePeriod::Week,
                    &product,
                    Some("product"),
                )
                .with_label(format!("{} 用量", product)),
            );
        }
    }

    // 4) 月度额度（cent → 美元字符串）
    if let Some(limit) = pick_val_amount(config, "monthlyLimit") {
        items.push(
            BalanceMetric::amount(
                "monthly_limit",
                BalanceDirection::Limit,
                serde_json::json!(cents_to_dollars(limit)),
                Some("$"),
            )
            .with_period(BalancePeriod::Month)
            .with_label("月度额度"),
        );
    }
    if let Some(used) = pick_val_amount(config, "used") {
        items.push(
            BalanceMetric::amount(
                "monthly_used",
                BalanceDirection::Used,
                serde_json::json!(cents_to_dollars(used)),
                Some("$"),
            )
            .with_period(BalancePeriod::Month)
            .with_label("月度已用"),
        );
    }

    // 5) 按量付费余额 = onDemandCap - onDemandUsed
    let on_demand_cap = pick_val_amount(config, "onDemandCap").unwrap_or(0.0);
    let on_demand_used = pick_val_amount(config, "onDemandUsed").unwrap_or(0.0);
    if on_demand_cap > 0.0 || on_demand_used > 0.0 {
        let remaining = (on_demand_cap - on_demand_used).max(0.0);
        items.push(
            BalanceMetric::amount(
                "on_demand_remaining",
                BalanceDirection::Remaining,
                serde_json::json!(cents_to_dollars(remaining)),
                Some("$"),
            )
            .with_period(BalancePeriod::Current)
            .with_label("按量付费余额"),
        );
    }

    // 6) 预付余额
    if let Some(prepaid) = pick_val_amount(config, "prepaidBalance") {
        items.push(
            BalanceMetric::amount(
                "prepaid_balance",
                BalanceDirection::Remaining,
                serde_json::json!(cents_to_dollars(prepaid)),
                Some("$"),
            )
            .with_label("预付余额"),
        );
    }

    // 7) 统一计费标识（布尔 → 状态类字符串指标）
    if let Some(unified) = pick_bool(config, "isUnifiedBillingUser") {
        items.push(BalanceMetric {
            id: "unified_billing".into(),
            metric_type: BalanceMetricType::Status,
            value: Some(serde_json::Value::String(
                if unified { "true".to_string() } else { "false".to_string() },
            )),
            ..Default::default()
        });
    }

    // 无 creditUsagePercent 时，将第一个指标作为主指标
    if !has_primary {
        if let Some(first) = items.first_mut() {
            first.primary = Some(true);
        }
    }

    items
}

/// 解析 billing 响应，返回额度指标列表
fn parse_billing_payload(payload: &serde_json::Value) -> Vec<BalanceMetric> {
    let Some(config) = resolve_billing_config(payload) else {
        return Vec::new();
    };
    build_billing_items(&config)
}

/// 解析 base_url：为空时使用默认值
fn resolve_base_url(input: &BalanceRefreshInput, default: &str) -> String {
    input
        .base_url
        .as_deref()
        .map(|s| s.trim().trim_end_matches('/'))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| default.trim_end_matches('/').to_string())
}

/// 构建 chat-proxy 请求头（对齐 CLIProxyAPI applyXAIChatHeaders）
fn build_chat_proxy_headers(token: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Ok(value) = reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token)) {
        headers.insert(reqwest::header::AUTHORIZATION, value);
    }
    headers.insert(
        "x-xai-token-auth",
        reqwest::header::HeaderValue::from_static(XAI_TOKEN_AUTH_VALUE),
    );
    headers.insert(
        "x-grok-client-version",
        reqwest::header::HeaderValue::from_static(GROK_CLI_VERSION),
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(XAI_USER_AGENT),
    );
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    headers
}

/// Grok Build 额度 Provider
pub struct GrokBuildBalanceProvider;

impl BalanceProvider for GrokBuildBalanceProvider {
    fn method(&self) -> &'static str {
        "grok-build"
    }

    fn refresh<'a>(
        &'a self,
        input: &'a BalanceRefreshInput,
    ) -> Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>> {
        Box::pin(async move {
            let token = input.api_key.as_deref().ok_or_else(|| {
                IcodeError::validation("Grok Build 额度查询需要 Access Token / API Key")
            })?;

            // 分支：OAuth 账号（免费档/订阅）走 chat-proxy billing；API Key 走 api.x.ai 健康探测
            let is_oauth = input
                .auth_method
                .as_deref()
                .is_some_and(|m| m == XAI_OAUTH_METHOD);

            if is_oauth {
                refresh_oauth_billing(input, token).await
            } else {
                refresh_api_key_health(input, token).await
            }
        })
    }
}

/// OAuth 分支：查询 chat-proxy billing（周 + 月）
async fn refresh_oauth_billing(
    input: &BalanceRefreshInput,
    token: &str,
) -> IcodeResult<BalanceSnapshot> {
    let base = resolve_base_url(input, DEFAULT_CHAT_PROXY_BASE_URL);
    let client = reqwest::Client::new();

    // 周额度查询（主请求）
    let weekly_url = format!("{}/billing?format=credits", base);
    let weekly_response = client
        .get(&weekly_url)
        .headers(build_chat_proxy_headers(token))
        .send()
        .await
        .map_err(|e| IcodeError::internal(format!("Grok Build 周额度查询请求失败: {}", e)))?;

    let weekly_status = weekly_response.status().as_u16();
    let weekly_text = weekly_response.text().await.unwrap_or_default();

    // 套餐耗尽 → 直接返回 exhausted 状态
    if is_free_usage_exhausted(weekly_status, &weekly_text) {
        return Ok(exhausted_snapshot());
    }

    if weekly_status != 200 {
        let reason = parse_error(&weekly_text)
            .and_then(|e| e.message)
            .unwrap_or_else(|| format!("HTTP {}", weekly_status));
        return Err(IcodeError::internal(format!(
            "Grok Build 额度查询失败: {}",
            reason
        )));
    }

    let weekly_json: serde_json::Value = serde_json::from_str(&weekly_text)
        .map_err(|e| IcodeError::internal(format!("Grok Build 周额度响应解析失败: {}", e)))?;
    let mut items = parse_billing_payload(&weekly_json);

    // 2) 月额度查询（失败不致命，仅 debug 日志）
    let monthly_url = format!("{}/billing", base);
    if let Ok(response) = client
        .get(&monthly_url)
        .headers(build_chat_proxy_headers(token))
        .send()
        .await
    {
        if response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&text) {
                let monthly_items = parse_billing_payload(&payload);
                // 按月维度合并（月度指标覆盖周响应中同 id 项）
                for m in monthly_items {
                    if m.period == Some(BalancePeriod::Month) {
                        items.retain(|item| item.id != m.id);
                        items.push(m);
                    }
                }
            }
        }
    } else {
        log::debug!("Grok Build 月额度查询跳过（请求失败），仅保留周额度");
    }

    if items.is_empty() {
        return Err(IcodeError::internal("Grok Build 额度响应中无有效指标"));
    }

    Ok(BalanceSnapshot {
        updated_at: now_ms(),
        items,
    })
}

/// API Key 分支：api.x.ai 健康探测（/v1/me + /v1/chat/completions）
async fn refresh_api_key_health(
    input: &BalanceRefreshInput,
    token: &str,
) -> IcodeResult<BalanceSnapshot> {
    let base = resolve_base_url(input, DEFAULT_API_X_AI_BASE_URL);
    let client = reqwest::Client::new();

    // 1) GET /v1/me → profile（user_id / team_id）
    let me_url = format!("{}/me", base);
    let me_response = client
        .get(&me_url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| IcodeError::internal(format!("xAI /me 请求失败: {}", e)))?;

    let me_status = me_response.status().as_u16();
    let me_text = me_response.text().await.unwrap_or_default();
    if me_status != 200 {
        let reason = parse_error(&me_text)
            .and_then(|e| e.message)
            .unwrap_or_else(|| format!("HTTP {}", me_status));
        return Err(IcodeError::internal(format!("xAI 账号验证失败: {}", reason)));
    }

    // 提取 user_id 用于展示
    let mut user_id: Option<String> = None;
    if let Ok(profile) = serde_json::from_str::<serde_json::Value>(&me_text) {
        if is_record(&profile) {
            user_id = ["user_id", "id"]
                .iter()
                .find_map(|k| pick_string(&profile, k))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }
    }

    // 2) POST /v1/chat/completions（max_tokens=1 健康探测）
    let chat_url = format!("{}/chat/completions", base);
    let chat_payload = serde_json::json!({
        "model": "grok-4.5",
        "max_tokens": 1,
        "stream": false
    });
    let chat_response = client
        .post(&chat_url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&chat_payload)
        .send()
        .await;

    // 429 → 套餐耗尽；其余失败以 /me 结果为准（不致命）
    match chat_response {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            if is_free_usage_exhausted(status, &text) {
                return Ok(exhausted_snapshot());
            }
            if !(200..300).contains(&status) {
                log::debug!(
                    "Grok Build 健康探测 POST /chat/completions 非 2xx（HTTP {}），以 /me 结果为准",
                    status
                );
            }
        }
        Err(e) => {
            log::debug!("Grok Build 健康探测 POST /chat/completions 请求失败，以 /me 结果为准: {}", e);
        }
    }

    // 组装可用性快照
    let mut items = vec![status_metric(
        "status",
        BalanceStatusValue::Ok,
        Some("账号有效，套餐可用（无精确剩余额度）"),
    )];
    items[0].primary = Some(true);

    if let Some(uid) = user_id {
        items.push(BalanceMetric {
            id: "user_id".into(),
            metric_type: BalanceMetricType::Status,
            value: Some(serde_json::Value::String(uid)),
            label: Some("xAI 用户 ID".into()),
            ..Default::default()
        });
    }

    Ok(BalanceSnapshot {
        updated_at: now_ms(),
        items,
    })
}