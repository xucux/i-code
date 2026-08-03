# i-code Gateway Runtime 设计文档

> 版本：v0.3.0
> 最后更新：2026-08-03
> 关联模块：`src-tauri/src/modules/gateway_runtime/`、`src-tauri/src/modules/ai-gateway/`、`src-tauri/src/modules/virtual_provider/`、`src-tauri/src/modules/logger/`、`src-tauri/src/modules/call_records/`  
> 参考项目：`参考项目/vscode-unify-chat-provider-7.12.3/src/client/`

---

## 1. 设计目标

`gateway-runtime` 负责在本地启动一个 HTTP Server（默认 `127.0.0.1:54321`），将外部客户端（CLI、IDE 插件、curl 等）发来的 OpenAI 兼容请求转发到真实的 LLM 供应商。核心目标：

1. **统一模型路由 ID**：对外暴露 `{provider_slug}/{model_id}`，网关解析后路由到真实供应商。
2. **协议透明**：上层调用方只需使用 OpenAI 兼容格式，底层自动适配 SSE、WebSocket、标准 REST 等协议。
3. **虚拟供应商故障转移**：通过 `virtual-provider` 模块按策略选择真实目标供应商。
4. **可观测性**：请求/响应日志写入 `logger`，调用统计写入 `call_records`，并支持协议标签（`sse`、`websocket`）。
5. **Secret 安全**：API Key 等敏感信息仅在后端解析，网关中只出现 `$SECRET:{uuid}$` 引用。

---

## 2. 参考项目 client 包架构

参考项目 `vscode-unify-chat-provider-7.12.3/src/client/` 是一个供应商 API 客户端适配层，为 VS Code `LanguageModelChatProvider` 提供统一的多供应商 LLM 调用能力。其设计对 i-code 网关转发层有直接借鉴意义。

### 2.1 目录结构

```
client/
├── interface.ts                 # 统一抽象接口 ApiProvider
├── definitions.ts               # 供应商注册表 PROVIDER_TYPES
├── types.ts                     # 共享类型（Feature、DataPartMimeTypes 等）
├── utils.ts                     # 工厂函数 createProvider、URL 匹配、工具函数
├── websocket-session-manager.ts # WebSocket 会话管理
├── anthropic/                   # Anthropic Messages API + Claude Code
├── github-copilot/              # GitHub Copilot API
├── google/                      # Google AI Studio / Vertex AI / Gemini CLI 等
├── ollama/                      # Ollama 本地模型
├── openai/                      # OpenAI Chat Completions / Responses / Codex
└── xai/                         # xAI Grok Build
```

### 2.2 关键设计

1. **统一接口 `ApiProvider`**  
   所有供应商实现相同的 `streamChat()` 方法，上层无需关心底层是 REST、SSE 还是 WebSocket：
   ```ts
   interface ApiProvider {
     streamChat(encodedModelId, model, messages, options, ...): AsyncGenerator<...>
     estimateTokenCount(text): number
     getAvailableModels?(credential): Promise<ModelConfig[]>
   }
   ```

2. **工厂模式 `createProvider(config)`**  
   根据 `ProviderConfig.type` 从 `PROVIDER_TYPES` 注册表查找并实例化对应的 Provider 类。i-code 网关可借鉴此模式，在 `upstream.rs` 中按 `provider.provider_type` 分发到不同的转发实现。

3. **Feature 特性系统**  
   每个供应商声明 `supportedFamilys`、`supportedModels`、`supportedProviders`（URL 模式），用于判断模型能力匹配（streaming、tool use、thinking 等）。i-code 当前在 `ai-gateway` 中通过 `Provider` 类型与 `ModelConfig` 维护类似能力，后续可在网关层引入 Feature 检查，用于决定是否启用流式、工具调用转换等。

4. **协议多样性**  
   - **OpenAI Responses / Codex**：使用 WebSocket（`responses-websocket-transport.ts`、`websocket-session-manager.ts`）。  
   - **Anthropic**：使用 official SDK 或 fetch + SSE。  
   - **OpenAI Chat Completion / Ollama**：标准 HTTP + SSE。  
   - **Google**：gRPC-like REST（`generateContent`）。  

