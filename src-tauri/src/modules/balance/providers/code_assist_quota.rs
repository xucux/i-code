//! # Code Assist 额度查询共享实现
//!
//! 被 `antigravity` 与 `gemini-cli` 复用，调用 Google Code Assist 的
//! `retrieveUserQuota` 接口获取各 quota bucket 的剩余比例。
//!
//! 端点：`POST {base}/v1internal:retrieveUserQuota`，请求体 `{ "project": "<projectId>" }`
//!
//! 响应中的 `buckets` 数组每项含 `remainingFraction`（0-1），
//! 据此推导已用百分比并附带 `resetTime` 重置时间。

use crate::error::{IcodeError, IcodeResult};

use super::super::provider::BalanceRefreshInput;
use super::super::types::{BalanceDirection, BalanceMetric, BalanceSnapshot};

/// Code Assist 查询选项
pub struct CodeAssistQuotaOptions<'a> {
    /// 供应商名称，用于错误消息
    pub provider_name: &'a str,
    /// 端点 fallback 列表
    pub endpoint_fallbacks: &'a [&'a str],
    /// 额外请求头
    pub request_headers: &'a [(&'a str, &'a str)],
    /// 解析 project id（使用 fn 指针，自动满足 Send + Sync）
    pub resolve_project_id: fn(&BalanceRefreshInput) -> Option<String>,
}

/// 解析后的 reset 时间
struct ParsedResetAt {
    value: String,
    timestamp_ms: Option<i64>,
}

/// 解析后的 bucket 指标
struct ParsedBucketMetric {
    label: String,
    scope: Option<String>,
    used_percent: f64,
    reset_at: Option<ParsedResetAt>,
}

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

/// 限制百分比在 [0, 100] 范围内
fn clamp_percent(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    value.max(0.0).min(100.0)
}

/// 解析错误响应文本，提取 message
fn parse_error_message(text: &str) -> Option<String> {
    let normalized = text.trim();
    if normalized.is_empty() {
        return None;
    }
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(normalized) {
        if is_record(&parsed) {
            if let Some(direct) = pick_string(&parsed, "message").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
                return Some(direct);
            }
            if let Some(error) = parsed.get("error") {
                if is_record(error) {
                    if let Some(msg) = pick_string(error, "message").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
                        return Some(msg);
                    }
                }
            }
        }
    }
    Some(normalized.to_string())
}

/// 规范化 base URL（去除尾部斜杠）
fn normalize_base_url(raw: &str) -> String {
    raw.trim_end_matches('/').to_string()
}

/// 解析 quota 端点列表（去重）
fn resolve_quota_endpoints(raw_base_url: &str, fallbacks: &[&str]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut endpoints = Vec::new();
    let candidates = std::iter::once(raw_base_url).chain(fallbacks.iter().copied());
    for candidate in candidates {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = normalize_base_url(trimmed);
        if !seen.insert(normalized.clone()) {
            continue;
        }
        endpoints.push(format!("{}/v1internal:retrieveUserQuota", normalized));
    }
    endpoints
}

/// 解析 reset 时间值
fn normalize_reset_at(value: &serde_json::Value) -> Option<ParsedResetAt> {
    if let Some(n) = value.as_f64() {
        if n.is_finite() {
            // 大于 1e12 视为毫秒，否则视为秒
            let millis = if n > 1e12 { n as i64 } else { (n * 1000.0) as i64 };
            return Some(ParsedResetAt {
                value: format_iso_from_millis(millis),
                timestamp_ms: Some(millis),
            });
        }
    }
    if let Some(s) = value.as_str() {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return None;
        }
        // 尝试数字
        if let Ok(n) = trimmed.parse::<f64>() {
            if n.is_finite() {
                let millis = if n > 1e12 { n as i64 } else { (n * 1000.0) as i64 };
                return Some(ParsedResetAt {
                    value: format_iso_from_millis(millis),
                    timestamp_ms: Some(millis),
                });
            }
        }
        // 尝试 ISO
        if let Some(millis) = parse_iso_to_millis(trimmed) {
            return Some(ParsedResetAt {
                value: format_iso_from_millis(millis),
                timestamp_ms: Some(millis),
            });
        }
        return Some(ParsedResetAt {
            value: trimmed.to_string(),
            timestamp_ms: None,
        });
    }
    None
}

