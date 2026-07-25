//! # Google Antigravity 额度查询实现
//!
//! 基于 `code_assist_quota` 共享模块，调用 Code Assist `retrieveUserQuota`。
//!
//! Project ID 解析顺序：managed_project_id → project_id → 默认值 `rising-fact-p41fc`。
//!
//! 查询完成后对快照做分组聚合：
//! - `Gemini 3.x Pro` / `Gemini 3 Flash` / `Claude/GPT-OSS` 三组
//! - 百分比取组内最大值，重置时间取组内最早
//! - 丢弃无法归组的标签条目

use std::future::Future;
use std::pin::Pin;

use crate::error::IcodeResult;

use super::super::provider::{BalanceProvider, BalanceRefreshInput};
use super::super::types::{BalanceMetric, BalanceMetricType, BalanceSnapshot};
use super::code_assist_quota::{refresh_code_assist_quota, CodeAssistQuotaOptions};

/// Antigravity 默认 project id
const ANTIGRAVITY_DEFAULT_PROJECT_ID: &str = "rising-fact-p41fc";

/// Antigravity 版本回退值
const ANTIGRAVITY_VERSION_FALLBACK: &str = "1.5.0";

/// Code Assist 端点 fallback 列表
const CODE_ASSIST_ENDPOINT_FALLBACKS: &[&str] = &[
    "https://daily-cloudcode-pa.sandbox.googleapis.com",
    "https://autopush-cloudcode-pa.sandbox.googleapis.com",
    "https://cloudcode-pa.googleapis.com",
    "https://daily-cloudcode-pa.googleapis.com",
];

/// 获取平台字符串
fn platform_str() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows/amd64"
    } else if cfg!(target_os = "macos") {
        "macos/arm64"
    } else {
        "linux/amd64"
    }
}

/// 获取 Client-Metadata 中的平台标识
fn client_metadata_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "WINDOWS"
    } else {
        "MACOS"
    }
}

/// 构建 Antigravity 请求头
fn build_request_headers() -> Vec<(&'static str, String)> {
    let user_agent = format!("antigravity/{} {}", ANTIGRAVITY_VERSION_FALLBACK, platform_str());
    let client_metadata = format!(
        "{{\"ideType\":\"ANTIGRAVITY\",\"platform\":\"{}\",\"pluginType\":\"GEMINI\"}}",
        client_metadata_platform()
    );
    vec![
        ("User-Agent", user_agent),
        ("X-Goog-Api-Client", "google-cloud-sdk vscode_cloudshelleditor/0.1".to_string()),
        ("Client-Metadata", client_metadata),
    ]
}

