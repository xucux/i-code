//! imagebed 模块 Tauri Command（参数校验 → 调 Service）

use tauri::State;

use crate::error::{IcodeError, IcodeResult};

use super::service::ImagebedHandle;
use super::types::ImagebedProvider;

/// 列出全部内置图床 provider（安全字段，不含注入脚本）
#[tauri::command]
pub fn imagebed_list(_state: State<'_, ImagebedHandle>) -> IcodeResult<Vec<ImagebedProvider>> {
    Ok(super::providers::ALL_PROVIDERS
        .iter()
        .map(ImagebedProvider::from_spec)
        .collect())
}

/// 打开图床浏览器窗口（按 provider id 查找内置图床）
///
/// 必须为 async command：Windows 上在同步 Command / 事件处理器中创建 Webview
/// 会 deadlock（见 tauri WebviewBuilder 文档 Known issues），表现为窗口白屏。
#[tauri::command]
pub async fn imagebed_open(state: State<'_, ImagebedHandle>, provider_id: String) -> IcodeResult<()> {
    let provider = super::providers::provider_by_id(&provider_id)
        .ok_or_else(|| IcodeError::not_found("图床", Some(&provider_id)))?;
    state.open(provider)
}

/// 关闭图床浏览器窗口
#[tauri::command]
pub async fn imagebed_close(state: State<'_, ImagebedHandle>) -> IcodeResult<()> {
    state.close()
}