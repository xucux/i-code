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
//!    - `fallback`：按 `priority` 升序返回健康优先的候选路由列表。
//!    - `load_balance`：加权随机选择 1 条健康路由（weight=0 不参与轮询）。
//!    - `on_all`：当前降级为 fallback 顺序尝试（并发实现留待后续迭代）。
//! 4. upstream.rs 拿到 `ResolvedVirtualRoute` 后，通过 `ai_gateway` 加载目标真实供应商，
//!    并将请求体中的 `model` 替换为 `target_model_id` 进行转发。

use std::sync::Arc;

use rand::Rng;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::ai_gateway::{types::ExposedModel, AiGatewayService};

use super::repository;
use super::types::{
    CreateVirtualModelInput, CreateVirtualModelRouteInput, CreateVirtualProviderInput,
    ExposedVirtualModel, GenerateVirtualProviderInput, GenerateVirtualProviderResult,
    ResolvedVirtualRoute, SaveVirtualModelInput, SaveVirtualModelRouteInput, SlotGenerationResult,
    UpdateVirtualModelInput, UpdateVirtualModelRouteInput, UpdateVirtualProviderInput,
    VirtualModel, VirtualModelRoute, VirtualProvider, VirtualProviderStrategy, VirtualSlotsConfig,
    VirtualSlotsConfigDto, VirtualSlotsConfigSetInput, VirtualSlotsSlot,
};

/// 虚拟供应商级默认重试次数
#[expect(dead_code)]
const DEFAULT_PROVIDER_MAX_RETRIES: i64 = 3;
/// 虚拟供应商级默认重试间隔（毫秒）
#[expect(dead_code)]
const DEFAULT_PROVIDER_RETRY_INTERVAL_MS: i64 = 1000;

/// 内置默认数据源 URL（用户 GitHub 仓库的 raw JSON 地址）
const DEFAULT_DATA_SOURCE_URL: &str =
    "https://raw.githubusercontent.com/xucux/i-code/main/src-tauri/data/virtual-slots.json";

/// 内置兜底数据源 JSON（随二进制编译嵌入，离线 / 远程失败时使用）
const BUILTIN_VIRTUAL_SLOTS_JSON: &str = include_str!("../../../data/virtual-slots.json");

/// 数据源 URL 在 `global_configs` 中的分组与键（免迁移的零散外部配置）
const SLOTS_CONFIG_GROUP: &str = "virtual_provider";
const SLOTS_CONFIG_KEY: &str = "preset_data_source_url";

/// 当前支持的数据源 schema 版本
const SLOTS_SCHEMA_VERSION: i64 = 1;

/// 路由选择器 trait
///
/// 按虚拟供应商的策略从候选路由中选择尝试顺序。
/// `select` 返回的 Vec 长度决定 VirtualForwarder 尝试多少次：
/// - Fallback：返回全部候选路由（健康在前，priority 升序）
/// - LoadBalance：返回 1 条加权随机选择的路由
/// - OnAll：当前降级为 fallback 顺序，未来可扩展为返回全部交由 forwarder 并发执行
pub trait RouteSelector: Send + Sync {
    /// 选择路由尝试顺序
    fn select<'a>(&self, routes: &'a [VirtualModelRoute]) -> Vec<&'a VirtualModelRoute>;
}

/// Fallback 选择器
///
/// 返回按健康度（健康在前）+ priority 升序排列的候选路由列表。
/// VirtualForwarder 按顺序逐条尝试，失败降级后继续下一条。
pub struct FallbackSelector;

impl RouteSelector for FallbackSelector {
    fn select<'a>(&self, routes: &'a [VirtualModelRoute]) -> Vec<&'a VirtualModelRoute> {
        let mut sorted: Vec<&VirtualModelRoute> = routes.iter().collect();
        sorted.sort_by(|a, b| {
            // 健康路由在前
            let health_cmp = b.is_healthy.cmp(&a.is_healthy);
            if health_cmp != std::cmp::Ordering::Equal {
                return health_cmp;
            }
            // 同健康组内按 priority 升序，再按创建时间升序
            a.priority.cmp(&b.priority).then_with(|| a.created_at.cmp(&b.created_at))
        });
        sorted
    }
}

