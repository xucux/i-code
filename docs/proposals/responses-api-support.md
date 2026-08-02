# 本地网关支持 OpenAI Responses API 方案

> 状态：**提案**（待评审）  
> 日期：2026-08-02  
> 关联模块：`gateway-runtime`、`ai-gateway`、`call-records`、`logger`  
> 参考项目：`参考项目/vscode-unify-chat-provider-7.12.3/src/client/openai/responses-client.ts`、`responses-websocket-transport.ts`、`websocket-session-manager.ts`  
> 外部参考：[从 Chat Completions 到 Responses，OpenAI Agent 接口设计的演变](https://article.juejin.cn/post/7540285101152763950)

---

## 1. 背景与目标

OpenAI 自 2025 年起主推 **Responses API**（`POST /v1/responses`），定位为 Chat Completions 的下一代接口，面向 Agent、多模态、检索增强场景。目前：

- 主流 SDK（openai-python / openai-node）已将 `client.responses.create` 作为首选入口；
- 大量 Agent 框架（OpenAI Agents SDK、MCP 集成、vLLM 0.10+ 等）已适配或默认使用 Responses 协议；
- 参考项目 `vscode-unify-chat-provider` 已完整实现 `OpenAIResponsesProvider`（SSE + WebSocket 双传输）。

本项目本地网关当前**对外只暴露** `/v1/chat/completions` 与 `/v1/messages`，不支持 `POST /v1/responses`；上游侧 `openai-responses` / `openai-codex` 供应商类型已定义但 Client 为**占位实现（调用即失败）**。

### 目标

1. **对外**：网关新增 `POST /v1/responses` 端点，支持流式（SSE 事件流）与非流式调用，错误体符合 OpenAI 标准格式。
2. **上游**：`openai-responses` 供应商类型可用（HTTP SSE 透传为主；WebSocket 传输作为二期）。
3. **可观测性**：Responses 请求接入现有转发日志（`logger`）与调用统计（`call_records`），usage 正确提取。
4. **兼容边界**：不与现有 Chat Completions / Anthropic Messages 路径冲突；内部聊天模块（`chat`）不受影响。

---

## 2. Responses API 与 Chat Completions 差异速览

| 维度 | Chat Completions | Responses API |
|------|------------------|---------------|
| 端点 | `POST /v1/chat/completions` | `POST /v1/responses` |
| 请求体 | `messages: [...]`（手动拼接历史） | `input: string \| item[]`；`previous_response_id` 承接上下文 |
| system | `messages[0].role=system` | `instructions` 字段 |
| 工具 | `functions` / `tools(type=function)` | `tools` 统一（function / `web_search_preview` / `file_search` / `code_interpreter` / MCP remote） |
| 多模态 | 部分（image_url） | 原生文本 + 图像，可扩展音频/视频 |
| 非流式响应 | `choices[].message` | `output[]`（message / function_call / reasoning / web_search_call …） |
| usage | `prompt_tokens / completion_tokens / total_tokens` | `input_tokens / output_tokens / total_tokens`（含 `input_tokens_details.cached_tokens`、`output_tokens_details.reasoning_tokens`） |
| 流式 | `chat.completion.chunk`（增量 token，`delta`） | 事件流（`response.output_text.delta`、`response.reasoning_summary_text.delta`、`response.output_item.done`、`response.completed` …） |
| 状态 | 无状态 | 有状态（`store` / `previous_response_id`）；也可无状态（`store: false`） |
| 传输 | HTTP（SSE） | HTTP（SSE）或 WebSocket（可选） |

**关键结论**：

- Responses 请求体仍含 `model` 字段、流式标志仍为 `stream: true`——模型路由、流式判定可复用现有管道。
- Responses 流式**不需要** `stream_options.include_usage`（usage 在 `response.completed` 事件中）。
- 有状态端点（`GET/PATCH/DELETE /v1/responses/{id}`）依赖 OpenAI 服务端状态，网关是**无状态透传代理**，不应本地实现状态管理；`store: false` 无状态调用是网关主要支持场景。

---

## 3. 现状调研

### 3.1 网关对外接口（`gateway_runtime/router.rs`）

| 端点 | 状态 |
|------|------|
| `GET /health` / `GET /readyz` | ✅ 已实现 |
| `GET /v1/models` | ✅ 已实现 |
| `POST /v1/chat/completions` | ✅ 已实现 |
| `POST /v1/messages` | ✅ 已实现 |
| `POST /v1/responses` | ❌ **未注册**（`docs/development.md` §5.8 有规划但未落地） |

- 认证中间件 `EXEMPT_PATHS` 仅 `/health`、`/readyz`，新增路由会自动纳入 API Key 认证，无需改动。
- 网关日志 `log_gateway_response` 按路径记录，自动兼容新端点。

### 3.2 上游客户端层（`gateway_runtime/client/`）

| 文件 | 现状 |
|------|------|
| `mod.rs` | `UpstreamProtocol` 仅 `ChatCompletions` / `AnthropicMessages`；`UpstreamResponse` 仅 `Streaming`（SSE）/ `Complete`，无 WebSocket 变体；`ClientFactory` 将 `openai-responses` / `openai-codex` / `websocket` 路由到 `WebSocketClient` |
| `openai_chat_client.rs` | 仅实现 `/chat/completions` 路径；`build_path(AnthropicMessages)` 兜底返回 `/chat/completions` |
| `anthropic_client.rs` | 独立实现 `/v1/messages` |
| `websocket_client.rs` | **占位**：`execute()` 直接返回 `UnsupportedProtocol`，不发送网络请求；`Cargo.toml` 无 WebSocket 依赖（无 `tokio-tungstenite`） |

### 3.3 类型与数据层（`ai-gateway`）

| 项 | 现状 |
|----|------|
| `ProviderType` | ✅ 已含 `OpenaiResponses`（`openai-responses`）、`OpenaiCodex` |
| `TransportType` | ✅ 已含 `Auto` / `Sse` / `Websocket`；`providers.transport` 列已在 DB schema |
| 内置数据 | ✅ `builtin-providers.json` 中 OpenAI 主供应商 `providerType = "openai-responses"`；`builtin-models.json` 中 gpt-4.1 等模型 `providerTypes` 含 `openai-responses` |
| 前端表单 | ✅ `provider-form.tsx` 已支持选择 `openai-responses` 类型 |
| 官方模型拉取 | ✅ `fetch_official_models` 已把 `openai-responses` 按 OpenAI 兼容处理（`GET {base_url}/models`） |

**结论**：类型/数据/UI 层基本就绪，**缺口集中在运行时转发链**。

### 3.4 转发层（`gateway_runtime/forwarding/`）

| 文件 | 现状与缺口 |
|------|-----------|
| `context.rs` | `GatewayProtocol` 无 `Responses` 变体；`to_upstream()` 需扩展 |
| `forwarder.rs` `prepare_body` | 替换 `model`、判定 `stream` 通用；`stream_options.include_usage` 仅在 `ChatCompletions` 注入 ✅ 对 Responses 恰好无需注入 |
| `util.rs` `build_log_url` | 按 `GatewayProtocol` 映射路径，需新增 `Responses => "/responses"` |
| `util.rs` `protocol_tags` | 对 `openai-responses` / `openai-codex` 一律打 `websocket` 标签——HTTP 透传场景应打 `sse`，需按实际传输方式修正 |
| `usage_extractor.rs` | 非流式：`prompt_tokens` 无 `input_tokens` fallback、cached 不读 `input_tokens_details.cached_tokens` → **Responses usage 提取不完整**；流式：无 `response.completed` 事件分支 → **流式 usage 完全缺失** |
| `response_handler.rs` | SSE 透传逻辑通用（字节流原样透传 + 行缓冲解析 usage）✅ 可复用 |
| `util.rs` `upstream_error_response` | 已按 OpenAI 标准 `{error:{message,type,param,code}}` 输出 ✅ 可复用 |

### 3.5 参考项目支持度（`vscode-unify-chat-provider-7.12.3`）

完整实现了 `OpenAIResponsesProvider`（`src/client/openai/responses-client.ts`），可借鉴：

- **双传输模式**：`transportMode: 'sse' | 'auto' | 'websocket'`，由 `provider.transport` 配置决定；WebSocket 通过 `responses-websocket-transport.ts`（session_key 握手、热会话复用）与 `websocket-session-manager.ts`（连接池、请求排队、abort）实现。
- **事件流处理**：`response.output_item.added` / `response.output_text.delta` / `response.reasoning_summary_text.delta` / `response.output_item.done`（function_call）/ `response.completed`（usage）/ `response.failed` / `response.incomplete` / `error` 完整 switch。
- **上下文管理**：`previous_response_id` 透传、50k token 上下文压缩（compaction）、`responses_multi_agent=v1` beta header。
- **认证**：Bearer token；`OpenAI-Beta` header 拼接。

参考项目是「客户端角色」（消费 Responses），i-code 网关是「服务端 + 上游客户端」双角色，事件流转换、usage 提取逻辑可直接借鉴；WebSocket 会话管理复杂度较高，建议二期。

### 3.6 内部 chat 模块

`src-tauri/src/modules/chat/` 仅走 `POST /v1/chat/completions`，不受本方案影响。

---

## 4. 差距分析汇总

| # | 差距 | 影响 | 工作量 |
|---|------|------|--------|
| 1 | 无 `POST /v1/responses` 路由 | 外部 Responses 客户端完全无法使用网关 | S |
| 2 | `GatewayProtocol` / `UpstreamProtocol` 无 Responses 变体 | 转发链无法表达协议 | S |
| 3 | `openai-responses` 上游 Client 为占位（WebSocket 占位） | 即使对外加了端点，上游 OpenAI 直连仍失败 | M |
| 4 | usage 提取不支持 Responses 字段/事件 | 调用记录与日志 token 统计缺失 | S |
| 5 | 日志标签 `websocket` 误打 | 观测误导 | XS |
| 6 | 前端网关 API 文档弹窗缺 `/v1/responses` | 文档不完整 | XS |
| 7 | 无 Responses ↔ ChatCompletions 协议转换 | 非 OpenAI 供应商无法通过 Responses 端点消费 | XL（可选） |
| 8 | 无 WebSocket 传输 | Codex CLI 等依赖 WS 的场景不可用 | L（可选） |

---

## 5. 方案设计

### 5.1 总体架构与数据流

```
外部客户端（OpenAI SDK / Agents SDK / curl）
    → POST /v1/responses（新增路由，GatewayProtocol::Responses）
        → ForwardPipeline::run（复用）
            → resolve_route：解析 model = {provider_slug}/{model_id}
            → prepare_body：替换 model；stream 判定（通用）；不注入 stream_options
            → ClientFactory::create(provider_type)
                ├─ "openai-responses" → OpenAiResponsesClient（新增，HTTP/SSE）
                └─ 其他类型（二期）→ 协议转换器（Responses → ChatCompletions）
            → UpstreamResponse::Streaming / Complete
        → build_response：SSE 透传 / JSON 透传（复用）
            → usage 提取扩展（response.completed 事件 / input_tokens 字段）
        → 转发日志（LogPipeline）+ 调用记录（call_records，复用）
```

### 5.2 协议枚举扩展

```rust
// context.rs
pub enum GatewayProtocol {
    ChatCompletions,
    AnthropicMessages,
    Responses,          // 新增
}
// to_upstream()：Responses => UpstreamProtocol::Responses

// client/mod.rs
pub enum UpstreamProtocol {
    ChatCompletions,
    AnthropicMessages,
    Responses,          // 新增
}
impl fmt::Display：Responses => "responses"
```

### 5.3 路由注册（router.rs）

```rust
.route("/v1/responses", post(responses))
```

新增 handler `responses`：结构完全仿照 `chat_completions`，构造 `ForwardRequest { protocol: GatewayProtocol::Responses, body, api_key_secret_id }` 交给 `ForwardPipeline::run`；错误统一 `upstream_error_response`（已是 OpenAI 标准格式，Responses 错误体 `{error:{...}}` 与之兼容）。

> 说明：Responses 与 ChatCompletions 的错误体结构一致（`{error:{message,type,param,code}}`），无需单独错误转换。

### 5.4 上游客户端：`OpenAiResponsesClient`（新增）

新建 `client/openai_responses_client.rs`，基于 `OpenAiChatClient` 模式改造：

- **路径**：`build_upstream_url(provider, "/responses")`（OpenAI `base_url = https://api.openai.com/v1`，自动模式不会重复 `/v1`；`use_raw_base_url = true` 时供应商负责带全路径）。
- **认证**：复用 `auth_resolver`（Bearer token），与现有客户端一致。
- **流式**：`stream: true` → 直接返回 `UpstreamResponse::Streaming { response, protocol: Responses }`，由 `response_handler` SSE 透传。
- **非流式**：读取完整 body 返回 `Complete`。
- **`stream_options` 不注入**（Responses 无需；现有 `prepare_body` 已按协议条件注入，天然正确）。
- **额外 header**：供应商 `extra_headers` 注入；如需 beta 功能（multi-agent 等）由用户在供应商表单配置，不做内置拼接。

```rust
// ClientFactory::create 调整
"openai-responses" => Ok(Box::new(OpenAiResponsesClient::new())),  // 不再走 WebSocket 占位
"openai-codex" | "websocket" => Ok(Box::new(WebSocketClient::new(provider_type))), // 保持占位
```

### 5.5 usage 提取扩展（`usage_extractor.rs`）

**非流式**（`parse_usage_from_response_body`）补充：

```rust
// prompt_tokens fallback：Responses 用 input_tokens
let prompt_tokens = usage.get("prompt_tokens")
    .and_then(|v| v.as_i64())
    .or_else(|| usage.get("input_tokens").and_then(|v| v.as_i64()));
// cached fallback：input_tokens_details.cached_tokens
.or_else(|| {
    usage.get("input_tokens_details")
        .and_then(|d| d.get("cached_tokens"))
        .and_then(|v| v.as_i64())
});
```

（`completion_tokens` 已有 `output_tokens` fallback、`total_tokens` 已有，无需改。）

**流式**（`parse_sse_event_for_usage`）新增分支：

```rust
UpstreamProtocol::Responses => {
    // data: {"type":"response.completed","response":{"usage":{...}}}
    if val.get("type") == "response.completed" {
        if let Some(usage_obj) = val.get("response").and_then(|r| r.get("usage")) {
            usage.prompt_tokens = usage_obj.get("input_tokens").and_then(|v| v.as_i64());
            usage.completion_tokens = usage_obj.get("output_tokens").and_then(|v| v.as_i64());
            usage.total_tokens = usage_obj.get("total_tokens").and_then(|v| v.as_i64());
            usage.cached_tokens = usage_obj.get("input_tokens_details")
                .and_then(|d| d.get("cached_tokens")).and_then(|v| v.as_i64());
            usage.cache_hit = usage.cached_tokens.map(|ct| ct > 0);
        }
    }
}
```

### 5.6 日志与标签

- `util.rs::protocol_tags`：改为根据 `provider.transport` + 实际传输判定——HTTP 模式打 `sse`；`websocket` 标签仅当真正走 WebSocket 时打（二期）。一期规则：`openai-responses` 且 `transport != websocket` → `sse`。
- `util.rs::build_log_url`：新增 `GatewayProtocol::Responses => "/responses"`。
- 自研 logger 网关日志格式不变（方法/URL/状态/耗时/token 自动带上）。

### 5.7 前端适配

- `gateway-api-docs-dialog.tsx`：`endpoints` 数组追加
  `{ method: 'POST', path: '/v1/responses', requireAuth: true }`。
- i18n（`zh-CN` / `en`）：新增 `gatewayApiDocs.endpoints.responses.*` 文案。
- 供应商表单：`openai-responses` 已可选；可考虑在帮助文案中提示「HTTP/SSE 传输可用，WebSocket 传输待实现」（可选，非阻塞）。

### 5.8 虚拟供应商与故障转移

`route_resolver` / `VirtualForwarder` 对协议中立（`GatewayProtocol` 透传），**无需改动**即可支持 Responses 请求走虚拟供应商路由。

---

## 6. 实施阶段

### Phase 1 — HTTP/SSE 透传（核心，本期）

1. `GatewayProtocol` / `UpstreamProtocol` 增加 `Responses` 变体（`context.rs`、`client/mod.rs`）。
2. 新增 `client/openai_responses_client.rs`，`ClientFactory` 将 `openai-responses` 路由到新 Client。
3. `router.rs` 注册 `POST /v1/responses` + handler。
4. `util.rs`：`build_log_url` 新增路径映射；`protocol_tags` 修正标签。
5. `usage_extractor.rs`：非流式 `input_tokens` fallback + 流式 `response.completed` 分支。
6. 前端：API 文档弹窗 + i18n。
7. 验证：`cargo check` + `pnpm type-check`；用 openai SDK 对本地网关发 `responses.create`（流式 + 非流式），核对日志页 token 统计与调用记录。

### Phase 2 — 可选：WebSocket 传输

- 引入 `tokio-tungstenite` 依赖。
- `UpstreamResponse` 增加 WebSocket 事件流变体（或抽象统一事件流枚举），`response_handler` 分支处理 WS 帧 → SSE 事件转换。
- 会话管理参考 `websocket-session-manager.ts`（连接复用、请求排队、abort）。
- `transport = websocket` 的供应商真正走 WS；`auto` 先默认 HTTP，探测失败再回退。

### Phase 3 — 可选：协议转换（Responses ↔ ChatCompletions）

- **Responses → ChatCompletions**（上游为非 OpenAI 类型时）：`input`/`instructions`/`tools` → `messages`；`output[]`/事件流 → `choices` 格式。让 DeepSeek 等兼容供应商可被 Responses 客户端消费。
- **ChatCompletions → Responses**：反向转换（参考项目 SDK 的 `convert` 工具）。
- 转换器为独立模块（如 `forwarding/convert/responses_conv.rs`），仅在路由目标供应商非 `openai-responses` 时启用；OpenAI 原生直连永远走透传。
- 复杂度高（事件流双向映射、usage 字段映射、工具调用语义差异），建议在 Phase 1 落地后按需求推进。

---

## 7. 风险与注意事项

| 项 | 说明 |
|----|------|
| 有状态 API | `store: true` / `previous_response_id` 依赖 OpenAI 服务端状态；网关透传即可，但**不做**本地会话存储。`GET /v1/responses/{id}` 等端点一期不暴露（如需，透传给上游 `{base_url}/responses/{id}` 即可，低风险）。 |
| 内置工具 | `web_search_preview` / `file_search` / `code_interpreter` 等是 OpenAI 服务端能力，网关透传天然支持；**不涉及**本地实现。 |
| WebSocket 占位 | Phase 1 完成后 `openai-codex` 仍不可用（保持占位）；`openai-responses` 供应商若配置 `transport = websocket`，Client 需显式报「未实现，请改用 auto/sse」而非静默走 HTTP（避免用户误以为走了 WS）。 |
| usage 兼容 | `parse_usage_from_response_body` 改动需回归测试既有 OpenAI/Anthropic/DeepSeek 解析（补 fallback 不影响原路径）。 |
| 错误格式 | Responses 端点错误体沿用 `{error:{message,type,param,code}}`（OpenAI 标准），与 ChatCompletions 一致，无需新格式；注意**不得**泄露内部错误码/堆栈（沿用 `upstream_error_response`）。 |
| 认证 | 新端点自动纳入 API Key 认证（`EXEMPT_PATHS` 不含它），内部 CLI 豁免逻辑（`inner-cli-api`）同样生效。 |
| 前端 chat | 内部聊天模块继续走 `/v1/chat/completions`，不迁移；Responses 支持仅面向外部客户端。 |

---

## 8. 验收标准（Phase 1）

1. `curl -X POST http://127.0.0.1:54321/v1/responses -H "Authorization: Bearer <key>" -d '{"model":"openai/gpt-4.1","input":"hi"}'` 返回标准 `{object:"response", output:[...], usage:{...}}`。
2. `stream: true` 时返回 `text/event-stream`，事件序列含 `response.output_text.delta` 与 `response.completed`（含 usage）。
3. 网关日志页与调用记录页正确显示 input/output/total/cached tokens。
4. 虚拟供应商路由下 Responses 请求正常故障转移。
5. 既有 `/v1/chat/completions`、`/v1/messages` 回归测试全部通过。
