//! # Moonshot AI（Kimi 国内站）额度查询实现
//!
//! 调用 Moonshot AI 余额 API：`POST {base_url}/v1/users/me/balance`
//! 响应格式：`{ data: { available_balance, voucher_balance, cash_balance } }`

use std::future::Future;
use std::pin::Pin;

use crate::error::{IcodeError, IcodeResult};

use super::super::provider::{BalanceProvider, BalanceRefreshInput};
use super::super::types::{BalanceDirection, BalanceMetric, BalancePeriod, BalanceSnapshot};

/// Moonshot AI 余额响应
#[derive(Debug, serde::Deserialize)]
struct MoonshotBalanceData {
    #[serde(rename = "available_balance")]
    available_balance: Option<f64>,
    #[serde(rename = "voucher_balance")]
    voucher_balance: Option<f64>,
    #[serde(rename = "cash_balance")]
    cash_balance: Option<f64>,
}

#[derive(Debug, serde::Deserialize)]
struct MoonshotBalanceResponse {
    data: Option<MoonshotBalanceData>,
}

pub struct MoonshotAiBalanceProvider;

impl BalanceProvider for MoonshotAiBalanceProvider {
    fn method(&self) -> &'static str {
        "moonshot-ai"
    }

    fn refresh<'a>(&'a self, input: &'a BalanceRefreshInput) -> Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>> {
        Box::pin(async move {
        let input = input.clone();
        let api_key = input.api_key.as_deref().ok_or_else(|| {
            IcodeError::validation("Moonshot AI 额度查询需要 API Key")
        })?;

        let base = input.base_url.as_deref().unwrap_or("https://api.moonshot.cn");
        let endpoint = format!("{}/v1/users/me/balance", base.trim_end_matches('/'));

        let client = reqwest::Client::new();
        let response = client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| IcodeError::internal(format!("Moonshot AI 额度查询请求失败: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(IcodeError::internal(format!(
                "Moonshot AI 额度查询失败 (HTTP {}): {}",
                status,
                text.chars().take(200).collect::<String>()
            )));
        }

        let body: MoonshotBalanceResponse = response
            .json()
            .await
            .map_err(|e| IcodeError::internal(format!("Moonshot AI 额度响应解析失败: {}", e)))?;

        let data = body.data.ok_or_else(|| {
            IcodeError::internal("Moonshot AI 额度响应中无数据")
        })?;

        let available = data.available_balance.unwrap_or(0.0);
        let voucher = data.voucher_balance.unwrap_or(0.0);
        let cash = data.cash_balance.unwrap_or(0.0);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let mut items = vec![
            BalanceMetric::amount(
                "balance-current",
                BalanceDirection::Remaining,
                serde_json::json!(available),
                Some("¥"),
            )
            .with_period(BalancePeriod::Current)
            .with_primary(true)
            .with_label("可用余额"),
        ];

        if cash > 0.0 {
            items.push(
                BalanceMetric::amount(
                    "cash-current",
                    BalanceDirection::Remaining,
                    serde_json::json!(cash),
                    Some("¥"),
                )
                .with_period(BalancePeriod::Current)
                .with_label("现金余额"),
            );
        }

        if voucher > 0.0 {
            items.push(
                BalanceMetric::amount(
                    "voucher-current",
                    BalanceDirection::Remaining,
                    serde_json::json!(voucher),
                    Some("¥"),
                )
                .with_period(BalancePeriod::Current)
                .with_label("代金券余额"),
            );
        }

        Ok(BalanceSnapshot {
            updated_at: now_ms,
            items,
        })
        })
    }
}
