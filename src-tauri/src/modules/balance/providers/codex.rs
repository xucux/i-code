//! # OpenAI Codex 额度查询实现
//!
//! 调用 Codex usage API（多端点探测）：
//! - `{origin}{prefix}/backend-api/wham/usage`
//! - `{origin}{prefix}/api/codex/usage`
//!
//! prefix 通过从 base_url path 中剥离已知段（如 `/backend-api/wham/usage`、`/v1/responses`）推导。
//!
//! 响应中的 `rate_limit` 与 `additional_rate_limits` 数组每项含
//! `primary_window` / `secondary_window`，各有 `used_percent` 与 `reset_at`。

use std::future::Future;
use std::pin::Pin;

use crate::error::{IcodeError, IcodeResult};

use super::super::provider::{BalanceProvider, BalanceRefreshInput};
use super::super::types::{BalanceDirection, BalanceMetric, BalanceMetricType, BalancePeriod, BalanceSnapshot};

/// 解析后的 reset 时间
struct ParsedResetAt {
    value: String,
    timestamp_ms: Option<i64>,
}

/// 解析后的速率限制指标
struct ParsedLimitMetric {
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

/// 规范化 path：折叠多余斜杠、去除尾部斜杠
fn normalize_path(pathname: &str) -> String {
    let collapsed = pathname.replace("//", "/");
    let trimmed = collapsed.trim_end_matches('/');
    if trimmed == "/" {
        String::new()
    } else {
        trimmed.to_string()
    }
}

/// 拼接 path 前后缀
fn join_path(prefix: &str, suffix: &str) -> String {
    let combined = format!("{}/{}", prefix, suffix).replace("//", "/");
    if combined.starts_with('/') {
        combined
    } else {
        format!("/{}", combined)
    }
}

/// 从 path 中推导前缀（剥离已知的 API 段）
fn derive_path_prefix(pathname: &str) -> String {
    let normalized = normalize_path(pathname);
    if normalized.is_empty() {
        return String::new();
    }
    let lower = normalized.to_lowercase();
    let known_segments = [
        "/backend-api/wham/usage",
        "/backend-api/codex/responses",
        "/backend-api/codex",
        "/backend-api",
        "/api/codex/usage",
        "/api/codex",
        "/api",
        "/v1/responses",
        "/v1",
    ];
    for segment in known_segments.iter() {
        if let Some(index) = lower.find(segment) {
            return if index == 0 {
                String::new()
            } else {
                normalized[..index].to_string()
            };
        }
    }
    normalized
}

/// 解析 base_url 为 (origin, pathname)
fn parse_base_url(raw: &str) -> Option<(String, String)> {
    let trimmed = raw.trim();
    let scheme_end = trimmed.find("://")?;
    let scheme = &trimmed[..scheme_end];
    if scheme.is_empty() || !scheme.chars().all(|c| c.is_alphanumeric() || c == '+' || c == '-' || c == '.') {
        return None;
    }
    let rest = &trimmed[scheme_end + 3..];
    let path_start = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..path_start];
    if authority.is_empty() {
        return None;
    }
    let origin = format!("{}://{}", scheme, authority);
    let pathname = if path_start < rest.len() {
        rest[path_start..].to_string()
    } else {
        String::new()
    };
    Some((origin, pathname))
}

/// 构建 Codex usage 端点列表（去重）
fn build_codex_usage_endpoints(raw_base_url: &str) -> Vec<String> {
    let (origin, pathname) = match parse_base_url(raw_base_url) {
        Some(v) => v,
        None => return Vec::new(),
    };

    let mut prefixes = vec![derive_path_prefix(&pathname), String::new()];
    prefixes.dedup();

    let mut endpoints = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for prefix in &prefixes {
        for suffix in ["backend-api/wham/usage", "api/codex/usage"] {
            let path = join_path(prefix, suffix);
            let endpoint = format!("{}{}", origin, path);
            if seen.insert(endpoint.clone()) {
                endpoints.push(endpoint);
            }
        }
    }
    endpoints
}

