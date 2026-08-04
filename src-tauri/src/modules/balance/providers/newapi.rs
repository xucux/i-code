//! # New API（OneAPI 系分支）额度查询实现
//!
//! 调用两个端点：
//! 1. API Key 余额：`GET {base_url}/api/usage/token`（Bearer apiKey）
//! 2. 用户余额（可选）：`GET {base_url}/api/user/self`（`New-Api-User: userId` + Bearer systemToken）
//!
//! 用户余额通过 quota 转换（默认 /500000）得到标准额度值。
//!
//! 注意：`system_token` 字段可能仍为 `$SECRET:{snowflake_id}$` 引用，需由调用方解析为明文。
//! 当前实现若检测到未解析的引用，会跳过用户余额查询并返回警告状态指标。

use std::future::Future;
use std::pin::Pin;

use crate::error::{IcodeError, IcodeResult};

use super::super::provider::{BalanceProvider, BalanceRefreshInput};
use super::super::types::{
    BalanceDirection, BalanceMetric, BalancePeriod, BalanceSnapshot, BalanceStatusValue,
    NewApiQuotaTransform,
};

const DEFAULT_QUOTA_FIELD: &str = "quota";
const DEFAULT_QUOTA_DIVISOR: f64 = 500000.0;
const DEFAULT_QUOTA_MULTIPLIER: f64 = 1.0;

/// 判断 JSON 值是否为对象
fn is_record(value: &serde_json::Value) -> bool {
    value.is_object()
}