/// 递归查找 buckets 数组
fn find_buckets(value: &serde_json::Value, depth: u32) -> Vec<&serde_json::Value> {
    if depth > 4 || !is_record(value) {
        return Vec::new();
    }
    if let Some(buckets) = value.get("buckets").and_then(|v| v.as_array()) {
        return buckets.iter().filter(|v| is_record(v)).collect();
    }
    for key in ["quota", "userQuota", "data", "result", "payload"] {
        if let Some(nested) = value.get(key) {
            let buckets = find_buckets(nested, depth + 1);
            if !buckets.is_empty() {
                return buckets;
            }
        }
    }
    Vec::new()
}

/// 解析 buckets，构建指标列表
fn parse_bucket_metrics(payload: &serde_json::Value) -> Vec<ParsedBucketMetric> {
    let buckets = find_buckets(payload, 0);
    if buckets.is_empty() {
        return Vec::new();
    }

    let mut metrics = Vec::new();
    for (index, bucket) in buckets.iter().enumerate() {
        let remaining_fraction = match pick_number(bucket, "remainingFraction") {
            Some(v) => v,
            None => continue,
        };

        let used_percent = clamp_percent((1.0 - remaining_fraction) * 100.0);

        let model_id = pick_string(bucket, "modelId").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let token_type = pick_string(bucket, "tokenType").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let quota_id = pick_string(bucket, "quotaId").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

        let label = model_id
            .clone()
            .or_else(|| quota_id.clone())
            .or_else(|| token_type.clone())
            .unwrap_or_else(|| format!("Quota Bucket {}", index + 1));

        let scope = token_type.or(quota_id);
        let reset_at = bucket
            .get("resetTime")
            .or_else(|| bucket.get("resetAt"))
            .or_else(|| bucket.get("reset_at"))
            .and_then(normalize_reset_at);

        metrics.push(ParsedBucketMetric {
            label,
            scope,
            used_percent,
            reset_at,
        });
    }
    metrics
}

/// 构建快照指标
fn build_snapshot_items(metrics: &[ParsedBucketMetric]) -> Vec<BalanceMetric> {
    let mut items: Vec<BalanceMetric> = Vec::new();
    let mut primary_index: Option<usize> = None;
    let mut highest_percent = f64::NEG_INFINITY;

    for (index, metric) in metrics.iter().enumerate() {
        let prefix = format!("quota-bucket-{}", index + 1);
        let percent_idx = items.len();

        let mut percent_metric = BalanceMetric::percent(
            format!("{}-used-percent", prefix),
            metric.used_percent,
            Some(BalanceDirection::Used),
        )
        .with_period(super::super::types::BalancePeriod::Current)
        .with_label(&metric.label);
        if let Some(scope) = &metric.scope {
            percent_metric = percent_metric.with_scope(scope);
        }
        items.push(percent_metric);

        if metric.used_percent > highest_percent {
            highest_percent = metric.used_percent;
            primary_index = Some(percent_idx);
        }

        if let Some(reset) = &metric.reset_at {
            let mut time_metric = BalanceMetric::time(
                format!("{}-reset-at", prefix),
                "resetAt",
                reset.value.clone(),
                reset.timestamp_ms,
            )
            .with_period(super::super::types::BalancePeriod::Current)
            .with_label(&metric.label);
            if let Some(scope) = &metric.scope {
                time_metric = time_metric.with_scope(scope);
            }
            items.push(time_metric);
        }
    }

    if let Some(idx) = primary_index {
        if idx < items.len() {
            items[idx].primary = Some(true);
        }
    }

    items
}

