//! # DeepSeek 额度查询实现
//!
//! 调用 DeepSeek 用户余额 API：`GET {base_url}/user/balance`
//! 响应格式：`{ data: { balance_infos: [{ currency, total_balance, topped_up_balance, granted_balance }] } }`

use std::future::Future;
use std::pin::Pin;

use crate::error::{IcodeError, IcodeResult};

use super::super::provider::{BalanceProvider, BalanceRefreshInput};
use super::super::types::{BalanceDirection, BalanceMetric, BalancePeriod, BalanceSnapshot};

/// DeepSeek 余额响应中的单项
#[derive(Debug, serde::Deserialize)]
struct DeepSeekBalanceItem {
    currency: Option<String>,
    #[serde(rename = "total_balance")]
    total_balance: Option<f64>,
    #[serde(rename = "topped_up_balance")]
    topped_up_balance: Option<f64>,
    #[serde(rename = "granted_balance")]
    granted_balance: Option<f64>,
}

/// DeepSeek 余额响应
#[derive(Debug, serde::Deserialize)]
struct DeepSeekBalanceResponse {
    #[serde(rename = "is_available")]
    is_available: Option<bool>,
    data: Option<DeepSeekBalanceData>,
}

#[derive(Debug, serde::Deserialize)]
struct DeepSeekBalanceData {
    #[serde(rename = "balance_infos")]
    balance_infos: Option<Vec<DeepSeekBalanceItem>>,
}

/// 获取货币符号
fn currency_symbol(currency: &str) -> Option<String> {
    match currency {
        "CNY" => Some("¥".to_string()),
        "USD" => Some("$".to_string()),
        _ => None,
    }
}

pub struct DeepSeekBalanceProvider;

impl BalanceProvider for DeepSeekBalanceProvider {
    fn method(&self) -> &'static str {
        "deepseek"
    }

    fn refresh<'a>(&'a self, input: &'a BalanceRefreshInput) -> Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>> {
        Box::pin(async move {
        let api_key = input.api_key.as_deref().ok_or_else(|| {
            IcodeError::validation("DeepSeek 额度查询需要 API Key")
        })?;

        // 构建端点 URL
        let base = input.base_url.as_deref().unwrap_or("https://api.deepseek.com");
        let endpoint = if base.to_lowercase().ends_with("/v1") {
            format!("{}/user/balance", base.trim_end_matches('/'))
        } else {
            format!("{}/v1/user/balance", base.trim_end_matches('/'))
        };

        let client = super::super::provider::build_balance_http_client()?;
        let response = client
            .get(&endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| IcodeError::internal(format!("DeepSeek 额度查询请求失败: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(IcodeError::internal(format!(
                "DeepSeek 额度查询失败 (HTTP {}): {}",
                status,
                text.chars().take(200).collect::<String>()
            )));
        }

        let body: DeepSeekBalanceResponse = response
            .json()
            .await
            .map_err(|e| IcodeError::internal(format!("DeepSeek 额度响应解析失败: {}", e)))?;

        // 检查 is_available
        if body.is_available == Some(false) {
            return Err(IcodeError::internal("DeepSeek 账户不可用"));
        }

        let items_data = body.data.and_then(|d| d.balance_infos).unwrap_or_default();
        if items_data.is_empty() {
            return Err(IcodeError::internal("DeepSeek 额度响应中无余额数据"));
        }

        // 选择主货币：CNY 优先 → USD → 第一个
        let primary = items_data
            .iter()
            .find(|i| i.currency.as_deref() == Some("CNY"))
            .or_else(|| items_data.iter().find(|i| i.currency.as_deref() == Some("USD")))
            .or_else(|| items_data.first())
            .unwrap();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let items: Vec<BalanceMetric> = items_data
            .iter()
            .filter_map(|item| {
                let total = item.total_balance
                    .or_else(|| {
                        let topped = item.topped_up_balance.unwrap_or(0.0);
                        let granted = item.granted_balance.unwrap_or(0.0);
                        Some(topped + granted)
                    })?;

                if !total.is_finite() {
                    return None;
                }

                let currency = item.currency.as_deref().unwrap_or("CNY");
                let is_primary = std::ptr::eq(item, primary);

                Some(BalanceMetric::amount(
                    format!("balance-current-{}", currency.to_lowercase()),
                    BalanceDirection::Remaining,
                    serde_json::json!(total),
                    currency_symbol(currency).as_deref(),
                )
                .with_period(BalancePeriod::Current)
                .with_primary(is_primary)
                .with_scope(currency.to_uppercase()))
            })
            .collect();

        Ok(BalanceSnapshot {
            updated_at: now_ms,
            items,
        })
        })
    }
}
