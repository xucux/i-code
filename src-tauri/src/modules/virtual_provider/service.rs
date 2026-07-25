//! # 虚拟供应商业务服务层
//!
//! 提供虚拟供应商、虚拟模型、虚拟模型路由的 CRUD，以及故障转移路由解析。
//!
//! ## 核心流程
//!
//! 1. 用户通过 `POST /v1/chat/completions` 请求模型 ID 形如 `{virtual_alias}/{model_id}`。
//! 2. `gateway_runtime/upstream.rs` 先在真实供应商中查找；未找到时调用
//!    [`VirtualProviderService::resolve_virtual_route`]。
//! 3. Service 根据 `virtual_providers.strategy` 选择路由：
//!    - `fallback`：按 `priority` 升序返回第一条启用且健康的路由。
//!    - `on_all` / `load_balance`：v0.1 直接返回未实现错误。
//! 4. upstream.rs 拿到 `ResolvedVirtualRoute` 后，通过 `ai_gateway` 加载目标真实供应商，
//!    并将请求体中的 `model` 替换为 `target_model_id` 进行转发。

use std::sync::Arc;

use crate::error::{IcodeError, IcodeResult};

use super::repository;
use super::types::{
    CreateVirtualModelInput, CreateVirtualModelRouteInput, CreateVirtualProviderInput,
    ExposedVirtualModel, ResolvedVirtualRoute, SaveVirtualModelInput, UpdateVirtualModelInput,
    UpdateVirtualModelRouteInput, UpdateVirtualProviderInput, VirtualModel, VirtualModelRoute,
    VirtualProvider, VirtualProviderStrategy,
};

/// 虚拟供应商级默认重试次数
#[expect(dead_code)]
const DEFAULT_PROVIDER_MAX_RETRIES: i64 = 3;
/// 虚拟供应商级默认重试间隔（毫秒）
#[expect(dead_code)]
const DEFAULT_PROVIDER_RETRY_INTERVAL_MS: i64 = 1000;

/// 虚拟供应商 Service 在 Tauri State 中的句柄
///
/// 使用 `Arc` 包装以便在多线程间共享。
#[derive(Clone)]
pub struct VirtualProviderHandle {
    inner: Arc<VirtualProviderService>,
}

impl VirtualProviderHandle {
    /// 创建虚拟供应商句柄
    pub fn new() -> Self {
        Self {
            inner: Arc::new(VirtualProviderService::new()),
        }
    }

    /// 获取内部 Service 引用
    pub fn service(&self) -> &VirtualProviderService {
        &self.inner
    }
}

impl Default for VirtualProviderHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// 虚拟供应商业务逻辑
pub struct VirtualProviderService;

impl VirtualProviderService {
    /// 创建 Service 实例
    pub fn new() -> Self {
        Self
    }

    // ===== 虚拟供应商 CRUD =====

    /// 创建虚拟供应商
    ///
    /// 校验：
    /// 1. name 非空
    /// 2. alias 非空且唯一
    /// 3. strategy 若提供则必须合法
    pub fn create_provider(
        &self,
        input: CreateVirtualProviderInput,
    ) -> IcodeResult<VirtualProvider> {
        if input.name.trim().is_empty() {
            return Err(IcodeError::validation("虚拟供应商 name 不能为空"));
        }
        if input.alias.trim().is_empty() {
            return Err(IcodeError::validation("虚拟供应商 alias 不能为空"));
        }

        // alias 唯一性校验
        if let Some(existing) = repository::find_provider_by_alias(&input.alias)? {
            return Err(IcodeError::conflict(format!(
                "虚拟供应商 alias '{}' 已存在 (id: {})",
                input.alias, existing.id
            )));
        }

        // strategy 合法性校验（若提供）
        if let Some(s) = &input.strategy {
            if VirtualProviderStrategy::from_str(s).is_none() {
                return Err(IcodeError::validation(format!(
                    "未知的路由策略: {}",
                    s
                )));
            }
        }

        repository::insert_provider(&input)
    }

    /// 获取虚拟供应商详情
    pub fn get_provider(&self, id: &str) -> IcodeResult<VirtualProvider> {
        repository::find_provider_by_id(id)
    }

    /// 列出所有虚拟供应商
    pub fn list_providers(&self) -> IcodeResult<Vec<VirtualProvider>> {
        repository::list_providers()
    }