### 2.3 与 i-code 的关系

参考项目的 `client` 包提供了完整的各供应商协议实现，是 i-code 网关运行时真实转发逻辑的重要参考源。当前 i-code v0.2.0 在 `gateway_runtime/upstream.rs` 中实现了基础转发层，尚未完全拆分出独立的 Provider Client，但已借鉴其“统一接口 + 按类型分发 + 协议透明”的思想。

---

## 3. i-code 网关运行时架构

```
外部客户端
    → axum Router (src-tauri/src/modules/gateway_runtime/router.rs)
        → auth_middleware (API Key / 内部白名单)
        → chat_completions / anthropic_messages / list_models handlers
            → upstream.rs 解析模型、构造上游上下文、调用 Client 层
                → client/ 按供应商类型执行实际协议请求
                    → 真实供应商 HTTP/SSE/WebSocket 响应
            ← 返回 Response 给客户端
    → 异步写 logger (请求/响应日志，含 tags)
    → 异步写 call_records (调用统计)
```

### 3.1 模块分层

| 文件/目录 | 职责 |
|------|------|
| `service.rs` | Gateway HTTP Server 生命周期（启动/停止）、共享状态 `GatewaySharedState`、Tauri Command 入口。 |
| `router.rs` | axum 路由树、handler、直连网关请求日志拦截。 |
| `upstream.rs` | 上游请求编排：模型解析、认证解析、调用记录、转发日志、协议标签、响应转换。不再直接处理 HTTP/SSE 细节。 |
| `client/` | 供应商协议 Client 抽象层：`UpstreamClient` trait、`ClientFactory`、OpenAI/Anthropic/WebSocket 具体实现。 |
| `auth.rs` | API Key 校验、错误状态码映射、内部请求白名单。 |
| `types.rs` | GatewayRuntimeState、启动/停止输入等 DTO。 |

---

## 4. 路由层（router.rs）

