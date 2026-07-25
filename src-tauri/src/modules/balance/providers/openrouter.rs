//! # OpenRouter 额度查询实现
//!
//! 调用 OpenRouter credits API：`GET {base_url}/api/v1/credits`
//! 响应格式：`{ data: { total_credits, total_usage } }`

use std::future::Future;
use std::pin::Pin;

use crate::error::{IcodeError, IcodeResult};

use super::super::provider::{BalanceProvider, BalanceRefreshInput};
use super::super::types::{BalanceDirection, BalanceMetric, BalancePeriod, BalanceSnapshot};

/// OpenRouter credits 响应
#[derive(Debug, serde::Deserialize)]
struct OpenRouterCreditsData {
    #[serde(rename = "total_credits")]
    total_credits: Option<f64>,
    #[serde(rename = "total_usage")]
    total_usage: Option<f64>,
}

#[derive(Debug, serde::Deserialize)]
struct OpenRouterCreditsResponse {
    data: Option<OpenRouterCreditsData>,
}

pub struct OpenRouterBalanceProvider;

impl BalanceProvider for OpenRouterBalanceProvider {
    fn method(&self) -> &'static str {
        "openrouter"
    }

    fn refresh<'a>(&'a self, input: &'a BalanceRefreshInput) -> Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>> {
        Box::pin(async move {
        let input = input.clone();
        let api_key = input.api_key.as_deref().ok_or_else(|| {
            IcodeError::validation("OpenRouter 额度查询需要 API Key")
        })?;

        // 构建端点 URL
        let base = input.base_url.as_deref().unwrap_or("https://openrouter.ai");
        let endpoint = if base.to_lowercase().ends_with("/api/v1") {
            format!("{}/credits", base.trim_end_matches('/'))
        } else {
            format!("{}/api/v1/credits", base.trim_end_matches('/'))
        };

        let client = reqwest::Client::new();
        let response = client
            .get(&endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| IcodeError::internal(format!("OpenRouter 额度查询请求失败: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(IcodeError::internal(format!(
                "OpenRouter 额度查询失败 (HTTP {}): {}",
                status,
                text.chars().take(200).collect::<String>()
            )));
        }

        let body: OpenRouterCreditsResponse = response
            .json()
            .await
            .map_err(|e| IcodeError::internal(format!("OpenRouter 额度响应解析失败: {}", e)))?;

        let data = body.data.ok_or_else(|| {
            IcodeError::internal("OpenRouter 额度响应中无数据")
        })?;

        let total_credits = data.total_credits.ok_or_else(|| {
            IcodeError::internal("OpenRouter 额度响应中缺少 total_credits")
        })?;
        let total_usage = data.total_usage.ok_or_else(|| {
            IcodeError::internal("OpenRouter 额度响应中缺少 total_usage")
        })?;

        let balance = total_credits - total_usage;

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let items = vec![
            BalanceMetric::amount(
                "balance-current",
                BalanceDirection::Remaining,
                serde_json::json!(balance),
                Some("$"),
            )
            .with_period(BalancePeriod::Current)
            .with_primary(true)
            .with_label("余额"),
            BalanceMetric::amount(
                "credits-total",
                BalanceDirection::Limit,
                serde_json::json!(total_credits),
                Some("$"),
            )
            .with_period(BalancePeriod::Total)
            .with_label("总额度"),
            BalanceMetric::amount(
                "usage-total",
                BalanceDirection::Used,
                serde_json::json!(total_usage),
                Some("$"),
            )
            .with_period(BalancePeriod::Total)
            .with_label("已用额度"),
        ];

        Ok(BalanceSnapshot {
            updated_at: now_ms,
            items,
        })
        })
    }
}
