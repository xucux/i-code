//! # 转发上下文与请求封装
//!
//! 定义转发层贯穿请求生命周期的数据结构。

use serde_json::Value;

use crate::modules::ai_gateway::types::{AuthConfig, Provider};
use crate::modules::gateway_runtime::client::{UpstreamContext, UpstreamProtocol};

use super::route_resolver::ResolvedRouteKind;

/// 网关对外协议
///
/// 客户端调用网关的接口形式，决定使用哪个 `UpstreamProtocol` 与上游通信。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayProtocol {
    /// OpenAI 兼容 `/v1/chat/completions`
    ChatCompletions,
    /// Anthropic 兼容 `/v1/messages`
    AnthropicMessages,
    /// OpenAI Responses API `/v1/responses`
    Responses,
}

impl GatewayProtocol {
    /// 转换为上游协议
    pub fn to_upstream(self) -> UpstreamProtocol {
        match self {
            Self::ChatCompletions => UpstreamProtocol::ChatCompletions,
            Self::AnthropicMessages => UpstreamProtocol::AnthropicMessages,
            Self::Responses => UpstreamProtocol::Responses,
        }
    }

    /// 日志标签
    pub fn label(self) -> &'static str {
        match self {
            Self::ChatCompletions => "ChatCompletions",
            Self::AnthropicMessages => "Anthropic Messages",
            Self::Responses => "Responses",
        }
    }
}

/// 转发请求
///
/// 由 router handler 构造，包含原始 body、网关协议、Gateway API Key 引用。
#[derive(Debug)]
pub struct ForwardRequest {
    /// 网关对外协议
    pub protocol: GatewayProtocol,
    /// 客户端原始请求体
    pub body: Value,
    /// 调用方使用的 Gateway API Key Secret ID（用于调用记录）
    pub api_key_secret_id: Option<String>,
    /// 调用方请求头（已去敏 JSON），透传到转发日志展示
    pub request_headers_json: Option<String>,
}

/// 已解析的转发上下文
///
/// 由 route_resolver 解析 model_id 后构造，包含目标供应商、真实模型 ID、
/// 虚拟路由信息（如有）等。供 `Forwarder` 执行实际请求。
#[derive(Debug, Clone)]
pub struct ForwardContext {
    /// 已解析为真实供应商的上游上下文（用于 `UpstreamClient::execute`）
    pub upstream: UpstreamContext,
    /// 路由类型：真实直连 or 虚拟路由
    pub kind: ResolvedRouteKind,
    /// 虚拟路由 ID（仅虚拟路由有值，用于失败时降级健康度）
    pub virtual_route_id: Option<String>,
    /// 网关对外协议
    pub gateway_protocol: GatewayProtocol,
    /// 客户端原始 model 字段（`{prefix}/{model_id}`）
    pub gateway_model_id: String,
}

impl ForwardContext {
    /// 构造真实供应商上下文
    pub fn direct(
        provider: Provider,
        gateway_model_id: Option<String>,
        upstream_model_id: String,
        request_id: String,
        auth_config: Option<AuthConfig>,
        extra_headers: Vec<(String, String)>,
        gateway_protocol: GatewayProtocol,
        gateway_model_id_str: String,
    ) -> Self {
        let upstream = UpstreamContext {
            provider,
            gateway_model_id,
            upstream_model_id,
            is_virtual: false,
            route_index: 0,
            request_id,
            is_stream: false,
            auth_config,
            extra_headers,
            request_headers_json: None,
        };
        Self {
            upstream,
            kind: ResolvedRouteKind::Direct,
            virtual_route_id: None,
            gateway_protocol,
            gateway_model_id: gateway_model_id_str,
        }
    }

    /// 构造虚拟路由上下文
    pub fn virtual_route(
        provider: Provider,
        upstream_model_id: String,
        request_id: String,
        auth_config: Option<AuthConfig>,
        extra_headers: Vec<(String, String)>,
        route_index: usize,
        virtual_route_id: String,
        gateway_protocol: GatewayProtocol,
        gateway_model_id_str: String,
    ) -> Self {
        let upstream = UpstreamContext {
            provider,
            gateway_model_id: None,
            upstream_model_id,
            is_virtual: true,
            route_index,
            request_id,
            is_stream: false,
            auth_config,
            extra_headers,
            request_headers_json: None,
        };
        Self {
            upstream,
            kind: ResolvedRouteKind::Virtual,
            virtual_route_id: Some(virtual_route_id),
            gateway_protocol,
            gateway_model_id: gateway_model_id_str,
        }
    }

    /// 设置流式标志
    pub fn set_stream(&mut self, is_stream: bool) {
        self.upstream.is_stream = is_stream;
    }
}
