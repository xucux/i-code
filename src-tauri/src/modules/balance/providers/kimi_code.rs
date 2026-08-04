//! # Kimi Code（Kimi 国际站）额度查询实现
//!
//! 调用 Kimi Code usages API：`GET {base_url}/v1/usages`
//! 响应结构较为动态，使用 `serde_json::Value` 解析。
//!
//! 响应示例：
//! ```json
//! {
//!   "data": {
//!     "usage": { "limit": 1000, "used": 200, "name": "Weekly usage" },
//!     "limits": [
//!       { "detail": { "limit": 100, "used": 10 }, "window": { "duration": 60, "timeUnit": "MINUTE" } }
//!     ]
//!   }
//! }
//! ```

use std::future::Future;
use std::pin::Pin;

use crate::error::{IcodeError, IcodeResult};

use super::super::provider::{BalanceProvider, BalanceRefreshInput};
use super::super::types::{
    BalanceDirection, BalanceMetric, BalancePeriod, BalanceSnapshot,
};

/// 一行用量数据（used/limit/reset 信息）
#[derive(Clone)]
struct UsageRow {
    label: String,
    used: i64,
    limit: i64,
    #[expect(dead_code)]
    reset_hint: Option<String>,
    reset_at: Option<String>,
}

/// 判断 JSON 值是否为对象
fn is_record(value: &serde_json::Value) -> bool {
    value.is_object()
}

/// 从 JSON 对象中取字符串字段
fn pick_string(record: &serde_json::Value, key: &str) -> Option<String> {
    record.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// 从 JSON 对象中取整数字段（支持 number / string 两种形式）
fn to_int(value: &serde_json::Value) -> Option<i64> {
    if let Some(n) = value.as_i64() {
        return Some(n);
    }
    if let Some(n) = value.as_f64() {
        if n.is_finite() {
            return Some(n.trunc() as i64);
        }
    }
    if let Some(s) = value.as_str() {
        if let Ok(n) = s.parse::<f64>() {
            if n.is_finite() {
                return Some(n.trunc() as i64);
            }
        }
    }
    None
}

/// 从 JSON 对象中取整数字段
fn pick_int(record: &serde_json::Value, key: &str) -> Option<i64> {
    record.get(key).and_then(to_int)
}

/// 将秒数格式化为短时长字符串（如 "1h 30m" / "2d 3h"）
fn format_duration_short(seconds: i64) -> String {
    let total_seconds = seconds.max(0);
    if total_seconds < 60 {
        return format!("{}s", total_seconds);
    }
    let total_minutes = total_seconds / 60;
    if total_minutes < 60 {
        return format!("{}m", total_minutes);
    }
    let total_hours = total_minutes / 60;
    if total_hours < 24 {
        let minutes = total_minutes % 60;
        return if minutes > 0 {
            format!("{}h {}m", total_hours, minutes)
        } else {
            format!("{}h", total_hours)
        };
    }
    let days = total_hours / 24;
    let hours = total_hours % 24;
    if hours > 0 {
        format!("{}d {}h", days, hours)
    } else {
        format!("{}d", days)
    }
}

/// 解析 reset_at 字段，返回 (原始值, 毫秒时间戳)
fn resolve_reset_at(data: &serde_json::Value) -> Option<(String, Option<i64>)> {
    for key in ["reset_at", "resetAt", "reset_time", "resetTime"] {
        if let Some(raw) = data.get(key) {
            if let Some(s) = raw.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some((trimmed.to_string(), parse_timestamp_ms(trimmed)));
                }
            }
        }
    }
    None
}

/// 尝试将时间字符串或数字解析为毫秒时间戳
fn parse_timestamp_ms(value: &str) -> Option<i64> {
    // 尝试数字
    if let Ok(n) = value.parse::<f64>() {
        if n.is_finite() {
            // 大于 1e12 视为毫秒，否则视为秒
            let millis = if n > 1e12 { n as i64 } else { (n * 1000.0) as i64 };
            return Some(millis);
        }
    }
    // 尝试 ISO 8601
    parse_iso_to_millis(value)
}

/// 解析 ISO 8601 时间字符串为毫秒时间戳
fn parse_iso_to_millis(value: &str) -> Option<i64> {
    // 简单解析 RFC3339 / ISO 8601
    // 优先尝试 chrono 不可用，使用手动解析常见格式
    // 这里使用 std::time 无法解析，借助 serde_json 不行
    // 退而求其次：尝试用 JavaScript Date 风格的解析
    // 实际项目中应使用 chrono，但为避免引入依赖，这里做简单处理
    parse_rfc3339(value)
}

/// 简单的 RFC3339 解析器，返回毫秒时间戳
fn parse_rfc3339(value: &str) -> Option<i64> {
    // 形如 2024-01-01T12:00:00Z 或 2024-01-01T12:00:00.000Z
    let s = value.trim();
    if s.len() < 19 {
        return None;
    }
    let bytes = s.as_bytes();
    // 解析 YYYY-MM-DD
    let year: i32 = s.get(0..4)?.parse().ok()?;
    if bytes.get(4)? != &b'-' {
        return None;
    }
    let month: u32 = s.get(5..7)?.parse().ok()?;
    if bytes.get(7)? != &b'-' {
        return None;
    }
    let day: u32 = s.get(8..10)?.parse().ok()?;
    // 解析 THH:MM:SS
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

    days_from_civil(year, month, day)
        .map(|d| (d as i64 * 86400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64) * 1000)
}

