//! # AIHubMix 额度查询实现
//!
//! 调用 AIHubMix remain API：`GET {origin}/dashboard/billing/remain`
//! 响应格式：`{ data: { total_usage } }` 或 `{ total_usage }`
//!
//! 特殊处理：
//! - `total_usage === -0.000002` 表示无限额度
//! - 错误响应中包含 `quota exhausted` 表示额度耗尽

use std::future::Future;
use std::pin::Pin;

use crate::error::{IcodeError, IcodeResult};

use super::super::provider::{BalanceProvider, BalanceRefreshInput};
use super::super::types::{
    BalanceDirection, BalanceMetric, BalancePeriod, BalanceSnapshot, BalanceStatusValue,
};

/// 无限额度的特殊标记值
const AIHUBMIX_INFINITE_REMAINING: f64 = -0.000002;

/// 判断 JSON 值是否为对象
fn is_record(value: &serde_json::Value) -> bool {
    value.is_object()
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

/// 从 JSON 对象中取字符串字段
fn pick_string(record: &serde_json::Value, key: &str) -> Option<String> {
    record.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// 解析 AIHubMix 错误响应，提取 (message, quota_exhausted)
fn parse_aihubmix_error(text: &str) -> (Option<String>, bool) {
    let normalized = text.trim();
    if normalized.is_empty() {
        return (None, false);
    }

    let mut message = Some(normalized.to_string());
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(normalized) {
        if is_record(&parsed) {
            if let Some(direct) = pick_string(&parsed, "message").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
                message = Some(direct);
            }
            if let Some(error) = parsed.get("error") {
                if is_record(error) {
                    if let Some(err_msg) = pick_string(error, "message").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
                        message = Some(err_msg);
                    }
                }
            }
        }
    }

    let quota_exhausted = message
        .as_deref()
        .map(|m| m.to_lowercase().contains("quota exhausted"))
        .unwrap_or(false);

    (message, quota_exhausted)
}

/// 从 base_url 解析 origin（scheme + host[:port]）
///
/// 手动解析以避免引入 `url` 显式依赖。
/// 支持 `https://host:port/path` 与 `https://host/path` 两种形式。
fn resolve_origin(base_url: &str) -> Option<String> {
    let trimmed = base_url.trim();
    // 查找 scheme 分隔符 "://"
    let scheme_end = trimmed.find("://")?;
    let scheme = &trimmed[..scheme_end];
    if scheme.is_empty() || !scheme.chars().all(|c| c.is_alphanumeric() || c == '+' || c == '-' || c == '.') {
        return None;
    }
    let rest = &trimmed[scheme_end + 3..];
    // host[:port] 部分到第一个 '/' 或 '?' 或 '#' 结束
    let end = rest.find(|c: char| c == '/' || c == '?' || c == '#').unwrap_or(rest.len());
    let authority = &rest[..end];
    if authority.is_empty() {
        return None;
    }
    Some(format!("{}://{}", scheme, authority))
}

/// 构建剩余额度查询端点
fn resolve_remain_endpoint(base_url: &str) -> Option<String> {
    let origin = resolve_origin(base_url)?;
    Some(format!("{}/dashboard/billing/remain", origin.trim_end_matches('/')))
}

pub struct AihubmixBalanceProvider;

impl BalanceProvider for AihubmixBalanceProvider {
    fn method(&self) -> &'static str {
        "aihubmix"
    }

    fn refresh<'a>(
        &'a self,
        input: &'a BalanceRefreshInput,
    ) -> Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>> {
        Box::pin(async move {
            let api_key = input.api_key.as_deref().ok_or_else(|| {
                IcodeError::validation("AIHubMix 额度查询需要 API Key")
            })?;

            let base = input.base_url.as_deref().unwrap_or("https://aihubmix.com");
            let endpoint = resolve_remain_endpoint(base).ok_or_else(|| {
                IcodeError::internal(format!("AIHubMix 端点解析失败: {}", base))
            })?;

            let client = super::super::provider::build_balance_http_client()?;
            let mut request = client
                .get(&endpoint)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Accept", "application/json");

            if let Some(app_code) = input.app_code.as_deref() {
                let trimmed = app_code.trim();
                if !trimmed.is_empty() {
                    request = request.header("APP-Code", trimmed);
                }
            }

            let response = request
                .send()
                .await
                .map_err(|e| IcodeError::internal(format!("AIHubMix 额度查询请求失败: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let text = response.text().await.unwrap_or_default();
                let (message, quota_exhausted) = parse_aihubmix_error(&text);
                if quota_exhausted {
                    return Ok(BalanceSnapshot {
                        updated_at: now_ms(),
                        items: vec![BalanceMetric::status(
                            "status-current",
                            BalanceStatusValue::Exhausted,
                            Some("额度已耗尽"),
                        )
                        .with_period(BalancePeriod::Current)
                        .with_primary(true)
                        .with_label("状态")],
                    });
                }
                return Err(IcodeError::internal(
                    message.unwrap_or_else(|| format!("AIHubMix 额度查询失败 (HTTP {})", status))
                ));
            }

            let json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| IcodeError::internal(format!("AIHubMix 额度响应解析失败: {}", e)))?;

            if !is_record(&json) {
                return Err(IcodeError::internal("AIHubMix 额度响应异常"));
            }

            let payload = json.get("data").filter(|v| is_record(v)).unwrap_or(&json);
            let remaining = pick_number(payload, "total_usage");

            let remaining = remaining.ok_or_else(|| {
                IcodeError::internal("AIHubMix 额度响应中无 total_usage 字段")
            })?;

            // 无限额度
            if (remaining - AIHUBMIX_INFINITE_REMAINING).abs() < 1e-9 {
                return Ok(BalanceSnapshot {
                    updated_at: now_ms(),
                    items: vec![BalanceMetric::status(
                        "status-current",
                        BalanceStatusValue::Unlimited,
                        None,
                    )
                    .with_period(BalancePeriod::Current)
                    .with_primary(true)
                    .with_label("状态")],
                });
            }

            Ok(BalanceSnapshot {
                updated_at: now_ms(),
                items: vec![BalanceMetric::amount(
                    "balance-current",
                    BalanceDirection::Remaining,
                    serde_json::json!(remaining),
                    Some("$"),
                )
                .with_period(BalancePeriod::Current)
                .with_primary(true)
                .with_label("余额")],
            })
        })
    }
}

/// 获取当前毫秒时间戳
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