/// LoadBalance 选择器
///
/// 加权随机选择 1 条启用且健康的路由；weight=0 不参与轮询。
/// 若所有路由 weight=0 或全部不健康，回退为 fallback 选择第一条。
pub struct LoadBalanceSelector;

impl RouteSelector for LoadBalanceSelector {
    fn select<'a>(&self, routes: &'a [VirtualModelRoute]) -> Vec<&'a VirtualModelRoute> {
        // 仅参与启用且健康、weight>0 的路由
        let candidates: Vec<&VirtualModelRoute> = routes
            .iter()
            .filter(|r| r.enabled && r.is_healthy && r.weight > 0)
            .collect();

        if candidates.is_empty() {
            // 全部候选路由不可用，回退到 fallback 顺序（让 forwarder 尝试 + 失败降级更新健康度）
            return FallbackSelector.select(routes);
        }

        // 加权随机
        let weights: Vec<u32> = candidates.iter().map(|r| r.weight as u32).collect();
        let total: u32 = weights.iter().sum();
        if total == 0 {
            return FallbackSelector.select(routes);
        }

        let mut rng = rand::thread_rng();
        let mut picked: Option<usize> = None;
        let mut remaining = rng.gen_range(0..total);
        for (i, w) in weights.iter().enumerate() {
            if remaining < *w {
                picked = Some(i);
                break;
            }
            remaining -= *w;
        }
        // 安全兜底：理论上不会触发
        let idx = picked.unwrap_or(0);
        vec![candidates[idx]]
    }
}

/// OnAll 选择器
///
/// 当前降级为 fallback 顺序尝试（与 Fallback 一致）。
/// 真正的并发实现需要 VirtualForwarder 改造为并发路径，留待后续迭代。
pub struct OnAllSelector;

impl RouteSelector for OnAllSelector {
    fn select<'a>(&self, routes: &'a [VirtualModelRoute]) -> Vec<&'a VirtualModelRoute> {
        FallbackSelector.select(routes)
    }
}