/// 执行 Code Assist quota 查询
pub fn refresh_code_assist_quota<'a>(
    input: &'a BalanceRefreshInput,
    options: &'a CodeAssistQuotaOptions<'a>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>> {
    let provider_name = options.provider_name.to_string();
    let endpoint_fallbacks: Vec<String> = options.endpoint_fallbacks.iter().map(|s| s.to_string()).collect();
    let request_headers: Vec<(String, String)> = options
        .request_headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let resolve_project_id = options.resolve_project_id;

    Box::pin(async move {
        let token = input.api_key.as_deref().ok_or_else(|| {
            IcodeError::validation(format!("{} 额度查询需要 Access Token", provider_name))
        })?;

        let project_id = resolve_project_id(input)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                IcodeError::validation(format!("{} 额度查询需要 Project ID", provider_name))
            })?;

        let raw_base = input.base_url.as_deref().unwrap_or("");
        let fallback_refs: Vec<&str> = endpoint_fallbacks.iter().map(|s| s.as_str()).collect();
        let endpoints = resolve_quota_endpoints(raw_base, &fallback_refs);
        if endpoints.is_empty() {
            return Err(IcodeError::internal(format!("{} 额度查询端点解析失败", provider_name)));
        }

        let client = reqwest::Client::new();
        let mut endpoint_errors: Vec<String> = Vec::new();
        let mut missing_percent_error: Option<String> = None;

        for endpoint in &endpoints {
            let mut request = client
                .post(endpoint)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/json")
                .header("Content-Type", "application/json")
                .body(serde_json::json!({ "project": project_id }).to_string());

            for (key, value) in &request_headers {
                request = request.header(key, value);
            }

            let response = match request.send().await {
                Ok(r) => r,
                Err(e) => {
                    endpoint_errors.push(format!("{}: {}", endpoint, e));
                    continue;
                }
            };

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let text = response.text().await.unwrap_or_default();
                let reason = parse_error_message(&text).unwrap_or_else(|| format!("HTTP {}", status));
                endpoint_errors.push(format!("{}: {}", endpoint, reason));
                continue;
            }

            let payload: serde_json::Value = match response.json().await {
                Ok(p) => p,
                Err(e) => {
                    endpoint_errors.push(format!("{}: {}", endpoint, e));
                    continue;
                }
            };

            let bucket_metrics = parse_bucket_metrics(&payload);
            if bucket_metrics.is_empty() {
                missing_percent_error = Some(format!(
                    "{} 来自 {} 的响应未包含 quota 百分比",
                    provider_name, endpoint
                ));
                continue;
            }

            return Ok(BalanceSnapshot {
                updated_at: now_ms(),
                items: build_snapshot_items(&bucket_metrics),
            });
        }

        if let Some(err) = missing_percent_error {
            return Err(IcodeError::internal(err));
        }
        if !endpoint_errors.is_empty() {
            return Err(IcodeError::internal(format!(
                "{} 额度查询所有端点均失败: {}",
                provider_name,
                endpoint_errors.join(" | ")
            )));
        }
        Err(IcodeError::internal(format!("{} 额度查询失败", provider_name)))
    })
}

/// 将毫秒时间戳格式化为 ISO 8601 字符串
fn format_iso_from_millis(millis: i64) -> String {
    let secs = millis / 1000;
    let millis_part = (millis % 1000) as u32;
    let days = secs.div_euclid(86400);
    let remainder = secs.rem_euclid(86400);
    let hour = (remainder / 3600) as u32;
    let minute = ((remainder % 3600) / 60) as u32;
    let second = (remainder % 60) as u32;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, month, day, hour, minute, second, millis_part
    )
}

/// 解析 ISO 8601 / RFC3339 时间字符串为毫秒时间戳
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

/// 将 Unix 天数转换为 (year, month, day)
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m, d)
}

/// 获取当前毫秒时间戳
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