    /// 更新虚拟供应商
    ///
    /// 若更新 alias，需保证唯一性。
    pub fn update_provider(
        &self,
        id: &str,
        input: UpdateVirtualProviderInput,
    ) -> IcodeResult<VirtualProvider> {
        // 校验记录存在
        let existing = repository::find_provider_by_id(id)?;

        // alias 唯一性校验
        if let Some(alias) = &input.alias {
            if alias.trim().is_empty() {
                return Err(IcodeError::validation("虚拟供应商 alias 不能为空"));
            }
            if alias != &existing.alias {
                if repository::find_provider_by_alias(alias)?.is_some() {
                    return Err(IcodeError::conflict(format!(
                        "虚拟供应商 alias '{}' 已存在",
                        alias
                    )));
                }
            }
        }

        // strategy 合法性校验（若提供）
        if let Some(s) = &input.strategy {
            if VirtualProviderStrategy::from_str(s).is_none() {
                return Err(IcodeError::validation(format!(
                    "未知的路由策略: {}",
                    s
                )));
            }
        }

        repository::update_provider(id, &input)
    }

    /// 删除虚拟供应商
    ///
    /// 关联的 virtual_models 与 virtual_model_routes 由外键级联删除。
    pub fn delete_provider(&self, id: &str) -> IcodeResult<()> {
        repository::delete_provider(id)
    }

    /// 保存虚拟模型（含子级路由）
    ///
    /// 前端一次性提交父级虚拟模型与全部子级真实模型路由。
    /// 后端在单一事务中：
    /// 1. 创建或更新虚拟模型；
    /// 2. 删除该虚拟模型的全部已有路由；
    /// 3. 按提交顺序重新写入子级路由。
    pub fn save_model(&self, input: SaveVirtualModelInput) -> IcodeResult<VirtualModel> {
        if input.model_id.trim().is_empty() {
            return Err(IcodeError::validation("虚拟模型 model_id 不能为空"));
        }

        // 校验所属供应商存在
        let provider = repository::find_provider_by_id(&input.virtual_provider_id)?;

        // 子级路由基础校验
        for route in &input.routes {
            if route.target_provider_id.trim().is_empty() {
                return Err(IcodeError::validation("路由目标供应商 ID 不能为空"));
            }
            if route.target_model_id.trim().is_empty() {
                return Err(IcodeError::validation("路由目标模型 ID 不能为空"));
            }
        }

        if let Some(id) = &input.id {
            // 更新模式：校验记录存在且属于该供应商
            let existing = repository::find_model_by_id(id)?;
            if existing.virtual_provider_id != input.virtual_provider_id {
                return Err(IcodeError::validation(format!(
                    "虚拟模型 '{}' 不属于供应商 '{}'",
                    id, provider.name
                )));
            }

            // model_id 唯一性校验（同一供应商下）
            if input.model_id != existing.model_id {
                if let Some(conflict) = repository::find_model_by_provider_and_model_id(
                    &input.virtual_provider_id,
                    &input.model_id,
                )? {
                    if conflict.id != *id {
                        return Err(IcodeError::conflict(format!(
                            "虚拟供应商下模型 ID '{}' 已存在",
                            input.model_id
                        )));
                    }
                }
            }
        } else {
            // 新建模式：model_id 唯一性校验（同一供应商下）
            if repository::find_model_by_provider_and_model_id(
                &input.virtual_provider_id,
                &input.model_id,
            )?
            .is_some()
            {
                return Err(IcodeError::conflict(format!(
                    "虚拟供应商下模型 ID '{}' 已存在",
                    input.model_id
                )));
            }
        }

        repository::save_model(&input)
    }

    // ===== 虚拟模型 CRUD =====

    /// 创建虚拟模型
    ///
    /// 校验：
    /// 1. virtual_provider_id 存在
    /// 2. model_id 非空
    /// 3. 同一虚拟供应商下 model_id 唯一
    pub fn create_model(&self, input: CreateVirtualModelInput) -> IcodeResult<VirtualModel> {
        if input.model_id.trim().is_empty() {
            return Err(IcodeError::validation("虚拟模型 model_id 不能为空"));
        }

        // 校验虚拟供应商存在
        let _provider = repository::find_provider_by_id(&input.virtual_provider_id)?;

        // 唯一性校验
        if repository::find_model_by_provider_and_model_id(
            &input.virtual_provider_id,
            &input.model_id,
        )?
        .is_some()
        {
            return Err(IcodeError::conflict(format!(
                "虚拟供应商下模型 ID '{}' 已存在",
                input.model_id
            )));
        }

        repository::insert_model(&input)
    }

