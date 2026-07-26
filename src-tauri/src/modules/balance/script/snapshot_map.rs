//! # Dynamic → BalanceSnapshot 映射与校验

use rhai::{Dynamic, Map};

use crate::error::{IcodeError, IcodeResult};
use crate::modules::balance::types::{
    BalanceDirection, BalanceMetric, BalanceMetricType, BalancePeriod, BalanceSnapshot,
    BalanceStatusValue,
};

/// 将脚本返回值映射为 BalanceSnapshot
pub fn map_to_snapshot(value: Dynamic) -> IcodeResult<BalanceSnapshot> {
    let map: Map = value.try_cast::<Map>().ok_or_else(|| {
        IcodeError::validation("脚本返回值必须是 map（含 items 数组）")
    })?;

    let updated_at = map
        .get("updatedAt")
        .or_else(|| map.get("updated_at"))
        .and_then(|v| v.as_int().ok())
        .unwrap_or(0);

    let items_dyn = map
        .get("items")
        .cloned()
        .ok_or_else(|| IcodeError::validation("脚本返回缺少 items 字段"))?;

    let items_arr: rhai::Array = items_dyn.try_cast::<rhai::Array>().ok_or_else(|| {
        IcodeError::validation("items 必须是数组")
    })?;

    let mut items = Vec::with_capacity(items_arr.len());
    for (idx, item) in items_arr.into_iter().enumerate() {
        let metric = map_metric(item, idx)?;
        items.push(metric);
    }

    Ok(BalanceSnapshot { updated_at, items })
}

fn map_metric(value: Dynamic, idx: usize) -> IcodeResult<BalanceMetric> {
    let map: Map = value.try_cast::<Map>().ok_or_else(|| {
        IcodeError::validation(format!("items[{idx}] 必须是 map"))
    })?;

    let id = get_string(&map, "id").ok_or_else(|| {
        IcodeError::validation(format!("items[{idx}].id 必填"))
    })?;
    if id.is_empty() {
        return Err(IcodeError::validation(format!("items[{idx}].id 不能为空")));
    }

    let type_str = get_string(&map, "type").ok_or_else(|| {
        IcodeError::validation(format!("items[{idx}].type 必填"))
    })?;

    let metric_type = match type_str.as_str() {
        "amount" => BalanceMetricType::Amount,
        "integer" => BalanceMetricType::Integer,
        "token" => BalanceMetricType::Token,
        "percent" => BalanceMetricType::Percent,
        "time" => BalanceMetricType::Time,
        "status" => BalanceMetricType::Status,
        other => {
            return Err(IcodeError::validation(format!(
                "items[{idx}].type 非法: {other}"
            )));
        }
    };

    let mut metric = BalanceMetric {
        id,
        metric_type,
        period: get_string(&map, "period").and_then(|p| parse_period(&p)),
        period_label: get_string(&map, "periodLabel").or_else(|| get_string(&map, "period_label")),
        scope: get_string(&map, "scope"),
        primary: get_bool(&map, "primary"),
        label: get_string(&map, "label"),
        ..Default::default()
    };

    match metric_type {
        BalanceMetricType::Amount => {
            metric.direction = Some(require_direction(&map, idx)?);
            metric.value = Some(require_value(&map, idx)?);
            metric.currency_symbol =
                get_string(&map, "currencySymbol").or_else(|| get_string(&map, "currency_symbol"));
        }
        BalanceMetricType::Integer => {
            metric.direction = Some(require_direction(&map, idx)?);
            metric.value = Some(require_value(&map, idx)?);
        }
        BalanceMetricType::Token => {
            metric.used = get_json_value(&map, "used");
            metric.limit = get_json_value(&map, "limit");
            metric.remaining = get_json_value(&map, "remaining");
            if metric.used.is_none() && metric.limit.is_none() && metric.remaining.is_none() {
                return Err(IcodeError::validation(format!(
                    "items[{idx}] token 类型至少需要 used/limit/remaining 之一"
                )));
            }
        }
        BalanceMetricType::Percent => {
            let v = require_value(&map, idx)?;
            let num = v.as_f64().ok_or_else(|| {
                IcodeError::validation(format!("items[{idx}].value 须为数字 (percent)"))
            })?;
            if !(0.0..=100.0).contains(&num) {
                return Err(IcodeError::validation(format!(
                    "items[{idx}].value 须在 0–100（当前 {num}）"
                )));
            }
            metric.value = Some(v);
            if let Some(basis) = get_string(&map, "basis") {
                metric.basis = match basis.as_str() {
                    "remaining" => Some(BalanceDirection::Remaining),
                    "used" => Some(BalanceDirection::Used),
                    other => {
                        return Err(IcodeError::validation(format!(
                            "items[{idx}].basis 非法: {other}"
                        )));
                    }
                };
            }
        }
        BalanceMetricType::Time => {
            let kind = get_string(&map, "kind").ok_or_else(|| {
                IcodeError::validation(format!("items[{idx}].kind 必填 (time)"))
            })?;
            if kind != "expiresAt" && kind != "resetAt" {
                return Err(IcodeError::validation(format!(
                    "items[{idx}].kind 须为 expiresAt 或 resetAt"
                )));
            }
            metric.kind = Some(kind);
            let val = get_string(&map, "value").ok_or_else(|| {
                IcodeError::validation(format!("items[{idx}].value 必填 (time)"))
            })?;
            metric.value = Some(serde_json::Value::String(val));
            metric.timestamp_ms = map
                .get("timestampMs")
                .or_else(|| map.get("timestamp_ms"))
                .and_then(|v| v.as_int().ok());
        }
        BalanceMetricType::Status => {
            let val = get_string(&map, "value").ok_or_else(|| {
                IcodeError::validation(format!("items[{idx}].value 必填 (status)"))
            })?;
            let status = match val.as_str() {
                "ok" => BalanceStatusValue::Ok,
                "unlimited" => BalanceStatusValue::Unlimited,
                "exhausted" => BalanceStatusValue::Exhausted,
                "error" => BalanceStatusValue::Error,
                "unavailable" => BalanceStatusValue::Unavailable,
                other => {
                    return Err(IcodeError::validation(format!(
                        "items[{idx}].value 非法 status: {other}"
                    )));
                }
            };
            metric.value = Some(serde_json::Value::String(val));
            metric.status_value = Some(status);
            metric.message = get_string(&map, "message");
        }
    }

    Ok(metric)
}

