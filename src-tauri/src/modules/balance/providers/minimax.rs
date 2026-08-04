//! # MiniMax 额度查询实现
//!
//! 调用 MiniMax coding plan 余额 API：
//! `GET {base_url}/v1/api/openplatform/coding_plan/remains`
//!
//! 响应格式：
//! ```json
//! {
//!   "base_resp": { "status_code": 0, "status_msg": "" },
//!   "model_remains": [
//!     {
//!       "current_interval_total_count": 1000,
//!       "current_interval_usage_count": 800,
//!       "end_time": 1234567890000
//!     }
//!   ]
//! }
//! ```
//!
//! 注意：`current_interval_usage_count` 为剩余配额，而非已用配额。

use std::future::Future;
use std::pin::Pin;

use crate::error::{IcodeError, IcodeResult};

use super::super::provider::{BalanceProvider, BalanceRefreshInput};
use super::super::types::{BalanceDirection, BalanceMetric, BalancePeriod, BalanceSnapshot};

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

pub struct MinimaxBalanceProvider;

impl BalanceProvider for MinimaxBalanceProvider {
    fn method(&self) -> &'static str {
        "minimax"
    }

    fn refresh<'a>(
        &'a self,
        input: &'a BalanceRefreshInput,
    ) -> Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>> {
        Box::pin(async move {
            let api_key = input.api_key.as_deref().ok_or_else(|| {
                IcodeError::validation("MiniMax 额度查询需要 API Key")
            })?;

            let base = input.base_url.as_deref().unwrap_or("https://api.minimax.chat");
            let base = base.trim_end_matches('/');
            let endpoint = format!("{}/v1/api/openplatform/coding_plan/remains", base);

            let client = super::super::provider::build_balance_http_client()?;
            let response = client
                .get(&endpoint)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Accept", "application/json")
                .send()
                .await
                .map_err(|e| IcodeError::internal(format!("MiniMax 额度查询请求失败: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let text = response.text().await.unwrap_or_default();
                return Err(IcodeError::internal(
                    parse_error_message(&text).unwrap_or_else(|| format!("MiniMax 额度查询失败 (HTTP {})", status))
                ));
            }

            let json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| IcodeError::internal(format!("MiniMax 额度响应解析失败: {}", e)))?;

            if !is_record(&json) {
                return Err(IcodeError::internal("MiniMax 额度响应异常"));
            }

            // 检查 base_resp 状态码
            if let Some(base_resp) = json.get("base_resp").filter(|v| is_record(v)) {
                if let Some(status_code) = pick_number(base_resp, "status_code") {
                    if status_code != 0.0 {
                        let status_msg = pick_string(base_resp, "status_msg").unwrap_or_else(|| "MiniMax 额度响应异常".to_string());
                        return Err(IcodeError::internal(status_msg));
                    }
                }
            }

            // 取 model_remains 数组的第一项
            let model_remains = json
                .get("model_remains")
                .and_then(|v| v.as_array())
                .filter(|a| !a.is_empty())
                .ok_or_else(|| IcodeError::internal("MiniMax 额度响应中无 model_remains 数据"))?;

            let first_model = model_remains[0].get("").map(|_| &model_remains[0]).unwrap_or(&model_remains[0]);
            if !is_record(first_model) {
                return Err(IcodeError::internal("MiniMax 额度响应中 model_remains 条目格式异常"));
            }

            let total_count = pick_number(first_model, "current_interval_total_count")
                .ok_or_else(|| IcodeError::internal("MiniMax 额度响应中无 current_interval_total_count"))?;
            let current_interval_usage_count = pick_number(first_model, "current_interval_usage_count")
                .ok_or_else(|| IcodeError::internal("MiniMax 额度响应中无 current_interval_usage_count"))?;
            let end_time = pick_number(first_model, "end_time")
                .ok_or_else(|| IcodeError::internal("MiniMax 额度响应中无 end_time"))?;

            // current_interval_usage_count 实为剩余配额
            let used_count = (total_count - current_interval_usage_count).max(0.0);
            let used_percent = if total_count > 0.0 {
                (used_count / total_count * 100.0).max(0.0).min(100.0)
            } else {
                0.0
            };

            // end_time 为毫秒时间戳
            let end_time_ms = end_time as i64;
            let end_time_iso = format_iso_from_millis(end_time_ms);

            let items = vec![
                BalanceMetric::integer(
                    "minimax-requests",
                    BalanceDirection::Used,
                    serde_json::json!(used_count as i64),
                )
                .with_period(BalancePeriod::Current)
                .with_primary(true),
                BalanceMetric::integer(
                    "minimax-requests-limit",
                    BalanceDirection::Limit,
                    serde_json::json!(total_count as i64),
                )
                .with_period(BalancePeriod::Current),
                BalanceMetric::percent(
                    "minimax-used-percent",
                    used_percent,
                    Some(BalanceDirection::Used),
                )
                .with_period(BalancePeriod::Current),
                BalanceMetric::time(
                    "minimax-period-end",
                    "resetAt",
                    end_time_iso,
                    Some(end_time_ms),
                )
                .with_period(BalancePeriod::Current),
            ];

            Ok(BalanceSnapshot {
                updated_at: now_ms(),
                items,
            })
        })
    }
}

/// 将毫秒时间戳格式化为 ISO 8601 字符串
fn format_iso_from_millis(millis: i64) -> String {
    // 简单格式化为 Unix 秒级 ISO 字符串
    let secs = millis / 1000;
    let nanos = ((millis % 1000) * 1_000_000) as u32;
    if let Some(dt) = chrono_like::DateTime::from_timestamp(secs, nanos) {
        return dt.to_rfc3339();
    }
    format!("{}", millis)
}

/// 简单的 chrono-like 时间格式化模块
///
/// 为避免引入 chrono 依赖，使用手动的 UTC 时间转换。
mod chrono_like {
    /// 简化的 UTC 时间戳
    pub struct DateTime {
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
        millis: u32,
    }

    impl DateTime {
        /// 从 Unix 秒与纳秒构造
        pub fn from_timestamp(secs: i64, nanos: u32) -> Option<Self> {
            let millis = nanos / 1_000_000;
            let days = secs.div_euclid(86400);
            let remainder = secs.rem_euclid(86400);
            let hour = (remainder / 3600) as u32;
            let minute = ((remainder % 3600) / 60) as u32;
            let second = (remainder % 60) as u32;
            let (year, month, day) = civil_from_days(days);
            Some(Self { year, month, day, hour, minute, second, millis })
        }

        /// 格式化为 RFC3339 字符串
        pub fn to_rfc3339(&self) -> String {
            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
                self.year, self.month, self.day, self.hour, self.minute, self.second, self.millis
            )
        }
    }

    /// 将 Unix 天数转换为 (year, month, day)（Howard Hinnant 算法）
    fn civil_from_days(days: i64) -> (i32, u32, u32) {
        let z = days + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = (z - era * 146097) as u32; // [0, 146096]
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
        let mp = (5 * doy + 2) / 153; // [0, 11]
        let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
        let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
        let year = if m <= 2 { y + 1 } else { y };
        (year as i32, m, d)
    }
}

/// 获取当前毫秒时间戳
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