impl VirtualProviderService {
    /// 根据策略返回对应的选择器
    pub fn selector_for(strategy: &VirtualProviderStrategy) -> Box<dyn RouteSelector> {
        match strategy {
            VirtualProviderStrategy::Fallback => Box::new(FallbackSelector),
            VirtualProviderStrategy::LoadBalance => Box::new(LoadBalanceSelector),
            VirtualProviderStrategy::OnAll => Box::new(OnAllSelector),
        }
    }
}

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
            // timeout_ms 不允许负值；None 表示继承
            if let Some(t) = route.timeout_ms {
                if t < 0 {
                    return Err(IcodeError::validation(format!(
                        "路由 timeout_ms 不能为负值：{}",
                        t
                    )));
                }
            }
            // extra_headers / extra_body 必须是 JSON 对象（非数组、非原始值）
            if let Some(v) = &route.extra_headers {
                if !v.is_object() {
                    return Err(IcodeError::validation(
                        "路由 extra_headers 必须是 JSON 对象",
                    ));
                }
            }
            if let Some(v) = &route.extra_body {
                if !v.is_object() {
                    return Err(IcodeError::validation(
                        "路由 extra_body 必须是 JSON 对象",
                    ));
                }
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

    /// 按指定策略解析候选路由
    ///
    /// 在 `resolve_fallback_routes` 取得全部启用路由后，调用 `RouteSelector` 进行策略选择：
    /// - `fallback`：返回全部，按健康优先 + priority 升序
    /// - `load_balance`：返回 1 条加权随机选择的路由
    /// - `on_all`：当前降级为 fallback 顺序（并发实现留待后续迭代）
    pub fn resolve_routes_by_strategy(
        &self,
        virtual_provider_id: &str,
        model_id: &str,
        strategy: &VirtualProviderStrategy,
    ) -> IcodeResult<Vec<VirtualModelRoute>> {
        let routes = self.resolve_fallback_routes(virtual_provider_id, model_id)?;
        let selector = Self::selector_for(strategy);
        let selected = selector.select(&routes);
        // 把 &VirtualModelRoute 转换为 owned，保留 selector 选择顺序
        Ok(selected.into_iter().cloned().collect())
    }

    /// 将指定路由标记为不健康（网关层重试耗尽后降级）
    pub fn degrade_route_health(&self, route_id: &str) -> IcodeResult<()> {
        repository::mark_route_unhealthy(route_id)
    }

    /// 探活成功：置健康，重置连续失败计数，更新 last_healthy_at / last_check_at / last_check_duration_ms
    ///
    /// 由调度器在探活请求成功后调用。
    pub fn mark_route_healthy(&self, route_id: &str, check_duration_ms: u64) -> IcodeResult<()> {
        repository::mark_route_healthy(route_id, check_duration_ms)
    }

    /// 探活失败：递增 consecutive_failures，记录 last_error_text / last_check_duration_ms
    ///
    /// `degrade=true` 时同时置 is_healthy=0；调度器根据当前 consecutive_failures 是否达到
    /// 恢复阈值（默认 3）来决定是否传入 degrade=true。
    pub fn mark_route_check_failed(
        &self,
        route_id: &str,
        error_text: &str,
        check_duration_ms: u64,
        degrade: bool,
    ) -> IcodeResult<()> {
        repository::mark_route_check_failed(route_id, error_text, check_duration_ms, degrade)
    }

    /// 列出待探活路由：is_healthy=0 OR consecutive_failures>0
    ///
    /// 调度器周期性调用，对每条候选路由发起轻量探活请求。
    pub fn list_routes_for_health_check(&self) -> IcodeResult<Vec<VirtualModelRoute>> {
        repository::list_routes_for_health_check()
    }

    // ===== 路由尝试历史 =====

    /// 异步写入一条路由尝试历史
    ///
    /// 由 `VirtualForwarder` 在每条路由尝试结束后调用。
    /// 写入失败仅记录日志，不影响主流程。
    pub fn record_route_attempt(
        &self,
        virtual_route_id: &str,
        virtual_provider_id: &str,
        request_id: &str,
        attempt_index: usize,
        success: bool,
        status_code: Option<u16>,
        error_message: Option<&str>,
        duration_ms: u64,
    ) {
        if let Err(e) = repository::insert_route_attempt(
            virtual_route_id,
            virtual_provider_id,
            request_id,
            attempt_index,
            success,
            status_code,
            error_message,
            duration_ms,
        ) {
            tracing::warn!(
                "写入路由尝试历史失败: route_id={}, error={}",
                virtual_route_id,
                e.message
            );
        }
    }

    /// 查询指定路由的最近 N 次尝试
    pub fn list_recent_attempts_by_route(
        &self,
        route_id: &str,
        limit: u32,
    ) -> IcodeResult<Vec<super::types::VirtualRouteAttempt>> {
        repository::list_recent_attempts_by_route(route_id, limit)
    }

    /// 查询指定供应商下所有路由的尝试统计
    pub fn list_route_attempt_stats_by_provider(
        &self,
        virtual_provider_id: &str,
    ) -> IcodeResult<Vec<super::types::RouteAttemptStats>> {
        repository::list_route_attempt_stats_by_provider(virtual_provider_id)
    }

    /// 测试单条路由：对目标供应商发起轻量探活请求（GET /v1/models，5s 超时）
    ///
    /// 与调度器的健康检查逻辑一致，但不写入 `virtual_route_attempts`（避免污染统计）。
    /// 由前端「测试」按钮调用，结果用 toast 展示。
    ///
    /// 需要 `AiGatewayService` 来解析目标供应商的认证配置与附加请求头。
    pub async fn test_route(
        &self,
        route_id: &str,
        ai_gateway: &crate::modules::ai_gateway::AiGatewayService,
    ) -> IcodeResult<super::types::RouteTestResult> {
        use std::time::Instant;

        // 加载路由
        let route = repository::find_route_by_id(route_id)?;

        // 加载目标真实供应商
        let provider = ai_gateway.get_provider(&route.target_provider_id)?;
        if !provider.is_enabled {
            return Ok(super::types::RouteTestResult {
                success: false,
                status_code: None,
                duration_ms: 0,
                error_message: Some(format!("目标供应商 '{}' 已禁用", provider.display_name)),
            });
        }

        // 解析认证配置
        let auth_config = ai_gateway
            .resolve_auth_for_request(&provider)
            .ok()
            .flatten();
        let extra_headers = ai_gateway
            .resolve_extra_headers_for_request(&provider.id)
            .unwrap_or_default();

        // 构造探活客户端（5s 超时）
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| {
                crate::error::IcodeError::internal(format!("构造探活 HTTP 客户端失败: {e}"))
            })?;

        let url = format!("{}/v1/models", provider.base_url.trim_end_matches('/'));
        let mut req = client.get(&url);

        // 注入认证头
        if let Some(auth) = &auth_config {
            match crate::modules::gateway_runtime::auth_resolver::resolve_auth(auth) {
                Ok(resolution) => {
                    if let Some(cred) = &resolution.credential {
                        match cred {
                            crate::modules::gateway_runtime::auth_resolver::AuthCredential::Bearer(t) => {
                                req = req.header("Authorization", format!("Bearer {t}"));
                            }
                            crate::modules::gateway_runtime::auth_resolver::AuthCredential::ApiKey(k) => {
                                req = req.header("Authorization", format!("Bearer {k}"));
                            }
                        }
                    }
                    for (k, v) in &resolution.extra_headers {
                        req = req.header(k, v);
                    }
                }
                Err(e) => {
                    return Ok(super::types::RouteTestResult {
                        success: false,
                        status_code: None,
                        duration_ms: 0,
                        error_message: Some(format!("认证解析失败: {}", e.message)),
                    });
                }
            }
        }
        // 注入供应商级 extra_headers（覆盖同名 auth 头）
        for (k, v) in &extra_headers {
            req = req.header(k, v);
        }

        let start = Instant::now();
        match req.send().await {
            Ok(resp) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let status = resp.status().as_u16();
                let success = resp.status().is_success();
                let error_message = if success {
                    None
                } else {
                    Some(format!("HTTP {}", status))
                };
                Ok(super::types::RouteTestResult {
                    success,
                    status_code: Some(status),
                    duration_ms,
                    error_message,
                })
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let msg = if e.is_timeout() {
                    format!("请求超时（5s）")
                } else if e.is_connect() {
                    format!("连接失败: {}", provider.slug)
                } else {
                    format!("网络错误: {e}")
                };
                Ok(super::types::RouteTestResult {
                    success: false,
                    status_code: None,
                    duration_ms,
                    error_message: Some(msg),
                })
            }
        }
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

    /// 检查 alias 变更的影响范围
    ///
    /// 当用户修改虚拟供应商 alias 时，所有使用 `{old_alias}/` 前缀的
    /// CLI 模型映射（`cli_model_mappings.gateway_model_id`）将失效。
    /// 此方法返回受影响的记录数，供前端展示警告。
    pub fn check_alias_impact(
        &self,
        virtual_provider_id: &str,
        new_alias: &str,
    ) -> IcodeResult<super::types::AliasImpactResult> {
        let provider = repository::find_provider_by_id(virtual_provider_id)?;
        let old_alias = provider.alias.clone();

        // alias 未变更时无影响
        if old_alias == new_alias {
            return Ok(super::types::AliasImpactResult {
                old_alias,
                new_alias: new_alias.to_string(),
                affected_cli_model_mappings: 0,
                has_impact: false,
            });
        }

        // 统计以旧 alias 为前缀的 CLI 模型映射
        let affected = repository::count_cli_model_mappings_by_alias_prefix(&old_alias)?;

        Ok(super::types::AliasImpactResult {
            old_alias,
            new_alias: new_alias.to_string(),
            affected_cli_model_mappings: affected,
            has_impact: affected > 0,
        })
    }

    // ===== 一键生成（三模型槽位）=====

    /// 一键生成虚拟供应商 + 三个虚拟模型（Opus / Sonnet / Haiku）
    ///
    /// 流程：
    /// 1. 确定数据源 URL：`input.dataSourceUrl` → `global_configs` 中已配置 URL → 内置默认；
    /// 2. 拉取数据源 JSON（远程优先，失败回退内置 JSON），校验 schemaVersion；
    /// 3. 校验 alias 唯一性（已存在则返回 CONFLICT）；
    /// 4. 通过 ai_gateway 获取「已开启显示的模型列表」，按槽位匹配规则命中实体模型；
    /// 5. 创建虚拟供应商，并对每个槽位保存虚拟模型 + 子级路由。
    pub async fn generate_preset(
        &self,
        ai_gateway: &AiGatewayService,
        input: GenerateVirtualProviderInput,
    ) -> IcodeResult<GenerateVirtualProviderResult> {
        // 1. 确定数据源 URL（显式传入 > 已配置 > 内置默认）
        let configured = self.get_configured_data_source_url()?;
        let url = input
            .data_source_url
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| configured.filter(|s| !s.trim().is_empty()))
            .unwrap_or_else(|| DEFAULT_DATA_SOURCE_URL.to_string());
        let url = normalize_data_source_url(&url);

        // 2. 获取数据源 JSON
        let config = if url.starts_with("http://") || url.starts_with("https://") {
            match self.fetch_remote_config(&url).await {
                Ok(cfg) => cfg,
                Err(e) => {
                    log::warn!("远程数据源拉取失败，回退内置 JSON：{e}");
                    parse_builtin_config()?
                }
            }
        } else if url.starts_with("file://") {
            // 本地文件数据源（供离线测试 / 自定义本地 JSON）
            let path = url.trim_start_matches("file://");
            let content = std::fs::read_to_string(path)
                .map_err(|e| IcodeError::validation(format!("读取本地数据源失败：{e}")))?;
            parse_slots_json(&content)?
        } else {
            return Err(IcodeError::validation(
                "数据源 URL 仅支持 http(s):// 或 file://",
            ));
        };

        // 校验 schemaVersion
        if config.schema_version != SLOTS_SCHEMA_VERSION {
            return Err(IcodeError::validation(format!(
                "数据源 schemaVersion={} 不兼容（期望 {}），请升级应用或更换数据源",
                config.schema_version, SLOTS_SCHEMA_VERSION
            )));
        }

        // 3. alias 唯一性校验
        if repository::find_provider_by_alias(&config.provider.alias)?.is_some() {
            return Err(IcodeError::conflict(format!(
                "虚拟供应商 alias '{}' 已存在，请删除后重试或更换数据源",
                config.provider.alias
            )));
        }

        // 4. 获取已开启显示的模型列表（is_exposed=1 且供应商启用）
        let exposed = ai_gateway.list_exposed_models()?;

        // 5. 创建虚拟供应商（strategy / 重试参数支持 input 覆盖）
        let provider = repository::insert_provider(&CreateVirtualProviderInput {
            name: config.provider.name.clone(),
            alias: config.provider.alias.clone(),
            display_name: config.provider.display_name.clone(),
            is_enabled: config.provider.is_enabled,
            strategy: Some(
                input
                    .strategy
                    .clone()
                    .unwrap_or(config.provider.strategy.clone()),
            ),
            max_retries: input.max_retries.unwrap_or(config.provider.max_retries),
            retry_interval_ms: input
                .retry_interval_ms
                .unwrap_or(config.provider.retry_interval_ms),
        })?;

        // 6. 匹配每个槽位并保存虚拟模型 + 子级路由
        let mut slot_results = Vec::new();
        for slot in &config.slots {
            let matched = match_slot(slot, &exposed);
            let empty = matched.is_empty();
            let route_count = matched.len() as i64;

            // 构造子级路由输入（priority 取自匹配规则；重试参数槽位级覆盖供应商级）
            let route_inputs: Vec<SaveVirtualModelRouteInput> = matched
                .into_iter()
                .map(|(priority, model)| SaveVirtualModelRouteInput {
                    target_provider_id: model.provider_id.clone(),
                    target_model_id: model.model_id.clone(),
                    priority,
                    enabled: true,
                    is_healthy: true,
                    max_retries: slot
                        .route_defaults
                        .as_ref()
                        .and_then(|d| d.max_retries)
                        .unwrap_or(provider.max_retries),
                    retry_interval_ms: slot
                        .route_defaults
                        .as_ref()
                        .and_then(|d| d.retry_interval_ms)
                        .unwrap_or(provider.retry_interval_ms),
                    timeout_ms: None,
                    extra_headers: None,
                    extra_body: None,
                    weight: 1,
                })
                .collect();

            let model = self.save_model(SaveVirtualModelInput {
                id: None,
                virtual_provider_id: provider.id.clone(),
                model_id: slot.model_id.clone(),
                display_name: slot.display_name.clone(),
                is_enabled: true,
                routes: route_inputs,
            })?;

            slot_results.push(SlotGenerationResult {
                key: slot.key.clone(),
                model_id: model.model_id.clone(),
                display_name: model.display_name.clone(),
                route_count,
                empty,
            });
        }

        Ok(GenerateVirtualProviderResult {
            provider,
            slots: slot_results,
        })
    }

    /// 读取数据源配置 DTO
    pub fn get_slots_config(&self) -> IcodeResult<VirtualSlotsConfigDto> {
        let configured = self.get_configured_data_source_url()?.unwrap_or_default();
        let effective = if configured.trim().is_empty() {
            DEFAULT_DATA_SOURCE_URL.to_string()
        } else {
            configured.clone()
        };
        Ok(VirtualSlotsConfigDto {
            data_source_url: configured.clone(),
            default_url: DEFAULT_DATA_SOURCE_URL.to_string(),
            effective_url: effective,
            use_default: configured.trim().is_empty(),
        })
    }

    /// 保存数据源配置（写 `global_configs`，空字符串表示恢复默认）
    pub fn set_slots_config(
        &self,
        input: &VirtualSlotsConfigSetInput,
    ) -> IcodeResult<VirtualSlotsConfigDto> {
        let v = input.data_source_url.trim().to_string();
        if !v.is_empty() {
            let normalized = normalize_data_source_url(&v);
            let is_valid = normalized.starts_with("http://")
                || normalized.starts_with("https://")
                || normalized.starts_with("file://");
            if !is_valid {
                return Err(IcodeError::validation(
                    "数据源 URL 仅支持 http(s):// 或 file://",
                ));
            }
        }
        let conn = crate::db::get_db_pool()?.get()?;
        crate::db::global_config::set_global_config(
            &conn,
            SLOTS_CONFIG_GROUP,
            SLOTS_CONFIG_KEY,
            &v,
        )?;
        self.get_slots_config()
    }

    /// 读取已配置的数据源 URL（`global_configs` 中的用户自定义值）
    fn get_configured_data_source_url(&self) -> IcodeResult<Option<String>> {
        let conn = crate::db::get_db_pool()?.get()?;
        crate::db::global_config::get_global_config(&conn, SLOTS_CONFIG_GROUP, SLOTS_CONFIG_KEY)
    }

    /// 从远程 URL 拉取数据源 JSON（10s 超时）
    async fn fetch_remote_config(&self, url: &str) -> IcodeResult<VirtualSlotsConfig> {
        let url = url.to_string();
        tauri::async_runtime::spawn_blocking(move || {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| {
                    IcodeError::internal(format!("构造数据源 HTTP 客户端失败：{e}"))
                })?;
            let resp = client
                .get(&url)
                .send()
                .map_err(|e| IcodeError::validation(format!("拉取数据源失败：{e}")))?;
            if !resp.status().is_success() {
                return Err(IcodeError::validation(format!(
                    "数据源 HTTP 状态异常：{}",
                    resp.status()
                )));
            }
            let text = resp
                .text()
                .map_err(|e| IcodeError::validation(format!("读取数据源响应失败：{e}")))?;
            parse_slots_json(&text)
        })
        .await
        .map_err(|e| IcodeError::internal(format!("数据源拉取任务失败：{e}")))?
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

