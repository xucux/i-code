//! # 路由解析
//!
//! 解析客户端请求中的 `model` 字段，返回真实供应商或虚拟供应商路由。

use crate::error::{IcodeError, IcodeResult};
use crate::modules::ai_gateway::types::is_media_generation_provider_type;
use crate::modules::gateway_runtime::service::GatewaySharedState;
use crate::modules::virtual_provider::types::VirtualModelRoute;

use super::context::{ForwardContext, GatewayProtocol};
use super::util::parse_model_id;

/// 路由类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedRouteKind {
    /// 真实供应商直连
    Direct,
    /// 虚拟供应商故障转移
    Virtual,
}

impl ResolvedRouteKind {
    /// 是否为虚拟路由
    pub fn is_virtual(self) -> bool {
        matches!(self, Self::Virtual)
    }
}

/// 解析后的路由结果
///
/// - `Direct`：直接命中真实供应商，构造单个 `ForwardContext`
/// - `Virtual`：命中虚拟供应商，返回全部候选路由列表，由
///   `VirtualForwarder` 按健康度排序逐条尝试
pub enum ResolvedRoute {
    /// 真实供应商直连
    Direct(ForwardContext),
    /// 虚拟供应商候选路由列表
    Virtual {
        /// 全部候选路由（按健康度排序）
        routes: Vec<VirtualModelRoute>,
        /// 虚拟供应商 alias（用于日志）
        alias: String,
        /// 客户端原始 model_id
        gateway_model_id: String,
        /// 上游真实 model_id（虚拟模型对应的 model_id）
        upstream_model_id: String,
        /// 请求 ID
        request_id: String,
        /// 网关协议
        gateway_protocol: GatewayProtocol,
    },
}

/// 解析模型 ID 并返回路由结果
///
/// 流程：
/// 1. 解析 `model` 字段为 `(provider_slug, upstream_model_id)`
/// 2. 优先匹配真实供应商 slug
/// 3. 未找到时尝试虚拟供应商 alias，返回全部候选路由
pub fn resolve_route(
    shared: &GatewaySharedState,
    model_id: &str,
    request_id: &str,
    gateway_protocol: GatewayProtocol,
) -> IcodeResult<ResolvedRoute> {
    let (provider_slug, upstream_model_id) = parse_model_id(model_id)?;

    if provider_slug.is_empty() {
        return Err(IcodeError::validation(
            "缺少 provider_slug / virtual_alias，无法路由请求。请使用 '{prefix}/{model_id}' 格式。",
        ));
    }

    // 优先：真实供应商
    let providers = shared.ai_gateway_handle.service().list_enabled_providers()?;
    if let Some(provider) = providers.into_iter().find(|p| p.slug == provider_slug) {
        // 隔离约束：视觉生成供应商不进入原网关转发逻辑，
        // 对外返回 model not found 语义的标准错误体（不路由、不桥接、不透传）
        if is_media_generation_provider_type(&provider.provider_type) {
            return Err(IcodeError::not_found("Model", Some(model_id)));
        }

        let gateway_models = shared
            .ai_gateway_handle
            .service()
            .list_gateway_models_by_provider(&provider.id)?;
        let gateway_model_id = gateway_models
            .into_iter()
            .find(|m| m.model_id == upstream_model_id && m.is_exposed)
            .map(|m| m.id);

        let auth_config = shared
            .ai_gateway_handle
            .service()
            .resolve_auth_for_request(&provider)?;

        let extra_headers = shared
            .ai_gateway_handle
            .service()
            .resolve_extra_headers_for_request(&provider.id)?;

        let ctx = ForwardContext::direct(
            provider,
            gateway_model_id,
            upstream_model_id,
            request_id.to_string(),
            auth_config,
            extra_headers,
            gateway_protocol,
            model_id.to_string(),
        );
        return Ok(ResolvedRoute::Direct(ctx));
    }

    // 次优：虚拟供应商
    let vp_list = shared.virtual_provider_handle.service().list_providers()?;
    let vp = vp_list.into_iter().find(|p| p.alias == provider_slug);

    if let Some(vp) = vp {
        if !vp.is_enabled {
            return Err(IcodeError::validation(format!(
                "虚拟供应商 '{}' 已禁用",
                provider_slug
            )));
        }

        let strategy = crate::modules::virtual_provider::types::VirtualProviderStrategy::from_str(&vp.strategy)
            .unwrap_or(crate::modules::virtual_provider::types::VirtualProviderStrategy::Fallback);

        let routes = shared
            .virtual_provider_handle
            .service()
            .resolve_routes_by_strategy(&vp.id, &upstream_model_id, &strategy)?;

        // 隔离约束：过滤掉目标供应商属于媒体生成协议族的路由
        // （虚拟模型不允许挂载视觉生成供应商，此处为运行时兜底过滤）
        let mut filtered_routes: Vec<VirtualModelRoute> = Vec::with_capacity(routes.len());
        for route in routes {
            let is_media = shared
                .ai_gateway_handle
                .service()
                .get_provider(&route.target_provider_id)
                .map(|p| is_media_generation_provider_type(&p.provider_type))
                .unwrap_or(false);
            if !is_media {
                filtered_routes.push(route);
            }
        }

        if filtered_routes.is_empty() {
            return Err(IcodeError::not_found(
                "VirtualModelRoute",
                Some(&format!("{}/{}", provider_slug, upstream_model_id)),
            ));
        }

        Ok(ResolvedRoute::Virtual {
            routes: filtered_routes,
            alias: provider_slug,
            gateway_model_id: model_id.to_string(),
            upstream_model_id,
            request_id: request_id.to_string(),
            gateway_protocol,
        })
    } else {
        Err(IcodeError::not_found("Provider", Some(&provider_slug)))
    }
}

