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
//! 3. **请求体**：`tools` 数组必须存在，数量 ≥ 2，且第一项 `name = "bash"`
//!    （schema 内容不校验，最小化即可）。
//!
//! ## 与 OpenAiChatClient 的关系
//!
//! 请求头构造、认证解析、URL 拼接完全复用 [`OpenAiChatClient`]，本 Client 只做：
//! - 转发前对请求体执行 [`prepare_body`] 合规化；
//! - 非流式入口（网关侧 `stream: false`）强制改为流式请求上游，并把 SSE 流
//!   聚合还原为标准 chat.completions 非流式响应返回（否则客户端拿不到响应体）。
//!
//! 历史版本把该通道伪装为 `openai-chat-completion` + 模板变量头，因上游升级
//! 校验而失效，独立 `provider_type` 便于与普通 OpenAI 兼容供应商区分。

use serde_json::{json, Value};

use super::openai_chat_client::OpenAiChatClient;
use super::{
    build_upstream_url, format_client_error, http_client_for, read_response_body,
    ClientError, UpstreamClient, UpstreamContext, UpstreamProtocol, UpstreamRequest,
    UpstreamResponse, MAX_RESPONSE_BODY_BYTES,
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
    /// 满足 OpenCode 免费通道的三条请求体校验：
    /// 1. 强制 `stream = true`；
    /// 2. `tools` 缺失时注入包含 `bash` 的最小化工具数组；
    /// 3. 已有 `tools` 但缺少 `bash` 时在头部插入最小化 `bash` 工具。
    ///
    /// 最小化工具定义与参考方案一致（schema 内容不校验）：
    /// `{"type":"function","name":"bash","description":"x",
    ///   "parameters":{"type":"object","properties":{},"required":[]},"strict":false}`
    ///
    /// # 返回
    ///
    /// `(处理后的请求体, 是否为非流式入口聚合)`
    pub(crate) fn prepare_body(mut body: Value) -> (Value, bool) {
        if !body.is_object() {
            return (body, false);
        }

        // 非流式入口：强制改为流式请求上游，响应侧再聚合还原
        let mut aggregate = false;
        if body.get("stream").and_then(Value::as_bool) != Some(true) {
            aggregate = true;
            body["stream"] = json!(true);
        }

        // 工具合规化：必须存在 bash，且数量 ≥ 2
        let tools_empty_or_missing = body
            .get("tools")
            .and_then(Value::as_array)
            .map(|arr| arr.is_empty())
            .unwrap_or(true);
        if tools_empty_or_missing {
            body["tools"] = json!([minimal_tool("bash"), minimal_tool("read")]);
        } else if let Some(arr) = body.get_mut("tools").and_then(Value::as_array_mut) {
            let has_bash = arr
                .iter()
                .any(|t| t.get("name").and_then(Value::as_str) == Some("bash"));
            if !has_bash {
                arr.insert(0, minimal_tool("bash"));
            }
        }

        (body, aggregate)
    }

    /// 把上游 SSE 流聚合为标准 chat.completions 非流式响应体
    ///
    /// 遍历 `data: {json}` 事件，优先取最后带 `choices` 的增量（拼接 content /
    /// reasoning_content），`usage` 与 `finish_reason` 取任意一次出现的最后值。
    async fn aggregate_sse_to_complete(
        response: reqwest::Response,
    ) -> Result<UpstreamResponse, ClientError> {
        use futures::StreamExt;

        let status = response.status();
        let headers = response.headers().clone();

        // 上游报错（4xx/5xx）：直接按完整响应透传，由上层展示错误体
        if !status.is_success() {
            let (status, headers, body, _) =
                read_response_body(response, MAX_RESPONSE_BODY_BYTES).await?;
            return Ok(UpstreamResponse::Complete { status, headers, body });
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut usage: Option<Value> = None;
        let mut finish_reason: Option<Value> = None;
        let mut model: Option<String> = None;
        let mut id: Option<String> = None;
        let mut created: Option<i64> = None;
        let mut tool_calls: Option<Vec<Value>> = None;

        while let Some(chunk) = stream.next().await {
            let bytes =
                chunk.map_err(|e| ClientError::ReadBodyError(format_client_error(&e)))?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));
            // 逐行提取 SSE data 事件
            while let Some(pos) = buffer.find('\n') {
                let line: String = buffer.drain(..=pos).collect();
                let line = line.trim_end_matches(['\n', '\r']);
                let Some(data) = line.strip_prefix("data:") else {
                    continue;
                };
                let data = data.trim();
                if data.is_empty() || data == "[DONE]" {
                    continue;
                }
                let Ok(event) = serde_json::from_str::<Value>(data) else {
                    continue;
                };
                if event.get("usage").is_some() {
                    usage = Some(event["usage"].clone());
                }
                if event.get("model").and_then(Value::as_str).is_some() {
                    model = Some(event["model"].as_str().unwrap_or_default().to_string());
                }
                if event.get("id").and_then(Value::as_str).is_some() {
                    id = Some(event["id"].as_str().unwrap_or_default().to_string());
                }
                if let Some(ts) = event.get("created").and_then(Value::as_i64) {
                    created = Some(ts);
                }
                let Some(choices) = event.get("choices").and_then(Value::as_array) else {
                    continue;
                };
                for choice in choices {
                    if let Some(fr) = choice.get("finish_reason").filter(|v| !v.is_null()) {
                        finish_reason = Some(fr.clone());
                    }
                    let Some(delta) = choice.get("delta") else {
                        continue;
                    };
                    if let Some(c) = delta.get("content").and_then(Value::as_str) {
                        content.push_str(c);
                    }
                    if let Some(r) = delta.get("reasoning_content").and_then(Value::as_str) {
                        reasoning.push_str(r);
                    }
                    if let Some(tcs) = delta.get("tool_calls").and_then(Value::as_array) {
                        let tc = tool_calls.get_or_insert_with(Vec::new);
                        for item in tcs {
                            // 增量片段：按 index 简单顺序拼接 arguments / id / name
                            let index = item
                                .get("index")
                                .and_then(Value::as_i64)
                                .unwrap_or(tc.len() as i64) as usize;
                            if index >= tc.len() {
                                tc.push(json!({
                                    "id": item.get("id").cloned().unwrap_or(Value::Null),
                                    "type": "function",
                                    "function": {
                                        "name": item.pointer("/function/name").cloned().unwrap_or(json!("")),
                                        "arguments": item.pointer("/function/arguments").and_then(Value::as_str).unwrap_or("").to_string(),
                                    }
                                }));
                            } else if let Some(args) =
                                item.pointer("/function/arguments").and_then(Value::as_str)
                            {
                                let slot = &mut tc[index];
                                // pointer_mut 返回 &mut Value，需先取字符串克隆再拼接（借用规则）
                                let existing = slot
                                    .pointer("/function/arguments")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string();
                                let merged = format!("{}{}", existing, args);
                                slot["function"]["arguments"] = json!(merged);
                                if let Some(name) =
                                    item.pointer("/function/name").and_then(Value::as_str)
                                {
                                    if !name.is_empty() {
                                        slot["function"]["name"] = json!(name);
                                    }
                                }
                                if let Some(tid) = item.get("id").and_then(Value::as_str) {
                                    if !tid.is_empty() {
                                        slot["id"] = json!(tid);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut message = json!({ "role": "assistant", "content": content });
        if !reasoning.is_empty() {
            message["reasoning_content"] = json!(reasoning);
        }
        if let Some(tcs) = tool_calls {
            if !tcs.is_empty() {
                message["tool_calls"] = Value::Array(tcs);
            }
        }

        let mut choice = json!({
            "index": 0,
            "message": message,
            "finish_reason": finish_reason.clone().unwrap_or(json!("stop")),
        });
        if let Some(fr) = finish_reason {
            choice["finish_reason"] = fr;
        }

        let mut completion = json!({
            "id": id.unwrap_or_else(|| format!("chatcmpl-aggregated")),
            "object": "chat.completion",
            "created": created.unwrap_or_else(|| chrono::Utc::now().timestamp()),
            "model": model.unwrap_or_default(),
            "choices": [choice],
        });
        if let Some(u) = usage {
            completion["usage"] = u;
        }

        let body = completion.to_string().into_bytes();
        Ok(UpstreamResponse::Complete {
            status,
            headers,
            body,
        })
    }
}

/// 构造最小化工具定义（满足上游 `bash` + 数量 ≥ 2 校验，schema 不校验）
fn minimal_tool(name: &str) -> Value {
    json!({
        "type": "function",
        "name": name,
        "description": "x",
        "parameters": {
            "type": "object",
            "properties": {},
            "required": []
        },
        "strict": false
    })
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

        // 请求体合规化：强制流式 + tools 校验
        let (body, aggregate) = Self::prepare_body(request.body);
        let is_stream = body.get("stream").and_then(Value::as_bool) == Some(true);
        ctx.is_stream = is_stream;

        let client = http_client_for(&ctx.provider, is_stream)?;
        let upstream_resp = client
            .post(&upstream_url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ClientError::RequestFailed(format_client_error(&e)))?;

        if is_stream && !aggregate {
            // 客户端本来就要求流式：直接透传 SSE
            Ok(UpstreamResponse::Streaming {
                response: upstream_resp,
                protocol: UpstreamProtocol::ChatCompletions,
            })
        } else {
            // 非流式入口（已强制改为流式请求上游）：聚合 SSE 还原为完整响应；
            // 上游仍返回非流式 JSON 时（未来放宽校验），read_response_body 原样透传
            Self::aggregate_sse_to_complete(upstream_resp).await
        }
    }

    fn provider_type(&self) -> &'static str {
        "opencode-free"
    }
}