    /// 获取虚拟模型详情
    pub fn get_model(&self, id: &str) -> IcodeResult<VirtualModel> {
        repository::find_model_by_id(id)
    }

    /// 列出指定虚拟供应商下的所有虚拟模型
    pub fn list_models_by_provider(
        &self,
        virtual_provider_id: &str,
    ) -> IcodeResult<Vec<VirtualModel>> {
        repository::list_models_by_provider(virtual_provider_id)
    }

    /// 列出所有对外暴露的虚拟模型
    ///
    /// 供 `gateway_runtime` 的 `/v1/models` 接口使用。
    /// 仅返回虚拟供应商与虚拟模型均启用的记录；对外 ID 为 `{virtual_alias}/{model_id}`。
    pub fn list_exposed_virtual_models(&self) -> IcodeResult<Vec<ExposedVirtualModel>> {
        repository::list_exposed_virtual_models()
    }

    /// 更新虚拟模型
    pub fn update_model(
        &self,
        id: &str,
        input: UpdateVirtualModelInput,
    ) -> IcodeResult<VirtualModel> {
        // 若更新 model_id，需保证同一供应商下唯一
        if let Some(model_id) = &input.model_id {
            if model_id.trim().is_empty() {
                return Err(IcodeError::validation("虚拟模型 model_id 不能为空"));
            }
            let existing = repository::find_model_by_id(id)?;
            if model_id != &existing.model_id {
                if repository::find_model_by_provider_and_model_id(
                    &existing.virtual_provider_id,
                    model_id,
                )?
                .is_some()
                {
                    return Err(IcodeError::conflict(format!(
                        "虚拟供应商下模型 ID '{}' 已存在",
                        model_id
                    )));
                }
            }
        }

        repository::update_model(id, &input)
    }

    /// 删除虚拟模型
    ///
    /// 关联的 virtual_model_routes 由外键级联删除。
    pub fn delete_model(&self, id: &str) -> IcodeResult<()> {
        repository::delete_model(id)
    }

    // ===== 虚拟模型路由 CRUD =====

    /// 创建虚拟模型路由
    ///
    /// 校验：
    /// 1. virtual_model_id 存在
    /// 2. target_provider_id 非空
    /// 3. target_model_id 非空
    pub fn create_route(
        &self,
        input: CreateVirtualModelRouteInput,
    ) -> IcodeResult<VirtualModelRoute> {
        if input.target_provider_id.trim().is_empty() {
            return Err(IcodeError::validation("路由目标供应商 ID 不能为空"));
        }
        if input.target_model_id.trim().is_empty() {
            return Err(IcodeError::validation("路由目标模型 ID 不能为空"));
        }

        // 校验虚拟模型存在
        let _model = repository::find_model_by_id(&input.virtual_model_id)?;

        repository::insert_route(&input)
    }

    /// 获取路由详情
    pub fn get_route(&self, id: &str) -> IcodeResult<VirtualModelRoute> {
        repository::find_route_by_id(id)
    }

    /// 列出指定虚拟模型下的所有启用路由
    pub fn list_routes_by_virtual_model(
        &self,
        virtual_model_id: &str,
    ) -> IcodeResult<Vec<VirtualModelRoute>> {
        repository::list_routes_by_virtual_model(virtual_model_id)
    }

    /// 列出指定虚拟供应商下所有启用路由
    pub fn list_routes_by_provider(
        &self,
        virtual_provider_id: &str,
    ) -> IcodeResult<Vec<VirtualModelRoute>> {
        repository::list_routes_by_provider(virtual_provider_id)
    }

    /// 更新虚拟模型路由
    pub fn update_route(
        &self,
        id: &str,
        input: UpdateVirtualModelRouteInput,
    ) -> IcodeResult<VirtualModelRoute> {
        if let Some(target_provider_id) = &input.target_provider_id {
            if target_provider_id.trim().is_empty() {
                return Err(IcodeError::validation("路由目标供应商 ID 不能为空"));
            }
        }
        if let Some(target_model_id) = &input.target_model_id {
            if target_model_id.trim().is_empty() {
                return Err(IcodeError::validation("路由目标模型 ID 不能为空"));
            }
        }

        repository::update_route(id, &input)
    }

    /// 删除路由
    pub fn delete_route(&self, id: &str) -> IcodeResult<()> {
        repository::delete_route(id)
    }

    // ===== 路由解析（故障转移） =====

