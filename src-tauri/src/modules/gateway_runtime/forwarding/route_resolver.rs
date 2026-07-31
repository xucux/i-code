//! # 路由解析
//!
//! 解析客户端请求中的 `model` 字段，返回真实供应商或虚拟供应商路由。

use crate::error::{IcodeError, IcodeResult};
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

        let routes = shared
            .virtual_provider_handle
            .service()
            .resolve_fallback_routes(&vp.id, &upstream_model_id)?;

        if routes.is_empty() {
            return Err(IcodeError::not_found(
                "VirtualModelRoute",
                Some(&format!("{}/{}", provider_slug, upstream_model_id)),
            ));
        }

        Ok(ResolvedRoute::Virtual {
            routes,
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

    let auth_config = shared
        .ai_gateway_handle
        .service()
        .resolve_auth_for_request(&provider)?;

    let extra_headers = shared
        .ai_gateway_handle
        .service()
        .resolve_extra_headers_for_request(&provider.id)?;

    Ok(ForwardContext::virtual_route(
        provider,
        route.target_model_id.clone(),
        request_id.to_string(),
        auth_config,
        extra_headers,
        route_index,
        route.id.clone(),
        gateway_protocol,
        gateway_model_id,
    ))
}