// ===== 辅助函数 =====

/// 将 blob 地址归一化为 raw 地址（仅 GitHub 仓库）；非 blob 地址原样返回
fn normalize_data_source_url(url: &str) -> String {
    let url = url.trim().to_string();
    // 检测 github.com blob 链接并转为 raw：/blob/main/ → /main/
    // 同时支持 /blob/<branch>/ 任意分支名
    if url.contains("github.com") && url.contains("/blob/") {
        url.replace("/blob/", "/")
    } else {
        url
    }
}

/// 解析 JSON 字符串为 VirtualSlotsConfig
fn parse_slots_json(text: &str) -> IcodeResult<VirtualSlotsConfig> {
    let config: VirtualSlotsConfig = serde_json::from_str(text)
        .map_err(|e| IcodeError::validation(format!("数据源 JSON 解析失败：{e}")))?;
    if config.slots.is_empty() {
        return Err(IcodeError::validation("数据源 JSON 中 slots 不能为空"));
    }
    Ok(config)
}

/// 解析内置兜底数据源 JSON
fn parse_builtin_config() -> IcodeResult<VirtualSlotsConfig> {
    parse_slots_json(BUILTIN_VIRTUAL_SLOTS_JSON)
}

/// 按匹配规则命中实体模型
///
/// 返回 `Vec<(priority, ExposedModel)>`，按 priority 升序排列。
/// 同一 `providerId + modelId` 只保留 priority 最小的匹配。
fn match_slot(slot: &VirtualSlotsSlot, exposed: &[ExposedModel]) -> Vec<(i64, ExposedModel)> {
    use std::collections::HashSet;

    let mut results: Vec<(i64, ExposedModel)> = Vec::new();
    // key=providerId/modelId, 用于去重
    let mut seen: HashSet<String> = HashSet::new();

    // 按 priority 升序遍历规则
    for rule in &slot.matches {
        for model in exposed {
            let dedup_key = format!("{}/{}", model.provider_id, model.model_id);
            if seen.contains(&dedup_key) {
                continue;
            }
            let matched = match rule.r#type.as_str() {
                "exact" => rule
                    .model_id
                    .as_ref()
                    .map_or(false, |mid| mid == &model.model_id),
                "prefix" => rule
                    .model_id
                    .as_ref()
                    .map_or(false, |mid| model.model_id.starts_with(mid)),
                "regex" => rule
                    .pattern
                    .as_ref()
                    .map_or(false, |pat| {
                        regex::Regex::new(pat)
                            .map(|re| re.is_match(&model.model_id))
                            .unwrap_or(false)
                    }),
                _ => false,
            };
            if matched {
                seen.insert(dedup_key);
                results.push((rule.priority, model.clone()));
            }
        }
    }

    // 按 priority 升序排列
    results.sort_by_key(|(p, _)| *p);
    results
}
