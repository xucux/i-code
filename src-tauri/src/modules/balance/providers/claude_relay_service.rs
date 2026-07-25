//! # Claude Relay Service 额度查询实现
//!
//! 调用 Claude Relay Service apiStats 系列接口：
//! 1. `POST {base}/apiStats/api/get-key-id` — 用 apiKey 换取 apiId
//! 2. `POST {base}/apiStats/api/user-stats` — 查询用户配额与限额
//! 3. `POST {base}/apiStats/api/user-model-stats` — 按 daily/weekly/monthly 查询模型用量
//!
//! 聚合后输出多窗口（日/周/月/累计）的 token 与金额指标，并标记最受限窗口为主指标。

use std::future::Future;
use std::pin::Pin;

use crate::error::{IcodeError, IcodeResult};

use super::super::provider::{BalanceProvider, BalanceRefreshInput};
use super::super::types::{
    BalanceDirection, BalanceMetric, BalancePeriod, BalanceSnapshot,
};

/// 判断 JSON 值是否为对象
fn is_record(value: &serde_json::Value) -> bool {
    value.is_object()
}

/// 从 JSON 对象中取字符串字段
fn pick_string(record: &serde_json::Value, key: &str) -> Option<String> {
    record.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
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

/// 构建 apiStats 端点 URL
///
/// 若 base_url 的 path 以 `/api` 结尾，先剥离再拼接 `/apiStats/api/{suffix}`。
fn create_endpoint(base_url: &str, suffix: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    // 剥离末尾的 /api
    let stripped = if trimmed.to_lowercase().ends_with("/api") {
        &trimmed[..trimmed.len() - 4]
    } else {
        trimmed
    };
    let joined = format!("{}/apiStats/api/{}", stripped, suffix)
        .replace("//", "/");
    // 修正 scheme 后可能的双斜杠
    joined.replace("://", "\0SCHEME\0").replace("//", "/").replace("\0SCHEME\0", "://")
}

/// 单个时间窗口的用量快照
struct CrsWindowSnapshot {
    summary_prefix: &'static str,
    period: BalancePeriod,
    used: f64,
    total: f64,
}

/// 完整用量快照
struct CrsUsageSnapshot {
    daily: WindowCost,
    weekly: WindowCost,
    monthly: WindowCost,
    total: WindowCost,
    group_weekly: Option<GroupCost>,
}

struct WindowCost {
    used: f64,
    total: f64,
    tokens: f64,
}

struct GroupCost {
    used: Option<f64>,
    total: Option<f64>,
}

/// 选择最受限的窗口（剩余最少）
fn resolve_most_constrained_window(usage: &CrsUsageSnapshot) -> Option<CrsWindowSnapshot> {
    let mut windows: Vec<CrsWindowSnapshot> = vec![
        CrsWindowSnapshot { summary_prefix: "今日配额", period: BalancePeriod::Day, used: usage.daily.used, total: usage.daily.total },
        CrsWindowSnapshot { summary_prefix: "本周配额", period: BalancePeriod::Week, used: usage.weekly.used, total: usage.weekly.total },
        CrsWindowSnapshot { summary_prefix: "本月配额", period: BalancePeriod::Month, used: usage.monthly.used, total: usage.monthly.total },
        CrsWindowSnapshot { summary_prefix: "总配额", period: BalancePeriod::Total, used: usage.total.used, total: usage.total.total },
    ];

    // 组级别周配额超限时加入候选
    if let Some(group) = &usage.group_weekly {
        if let (Some(used), Some(total)) = (group.used, group.total) {
            if total > 0.0 && used >= total {
                windows.push(CrsWindowSnapshot {
                    summary_prefix: "组周配额",
                    period: BalancePeriod::Week,
                    used,
                    total,
                });
            }
        }
    }

    let constrained: Vec<&CrsWindowSnapshot> = windows.iter().filter(|w| w.total > 0.0).collect();
    if constrained.is_empty() {
        return None;
    }
    let mut best = constrained[0];
    let mut best_remaining = best.total - best.used;
    for &current in constrained.iter().skip(1) {
        let current_remaining = current.total - current.used;
        if current_remaining < best_remaining {
            best = current;
            best_remaining = current_remaining;
        }
    }
    // 返回克隆（CrsWindowSnapshot 含 Copy 字段，手动构造）
    Some(CrsWindowSnapshot {
        summary_prefix: best.summary_prefix,
        period: best.period,
        used: best.used,
        total: best.total,
    })
}

/// 构建快照指标列表
fn build_snapshot(usage: &CrsUsageSnapshot) -> Vec<BalanceMetric> {
    let constrained = resolve_most_constrained_window(usage);
    let mut items: Vec<BalanceMetric> = Vec::new();
    let mut primary_id: Option<String> = None;

    if let Some(c) = &constrained {
        if c.total > 0.0 {
            let remaining_amount = c.total - c.used;
            let remaining_percent = clamp_percent((remaining_amount / c.total) * 100.0);
            primary_id = Some("remaining-percent".to_string());
            items.push(
                BalanceMetric::percent("remaining-percent", remaining_percent, Some(BalanceDirection::Remaining))
                    .with_period(c.period)
                    .with_label(c.summary_prefix)
                    .with_primary(true),
            );
            items.push(
                BalanceMetric::amount("remaining-amount", BalanceDirection::Remaining, serde_json::json!(remaining_amount), Some("$"))
                    .with_period(c.period)
                    .with_label(c.summary_prefix),
            );
            items.push(
                BalanceMetric::amount("constrained-used", BalanceDirection::Used, serde_json::json!(c.used), Some("$"))
                    .with_period(c.period)
                    .with_label(c.summary_prefix),
            );
            items.push(
                BalanceMetric::amount("constrained-limit", BalanceDirection::Limit, serde_json::json!(c.total), Some("$"))
                    .with_period(c.period)
                    .with_label(c.summary_prefix),
            );
        }
    }

    // 各窗口的 token 与 cost
    let windows = [
        ("day", BalancePeriod::Day, "今日用量", &usage.daily),
        ("week", BalancePeriod::Week, "本周用量", &usage.weekly),
        ("month", BalancePeriod::Month, "本月用量", &usage.monthly),
        ("total", BalancePeriod::Total, "总用量", &usage.total),
    ];

    for (id_prefix, period, label, window) in windows.iter() {
        items.push(
            BalanceMetric::token(
                format!("{}-tokens", id_prefix),
                Some(serde_json::json!(window.tokens)),
                None,
                None,
            )
            .with_period(*period)
            .with_label(*label),
        );
        items.push(
            BalanceMetric::amount(
                format!("{}-cost-used", id_prefix),
                BalanceDirection::Used,
                serde_json::json!(window.used),
                Some("$"),
            )
            .with_period(*period)
            .with_label(*label),
        );
        if window.total > 0.0 {
            items.push(
                BalanceMetric::amount(
                    format!("{}-cost-limit", id_prefix),
                    BalanceDirection::Limit,
                    serde_json::json!(window.total),
                    Some("$"),
                )
                .with_period(*period)
                .with_label(*label),
            );
        }
    }

    // 组级别周配额
    if let Some(group) = &usage.group_weekly {
        let has_used = group.used.map(|v| v.is_finite()).unwrap_or(false);
        let has_total = group.total.map(|v| v.is_finite() && v > 0.0).unwrap_or(false);
        if has_used || has_total {
            if let Some(used) = group.used {
                items.push(
                    BalanceMetric::amount("group-weekly-used", BalanceDirection::Used, serde_json::json!(used), Some("$"))
                        .with_period(BalancePeriod::Week)
                        .with_scope("group")
                        .with_label("组周用量"),
                );
            }
            if let Some(total) = group.total {
                if total.is_finite() && total > 0.0 {
                    items.push(
                        BalanceMetric::amount("group-weekly-limit", BalanceDirection::Limit, serde_json::json!(total), Some("$"))
                            .with_period(BalancePeriod::Week)
                            .with_scope("group")
                            .with_label("组周用量"),
                    );
                }
            }
        }
    }

    // 兜底主指标
    if primary_id.is_none() {
        primary_id = items.first().map(|i| i.id.clone());
    }
    if let Some(pid) = &primary_id {
        for item in &mut items {
            item.primary = Some(&item.id == pid);
        }
    }

    items
}

/// 模型统计条目
struct ModelStatsEntry {
    all_tokens: Option<f64>,
    cost_total: Option<f64>,
}

/// 汇总模型统计数据
fn sum_model_stats(entries: &[ModelStatsEntry]) -> (f64, f64) {
    let mut tokens = 0.0;
    let mut cost = 0.0;
    for entry in entries {
        if let Some(t) = entry.all_tokens {
            if t.is_finite() {
                tokens += t;
            }
        }
        if let Some(c) = entry.cost_total {
            if c.is_finite() {
                cost += c;
            }
        }
    }
    (tokens, cost)
}

pub struct ClaudeRelayServiceBalanceProvider;

impl BalanceProvider for ClaudeRelayServiceBalanceProvider {
    fn method(&self) -> &'static str {
        "claude-relay-service"
    }

    fn refresh<'a>(
        &'a self,
        input: &'a BalanceRefreshInput,
    ) -> Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>> {
        Box::pin(async move {
            let api_key = input.api_key.as_deref().ok_or_else(|| {
                IcodeError::validation("Claude Relay Service 额度查询需要 API Key")
            })?;

            // 解析 base_url：优先 config.baseUrl，其次 input.base_url
            let config_base = input
                .claude_relay_config
                .as_ref()
                .and_then(|c| c.base_url.as_deref())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty());
            let base = config_base
                .or_else(|| input.base_url.as_deref().map(|s| s.trim()))
                .unwrap_or("https://relay.example.com");
            let base = base.trim_end_matches('/');

            let client = reqwest::Client::new();

            // 1. 获取 apiId
            let api_id = fetch_api_id(&client, base, api_key).await?;

            // 2. 并行查询 user-stats 与 3 个周期的 model-stats
            let (user_stats, daily_stats, weekly_stats, monthly_stats) = tokio::try_join!(
                fetch_user_stats(&client, base, &api_id),
                fetch_user_model_stats(&client, base, &api_id, "daily"),
                fetch_user_model_stats(&client, base, &api_id, "natural_weekly"),
                fetch_user_model_stats(&client, base, &api_id, "monthly"),
            )?;

            // 3. 聚合
            let (daily_tokens, daily_cost) = sum_model_stats(&daily_stats);
            let (weekly_tokens, weekly_cost) = sum_model_stats(&weekly_stats);
            let (monthly_tokens, monthly_cost) = sum_model_stats(&monthly_stats);

            let usage = CrsUsageSnapshot {
                daily: WindowCost {
                    used: user_stats.current_daily_cost.unwrap_or(daily_cost),
                    total: user_stats.daily_cost_limit.unwrap_or(0.0),
                    tokens: daily_tokens,
                },
                weekly: WindowCost {
                    used: user_stats.current_weekly_cost.unwrap_or(weekly_cost),
                    total: user_stats.weekly_cost_limit.unwrap_or(0.0),
                    tokens: weekly_tokens,
                },
                monthly: WindowCost {
                    used: monthly_cost,
                    total: 0.0,
                    tokens: monthly_tokens,
                },
                total: WindowCost {
                    used: user_stats.current_total_cost.unwrap_or(0.0),
                    total: user_stats.total_cost_limit.unwrap_or(0.0),
                    tokens: user_stats.total_all_tokens.or(user_stats.total_tokens).unwrap_or(0.0),
                },
                group_weekly: user_stats.group_weekly_cost.map(|g| GroupCost {
                    used: g.weekly_cost,
                    total: g.weekly_cost_limit,
                }),
            };

            let items = build_snapshot(&usage);

            Ok(BalanceSnapshot {
                updated_at: now_ms(),
                items,
            })
        })
    }
}

