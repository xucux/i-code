//! # 额度监控业务服务层
//!
//! 按 `BalanceConfig.method` 分发到对应的 Provider 实现。
//!
//! ## 实现状态
//!
//! - `none`：返回空快照
//! - `synthetic`：返回合成测试数据
//! - `deepseek`：调用 DeepSeek 用户余额 API ✅
//! - `openrouter`：调用 OpenRouter credits API ✅
//! - `siliconflow`：调用硅基流动用户信息 API ✅
//! - `moonshot-ai`：调用 Moonshot AI 余额 API ✅
//! - `kimi-code`：调用 Kimi Code usages API ✅
//! - `newapi`：调用 New API / OneAPI 系额度 API ✅
//! - `aihubmix`：调用 AIHubMix remain API ✅
//! - `claude-relay-service`：调用 Claude Relay Service apiStats API ✅
//! - `minimax`：调用 MiniMax coding plan remains API ✅
//! - `antigravity`：调用 Code Assist retrieveUserQuota ✅
//! - `gemini-cli`：调用 Code Assist retrieveUserQuota ✅
//! - `codex`：调用 Codex usage API（多端点探测）✅

use std::sync::Arc;

use crate::error::IcodeResult;

use super::provider::BalanceRefreshInput;
use super::providers::dispatch_refresh;
use super::types::{BalanceConfig, BalanceRefreshResult};

/// Balance Service 在 Tauri State 中的句柄
#[derive(Clone, Default)]
pub struct BalanceServiceHandle {
    inner: Arc<BalanceService>,
}

impl BalanceServiceHandle {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(BalanceService),
        }
    }

    pub fn service(&self) -> &BalanceService {
        &self.inner
    }
}

/// Balance Service 业务逻辑
#[derive(Default)]
pub struct BalanceService;

impl BalanceService {
    /// 查询额度
    ///
    /// 按 `config.method` 分发到对应 Provider。
    /// 返回的 `BalanceSnapshot` 由调用方持久化到 `providers.balance_provider_json`。
    ///
    /// # 参数
    /// - `cli_provider_id`：供应商 ID，仅用于结果标记
    /// - `config`：额度监控配置（method + 可选参数）
    /// - `input`：已解析的查询参数（API Key、Base URL 等）
    pub async fn query_balance(
        &self,
        cli_provider_id: &str,
        config: &BalanceConfig,
        input: &BalanceRefreshInput,
    ) -> IcodeResult<BalanceRefreshResult> {
        let snapshot = dispatch_refresh(config, input).await?;

        Ok(BalanceRefreshResult {
            cli_provider_id: cli_provider_id.to_string(),
            snapshot,
            warnings: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::balance::types::BalanceMethod;

    #[test]
    fn test_query_none() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let svc = BalanceService;
        let result = rt
            .block_on(svc.query_balance("test-id", &BalanceConfig::None, &BalanceRefreshInput::default()))
            .unwrap();
        assert_eq!(result.cli_provider_id, "test-id");
        assert!(result.snapshot.items.is_empty());
    }

    #[test]
    fn test_query_synthetic() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let svc = BalanceService;
        let result = rt
            .block_on(svc.query_balance("test-id", &BalanceConfig::Synthetic, &BalanceRefreshInput::default()))
            .unwrap();
        assert_eq!(result.snapshot.items.len(), 4);
        assert_eq!(
            result.snapshot.items[0].metric_type,
            super::super::types::BalanceMetricType::Amount
        );
    }

    #[test]
    fn test_missing_api_key_returns_error() {
        // 所有真实 Provider 都要求 api_key；缺失时应返回校验错误，不会发起网络请求
        let rt = tokio::runtime::Runtime::new().unwrap();
        let svc = BalanceService;
        let result = rt.block_on(svc.query_balance(
            "test-id",
            &BalanceConfig::Minimax,
            &BalanceRefreshInput::default(),
        ));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("MiniMax"), "unexpected message: {}", err.message);
    }

    #[test]
    fn test_all_methods_have_provider_or_special() {
        // 确保所有 BalanceMethod 都已注册 Provider 或属于特殊（none/synthetic）分支
        for method in [
            BalanceMethod::MoonshotAi,
            BalanceMethod::KimiCode,
            BalanceMethod::Newapi,
            BalanceMethod::Deepseek,
            BalanceMethod::Openrouter,
            BalanceMethod::Siliconflow,
            BalanceMethod::Aihubmix,
            BalanceMethod::ClaudeRelayService,
            BalanceMethod::Antigravity,
            BalanceMethod::GeminiCli,
            BalanceMethod::Codex,
            BalanceMethod::Minimax,
        ] {
            assert!(
                super::super::providers::get_provider(method).is_some(),
                "method {:?} should have a registered provider",
                method
            );
        }
        assert!(super::super::providers::get_provider(BalanceMethod::None).is_none());
        assert!(super::super::providers::get_provider(BalanceMethod::Synthetic).is_none());
    }
}