fn require_direction(map: &Map, idx: usize) -> IcodeResult<BalanceDirection> {
    let d = get_string(map, "direction").ok_or_else(|| {
        IcodeError::validation(format!("items[{idx}].direction 必填"))
    })?;
    match d.as_str() {
        "remaining" => Ok(BalanceDirection::Remaining),
        "used" => Ok(BalanceDirection::Used),
        "limit" => Ok(BalanceDirection::Limit),
        other => Err(IcodeError::validation(format!(
            "items[{idx}].direction 非法: {other}"
        ))),
    }
}

fn require_value(map: &Map, idx: usize) -> IcodeResult<serde_json::Value> {
    get_json_value(map, "value").ok_or_else(|| {
        IcodeError::validation(format!("items[{idx}].value 必填"))
    })
}

fn get_string(map: &Map, key: &str) -> Option<String> {
    map.get(key).and_then(|v| {
        if let Ok(s) = v.clone().into_string() {
            Some(s)
        } else {
            None
        }
    })
}

fn get_bool(map: &Map, key: &str) -> Option<bool> {
    map.get(key).and_then(|v| v.clone().try_cast::<bool>())
}

fn get_json_value(map: &Map, key: &str) -> Option<serde_json::Value> {
    let v = map.get(key)?;
    if v.is_unit() {
        return None;
    }
    if let Ok(i) = v.as_int() {
        return Some(serde_json::json!(i));
    }
    if let Ok(f) = v.as_float() {
        return Some(serde_json::json!(f));
    }
    if let Ok(s) = v.clone().into_string() {
        // 尝试解析为数字字符串也保留字符串
        return Some(serde_json::Value::String(s));
    }
    if let Some(b) = v.clone().try_cast::<bool>() {
        return Some(serde_json::Value::Bool(b));
    }
    Some(serde_json::Value::String(v.to_string()))
}

fn parse_period(s: &str) -> Option<BalancePeriod> {
    match s {
        "current" => Some(BalancePeriod::Current),
        "month" => Some(BalancePeriod::Month),
        "day" => Some(BalancePeriod::Day),
        "week" => Some(BalancePeriod::Week),
        "total" => Some(BalancePeriod::Total),
        _ => None,
    }
}
