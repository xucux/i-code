//! # 虚拟供应商模块 Tauri Command 声明
//!
//! 前端通过 `invoke('virtual_provider_*', payload)` 调用这些命令。
//!
//! ## 命令清单
//!
//! ### 虚拟供应商
//! - `virtual_provider_list`：列出所有虚拟供应商
//! - `virtual_provider_get`：获取虚拟供应商详情
//! - `virtual_provider_create`：创建虚拟供应商
//! - `virtual_provider_update`：更新虚拟供应商
//! - `virtual_provider_delete`：删除虚拟供应商
//!
//! ### 虚拟模型
//! - `virtual_provider_model_list`：列出指定虚拟供应商下的虚拟模型
//! - `virtual_provider_model_create`：创建虚拟模型
//! - `virtual_provider_model_delete`：删除虚拟模型
//!
//! ### 虚拟模型路由
//! - `virtual_provider_route_list`：列出指定虚拟模型下的路由
//! - `virtual_provider_routes_by_provider`：列出指定虚拟供应商下所有路由
//! - `virtual_provider_route_create`：创建路由
//! - `virtual_provider_route_delete`：删除路由

use tauri::State;

use crate::error::IcodeResult;

use super::service::VirtualProviderHandle;
use super::types::{
    CreateVirtualModelInput, CreateVirtualModelRouteInput, CreateVirtualProviderInput,
    SaveVirtualModelInput, UpdateVirtualModelInput, UpdateVirtualModelRouteInput,
    UpdateVirtualProviderInput, VirtualModel, VirtualModelRoute, VirtualProvider,
};

// ===== 虚拟供应商命令 =====

/// 列出所有虚拟供应商
#[tauri::command]
pub async fn virtual_provider_list(
    state: State<'_, VirtualProviderHandle>,
) -> IcodeResult<Vec<VirtualProvider>> {
    state.service().list_providers()
}

/// 获取虚拟供应商详情
#[tauri::command]
pub async fn virtual_provider_get(
    state: State<'_, VirtualProviderHandle>,
    id: String,
) -> IcodeResult<VirtualProvider> {
    state.service().get_provider(&id)
}

/// 创建虚拟供应商
#[tauri::command]
pub async fn virtual_provider_create(
    state: State<'_, VirtualProviderHandle>,
    input: CreateVirtualProviderInput,
) -> IcodeResult<VirtualProvider> {
    state.service().create_provider(input)
}

/// 保存虚拟模型（含子级路由）
///
/// 前端一次性提交父级虚拟模型与全部子级真实模型路由；后端在事务中完成创建/更新并重新关联子级路由。
#[tauri::command]
pub async fn virtual_model_save(
    state: State<'_, VirtualProviderHandle>,
    input: SaveVirtualModelInput,
) -> IcodeResult<VirtualModel> {
    state.service().save_model(input)
}

/// 更新虚拟供应商
#[tauri::command]
pub async fn virtual_provider_update(
    state: State<'_, VirtualProviderHandle>,
    id: String,
    input: UpdateVirtualProviderInput,
) -> IcodeResult<VirtualProvider> {
    state.service().update_provider(&id, input)
}

/// 删除虚拟供应商
#[tauri::command]
pub async fn virtual_provider_delete(
    state: State<'_, VirtualProviderHandle>,
    id: String,
) -> IcodeResult<()> {
    state.service().delete_provider(&id)
}

// ===== 虚拟模型命令 =====

/// 列出指定虚拟供应商下的所有虚拟模型
#[tauri::command]
pub async fn virtual_provider_model_list(
    state: State<'_, VirtualProviderHandle>,
    virtual_provider_id: String,
) -> IcodeResult<Vec<VirtualModel>> {
    state.service().list_models_by_provider(&virtual_provider_id)
}

/// 获取虚拟模型详情
#[tauri::command]
pub async fn virtual_provider_model_get(
    state: State<'_, VirtualProviderHandle>,
    id: String,
) -> IcodeResult<VirtualModel> {
    state.service().get_model(&id)
}

/// 创建虚拟模型
#[tauri::command]
pub async fn virtual_provider_model_create(
    state: State<'_, VirtualProviderHandle>,
    input: CreateVirtualModelInput,
) -> IcodeResult<VirtualModel> {
    state.service().create_model(input)
}

/// 更新虚拟模型
#[tauri::command]
pub async fn virtual_provider_model_update(
    state: State<'_, VirtualProviderHandle>,
    id: String,
    input: UpdateVirtualModelInput,
) -> IcodeResult<VirtualModel> {
    state.service().update_model(&id, input)
}

/// 删除虚拟模型
#[tauri::command]
pub async fn virtual_provider_model_delete(
    state: State<'_, VirtualProviderHandle>,
    id: String,
) -> IcodeResult<()> {
    state.service().delete_model(&id)
}

// ===== 虚拟模型路由命令 =====

/// 列出指定虚拟模型下的所有启用路由
#[tauri::command]
pub async fn virtual_provider_route_list(
    state: State<'_, VirtualProviderHandle>,
    virtual_model_id: String,
) -> IcodeResult<Vec<VirtualModelRoute>> {
    let result = state.service().list_routes_by_virtual_model(&virtual_model_id)?;
    tracing::info!("virtual_provider_route_list({}) => {} routes", virtual_model_id, result.len());
    Ok(result)
}

/// 列出指定虚拟供应商下的所有启用路由
///
/// 用于前端渲染「虚拟模型关系图」，一次性拉取某供应商下全部虚拟模型的子级路由。
#[tauri::command]
pub async fn virtual_provider_routes_by_provider(
    state: State<'_, VirtualProviderHandle>,
    virtual_provider_id: String,
) -> IcodeResult<Vec<VirtualModelRoute>> {
    let result = state.service().list_routes_by_provider(&virtual_provider_id)?;
    log::info!("virtual_provider_routes_by_provider({}) => {} routes", virtual_provider_id, result.len());
    Ok(result)
}

/// 获取路由详情
#[tauri::command]
pub async fn virtual_provider_route_get(
    state: State<'_, VirtualProviderHandle>,
    id: String,
) -> IcodeResult<VirtualModelRoute> {
    state.service().get_route(&id)
}

/// 创建虚拟模型路由
#[tauri::command]
pub async fn virtual_provider_route_create(
    state: State<'_, VirtualProviderHandle>,
    input: CreateVirtualModelRouteInput,
) -> IcodeResult<VirtualModelRoute> {
    state.service().create_route(input)
}

/// 更新虚拟模型路由
#[tauri::command]
pub async fn virtual_provider_route_update(
    state: State<'_, VirtualProviderHandle>,
    id: String,
    input: UpdateVirtualModelRouteInput,
) -> IcodeResult<VirtualModelRoute> {
    state.service().update_route(&id, input)
}

/// 删除虚拟模型路由
#[tauri::command]
pub async fn virtual_provider_route_delete(
    state: State<'_, VirtualProviderHandle>,
    id: String,
) -> IcodeResult<()> {
    state.service().delete_route(&id)
}