/// 公元 1970-01-01 到指定日期的天数（Howard Hinnant 算法）
fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if month == 0 || month > 12 || day == 0 || day > 31 {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32; // [0, 399]
    let doy = (153 * if month > 2 { month - 3 } else { month + 9 } + 2) / 5 + day - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    Some(era as i64 * 146097 + doe as i64 - 719468)
}

/// 生成 reset 提示文本
fn reset_hint(data: &serde_json::Value) -> Option<String> {
    if let Some((raw, _)) = resolve_reset_at(data) {
        let ts = parse_timestamp_ms(&raw);
        if let Some(ts) = ts {
            let now = now_secs();
            let delta = (ts / 1000) - now;
            if delta <= 0 {
                return Some("已重置".to_string());
            }
            return Some(format!("{} 后重置", format_duration_short(delta)));
        }
        return Some(format!("重置于 {}", raw));
    }
    for key in ["reset_in", "resetIn", "ttl", "window"] {
        if let Some(seconds) = pick_int(data, key) {
            return Some(format!("{} 后重置", format_duration_short(seconds)));
        }
    }
    None
}

/// 从 JSON 对象构建 UsageRow
fn to_usage_row(data: &serde_json::Value, default_label: &str) -> Option<UsageRow> {
    let limit = pick_int(data, "limit");
    let mut used = pick_int(data, "used");

    if used.is_none() {
        if let (Some(remaining), Some(limit_val)) = (pick_int(data, "remaining"), limit) {
            used = Some(limit_val - remaining);
        }
    }

    if used.is_none() && limit.is_none() {
        return None;
    }

    let label = pick_string(data, "name")
        .or_else(|| pick_string(data, "title"))
        .unwrap_or_else(|| default_label.to_string());

    Some(UsageRow {
        label,
        used: used.unwrap_or(0),
        limit: limit.unwrap_or(0),
        reset_hint: reset_hint(data),
        reset_at: resolve_reset_at(data).map(|(v, _)| v),
    })
}

/// 计算剩余比例
fn remaining_ratio(row: &UsageRow) -> Option<f64> {
    if row.limit <= 0 {
        return None;
    }
    Some((row.limit - row.used) as f64 / row.limit as f64)
}

/// 从标签推断周期
fn infer_period_from_label(label: &str) -> (BalancePeriod, Option<String>) {
    let lower = label.to_lowercase();
    if lower.contains("day") || lower.contains("today") {
        return (BalancePeriod::Day, None);
    }
    if lower.contains("week") {
        return (BalancePeriod::Week, None);
    }
    if lower.contains("month") {
        return (BalancePeriod::Month, None);
    }
    if lower.contains("total") {
        return (BalancePeriod::Total, None);
    }
    // 自定义周期
    (BalancePeriod::Current, Some(label.to_string()))
}

/// 解析 usages 响应，提取 usage 与 limits
fn parse_usage_payload(payload: &serde_json::Value) -> (Option<UsageRow>, Vec<UsageRow>) {
    let mut usage = None;
    let mut limits = Vec::new();

    if let Some(raw_usage) = payload.get("usage") {
        if is_record(raw_usage) {
            usage = to_usage_row(raw_usage, "每周用量");
        }
    }

    if let Some(raw_limits) = payload.get("limits").and_then(|v| v.as_array()) {
        for (index, item) in raw_limits.iter().enumerate() {
            if !is_record(item) {
                continue;
            }
            let detail = item.get("detail").filter(|v| is_record(v)).unwrap_or(item);
            let label = format!("窗口 #{}", index + 1);
            if let Some(row) = to_usage_row(detail, &label) {
                limits.push(row);
            }
        }
    }

    (usage, limits)
}

/// 选择摘要行（优先 usage，否则选择剩余比例最小的 limit）
fn pick_summary_row(usage: &Option<UsageRow>, limits: &[UsageRow]) -> Option<UsageRow> {
    if usage.is_some() {
        return usage.clone();
    }
    if limits.is_empty() {
        return None;
    }

    let mut best: Option<(usize, f64)> = None;
    for (i, row) in limits.iter().enumerate() {
        if let Some(ratio) = remaining_ratio(row) {
            match best {
                None => best = Some((i, ratio)),
                Some((_, br)) if ratio < br => best = Some((i, ratio)),
                _ => {}
            }
        }
    }
    best.map(|(i, _)| limits[i].clone())
}

/// 获取当前秒级时间戳
fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub struct KimiCodeBalanceProvider;