/// 解析 Antigravity project id
fn resolve_antigravity_project_id(input: &BalanceRefreshInput) -> Option<String> {
    if let Some(managed) = input.managed_project_id.as_deref() {
        let trimmed = managed.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(pid) = input.project_id.as_deref() {
        let trimmed = pid.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    Some(ANTIGRAVITY_DEFAULT_PROJECT_ID.to_string())
}

/// 归一化标签用于分组匹配
fn normalize_metric_label(value: &str) -> String {
    value.to_lowercase().replace(|c: char| c.is_whitespace() || c == '-' || c == '_' || c == '/' || c == '.', "")
}

/// 解析指标所属分组
///
/// 返回 None 表示丢弃该条目；Some(label) 表示归入对应分组。
fn resolve_antigravity_quota_group(metric: &BalanceMetric) -> Option<String> {
    let joined = [
        metric.label.as_deref().unwrap_or("").trim(),
        metric.scope.as_deref().unwrap_or("").trim(),
    ]
    .iter()
    .filter(|s| !s.is_empty())
    .copied()
    .collect::<Vec<_>>()
    .join(" ");

    if joined.is_empty() {
        return None;
    }

    let normalized = normalize_metric_label(&joined);
    let without_requests_prefix = normalized
        .strip_prefix("requests")
        .unwrap_or(&normalized)
        .trim_end_matches("quota");

    if without_requests_prefix.contains("gemini25") {
        return None;
    }
    if without_requests_prefix.contains("gemini") && without_requests_prefix.contains("pro") {
        return Some("Gemini 3.x Pro".to_string());
    }
    if without_requests_prefix.contains("gemini") && without_requests_prefix.contains("flash") {
        return Some("Gemini 3 Flash".to_string());
    }
    if without_requests_prefix.contains("claude") || without_requests_prefix.contains("gptoss") {
        return Some("Claude/GPT-OSS".to_string());
    }
    if without_requests_prefix.starts_with("chat") || without_requests_prefix.starts_with("tab") {
        return None;
    }
    None
}

/// 对快照做分组聚合
fn group_antigravity_snapshot(snapshot: BalanceSnapshot) -> BalanceSnapshot {
    let mut passthrough: Vec<BalanceMetric> = Vec::new();
    // 使用索引方式存储分组，便于后续合并
    // key = (group_label, period, period_label, basis/kind)
    use std::collections::HashMap;
    let mut grouped_percent: HashMap<String, BalanceMetric> = HashMap::new();
    let mut grouped_time: HashMap<String, BalanceMetric> = HashMap::new();

    for metric in snapshot.items {
        let group_label = match resolve_antigravity_quota_group(&metric) {
            Some(g) => g,
            None => continue,
        };

        match metric.metric_type {
            BalanceMetricType::Percent => {
                let key = format!(
                    "{}|{:?}|{}|{:?}",
                    group_label,
                    metric.period,
                    metric.period_label.as_deref().unwrap_or(""),
                    metric.basis
                );
                let value = metric.value.as_ref().and_then(|v| v.as_f64()).unwrap_or(0.0);
                let existing = grouped_percent.get(&key);
                if existing.is_none()
                    || existing.and_then(|e| e.value.as_ref().and_then(|v| v.as_f64())).unwrap_or(0.0) < value
                {
                    let mut new_metric = metric.clone();
                    new_metric.label = Some(group_label.clone());
                    new_metric.scope = None;
                    new_metric.primary = Some(false);
                    grouped_percent.insert(key, new_metric);
                }
            }
            BalanceMetricType::Time => {
                let key = format!(
                    "{}|{:?}|{}|{}",
                    group_label,
                    metric.period,
                    metric.period_label.as_deref().unwrap_or(""),
                    metric.kind.as_deref().unwrap_or("")
                );
                let entry = grouped_time.get(&key).cloned();
                let chosen = match entry {
                    None => {
                        let mut m = metric.clone();
                        m.label = Some(group_label.clone());
                        m.scope = None;
                        m
                    }
                    Some(existing) => {
                        let existing_ts = existing.timestamp_ms.unwrap_or(i64::MAX);
                        let current_ts = metric.timestamp_ms.unwrap_or(i64::MAX);
                        let mut chosen = if current_ts < existing_ts {
                            metric.clone()
                        } else {
                            existing
                        };
                        chosen.label = Some(group_label.clone());
                        chosen.scope = None;
                        chosen
                    }
                };
                grouped_time.insert(key, chosen);
            }
            _ => {
                passthrough.push(metric);
            }
        }
    }

    let mut merged: Vec<BalanceMetric> = passthrough;
    merged.extend(grouped_percent.into_values());
    merged.extend(grouped_time.into_values());

    // 选取百分比最大的作为主指标
    let mut primary_idx: Option<usize> = None;
    let mut highest = f64::NEG_INFINITY;
    for (i, item) in merged.iter().enumerate() {
        if item.metric_type != BalanceMetricType::Percent {
            continue;
        }
        let val = item.value.as_ref().and_then(|v| v.as_f64()).unwrap_or(0.0);
        if primary_idx.is_none() || val > highest {
            highest = val;
            primary_idx = Some(i);
        }
    }
    for (i, item) in merged.iter_mut().enumerate() {
        if item.metric_type == BalanceMetricType::Percent {
            item.primary = Some(i == primary_idx.unwrap_or(usize::MAX));
        }
    }

    BalanceSnapshot {
        updated_at: snapshot.updated_at,
        items: merged,
    }
}

pub struct AntigravityBalanceProvider;

impl BalanceProvider for AntigravityBalanceProvider {
    fn method(&self) -> &'static str {
        "antigravity"
    }

    fn refresh<'a>(
        &'a self,
        input: &'a BalanceRefreshInput,
    ) -> Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>> {
        Box::pin(async move {
            // 构建静态 headers（需在 async 外完成借用，转成 'static）
            let headers = build_request_headers();
            let header_refs: Vec<(&str, &str)> = headers
                .iter()
                .map(|(k, v)| (*k, v.as_str()))
                .collect();
            let options = CodeAssistQuotaOptions {
                provider_name: "Antigravity",
                endpoint_fallbacks: CODE_ASSIST_ENDPOINT_FALLBACKS,
                request_headers: &header_refs,
                resolve_project_id: resolve_antigravity_project_id,
            };

            let snapshot = refresh_code_assist_quota(input, &options).await?;
            Ok(group_antigravity_snapshot(snapshot))
        })
    }
}