### 4.1 路由清单

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/health` | 存活检查，always 200。 |
| GET | `/readyz` | 就绪检查，含数据库连通性。 |
| GET | `/v1/models` | 列出所有对外暴露模型（真实 + 虚拟）。 |
| POST | `/v1/chat/completions` | OpenAI 兼容聊天补全接口。 |
| POST | `/v1/responses` | OpenAI Responses API（Agent 场景，SSE 事件流 / 非流式）。 |
| POST | `/v1/messages` | Anthropic Messages 兼容接口。 |

### 4.2 认证中间件

- 除 `/health`、`/readyz` 外，所有路由经过 `auth_middleware`。
- 优先检查 `inner-cli-api` 请求头：
  - 值与本进程启动时生成的全局 `inner_cli_api_key` 一致 → 直接放行，用于内部 CLI 请求豁免。
  - 头存在但值不正确 → 返回 403 Forbidden，不再回退校验 `Authorization`。
- 未携带 `inner-cli-api` 请求头时，必须校验 `Authorization: Bearer {gateway_key}` 或者 `X-API-Key: {gateway_key}`，密钥来自 `gateway_auth_keys.api_key_secret_id` 指向的 Secret或者 `gateway_settings.default_api_key_secret_id` 指向的 Secret。
- 不允许使用 IP 白名单豁免。

### 4.3 直连网关请求日志

- 通过 `log_settings.enable_gateway_request_log / enable_gateway_response_log / gateway_max_body_length` 控制。
- 在 `chat_completions` / `anthropic_messages` handler 中记录请求体。
- 在 `log_gateway_response` 中根据响应头检测协议标签：
  - `Content-Type: text/event-stream` → `sse`
  - `Upgrade: websocket` → `websocket`
- 流式/WebSocket 响应不读取响应体，避免阻塞数据流；非流式响应按需读取并重建返回。

---

## 5. 转发层（upstream.rs）

### 5.1 模型 ID 解析

对外模型 ID 格式：`{provider_slug}/{model_id}` 或 `{virtual_alias}/{model_id}`。

```rust
pub fn parse_model_id(model_id: &str) -> IcodeResult<(String, String)>
```

解析后剥离前缀，仅将真实 `model_id` 传给上游。

### 5.2 上游上下文解析

```rust
pub struct UpstreamContext {
    pub provider: Provider,              // 目标真实供应商
    pub gateway_model_id: Option<String>,// 网关暴露模型记录 ID
    pub upstream_model_id: String,       // 去除前缀后的真实模型 ID
    pub is_virtual: bool,                // 是否通过虚拟供应商路由
    pub route_index: usize,              // 虚拟供应商路由索引
    pub request_id: String,              // 请求追踪 ID
}
```

解析顺序：
1. 先匹配 `ai-gateway` 中启用的真实供应商 `provider_slug`。
2. 未命中时，通过 `virtual-provider` 解析故障转移路由，得到目标供应商与模型。
3. 仍未命中返回 `NOT_FOUND`。

### 5.3 认证构造

当前 v0.2.0 支持：
- OpenAI 兼容供应商：`Authorization: Bearer {api_key}`。
- Anthropic：`x-api-key` + `anthropic-version: 2023-06-01`。

API Key 通过 `ai_gateway.service().resolve_auth_for_request(&provider)` 获取 `AuthConfig`，再提取明文。Secret 解析仅在 Rust 后端完成。

### 5.4 协议标签识别

在转发层根据供应商类型、传输方式与流式标志生成协议标签：

```rust
fn protocol_tags(provider_type: &str, transport: Option<&str>, is_stream: bool) -> Vec<String> {
    let mut tags = Vec::new();
    if is_stream { tags.push("sse".to_string()); }
    if (provider_type == "openai-responses" || provider_type == "openai-codex")
        && transport == Some("websocket") {
        tags.push("websocket".to_string());
    }
    tags
}
```

标签随 `LogEntry` 写入 `logger`，用于前端区分 SSE / WebSocket / 普通 REST 请求。
仅当供应商显式配置 `transport = websocket` 时才打 `websocket` 标签，HTTP 透传场景统一为 `sse`。

### 5.5 流式与非流式响应

- **流式**：使用 `reqwest` 发送请求后，通过 `upstream_resp.bytes_stream()` 转换为 axum `Sse` 响应，保持长连接透传。
- **非流式**：读取完整响应体，按需记录日志后重建 `Response` 返回。

### 5.6 错误处理

- 上游连接失败、认证失败、超时等统一转换为 `IcodeError`。
- `upstream_error_response(err)` 构造 OpenAI 兼容错误体返回客户端。
- 错误信息同时写入 `call_records` 和 `logger`。

### 5.7 供应商级重试

`DirectForwarder::execute` 在 HTTP 请求层面实现供应商级自动重试，读取 `provider.retry_json` 解析 `RetryConfig`。

#### 配置结构

`retry_json` 存储在 `providers` 表中，JSON 格式（camelCase）：

```json
{
  "maxRetries": 3,
  "initialDelayMs": 2000,
  "maxDelayMs": 8000,
  "backoffMultiplier": 2.0,
  "jitterFactor": 0.2,
  "statusCodes": [429, 500, 502, 503, 504]
}
```

前端仅暴露 `maxRetries`（最大重试次数）与 `initialDelayMs`（重试间隔），其余字段使用默认值。

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `maxRetries` | 3 | 最大重试次数（不含首次请求）；设为 0 禁用重试 |
| `initialDelayMs` | 2000 | 初始退避延迟（毫秒），前端展示为「重试间隔」 |
| `maxDelayMs` | 8000 | 退避延迟上限（毫秒） |
| `backoffMultiplier` | 2.0 | 指数退避倍率 |
| `jitterFactor` | 0.2 | 抖动因子（0.0-1.0），避免雪崩 |
| `statusCodes` | [429,500,502,503,504] | 触发重试的 HTTP 状态码 |

`retry_json` 为空或未设置时使用全部默认值（等同于上述配置）。

#### 重试触发条件

| 场景 | 是否重试 | 说明 |
|------|----------|------|
| 网络错误（DNS/TCP/TLS/超时） | ✅ | `ClientError::RequestFailed` |
| HTTP 429 / 500 / 502 / 503 / 504 | ✅ | 状态码在 `statusCodes` 列表中 |
| HTTP 4xx（除 429） | ❌ | 认证/请求格式错误，重试无意义 |
| HTTP 2xx | ❌ | 请求成功 |
| 流式响应已开始（2xx + SSE） | ❌ | 流已透传到客户端，无法中断重传 |

#### 退避延迟计算

第 N 次重试（从 1 开始）的延迟：

```
base = initialDelayMs × backoffMultiplier^(N-1)
delay = min(base, maxDelayMs)
jitter = delay × jitterFactor × (random ∈ [-1, 1])
final = delay + jitter
```

示例（`initialDelayMs=2000`, `backoffMultiplier=2.0`, `maxDelayMs=8000`）：

| 重试次数 | 基础延迟 | 实际延迟范围（含 ±20% 抖动） |
|----------|----------|-------------------------------|
| 1 | 2000ms | 1600ms ~ 2400ms |
| 2 | 4000ms | 3200ms ~ 4800ms |
| 3 | 8000ms | 6400ms ~ 9600ms |

#### 与虚拟供应商故障转移的关系

`VirtualForwarder` 内部使用 `DirectForwarder`，因此虚拟路由的每条候选路由都会先执行供应商级重试，
重试耗尽后才降级健康度并切换到下一条路由。执行顺序：

```
请求 → 虚拟路由 1 → DirectForwarder 重试（maxRetries 次）→ 全部失败 → 降级路由 1
     → 虚拟路由 2 → DirectForwarder 重试（maxRetries 次）→ 成功 → 返回
