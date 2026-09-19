//! # OpenCode Zen 免费通道客户端（`opencode-free`）
//!
//! 处理 OpenCode Zen 免费模型的转发。底层为 OpenAI Chat Completions 兼容协议，
//! 但上游自 2025-09 起强制校验「真实 OpenCode CLI 请求」的三个特征，任一不满足
//! 直接返回 `HTTP 403 FreeTierError: OpenCode's free tier can only be used
//! from within OpenCode`：
//!
//! 1. **请求头**：`User-Agent` 必须为 `opencode/x.y.z ...`，且携带 CLI 本地计算的
//!    `x-opencode-session`（`ses_`）与 `x-opencode-request`（`msg_`）ID——
//!    乱填或 UUID 形态会被服务端校验时间戳新鲜度后拒绝；
//! 2. **请求体**：必须 `"stream": true`（服务端仅接受流式响应）；
//! 3. **请求体**：`tools` 数组必须存在、数量 ≥ 2 且包含 `bash`；每个工具须为
//!    OpenAI 标准嵌套结构（`tool.function` 必须为对象，schema 内容不校验）。
//!
//! ## 与 OpenAiChatClient 的关系
//!
//! 请求头构造、认证解析、URL 拼接完全复用 [`OpenAiChatClient`]，本 Client 只做：
//! - 校验请求体必须 `stream: true`——非流式入口（网关侧 `stream: false`）
//!   **直接拦截**并返回 `StreamRequired` 错误，由网关转换为 OpenAI 标准错误体
//!   （HTTP 400 `invalid_request_error / bad_request`）提醒客户端改用流式，
//!   不再静默改写请求体或聚合 SSE 还原；
//! - 转发前对请求体执行 [`prepare_body`] 工具合规化（注入 `bash`，数量 ≥ 2）。
//!
//! 历史版本把该通道伪装为 `openai-chat-completion` + 模板变量头，因上游升级
//! 校验而失效，独立 `provider_type` 便于与普通 OpenAI 兼容供应商区分。

use serde_json::{json, Value};

use super::openai_chat_client::OpenAiChatClient;
use super::{
    build_upstream_url, format_client_error, http_client_for, ClientError, UpstreamClient,
    UpstreamContext, UpstreamProtocol, UpstreamRequest, UpstreamResponse,
};

/// OpenCode Zen 免费通道客户端
#[derive(Debug, Clone, Default)]
pub struct OpenCodeFreeClient;

impl OpenCodeFreeClient {
    /// 创建新的 OpenCode Free Client
    pub fn new() -> Self {
        Self
    }

