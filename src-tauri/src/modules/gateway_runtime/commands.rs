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
//! - `gateway_list_local_ips`：枚举本机网卡 IPv4 地址（网关监听 0.0.0.0 时用于展示可访问地址）
//! - `gateway_get_forward_log_config`：读取转发详细日志配置
//! - `gateway_set_forward_log_config`：更新转发详细日志配置

use std::net::IpAddr;

use local_ip_address::list_afinet_netifas;
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

/// 枚举本机网卡 IPv4 地址
///
/// 当网关监听 `0.0.0.0` 时，前端需要展示实际可访问本机的 LAN 地址。
/// 返回的列表已剔除 loopback（127.0.0.0/8）与 link-local（169.254.0.0/16）地址，
/// 顺序保持系统接口枚举顺序，前端按优先级（192.168.0.0/24 > 192.168/16 > 172.16/12 > 10/8）再排序。
#[tauri::command]
pub fn gateway_list_local_ips() -> IcodeResult<Vec<String>> {
    let ifas = list_afinet_netifas().map_err(|e| {
        crate::error::IcodeError::internal(format!("枚举本机网卡地址失败: {e}"))
    })?;

    let ips = ifas
        .into_iter()
        .filter_map(|(_, ip)| match ip {
            IpAddr::V4(v4) => {
                // 跳过 loopback 与 link-local，避免在 UI 上展示无效地址
                if v4.is_loopback() || v4.is_link_local() {
                    None
                } else {
                    Some(v4.to_string())
                }
            }
            IpAddr::V6(_) => None,
        })
        .collect::<Vec<_>>();

    Ok(ips)
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
