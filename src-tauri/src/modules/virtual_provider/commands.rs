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
//!
//! ### 路由尝试历史
//! - `virtual_provider_route_attempts_list`：查询指定路由最近 N 次尝试
//! - `virtual_provider_route_attempt_stats_list`：查询指定供应商下所有路由的尝试统计
//! - `virtual_provider_route_test`：测试单条路由（探活请求，不写入历史）
//!
//! ### alias 变更影响检查
//! - `virtual_provider_check_alias_impact`：检查修改 alias 对 CLI 模型映射的影响
//!
//! ### 一键生成（三模型槽位）
//! - `virtual_provider_generate_preset`：一键生成虚拟供应商 + 三个虚拟模型（按数据源 JSON 匹配实体模型）
//! - `virtual_slots_config_get`：读取数据源 URL 配置
//! - `virtual_slots_config_set`：保存数据源 URL 配置

use tauri::State;

use crate::error::IcodeResult;
use crate::modules::ai_gateway::AiGatewayServiceHandle;

use super::service::VirtualProviderHandle;
use super::types::{
    AliasImpactResult, CreateVirtualModelInput, CreateVirtualModelRouteInput,
    CreateVirtualProviderInput, GenerateVirtualProviderInput, GenerateVirtualProviderResult,
    RouteAttemptStats, RouteTestResult, SaveVirtualModelInput, UpdateVirtualModelInput,
    UpdateVirtualModelRouteInput, UpdateVirtualProviderInput, VirtualModel, VirtualModelRoute,
    VirtualProvider, VirtualRouteAttempt, VirtualSlotsConfigDto, VirtualSlotsConfigSetInput,
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
/// 注入 ai_gateway Service 用于隔离校验（视觉生成供应商不允许加入虚拟路由）。
#[tauri::command]
pub async fn virtual_model_save(
    state: State<'_, VirtualProviderHandle>,
    ai_gateway: State<'_, AiGatewayServiceHandle>,
    input: SaveVirtualModelInput,
) -> IcodeResult<VirtualModel> {
    state.service().save_model(ai_gateway.service(), input)
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

// ===== 路由尝试历史命令 =====

/// 查询指定路由的最近 N 次尝试历史
///
/// 默认返回最近 50 条，最大 200 条。
#[tauri::command]
pub async fn virtual_provider_route_attempts_list(
    state: State<'_, VirtualProviderHandle>,
    route_id: String,
    limit: Option<u32>,
) -> IcodeResult<Vec<VirtualRouteAttempt>> {
    // 限制最大 200 条，默认 50 条
    let limit = limit.unwrap_or(50).min(200);
    state
        .service()
        .list_recent_attempts_by_route(&route_id, limit)
}

/// 查询指定供应商下所有路由的尝试统计
///
/// 返回每条路由的总数 / 成功数 / 失败数 / 成功率 / 平均耗时 / 最近失败原因 / 最近尝试时间，
/// 用于前端「路由历史」Tab 展示。
#[tauri::command]
pub async fn virtual_provider_route_attempt_stats_list(
    state: State<'_, VirtualProviderHandle>,
    virtual_provider_id: String,
) -> IcodeResult<Vec<RouteAttemptStats>> {
    state
        .service()
        .list_route_attempt_stats_by_provider(&virtual_provider_id)
}

/// 测试单条路由：对目标供应商发起轻量探活请求（GET /v1/models，5s 超时）
///
/// 与调度器健康检查逻辑一致，但不写入 `virtual_route_attempts`（避免污染统计）。
/// 结果用 toast 展示，供用户手动验证路由配置是否可用。
#[tauri::command]
pub async fn virtual_provider_route_test(
    state: State<'_, VirtualProviderHandle>,
    ai_gateway: State<'_, AiGatewayServiceHandle>,
    route_id: String,
) -> IcodeResult<RouteTestResult> {
    state.service().test_route(&route_id, ai_gateway.service()).await
}

/// 检查修改虚拟供应商 alias 的影响范围
///
/// 返回受影响的 CLI 模型映射数量（`cli_model_mappings.gateway_model_id` 以旧 alias 为前缀）。
/// 前端在用户修改 alias 时调用，展示警告提示，用户确认后才允许提交。
#[tauri::command]
pub async fn virtual_provider_check_alias_impact(
    state: State<'_, VirtualProviderHandle>,
    virtual_provider_id: String,
    new_alias: String,
) -> IcodeResult<AliasImpactResult> {
    state
        .service()
        .check_alias_impact(&virtual_provider_id, &new_alias)
}

// ===== 一键生成（三模型槽位）=====

/// 一键生成虚拟供应商 + 三个虚拟模型
///
/// 从数据源 JSON（远程优先，失败回退内置）读取虚拟供应商元信息与三个槽位规则，
/// 从「已开启显示的模型列表」中按优先级匹配实体模型，自动创建虚拟供应商与子级路由。
#[tauri::command]
pub async fn virtual_provider_generate_preset(
    state: State<'_, VirtualProviderHandle>,
    ai_gateway: State<'_, AiGatewayServiceHandle>,
    input: GenerateVirtualProviderInput,
) -> IcodeResult<GenerateVirtualProviderResult> {
    state.service().generate_preset(ai_gateway.service(), input).await
}

/// 读取数据源 URL 配置
///
/// 返回用户已保存的值、内置默认值、实际生效值。
#[tauri::command]
pub async fn virtual_slots_config_get(
    state: State<'_, VirtualProviderHandle>,
) -> IcodeResult<VirtualSlotsConfigDto> {
    state.service().get_slots_config()
}

/// 保存数据源 URL 配置
///
/// 传空字符串表示清空（恢复默认）。非法 URL 返回 `VALIDATION`。
#[tauri::command]
pub async fn virtual_slots_config_set(
    state: State<'_, VirtualProviderHandle>,
    input: VirtualSlotsConfigSetInput,
) -> IcodeResult<VirtualSlotsConfigDto> {
    state.service().set_slots_config(&input)
}
