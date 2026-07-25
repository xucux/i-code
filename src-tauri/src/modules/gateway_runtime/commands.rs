//! # Gateway Runtime 模块 Tauri Command 声明
//!
//! 前端通过 `invoke('gateway_*', payload)` 调用这些命令。
//!
//! ## 命令清单
//!
//! - `gateway_start`：启动 HTTP Server
//! - `gateway_stop`：停止 HTTP Server
//! - `gateway_status`：获取当前运行时状态
//! - `gateway_health`：健康检查
//! - `gateway_get_forward_log_config`：读取转发详细日志配置
//! - `gateway_set_forward_log_config`：更新转发详细日志配置

use tauri::State;

use crate::error::IcodeResult;
use crate::modules::logger::types::ForwardLogConfig;

use super::service::GatewayRuntimeHandle;
use super::types::{GatewayRuntimeState, HealthCheckResult, StartGatewayInput, StartGatewayResult};

/// 启动网关 HTTP Server
///
/// 输入参数为空时使用 `app_settings` 中的配置。
#[tauri::command]
pub async fn gateway_start(
    state: State<'_, GatewayRuntimeHandle>,
    input: Option<StartGatewayInput>,
) -> IcodeResult<StartGatewayResult> {
    state
        .service()
        .start(input.unwrap_or_default())
        .await
}

/// 停止网关 HTTP Server
#[tauri::command]
pub async fn gateway_stop(state: State<'_, GatewayRuntimeHandle>) -> IcodeResult<()> {
    state.service().stop().await
}

/// 获取网关运行时状态
#[tauri::command]
pub async fn gateway_status(
    state: State<'_, GatewayRuntimeHandle>,
) -> IcodeResult<GatewayRuntimeState> {
    state.service().status()
}

/// 健康检查
///
/// 检查数据库连接与（后续）上游供应商可达性。
#[tauri::command]
pub async fn gateway_health(
    state: State<'_, GatewayRuntimeHandle>,
) -> IcodeResult<HealthCheckResult> {
    state.service().health()
}

/// 读取转发详细日志配置
#[tauri::command]
pub async fn gateway_get_forward_log_config(
    state: State<'_, GatewayRuntimeHandle>,
) -> IcodeResult<ForwardLogConfig> {
    Ok(state.service().get_forward_log_config())
}

/// 更新转发详细日志配置
///
/// 开启后，网关转发时将请求体/响应体写入日志缓冲区，便于调试。
#[tauri::command]
pub async fn gateway_set_forward_log_config(
    state: State<'_, GatewayRuntimeHandle>,
    config: ForwardLogConfig,
) -> IcodeResult<ForwardLogConfig> {
    state.service().set_forward_log_config(config);
    Ok(state.service().get_forward_log_config())
}