/// user-stats 返回的限额信息
struct UserStats {
    current_daily_cost: Option<f64>,
    daily_cost_limit: Option<f64>,
    current_weekly_cost: Option<f64>,
    weekly_cost_limit: Option<f64>,
    current_total_cost: Option<f64>,
    total_cost_limit: Option<f64>,
    total_all_tokens: Option<f64>,
    total_tokens: Option<f64>,
    group_weekly_cost: Option<GroupWeeklyCost>,
}

struct GroupWeeklyCost {
    weekly_cost: Option<f64>,
    weekly_cost_limit: Option<f64>,
}

/// 调用 get-key-id 获取 apiId
async fn fetch_api_id(client: &reqwest::Client, base: &str, api_key: &str) -> IcodeResult<String> {
    let endpoint = create_endpoint(base, "get-key-id");
    let response = client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(serde_json::json!({ "apiKey": api_key }).to_string())
        .send()
        .await
        .map_err(|e| IcodeError::internal(format!("Claude Relay Service get-key-id 请求失败: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        return Err(IcodeError::internal(
            parse_error_message(&text).unwrap_or_else(|| format!("Claude Relay Service 查询失败 (HTTP {})", status))
        ));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| IcodeError::internal(format!("Claude Relay Service get-key-id 响应解析失败: {}", e)))?;

    if !is_record(&json) {
        return Err(IcodeError::internal("Claude Relay Service get-key-id 响应异常"));
    }
    if pick_bool(&json, "success") == Some(false) {
        return Err(IcodeError::internal(
            pick_string(&json, "message").unwrap_or_else(|| "Claude Relay Service get-key-id 响应异常".to_string())
        ));
    }
    let payload = json.get("data").filter(|v| is_record(v)).ok_or_else(|| {
        IcodeError::internal("Claude Relay Service get-key-id 响应中无 data")
    })?;
    let id = pick_string(payload, "id").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).ok_or_else(|| {
        IcodeError::internal("Claude Relay Service get-key-id 响应中无 id")
    })?;
    Ok(id)
}