    /// 解析虚拟模型到真实供应商路由
    ///
    /// 输入：
    /// - `provider_alias`：虚拟供应商 alias
    /// - `model_id`：虚拟模型 ID
    ///
    /// 返回：
    /// - `Ok(Some(ResolvedVirtualRoute))`：找到可用路由
    /// - `Ok(None)`：未找到虚拟供应商或虚拟模型
    /// - `Err(...)`：策略未实现或其他业务错误
    ///
    /// ## Fallback 策略
    ///
    /// 1. 按 `priority` 升序、创建时间升序查询启用的路由。
    /// 2. 返回第一条 `is_healthy = 1` 的路由。
    /// 3. 若所有路由都不健康，返回优先级最高的一条（让上游请求触发健康检查更新）。
    #[expect(dead_code)]
    pub fn resolve_virtual_route(
        &self,
        provider_alias: &str,
        model_id: &str,
    ) -> IcodeResult<Option<ResolvedVirtualRoute>> {
        let provider = match repository::find_provider_by_alias(provider_alias)? {
            Some(p) => p,
            None => return Ok(None),
        };

        if !provider.is_enabled {
            return Err(IcodeError::validation(format!(
                "虚拟供应商 '{}' 已禁用",
                provider_alias
            )));
        }

        let strategy = VirtualProviderStrategy::from_str(&provider.strategy)
            .unwrap_or(VirtualProviderStrategy::Fallback);

        match strategy {
            VirtualProviderStrategy::Fallback => {
                self.resolve_fallback_route(&provider.id, model_id)
            }
            other => Err(IcodeError::not_implemented(format!(
                "虚拟供应商策略 '{}' 尚未实现",
                other.as_str()
            ))),
        }
    }

    /// 按 fallback 策略解析路由
    fn resolve_fallback_route(
        &self,
        virtual_provider_id: &str,
        model_id: &str,
    ) -> IcodeResult<Option<ResolvedVirtualRoute>> {
        let routes = self.resolve_fallback_routes(virtual_provider_id, model_id)?;
        if routes.is_empty() {
            return Ok(None);
        }

        let selected = &routes[0];
        Ok(Some(ResolvedVirtualRoute {
            target_provider_id: selected.target_provider_id.clone(),
            target_model_id: selected.target_model_id.clone(),
            route_index: 0,
        }))
    }

    /// 按 fallback 策略解析全部候选路由
    ///
    /// 返回按 `priority` 升序排列的启用路由列表。健康路由排在前面，不健康路由排在后面，
    /// 供网关层按顺序重试并在失败后降级。
    pub fn resolve_fallback_routes(
        &self,
        virtual_provider_id: &str,
        model_id: &str,
    ) -> IcodeResult<Vec<VirtualModelRoute>> {
        let virtual_model = match repository::find_model_by_provider_and_model_id(
            virtual_provider_id,
            model_id,
        )? {
            Some(m) => m,
            None => return Ok(Vec::new()),
        };

        if !virtual_model.is_enabled {
            return Err(IcodeError::validation(format!(
                "虚拟模型 '{}' 已禁用",
                model_id
            )));
        }

        let mut routes = repository::list_routes_by_virtual_model(&virtual_model.id)?;
        if routes.is_empty() {
            return Err(IcodeError::not_found(
                "VirtualModelRoute",
                Some(&virtual_model.id),
            ));
        }

        // 健康路由在前， unhealthy 在后；同一组内按 priority 升序、创建时间升序
        routes.sort_by(|a, b| {
            let health_cmp = b.is_healthy.cmp(&a.is_healthy);
            if health_cmp != std::cmp::Ordering::Equal {
                return health_cmp;
            }
            a.priority.cmp(&b.priority).then_with(|| a.created_at.cmp(&b.created_at))
        });

        Ok(routes)
    }

    /// 将指定路由标记为不健康（网关层重试耗尽后降级）
    pub fn degrade_route_health(&self, route_id: &str) -> IcodeResult<()> {
        repository::mark_route_unhealthy(route_id)
    }

    /// 获取虚拟供应商级重试配置
    ///
    /// 返回 `(max_retries, retry_interval_ms)`，用于网关层按虚拟供应商策略重试。
    #[expect(dead_code)]
    pub fn get_provider_retry_config(
        &self,
        virtual_provider_id: &str,
    ) -> IcodeResult<(u32, u64)> {
        let provider = repository::find_provider_by_id(virtual_provider_id)?;
        let max_retries = provider.max_retries.max(0) as u32;
        let retry_interval_ms = provider.retry_interval_ms.max(0) as u64;
        Ok((max_retries, retry_interval_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_clone() {
        let handle = VirtualProviderHandle::new();
        let _cloned = handle.clone();
    }
}
