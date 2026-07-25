//! # 硅基流动（SiliconFlow）额度查询实现
//!
//! 调用 SiliconFlow 用户信息 API：`GET {base_url}/v1/user/info`
//! 响应格式：`{ data: { balance, chargeBalance, totalBalance } }`

use std::future::Future;
use std::pin::Pin;

use crate::error::{IcodeError, IcodeResult};

use super::super::provider::{BalanceProvider, BalanceRefreshInput};
use super::super::types::{BalanceDirection, BalanceMetric, BalancePeriod, BalanceSnapshot};

/// SiliconFlow 用户信息响应
#[derive(Debug, serde::Deserialize)]
struct SiliconFlowUserInfoData {
    balance: Option<f64>,
    #[serde(rename = "chargeBalance")]
    charge_balance: Option<f64>,
    #[serde(rename = "totalBalance")]
    total_balance: Option<f64>,
}

#[derive(Debug, serde::Deserialize)]
struct SiliconFlowUserInfoResponse {
    code: Option<i64>,
    status: Option<bool>,
    message: Option<String>,
    data: Option<SiliconFlowUserInfoData>,
}

pub struct SiliconFlowBalanceProvider;

impl BalanceProvider for SiliconFlowBalanceProvider {
    fn method(&self) -> &'static str {
        "siliconflow"
    }

    fn refresh<'a>(&'a self, input: &'a BalanceRefreshInput) -> Pin<Box<dyn Future<Output = IcodeResult<BalanceSnapshot>> + Send + 'a>> {
        Box::pin(async move {
        let input = input.clone();
        let api_key = input.api_key.as_deref().ok_or_else(|| {
            IcodeError::validation("硅基流动额度查询需要 API Key")
        })?;

        // 构建端点 URL
        let base = input.base_url.as_deref().unwrap_or("https://api.siliconflow.cn");
        let endpoint = if base.to_lowercase().ends_with("/v1") {
            format!("{}/user/info", base.trim_end_matches('/'))
        } else {
            format!("{}/v1/user/info", base.trim_end_matches('/'))
        };

        let client = reqwest::Client::new();
        let response = client
            .get(&endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| IcodeError::internal(format!("硅基流动额度查询请求失败: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(IcodeError::internal(format!(
                "硅基流动额度查询失败 (HTTP {}): {}",
                status,
                text.chars().take(200).collect::<String>()
            )));
        }

        let body: SiliconFlowUserInfoResponse = response
            .json()
            .await
            .map_err(|e| IcodeError::internal(format!("硅基流动额度响应解析失败: {}", e)))?;

        // 检查业务状态码
        if body.code.map_or(false, |c| c != 20000) || body.status == Some(false) {
            let msg = body.message.as_deref().unwrap_or("未知错误");
            return Err(IcodeError::internal(format!("硅基流动额度查询业务错误: {}", msg)));
        }

        let data = body.data.ok_or_else(|| {
            IcodeError::internal("硅基流动额度响应中无数据")
        })?;

        let granted = data.balance.unwrap_or(0.0);
        let paid = data.charge_balance.unwrap_or(0.0);
        let total = data.total_balance.unwrap_or(paid + granted);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let items = vec![
            BalanceMetric::amount(
                "balance-current",
                BalanceDirection::Remaining,
                serde_json::json!(total),
                Some("¥"),
            )
            .with_period(BalancePeriod::Current)
            .with_primary(true)
            .with_label("余额"),
            BalanceMetric::amount(
                "charge-current",
                BalanceDirection::Remaining,
                serde_json::json!(paid),
                Some("¥"),
            )
            .with_period(BalancePeriod::Current)
            .with_label("充值余额"),
            BalanceMetric::amount(
                "granted-current",
                BalanceDirection::Remaining,
                serde_json::json!(granted),
                Some("¥"),
            )
            .with_period(BalancePeriod::Current)
            .with_label("赠送余额"),
        ];

        Ok(BalanceSnapshot {
            updated_at: now_ms,
            items,
        })
        })
    }
}