/// 根据虚拟路由构造 `ForwardContext`
///
/// 由 `VirtualForwarder` 在尝试某条路由时调用。
///
/// 路由级字段处理：
/// - `extra_headers_json`：反序列化后追加到 `ctx.upstream.extra_headers`，
///   追加在供应商级头之后，由 client 层 `build_headers` 以"后写覆盖先写"语义应用。
/// - `extra_body_json`：解析后存入 `ctx.route_extra_body`，由 `prepare_body` 浅合并到请求体。
/// - `timeout_ms`：存入 `ctx.route_timeout_ms`，实际应用待后续阶段。
pub fn build_virtual_context(
    shared: &GatewaySharedState,
    route: &VirtualModelRoute,
    route_index: usize,
    request_id: &str,
    gateway_protocol: GatewayProtocol,
    gateway_model_id: String,
) -> IcodeResult<ForwardContext> {
    let provider = shared
        .ai_gateway_handle
        .service()
        .get_provider(&route.target_provider_id)?;

    if !provider.is_enabled {
        return Err(IcodeError::validation(format!(
            "虚拟供应商路由目标 '{}' 已禁用",
            provider.slug
        )));
    }

    // 隔离约束兜底：视觉生成供应商不参与虚拟供应商转发
    if is_media_generation_provider_type(&provider.provider_type) {
        return Err(IcodeError::validation(format!(
            "虚拟供应商路由目标 '{}' 为视觉生成供应商，不允许参与聊天转发",
            provider.slug
        )));
    }

    let auth_config = shared
        .ai_gateway_handle
        .service()
        .resolve_auth_for_request(&provider)?;

    let mut extra_headers = shared
        .ai_gateway_handle
        .service()
        .resolve_extra_headers_for_request(&provider.id)?;

    // 路由级 extra_headers 追加在供应商级头之后；client 层遍历时后写覆盖先写
    if let Some(json) = &route.extra_headers_json {
        if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(json) {
            for (k, v) in map {
                // 仅接受字符串值，跳过非字符串以避免运行时类型错误
                if let Some(s) = v.as_str() {
                    extra_headers.push((k, s.to_string()));
                }
            }
        }
    }

    // 路由级 extra_body 解析；prepare_body 阶段浅合并到请求体
    let route_extra_body: Option<serde_json::Value> = route
        .extra_body_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());

    let mut ctx = ForwardContext::virtual_route(
        provider,
        route.target_model_id.clone(),
        request_id.to_string(),
        auth_config,
        extra_headers,
        route_index,
        route.id.clone(),
        gateway_protocol,
        gateway_model_id,
    );

    ctx.route_extra_body = route_extra_body;
    ctx.route_timeout_ms = route.timeout_ms;

    Ok(ctx)
}
