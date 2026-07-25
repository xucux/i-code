//! # 额度监控模块 Tauri Command 声明
//!
//! 前端通过 `invoke('balance_*', payload)` 调用这些命令。
//!
//! ## 职责
//!
//! - `balance_refresh`：根据 BalanceConfig 查询额度，返回快照
//!
//! ## 安全
//!
//! API Key 不由前端传入，而是从 provider 的 authJson 中解析 $SECRET 引用后获得。

use tauri::{AppHandle, Emitter, State};

use crate::error::IcodeResult;

use super::provider::BalanceRefreshInput;
use super::service::BalanceServiceHandle;
use super::types::{BalanceConfig, BalanceRefreshResult};

/// 查询额度
///
/// 按 `BalanceConfig.method` 调用对应供应商 API。
/// 返回的快照由前端自行持久化到 `providers.balance_provider_json`。
/// 查询完成后广播 `balance:snapshot-updated` 事件，方便监听方自动刷新。
///
/// # 参数
/// - `providerId`：供应商 ID，用于结果标记
/// - `config`：额度监控配置
/// - `apiKey`：已解析的 API Key / Access Token 明文
/// - `baseUrl`：供应商 API 基础 URL（可选，覆盖 provider 默认值）
/// - `appCode`：AIHubMix 可选 APP-Code
/// - `projectId` / `managedProjectId`：Code Assist 类额度查询
/// - `accountId`：Codex ChatGPT Account Id
#[tauri::command]
pub async fn balance_refresh(
    app_handle: AppHandle,
    state: State<'_, BalanceServiceHandle>,
    provider_id: String,
    config: BalanceConfig,
    api_key: Option<String>,
    base_url: Option<String>,
    app_code: Option<String>,
    project_id: Option<String>,
    managed_project_id: Option<String>,
    account_id: Option<String>,
) -> IcodeResult<BalanceRefreshResult> {
    let input = BalanceRefreshInput {
        api_key,
        base_url,
        newapi_config: match &config {
            BalanceConfig::Newapi(cfg) => Some(cfg.clone()),
            _ => None,
        },
        claude_relay_config: match &config {
            BalanceConfig::ClaudeRelayService(cfg) => Some(cfg.clone()),
            _ => None,
        },
        app_code,
        project_id,
        managed_project_id,
        account_id,
    };

    let result = state.service().query_balance(&provider_id, &config, &input).await?;
    let _ = app_handle.emit("balance:snapshot-updated", &result);
    Ok(result)
}