```

#### 重试日志（双写策略）

重试关键节点同时写入两套日志，遵循项目 §11 双日志框架约定：

| 日志通道 | 写法 | 输出目标 | 用途 |
|----------|------|----------|------|
| `tracing` | `tracing::info!` / `warn!` / `error!` | 终端、WebView 控制台、日志文件 | 开发调试、全链路追踪（含 `[tid=...]`） |
| 自研 logger | `Log::info` / `Log::warn` / `Log::error` | 应用内「日志」页面（内存环形缓冲区） | 用户/运维可见的业务诊断 |

**镜像规则**：`warn` / `error` / `info` 级别的重试事件同时写入两套日志；`debug` 级别（首次请求、配置加载等）仅写入 `tracing`，不写入自研 logger 以避免噪音。

**日志覆盖的关键节点**：

| 事件 | 级别 | 日志内容（含字段） |
|------|------|-------------------|
| 转发重试开始（退避前） | `info` | provider / model / attempt / max_retries / backoff_delay / reason |
| 上游返回可重试状态码，准备重试 | `warn` | provider / model / status / attempt / max_retries / remaining |
| 网络错误，准备重试 | `warn` | provider / model / attempt / max_retries / remaining / error |
| 转发重试成功 | `info` | provider / model / status / attempts_used |
| 可重试状态码，重试已耗尽 | `warn` | provider / model / status / total_attempts |
| 网络错误，重试已耗尽 | `error` | provider / model / total_attempts / error |
| 重试全部耗尽（返回可重试响应） | `error` | provider / model / final_status / max_retries |
| 重试全部耗尽（返回网络错误） | `error` | provider / model / max_retries |
| 禁用重试 / 首次请求 / 配置加载 / 首次成功 / 不可重试错误 | `debug` | provider / model / stream / retryable_codes 等 |

> 用户可在应用内「日志」页面按 `warn` / `error` 级别筛选，直接查看重试相关业务诊断信息；开发者在终端/日志文件中通过 `[tid=...]` 前缀关联完整请求链路。

---

## 6. 调用记录与日志

### 6.1 call_records

- 每次向上游发送请求前，调用 `start_call_log` 写入 `model_call_logs` 初始记录。
- 请求结束后调用 `finish_call_log` 更新状态码、错误信息、耗时。
- 当上游响应不含 `usage` 字段时，使用 `tokenizer` 模块估算 `prompt_tokens` 并补充。

### 6.2 logger

- 直连网关日志：`source = Gateway`，记录客户端 → 网关的请求/响应。
- 转发日志：`source = ProviderApi`，记录网关 → 供应商的请求/响应。
- 请求体/响应体记录受 `log_settings` 中独立开关控制：
  - `enable_request_log / enable_response_log / forward_max_body_length`：控制转发日志。
  - `enable_gateway_request_log / enable_gateway_response_log / gateway_max_body_length`：控制直连网关日志。
- 日志标签 `tags` 在 SSE / WebSocket 场景下分别填入 `sse` / `websocket`。

---

## 7. 与参考项目的对齐与差异

| 维度 | 参考项目 client 包 | i-code gateway-runtime（当前） |
|------|-------------------|------------------------------|
| 抽象接口 | `ApiProvider` 统一接口 | 当前在 `upstream.rs` 中按类型分支转发，尚未抽象出统一 Client trait，但 `GatewaySharedState` 已具备注入不同 Client 的能力。 |
| 工厂模式 | `createProvider(config)` 从 `PROVIDER_TYPES` 实例化 | 当前直接在 `forward_chat_completions` / `forward_anthropic_messages` 中按 `provider_type` 构造请求，可进一步抽象为 `UpstreamClientFactory`。 |
| Feature 系统 | 支持 `supportedFamilys / supportedModels / supportedProviders` | 当前通过 `Provider` 和 `ModelConfig` 维护基础能力字段，尚未引入 Feature 检查。 |
| 协议支持 | SSE、WebSocket、REST、gRPC-like 齐全 | 支持 REST 与 SSE；OpenAI Responses API 支持 HTTP/SSE 透传（`openai-responses`）与 WebSocket 传输（`transport = websocket`）；`openai-codex` WebSocket 转发待实现。 |
| 认证 | SDK 内统一处理 | 当前仅支持 `api-key` 认证，其他认证方式（OAuth、Azure、GCP 等）待扩展。 |
| 日志 | `RequestLogger` 注入 Provider | i-code 通过 `GatewaySharedState` 共享 `logger_handle`，在 handler 与 upstream 中统一写入。 |

---

## 8. 演进方向

1. **抽象 `UpstreamClient` trait**：已落地——`client/` 按 `provider_type` 工厂分发（`OpenAiChatClient`、`AnthropicClient`、`OpenAiResponsesClient`、`WebSocketClient` 占位）。
2. **WebSocket 转发**：已部分落地——OpenAI Responses API 支持 WebSocket 传输（`openai-responses` + `transport=websocket`，WS 帧转 SSE 字节流复用 usage 拦截）；`openai-codex` 仍为占位待实现。
3. **引入 Feature 检查**：在模型配置中声明 `streaming`、`tool_use`、`thinking`、`vision` 等能力，网关层根据 Feature 决定是否转换请求体或响应体。
4. **扩展认证方式**：支持 OAuth、Azure AD、GCP Service Account 等多态认证。
5. **请求/响应体转换**：在虚拟供应商或多供应商场景下，实现 Anthropic ↔ OpenAI 等格式转换（含 Responses ↔ ChatCompletions）。
6. **供应商级 extra_headers / extra_body**：extra_headers 已支持，extra_body 待合并到上游请求构造。

---

## 9. 关键文件速查

| 文件 | 说明 |
|------|------|
| `src-tauri/src/modules/gateway_runtime/router.rs` | axum 路由与直连网关日志。 |
| `src-tauri/src/modules/gateway_runtime/upstream.rs` | 上游转发、协议标签、调用记录。 |
| `src-tauri/src/modules/gateway_runtime/service.rs` | HTTP Server 生命周期与共享状态。 |
| `src-tauri/src/modules/gateway_runtime/auth.rs` | API Key 校验与错误映射。 |
| `src-tauri/src/modules/logger/types.rs` | `LogEntry`、`GatewayLogConfig`、`ForwardLogConfig`。 |
| `src-tauri/src/modules/call_records/types.rs` | `CreateModelCallLogInput`、`RouteMode`。 |
| `src-tauri/src/modules/virtual_provider/service.rs` | 虚拟供应商路由解析。 |
| `src/routes/logs/index.tsx` | 日志页面，含日志设置。 |
| `src/components/ui/log-viewer.tsx` | 日志列表与 tags 展示。 |
| `参考项目/vscode-unify-chat-provider-7.12.3/src/client/` | 供应商协议实现参考。 |