impl BalanceProvider for KimiCodeBalanceProvider {
    fn method(&self) -> &'static str {
        "kimi-code"
    }

    fn refresh<'a>(
        &'a self,
        input: &'a BalanceRefreshInput,
    ) -> Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>> {
        Box::pin(async move {
            let api_key = input.api_key.as_deref().ok_or_else(|| {
                IcodeError::validation("Kimi Code 额度查询需要 API Key")
            })?;

            // 构建端点 URL
            let base = input.base_url.as_deref().unwrap_or("https://api.kimi.com");
            let endpoint = if base.to_lowercase().ends_with("/v1") {
                format!("{}/usages", base.trim_end_matches('/'))
            } else {
                format!("{}/v1/usages", base.trim_end_matches('/'))
            };

            let client = super::super::provider::build_balance_http_client()?;
            let response = client
                .get(&endpoint)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Accept", "application/json")
                .send()
                .await
                .map_err(|e| IcodeError::internal(format!("Kimi Code 额度查询请求失败: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let text = response.text().await.unwrap_or_default();
                if status == 401 {
                    return Err(IcodeError::internal("Kimi Code 授权失败，请检查 API Key"));
                }
                if status == 404 {
                    return Err(IcodeError::internal("Kimi Code 用量端点不可用，请使用 Kimi For Coding"));
                }
                return Err(IcodeError::internal(format!(
                    "Kimi Code 额度查询失败 (HTTP {}): {}",
                    status,
                    text.chars().take(200).collect::<String>()
                )));
            }

            let json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| IcodeError::internal(format!("Kimi Code 额度响应解析失败: {}", e)))?;

            // 解包 data 字段
            let payload = json
                .get("data")
                .filter(|v| is_record(v))
                .unwrap_or(&json);

            let (usage, limits) = parse_usage_payload(payload);
            let summary = pick_summary_row(&usage, &limits);

            // 组装展示行：usage + limits，或 summary + 其他 limits
            let mut rows: Vec<UsageRow> = Vec::new();
            if let Some(u) = &usage {
                rows.push(u.clone());
                rows.extend(limits.iter().cloned());
            } else if let Some(s) = &summary {
                rows.push(s.clone());
                for row in &limits {
                    if let Some(ss) = &summary {
                        if row.label == ss.label && row.used == ss.used && row.limit == ss.limit {
                            continue;
                        }
                    }
                    rows.push(row.clone());
                }
            }

            let mut items: Vec<BalanceMetric> = Vec::new();
            for (index, row) in rows.iter().enumerate() {
                let (period, period_label) = infer_period_from_label(&row.label);
                let remaining = if row.limit > 0 {
                    Some((row.limit - row.used).max(0))
                } else {
                    None
                };

                let mut metric = BalanceMetric::token(
                    format!("tokens-{}", index + 1),
                    Some(serde_json::json!(row.used)),
                    if row.limit > 0 {
                        Some(serde_json::json!(row.limit))
                    } else {
                        None
                    },
                    remaining.map(|r| serde_json::json!(r)),
                )
                .with_period(period)
                .with_label(&row.label);
                if let Some(label) = period_label {
                    metric = metric.with_period_label(label);
                }
                items.push(metric);
            }

            // 主指标：剩余百分比
            let mut primary_id: Option<String> = None;
            if let Some(summary_row) = &summary {
                if let Some(ratio) = remaining_ratio(summary_row) {
                    let percent_value = (ratio * 100.0).round() as f64;
                    let (period, period_label) = infer_period_from_label(&summary_row.label);
                    let percent_id = "remaining-percent".to_string();
                    let mut metric = BalanceMetric::percent(
                        &percent_id,
                        percent_value,
                        Some(BalanceDirection::Remaining),
                    )
                    .with_period(period)
                    .with_label("剩余");
                    if let Some(label) = period_label {
                        metric = metric.with_period_label(label);
                    }
                    items.push(metric);
                    primary_id = Some(percent_id);
                }
            }

            // 重置时间
            if let Some(summary_row) = &summary {
                if let Some(reset_at) = &summary_row.reset_at {
                    let timestamp_ms = parse_timestamp_ms(reset_at);
                    let (period, period_label) = infer_period_from_label(&summary_row.label);
                    let mut metric = BalanceMetric::time(
                        "reset-time",
                        "resetAt",
                        reset_at.clone(),
                        timestamp_ms,
                    )
                    .with_period(period)
                    .with_label("重置");
                    if let Some(label) = period_label {
                        metric = metric.with_period_label(label);
                    }
                    items.push(metric);
                    if primary_id.is_none() {
                        primary_id = Some("reset-time".to_string());
                    }
                }
            }

            // 兜底主指标
            if primary_id.is_none() {
                if let Some(summary_row) = &summary {
                    for item in &items {
                        if item.metric_type == super::super::types::BalanceMetricType::Token
                            && item.label.as_deref() == Some(&summary_row.label)
                        {
                            primary_id = Some(item.id.clone());
                            break;
                        }
                    }
                }
                if primary_id.is_none() {
                    primary_id = items.first().map(|i| i.id.clone());
                }
            }

            // 标记 primary
            if let Some(pid) = &primary_id {
                for item in &mut items {
                    if &item.id == pid {
                        item.primary = Some(true);
                    }
                }
            }

            Ok(BalanceSnapshot {
                updated_at: now_ms(),
                items,
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