    /// 上游请求路径（与 OpenAI Chat 一致：`{base_url}/chat/completions`）
    fn build_path() -> &'static str {
        "/chat/completions"
    }

    /// 请求体合规化
    ///
    /// `opencode-free` 上游强制校验请求体两条规则，任一不满足直接 403：
    /// 1. 必须 `"stream": true`（仅接受流式响应）——非流式入口**直接拦截**，
    ///    返回 `StreamRequired` 错误，由网关转换为 OpenAI 标准错误体
    ///    （HTTP 400 `invalid_request_error / bad_request`）提醒客户端改用流式；
    /// 2. `tools` 必须存在且数量 ≥ 2、包含 `bash`——缺失时自动注入，已有则规范化
    ///    补全（详见下方逻辑）。
    ///
    /// ## tools 处理规则（上游要求 OpenAI 标准嵌套结构）
    ///
    /// 上游对 `tools` 逐项校验 `tools[i].function` 必须为对象，故注入的最小化工具
    /// 使用标准格式（schema 内容不校验）：
    /// `{"type":"function","function":{"name":"bash","description":"x",
    ///   "parameters":{"type":"object","properties":{},"required":[]},"strict":false}}`
    ///
    /// - tools 缺失或空数组：注入 `[bash, read]` 两个最小化工具；
    /// - tools 已有：先把旧版扁平格式工具（顶层 `name` / `description` /
    ///   `parameters` / `strict`，无 `function` 字段）规范化为上述嵌套结构，
    ///   再检查是否含 `bash`（兼容扁平 `name` 与嵌套 `function.name`），
    ///   缺失则在头部插入最小化 `bash`；数量不足 2 时追加最小化 `read`。
    ///
    /// # 返回
    ///
    /// * `Ok(body)` - 合规化后的请求体（`stream` 恒为 `true`）
    /// * `Err(StreamRequired)` - 非流式入口被拦截（不发起上游请求）
    pub(crate) fn prepare_body(mut body: Value) -> Result<Value, ClientError> {
        if !body.is_object() {
            return Err(ClientError::Other(
                "opencode-free 请求体必须为 JSON 对象".to_string(),
            ));
        }

        // 强制流式：非流式入口直接拦截提醒（不再静默改写为流式并聚合还原）
        if body.get("stream").and_then(Value::as_bool) != Some(true) {
            return Err(ClientError::StreamRequired(
                "OpenCode Free 通道仅支持流式请求（stream 必须为 true），请携带 \"stream\": true 后重试。"
                    .to_string(),
            ));
        }

        // 工具合规化：缺失注入、扁平格式规范化、补 bash、数量 ≥ 2
        let tools_empty_or_missing = body
            .get("tools")
            .and_then(Value::as_array)
            .map(|arr| arr.is_empty())
            .unwrap_or(true);
        if tools_empty_or_missing {
            body["tools"] = json!([Self::minimal_tool("bash"), Self::minimal_tool("read")]);
        } else if let Some(arr) = body.get_mut("tools").and_then(Value::as_array_mut) {
            // 1. 扁平格式工具（旧版 `{"type":"function","name":...}`）规范化
            for tool in arr.iter_mut() {
                if tool.get("function").is_some() {
                    continue;
                }
                let Some(flat_name) = tool.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let mut func = json!({ "name": flat_name });
                if let Some(d) = tool.get("description") {
                    func["description"] = d.clone();
                }
                if let Some(p) = tool.get("parameters") {
                    func["parameters"] = p.clone();
                }
                if let Some(s) = tool.get("strict") {
                    func["strict"] = s.clone();
                }
                *tool = json!({ "type": "function", "function": func });
            }
            // 2. 检查是否已含 bash（兼容扁平 name 与嵌套 function.name）
            let has_bash = arr.iter().any(|t| {
                t.get("name").and_then(Value::as_str) == Some("bash")
                    || t.pointer("/function/name").and_then(Value::as_str) == Some("bash")
            });
            if !has_bash {
                arr.insert(0, Self::minimal_tool("bash"));
            }
            // 3. 数量 ≥ 2（第二个可为任意工具名）
            if arr.len() < 2 {
                arr.push(Self::minimal_tool("read"));
            }
        }

        Ok(body)
    }

    /// 构造最小化工具定义（OpenAI 标准嵌套结构，满足上游 `tools[i].function`
    /// 必须为对象的校验；schema 内容不校验）
    fn minimal_tool(name: &str) -> Value {
        json!({
            "type": "function",
            "function": {
                "name": name,
                "description": "x",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                },
                "strict": false
            }
        })
    }
}

#[async_trait::async_trait]
impl UpstreamClient for OpenCodeFreeClient {
    async fn execute(
        &self,
        ctx: &mut UpstreamContext,
        request: UpstreamRequest,
    ) -> Result<UpstreamResponse, ClientError> {
        // 仅接受 ChatCompletions 上游协议（桥接层已保证 O→O / A→O 均落到该协议）
        if request.protocol != UpstreamProtocol::ChatCompletions {
            return Err(ClientError::UnsupportedProtocol {
                provider_type: ctx.provider.provider_type.clone(),
                protocol: request.protocol,
            });
        }

        let path = Self::build_path();
        let upstream_url = build_upstream_url(&ctx.provider, path)?;
        let auth_resolution = OpenAiChatClient::resolve_auth(ctx.auth_config.clone())?;

        // 请求头复用 OpenAI Chat 构造（Bearer + extra headers；extra 头最后注入可覆盖默认）
        let headers = OpenAiChatClient::build_headers(
            &ctx.provider,
            &auth_resolution,
            &ctx.extra_headers,
        )?;
        // 捕获上游请求头去敏快照，供 provider-api 日志展示真实发出的请求头
        ctx.request_headers_json =
            crate::modules::gateway_runtime::logging::headers::request_headers_to_json(&headers);

        // 请求体合规化：校验强制流式（非流式入口直接拦截提醒）+ tools 校验
        let body = Self::prepare_body(request.body)?;
        // 已通过流式校验，此处恒为 true
        ctx.is_stream = true;
        // 记录最终发给上游的请求体（已注入/规范化 tools），供转发日志展示真实请求体
        ctx.request_body_json = Some(body.to_string());

        let client = http_client_for(&ctx.provider, true)?;
        let upstream_resp = client
            .post(&upstream_url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ClientError::RequestFailed(format_client_error(&e)))?;

        // 已强制流式：直接透传 SSE（上游 4xx/5xx 由 response_handler 原样透传错误体）
        Ok(UpstreamResponse::Streaming {
            response: upstream_resp,
            protocol: UpstreamProtocol::ChatCompletions,
        })
    }

    fn provider_type(&self) -> &'static str {
        "opencode-free"
    }
}