/// 调用 user-stats 获取限额
async fn fetch_user_stats(client: &reqwest::Client, base: &str, api_id: &str) -> IcodeResult<UserStats> {
    let endpoint = create_endpoint(base, "user-stats");
    let response = client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(serde_json::json!({ "apiId": api_id }).to_string())
        .send()
        .await
        .map_err(|e| IcodeError::internal(format!("Claude Relay Service user-stats 请求失败: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        return Err(IcodeError::internal(
            parse_error_message(&text).unwrap_or_else(|| format!("Claude Relay Service 查询失败 (HTTP {})", status))
        ));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| IcodeError::internal(format!("Claude Relay Service user-stats 响应解析失败: {}", e)))?;

    if !is_record(&json) {
        return Err(IcodeError::internal("Claude Relay Service user-stats 响应异常"));
    }
    if pick_bool(&json, "success") == Some(false) {
        return Err(IcodeError::internal(
            pick_string(&json, "message").unwrap_or_else(|| "Claude Relay Service user-stats 响应异常".to_string())
        ));
    }
    let payload = json.get("data").filter(|v| is_record(v)).ok_or_else(|| {
        IcodeError::internal("Claude Relay Service user-stats 响应中无 data")
    })?;
    let limits = payload.get("limits").filter(|v| is_record(v)).ok_or_else(|| {
        IcodeError::internal("Claude Relay Service user-stats 响应中无 limits")
    })?;

    let usage_total = payload
        .get("usage")
        .and_then(|u| u.get("total"))
        .filter(|v| is_record(v));

    let group = payload.get("group").filter(|v| is_record(v));

    Ok(UserStats {
        current_daily_cost: pick_number(limits, "currentDailyCost"),
        daily_cost_limit: pick_number(limits, "dailyCostLimit"),
        current_weekly_cost: pick_number(limits, "currentWeeklyCost"),
        weekly_cost_limit: pick_number(limits, "weeklyCostLimit"),
        current_total_cost: pick_number(limits, "currentTotalCost"),
        total_cost_limit: pick_number(limits, "totalCostLimit"),
        total_all_tokens: usage_total.and_then(|t| pick_number(t, "allTokens")),
        total_tokens: usage_total.and_then(|t| pick_number(t, "tokens")),
        group_weekly_cost: group.map(|g| GroupWeeklyCost {
            weekly_cost: pick_number(g, "weeklyCost"),
            weekly_cost_limit: pick_number(g, "weeklyCostLimit"),
        }),
    })
}