/// 从 JSON 对象中取布尔字段
fn pick_bool(record: &serde_json::Value, key: &str) -> Option<bool> {
    record.get(key).and_then(|v| v.as_bool())
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
#[expect(dead_code)]
fn pick_string(record: &serde_json::Value, key: &str) -> Option<String> {
    record.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// 解包 envelope，提取 data 字段
fn extract_payload(value: &serde_json::Value) -> Option<&serde_json::Value> {
    if !is_record(value) {
        return None;
    }
    if let Some(data) = value.get("data") {
        if is_record(data) {
            return Some(data);
        }
    }
    Some(value)
}

/// 解析 envelope，提取 (payload, message, success)
fn parse_envelope(value: &serde_json::Value) -> (Option<&serde_json::Value>, Option<String>, Option<bool>) {
    let payload = extract_payload(value);
    let message = value
        .get("message")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let success = value.get("code").and_then(|v| {
        if let Some(b) = v.as_bool() {
            return Some(b);
        }
        if let Some(n) = v.as_f64() {
            if n.is_finite() {
                return Some(n == 0.0);
            }
        }
        if let Some(s) = v.as_str() {
            let trimmed = s.trim().to_lowercase();
            if trimmed.is_empty() {
                return None;
            }
            if let Ok(n) = trimmed.parse::<f64>() {
                return Some(n == 0.0);
            }
            if trimmed == "true" || trimmed == "ok" {
                return Some(true);
            }
            if trimmed == "false" {
                return Some(false);
            }
        }
        None
    });

    (payload, message, success)
}

/// 规范化 quota 转换配置，填充默认值
fn normalize_quota_transform(transform: Option<&NewApiQuotaTransform>) -> (String, Vec<String>, f64, f64) {
    let quota_field = transform
        .and_then(|t| t.quota_field.as_deref().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_QUOTA_FIELD.to_string());

    let extra_fields = transform
        .map(|t| {
            t.extra_quota_fields
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let divisor = transform
        .and_then(|t| t.divisor)
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(DEFAULT_QUOTA_DIVISOR);

    let multiplier = transform
        .and_then(|t| t.multiplier)
        .filter(|m| m.is_finite())
        .unwrap_or(DEFAULT_QUOTA_MULTIPLIER);

    (quota_field, extra_fields, divisor, multiplier)
}

/// 根据 quota 转换配置计算余额
fn calculate_quota_balance(payload: &serde_json::Value, transform: Option<&NewApiQuotaTransform>) -> Option<f64> {
    let (quota_field, extra_fields, divisor, multiplier) = normalize_quota_transform(transform);
    let mut fields = vec![quota_field];
    fields.extend(extra_fields);

    let mut has_quota = false;
    let mut raw_quota = 0.0;
    for field in &fields {
        if let Some(quota) = pick_number(payload, field) {
            has_quota = true;
            raw_quota += quota;
        }
    }

    if !has_quota {
        return None;
    }
    Some(raw_quota / divisor * multiplier)
}

pub struct NewApiBalanceProvider;

impl BalanceProvider for NewApiBalanceProvider {
    fn method(&self) -> &'static str {
        "newapi"
    }

    fn refresh<'a>(
        &'a self,
        input: &'a BalanceRefreshInput,
    ) -> Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>> {
        Box::pin(async move {
            let api_key = input.api_key.as_deref().ok_or_else(|| {
                IcodeError::validation("New API 额度查询需要 API Key")
            })?;

            let base = input.base_url.as_deref().unwrap_or("https://api.newapi.com");
            let base = base.trim_end_matches('/');
            let client = super::super::provider::build_balance_http_client()?;
            let mut items: Vec<BalanceMetric> = Vec::new();

            // 1. 查询 API Key 余额
            let key_endpoint = format!("{}/api/usage/token", base);
            let key_response = client
                .get(&key_endpoint)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Accept", "application/json")
                .send()
                .await
                .map_err(|e| IcodeError::internal(format!("New API Key 余额查询请求失败: {}", e)))?;

            if !key_response.status().is_success() {
                let status = key_response.status().as_u16();
                let text = key_response.text().await.unwrap_or_default();
                return Err(IcodeError::internal(format!(
                    "New API Key 余额查询失败 (HTTP {}): {}",
                    status,
                    text.chars().take(200).collect::<String>()
                )));
            }

            let key_json: serde_json::Value = key_response
                .json()
                .await
                .map_err(|e| IcodeError::internal(format!("New API Key 余额响应解析失败: {}", e)))?;

            let (key_payload, key_msg, key_success) = parse_envelope(&key_json);
            if key_success == Some(false) {
                return Err(IcodeError::internal(
                    key_msg.unwrap_or_else(|| "New API Key 余额响应异常".to_string())
                ));
            }

            let key_payload = key_payload.ok_or_else(|| {
                IcodeError::internal("New API Key 余额响应中无数据")
            })?;

            // 检查是否无限额度
            let unlimited = pick_bool(key_payload, "unlimited_quota")
                .or_else(|| pick_bool(key_payload, "unlimitedQuota"))
                .unwrap_or(false);

            if unlimited {
                items.push(
                    BalanceMetric::status(
                        "api-key-unlimited",
                        BalanceStatusValue::Unlimited,
                        None,
                    )
                    .with_period(BalancePeriod::Current)
                    .with_scope("api-key")
                    .with_label("API Key 余额"),
                );
            } else {
                let total_available = pick_number(key_payload, "total_available")
                    .or_else(|| pick_number(key_payload, "available"))
                    .or_else(|| pick_number(key_payload, "balance"));

                if let Some(available) = total_available {
                    items.push(
                        BalanceMetric::amount(
                            "api-key-balance",
                            BalanceDirection::Remaining,
                            serde_json::json!(available),
                            Some("$"),
                        )
                        .with_period(BalancePeriod::Current)
                        .with_scope("api-key")
                        .with_label("API Key 余额"),
                    );
                } else {
                    items.push(
                        BalanceMetric::status(
                            "api-key-unavailable",
                            BalanceStatusValue::Unavailable,
                            None,
                        )
                        .with_period(BalancePeriod::Current)
                        .with_scope("api-key")
                        .with_label("API Key 余额"),
                    );
                }
            }

            // 2. 查询用户余额（可选，需要 userId + systemToken）
            if let Some(config) = &input.newapi_config {
                let user_id = config.user_id.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty());
                let system_token = config.system_token.as_deref().filter(|s| !s.is_empty());

                if let (Some(user_id), Some(system_token)) = (user_id, system_token) {
                    // 检查 system_token 是否仍为 $SECRET 引用（未解析）
                    if system_token.starts_with("$SECRET:") {
                        items.push(
                            BalanceMetric::status(
                                "user-error",
                                BalanceStatusValue::Error,
                                Some("系统 Token 未解析，无法查询用户余额"),
                            )
                            .with_period(BalancePeriod::Current)
                            .with_scope("user")
                            .with_label("用户余额"),
                        );
                    } else {
                        match fetch_user_balance(&client, base, user_id, system_token, &config.quota_transform).await {
                            Ok(user_items) => {
                                items.extend(user_items);
                            }
                            Err(e) => {
                                items.push(
                                    BalanceMetric::status(
                                        "user-error",
                                        BalanceStatusValue::Error,
                                        Some(&e.message),
                                    )
                                    .with_period(BalancePeriod::Current)
                                    .with_scope("user")
                                    .with_label("用户余额"),
                                );
                            }
                        }
                    }
                }
            }

            // 分配主指标：优先用户余额 amount，其次 API Key amount，最后第一个
            assign_primary(&mut items);

            Ok(BalanceSnapshot {
                updated_at: now_ms(),
                items,
            })
        })
    }
}

/// 查询 New API 用户余额
async fn fetch_user_balance(
    client: &reqwest::Client,
    base: &str,
    user_id: &str,
    system_token: &str,
    quota_transform: &Option<NewApiQuotaTransform>,
) -> IcodeResult<Vec<BalanceMetric>> {
    let endpoint = format!("{}/api/user/self", base);
    let response = client
        .get(&endpoint)
        .header("New-Api-User", user_id)
        .header("Authorization", format!("Bearer {}", system_token))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| IcodeError::internal(format!("New API 用户余额查询请求失败: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        return Err(IcodeError::internal(format!(
            "New API 用户余额查询失败 (HTTP {}): {}",
            status,
            text.chars().take(200).collect::<String>()
        )));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| IcodeError::internal(format!("New API 用户余额响应解析失败: {}", e)))?;

    let (payload, msg, success) = parse_envelope(&json);
    if success == Some(false) {
        return Err(IcodeError::internal(
            msg.unwrap_or_else(|| "New API 用户余额响应异常".to_string())
        ));
    }

    let payload = payload.ok_or_else(|| {
        IcodeError::internal("New API 用户余额响应中无数据")
    })?;

    let actual_quota = calculate_quota_balance(payload, quota_transform.as_ref());

    let items = if let Some(quota) = actual_quota {
        vec![
            BalanceMetric::amount(
                "user-balance",
                BalanceDirection::Remaining,
                serde_json::json!(quota),
                Some("$"),
            )
            .with_period(BalancePeriod::Current)
            .with_scope("user")
            .with_label("用户余额"),
        ]
    } else {
        vec![
            BalanceMetric::status(
                "user-unavailable",
                BalanceStatusValue::Unavailable,
                None,
            )
            .with_period(BalancePeriod::Current)
            .with_scope("user")
            .with_label("用户余额"),
        ]
    };

    Ok(items)
}

/// 分配主指标
fn assign_primary(items: &mut [BalanceMetric]) {
    let target_id = items
        .iter()
        .find(|item| {
            item.metric_type == super::super::types::BalanceMetricType::Amount
                && item.scope.as_deref() == Some("user")
                && item.direction == Some(BalanceDirection::Remaining)
        })
        .or_else(|| {
            items.iter().find(|item| {
                item.metric_type == super::super::types::BalanceMetricType::Amount
                    && item.scope.as_deref() == Some("api-key")
                    && item.direction == Some(BalanceDirection::Remaining)
            })
        })
        .or_else(|| items.first())
        .map(|i| i.id.clone());

    if let Some(tid) = target_id {
        for item in items.iter_mut() {
            item.primary = Some(item.id == tid);
        }
    }
}

/// 获取当前毫秒时间戳
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