/// 解析 reset 时间值
fn normalize_reset_at(value: &serde_json::Value) -> Option<ParsedResetAt> {
    if let Some(n) = value.as_f64() {
        if n.is_finite() {
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
        if let Ok(n) = trimmed.parse::<f64>() {
            if n.is_finite() {
                let millis = if n > 1e12 { n as i64 } else { (n * 1000.0) as i64 };
                return Some(ParsedResetAt {
                    value: format_iso_from_millis(millis),
                    timestamp_ms: Some(millis),
                });
            }
        }
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

/// 解析单个 rate_limit 窗口
fn parse_rate_limit_window(
    rate_limit: &serde_json::Value,
    key: &str,
) -> Option<(f64, Option<ParsedResetAt>)> {
    let raw_window = rate_limit.get(key).filter(|v| is_record(v))?;
    let used_percent = pick_number(raw_window, "used_percent")?;
    let reset_at = raw_window.get("reset_at").and_then(normalize_reset_at);
    Some((clamp_percent(used_percent), reset_at))
}

/// 解析 rate_limit 下的 primary/secondary 窗口
fn parse_rate_limit_metrics(
    rate_limit: &serde_json::Value,
    label: &str,
    scope: Option<&str>,
) -> Vec<ParsedLimitMetric> {
    let mut metrics = Vec::new();
    for (key, window_label) in [
        ("primary_window", "5 小时限额"),
        ("secondary_window", "周限额"),
    ] {
        if let Some((used_percent, reset_at)) = parse_rate_limit_window(rate_limit, key) {
            metrics.push(ParsedLimitMetric {
                label: window_label.to_string(),
                scope: scope.map(|s| s.to_string()),
                used_percent,
                reset_at,
            });
        }
    }
    // label 为 "Primary rate limit" 时使用标准窗口名，否则附加窗口名
    if label != "Primary rate limit" {
        let _ = label; // 保留参数语义
    }
    metrics
}

/// 判断是否为 usage quota 结构
fn has_usage_quota_shape(record: &serde_json::Value) -> bool {
    record.get("rate_limit").map_or(false, |v| is_record(v))
        || record.get("additional_rate_limits").map_or(false, |v| v.is_array())
}

/// 解析 usage payload
fn resolve_usage_payload(value: &serde_json::Value) -> Option<&serde_json::Value> {
    if !is_record(value) {
        return None;
    }
    if has_usage_quota_shape(value) {
        return Some(value);
    }
    for key in ["data", "usage", "result", "payload"] {
        if let Some(nested) = value.get(key) {
            if is_record(nested) && has_usage_quota_shape(nested) {
                return Some(nested);
            }
        }
    }
    Some(value)
}

/// 解析 usage metrics
fn parse_usage_metrics(payload: &serde_json::Value) -> Vec<ParsedLimitMetric> {
    let body = match resolve_usage_payload(payload) {
        Some(b) => b,
        None => return Vec::new(),
    };

    let mut metrics = Vec::new();

    // 主 rate_limit
    if let Some(primary_rate_limit) = body.get("rate_limit").filter(|v| is_record(v)) {
        let parsed = parse_rate_limit_metrics(primary_rate_limit, "Primary rate limit", None);
        metrics.extend(parsed);
    }

    // additional_rate_limits 数组
    if let Some(additional) = body.get("additional_rate_limits").and_then(|v| v.as_array()) {
        for (index, item) in additional.iter().enumerate() {
            if !is_record(item) {
                continue;
            }
            let rate_limit = match item.get("rate_limit").filter(|v| is_record(v)) {
                Some(r) => r,
                None => continue,
            };

            let name = pick_string(item, "name")
                .or_else(|| pick_string(item, "label"))
                .or_else(|| pick_string(item, "scope"))
                .or_else(|| pick_string(item, "id"))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let label = name.clone().unwrap_or_else(|| format!("Additional rate limit {}", index + 1));
            let parsed = parse_rate_limit_metrics(rate_limit, &label, name.as_deref());
            metrics.extend(parsed);
        }
    }

    metrics
}

/// 构建快照指标
fn build_snapshot_items(metrics: &[ParsedLimitMetric]) -> Vec<BalanceMetric> {
    let mut items: Vec<BalanceMetric> = Vec::new();
    let mut primary_index: Option<usize> = None;
    let mut highest_percent = f64::NEG_INFINITY;

    for (index, metric) in metrics.iter().enumerate() {
        let prefix = format!("rate-limit-{}", index + 1);
        let percent_idx = items.len();

        let mut percent_metric = BalanceMetric::percent(
            format!("{}-used-percent", prefix),
            metric.used_percent,
            Some(BalanceDirection::Used),
        )
        .with_period(BalancePeriod::Current)
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
            .with_period(BalancePeriod::Current)
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

pub struct CodexBalanceProvider;

impl BalanceProvider for CodexBalanceProvider {
    fn method(&self) -> &'static str {
        "codex"
    }

    fn refresh<'a>(
        &'a self,
        input: &'a BalanceRefreshInput,
    ) -> Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>> {
        Box::pin(async move {
            let token = input.api_key.as_deref().ok_or_else(|| {
                IcodeError::validation("Codex 额度查询需要 Access Token")
            })?;

            let base = input.base_url.as_deref().unwrap_or("https://chatgpt.com");
            let endpoints = build_codex_usage_endpoints(base);
            if endpoints.is_empty() {
                return Err(IcodeError::internal("Codex 额度查询端点解析失败"));
            }

            let client = reqwest::Client::new();
            let account_id = input.account_id.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty());

            let mut endpoint_errors: Vec<String> = Vec::new();
            let mut missing_percent_error: Option<String> = None;

            for endpoint in &endpoints {
                let mut request = client
                    .get(endpoint)
                    .header("Authorization", format!("Bearer {}", token))
                    .header("Accept", "application/json");

                if let Some(acc) = account_id {
                    request = request.header("ChatGPT-Account-Id", acc);
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

                let usage_metrics = parse_usage_metrics(&payload);
                if usage_metrics.is_empty() {
                    missing_percent_error = Some(format!(
                        "Codex 来自 {} 的响应未包含 quota 百分比",
                        endpoint
                    ));
                    continue;
                }

                return Ok(BalanceSnapshot {
                    updated_at: now_ms(),
                    items: build_snapshot_items(&usage_metrics),
                });
            }

            if let Some(err) = missing_percent_error {
                return Err(IcodeError::internal(err));
            }
            if !endpoint_errors.is_empty() {
                return Err(IcodeError::internal(format!(
                    "Codex 额度查询所有端点均失败: {}",
                    endpoint_errors.join(" | ")
                )));
            }
            Err(IcodeError::internal("Codex 额度查询失败"))
        })
    }
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