/// 调用 user-model-stats 获取模型级统计
async fn fetch_user_model_stats(
    client: &reqwest::Client,
    base: &str,
    api_id: &str,
    period: &str,
) -> IcodeResult<Vec<ModelStatsEntry>> {
    let endpoint = create_endpoint(base, "user-model-stats");
    let response = client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(serde_json::json!({ "apiId": api_id, "period": period }).to_string())
        .send()
        .await
        .map_err(|e| IcodeError::internal(format!("Claude Relay Service user-model-stats 请求失败: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        return Err(IcodeError::internal(
            parse_error_message(&text).unwrap_or_else(|| format!("Claude Relay Service 查询失败 (HTTP {})", status))
        ));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| IcodeError::internal(format!("Claude Relay Service user-model-stats 响应解析失败: {}", e)))?;

    if !is_record(&json) {
        return Err(IcodeError::internal("Claude Relay Service user-model-stats 响应异常"));
    }
    if pick_bool(&json, "success") == Some(false) {
        return Err(IcodeError::internal(
            pick_string(&json, "message").unwrap_or_else(|| "Claude Relay Service user-model-stats 响应异常".to_string())
        ));
    }
    let payload = json.get("data").and_then(|v| v.as_array()).ok_or_else(|| {
        IcodeError::internal("Claude Relay Service user-model-stats 响应中无 data 数组")
    })?;

    let mut entries = Vec::new();
    for entry in payload {
        if !is_record(entry) {
            continue;
        }
        let costs = entry.get("costs").filter(|v| is_record(v));
        entries.push(ModelStatsEntry {
            all_tokens: pick_number(entry, "allTokens"),
            cost_total: costs.and_then(|c| pick_number(c, "total")),
        });
    }
    Ok(entries)
}

/// 获取当前毫秒时间戳
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
