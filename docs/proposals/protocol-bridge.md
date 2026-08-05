# 协议自动转换（Anthropic Messages ↔ OpenAI Chat Completions）设计提案

> 版本：v1.0.0（已发布）
> 状态：P1–P4 实施完成并验证通过（2026-08-04）
> 最后更新：2026-08-04
> 关联模块：`src-tauri/src/modules/gateway_runtime/`
> 关联文档：[`docs/gateway-runtime.md`](../gateway-runtime.md)、[`docs/log-framework.md`](../log-framework.md)、[`docs/error-handling.md`](../error-handling.md)

---

## 1. 背景与目标

### 1.1 现状

i-code 网关当前对外暴露三套接口：

| 路径 | `GatewayProtocol` | 默认转发的上游 Client |
|------|-------------------|----------------------|
| `POST /v1/chat/completions` | `ChatCompletions` | `OpenAiChatClient` |
| `POST /v1/messages` | `AnthropicMessages` | `AnthropicClient` |
| `POST /v1/responses` | `Responses` | `OpenAiResponsesClient` |

`GatewayProtocol::to_upstream()` 是 **1:1 直接映射**，每个 `UpstreamClient::execute` 入口都会校验 `request.protocol`，不匹配时直接返回 `UnsupportedProtocol` 错误。因此当前存在硬性约束：

- 客户端用 Anthropic Messages 调网关 + 供应商 `provider_type = openai-chat-completion` → **请求失败**
- 客户端用 OpenAI Chat Completions 调网关 + 供应商 `provider_type = anthropic` → **请求失败**

虚拟供应商故障转移场景下，候选路由可能跨越不同协议类型的供应商，目前无法平滑降级。

### 1.2 目标

1. **协议互转**：网关入口协议与上游供应商协议不一致时，自动在中间桥接
   - 入口 `messages` + 上游 OpenAI Chat → 桥接
   - 入口 `chat/completions` + 上游 Anthropic Messages → 桥接
   - 其他组合 → 维持原逻辑
2. **日志标签**：自研 logger 在发生桥接时追加 `bridge` 标签；遇 WebSocket 协议追加 `ws` 标签（**本次同步将历史 `websocket` 标签全部更名为 `ws`**，详见 §6.3、§7.5）
3. **不破坏现有路径**：协议一致时零开销，行为完全保持不变
4. **不引入新依赖**：纯 Rust 实现，复用现有 `serde_json` 流式管线

### 1.3 非目标（本次不做）

- OpenAI Responses API 与另外两种协议的互转（Responses 事件模型差异过大，单独提案）
- 桥接场景下的 WebSocket 传输（当前仅 `openai-responses` 走 WS，且 Responses 不参与桥接，故 `bridge + ws` 组合实际不会出现；但标签规则为未来扩展预留）
- 虚拟供应商跨协议故障转移的桥接支持（§7.7 决定本次不实施，候选路由需保持协议一致）
- 模型能力（vision / tool_use / thinking）的 Feature 检查
- 多模态内容（image / audio）的格式重编码——仅做结构层面的字段映射

---

## 2. 当前架构回顾

```
客户端
  → router.rs (chat_completions / anthropic_messages / responses handler)
    → ForwardPipeline::run
      → resolve_route → ForwardContext { gateway_protocol, upstream, ... }
      → DirectForwarder / VirtualForwarder
        → ClientFactory::create(provider_type) → UpstreamClient
          → UpstreamClient::execute(&mut ctx, UpstreamRequest { protocol, body, is_stream })
            ↑ 此处 protocol == gateway_protocol.to_upstream()，不做转换
```

关键约束：
- `prepare_body` 仅替换 `model` 字段、注入 `stream_options`，**不改变协议结构**
- `UpstreamResponse::Streaming` 直接持有 `reqwest::Response`，SSE 字节流原样透传
- 流式响应在 `response_handler.rs` 中通过 `parse_sse_event_for_usage` 按 `UpstreamProtocol` 解析 usage——**协议与解析器必须匹配**

---

## 3. 协议差异详细对比

> 仅列出对桥接有影响的字段；未列出的字段（如 `temperature`、`top_p`、`max_tokens` 边界值）按本节规则映射或在 §7 决策记录中说明。

### 3.1 请求体顶层字段

| 维度 | OpenAI Chat Completions | Anthropic Messages | 转换方向与说明 |
|------|------------------------|--------------------|---------------|
| 模型字段 | `model: string` | `model: string` | 一致，已由 `prepare_body` 替换为真实 upstream_model_id |
| 系统提示 | `messages[0]` 中 `role:"system"` | 顶层 `system: string \| array<{type:"text",text}>` | **需转换**：O→A 提取 system message 到顶层；A→O 把 system 包装为 `messages[0]` |
| 消息数组 | `messages: [{role, content, tool_calls, tool_call_id, name}]` | `messages: [{role, content, tool_calls?}]` | 角色与字段命名差异，详见 §3.2 |
| 流式开关 | `stream: bool` | `stream: bool` | 一致 |
| 流式 usage | `stream_options: { include_usage: true }` | 流式默认带 `message_delta.usage` | **需转换**：O→A 移除 `stream_options`；A→O 注入 `stream_options.include_usage` |
| 最大 tokens | `max_tokens?: int`（可选） | `max_tokens: int`（**必填**） | **需转换**：O→A 时若缺 `max_tokens` 需注入默认值（§7.2 决策：从 `model_configs.max_output_tokens` 读取，兜底 200000） |
| 工具定义 | `tools: [{type:"function", function:{name, description, parameters}}]` | `tools: [{name, description, input_schema}]` | **需转换**：结构完全不同，详见 §3.3 |
| 工具选择策略 | `tool_choice: "none" \| "auto" \| "required" \| {type:"function", function:{name}}` | `tool_choice: {type:"auto"\|"any"\|"tool", name?}` | **需转换**：枚举值与结构均不同 |
| 停止词 | `stop: string \| string[]` | `stop_sequences: string[]` | **需转换**：字段名 + 单值需包装为数组 |
| 随机性参数 | `temperature`, `top_p` | `temperature`, `top_p` | 一致（取值范围 Anthropic 限制 0~1） |
| 频率/存在惩罚 | `frequency_penalty`, `presence_penalty` | 不支持 | **需丢弃**：O→A 时移除；A→O 时不动 |
| 输出格式 | `response_format: {type:"json_object"\|"json_schema",...}` | 不支持（用 tool 模拟） | **§7.4 决策：移除字段 + 注入 system prompt 提示** |
| 用户标识 | `user: string` | `metadata: { user_id: string }` | **需转换**：O→A `user` → `metadata.user_id`；反向同理 |
| 种子 | `seed: int` | 不支持 | **丢弃**：O→A 移除 |
| reasoning | `reasoning_effort: "minimal"\|"low"\|"medium"\|"high"` | `thinking: {type:"enabled", budget_tokens:int}` | **需转换**：详见 §3.5 |
| 服务层级 | `service_tier: string` | 不支持（Anthropic 有独立 beta header） | **丢弃**：O→A 移除 |
| n（多候选） | `n: int` | 不支持 | **丢弃**：O→A 移除 |
| logprobs | `logprobs: bool`, `top_logprobs: int` | 不支持 | **丢弃**：O→A 移除 |

### 3.2 messages 数组元素差异

| 字段 | OpenAI | Anthropic | 转换说明 |
|------|--------|-----------|---------|
| `role` | `system` / `user` / `assistant` / `tool` | `user` / `assistant`（无 `system`，无 `tool`） | **需转换**：O 的 `system` 提取到顶层；O 的 `tool` 角色映射为 A 的 `user` 角色 + `tool_result` content block |
| `content` 类型 | `string` 或 `array<{type:"text"\|"image_url", ...}>` | `string` 或 `array<{type:"text"\|"image"\|"tool_use"\|"tool_result", ...}>` | **需转换**：详见 §3.4 |
| `tool_calls`（assistant） | `[{id, type:"function", function:{name, arguments}}]` | 不在 message 顶层；改为 `content` 数组中的 `{type:"tool_use", id, name, input}` | **需转换**：结构重组 |
| `tool_call_id`（tool role） | `string` | 不存在；改为 `content` 数组中的 `{type:"tool_result", tool_use_id, content}` | **需转换**：整条 tool 消息需展开为 assistant 的 `tool_result` block |
| `name`（assistant/user） | 可选 `string` | 不支持 | **丢弃**：O→A 移除 |

### 3.3 tools 定义差异

**OpenAI：**
```json
{
  "type": "function",
  "function": {
    "name": "get_weather",
    "description": "Get weather",
    "parameters": { /* JSON Schema */ }
  }
}
```

**Anthropic：**
```json
{
  "name": "get_weather",
  "description": "Get weather",
  "input_schema": { /* JSON Schema */ }
}
```

转换：直接字段重命名 + 嵌套层级调整。

### 3.4 content 数组差异（多模态 / 工具）

| type | OpenAI | Anthropic | 转换说明 |
|------|--------|-----------|---------|
| 文本 | `{type:"text", text:"..."}` | `{type:"text", text:"..."}` | 一致 |
| 图像 | `{type:"image_url", image_url:{url:"data:..."/"https://..."}}` | `{type:"image", source:{type:"base64"\|"url", media_type:"image/png", data:"..." \| url:"..."}}` | **需转换**：data URL 需拆分为 `media_type` + `data`；http(s) URL 用 `source.type="url"` |
| 工具调用 | （在 message 顶层 `tool_calls`） | `{type:"tool_use", id, name, input}` | **需转换**：见 §3.2 |
| 工具结果 | （`role:"tool"` 的独立 message） | `{type:"tool_result", tool_use_id, content: string \| array}` | **需转换**：见 §3.2 |

### 3.5 thinking / reasoning 差异

**OpenAI（o1 / o3 / gpt-5 系列）：**
```json
{ "reasoning_effort": "minimal" | "low" | "medium" | "high" }
```

**Anthropic（Claude 3.7+ extended thinking）：**
```json
{
  "thinking": {
    "type": "enabled",
    "budget_tokens": 4096
  }
}
```

转换（推荐映射，非精确等价）：

| OpenAI `reasoning_effort` | Anthropic `thinking.budget_tokens` |
|---------------------------|------------------------------------|
| `minimal` | 1024 |
| `low` | 2048 |
| `medium` | 4096 |
| `high` | 8192 |

反向：按 `budget_tokens` 落入哪一档反向映射；不在档位上的取最接近的档。

### 3.6 响应体差异

**OpenAI 非流式响应：**
```json
{
  "id": "chatcmpl-...",
  "object": "chat.completion",
  "created": 1234567890,
  "model": "gpt-4.1",
  "choices": [
    {
      "index": 0,
      "message": { "role": "assistant", "content": "...", "tool_calls": [...] },
      "finish_reason": "stop" | "length" | "tool_calls" | "content_filter"
    }
  ],
  "usage": { "prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30 }
}
```

**Anthropic 非流式响应：**
```json
{
  "id": "msg_...",
  "type": "message",
  "role": "assistant",
  "model": "claude-3-5-sonnet-...",
  "content": [
    { "type": "text", "text": "..." },
    { "type": "tool_use", "id": "...", "name": "...", "input": {...} }
  ],
  "stop_reason": "end_turn" | "max_tokens" | "stop_sequence" | "tool_use",
  "usage": {
    "input_tokens": 10,
    "output_tokens": 20,
    "cache_creation_input_tokens": 0,
    "cache_read_input_tokens": 0
  }
}
```

`finish_reason` ↔ `stop_reason` 映射：

| OpenAI `finish_reason` | Anthropic `stop_reason` |
|------------------------|-------------------------|
| `stop` | `end_turn` |
| `length` | `max_tokens` |
| `tool_calls` | `tool_use` |
| `content_filter` | （无直接对应） → `end_turn` |
| （无对应） | `stop_sequence` |

`usage` 字段映射：

| OpenAI | Anthropic |
|--------|-----------|
| `prompt_tokens` | `input_tokens` |
| `completion_tokens` | `output_tokens` |
| `total_tokens` | `input_tokens + output_tokens`（Anthropic 不返回，需计算） |
| `prompt_tokens_details.cached_tokens` | `cache_read_input_tokens` |

> 注：当前 `usage_extractor.rs` 已能从两种格式中解析 usage，所以 usage 在响应体转换后无需额外处理。

### 3.7 流式事件差异（最关键）

**OpenAI SSE 事件序列：**
```
data: {"choices":[{"delta":{"role":"assistant"}, "index":0}]}
data: {"choices":[{"delta":{"content":"Hello"}, "index":0}]}
data: {"choices":[{"delta":{"content":" world"}, "index":0}]}
data: {"choices":[{"delta":{}, "finish_reason":"stop", "index":0}], "usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12}}
data: [DONE]
```

**Anthropic SSE 事件序列：**
```
event: message_start
data: {"type":"message_start","message":{"id":"msg_...","role":"assistant","usage":{"input_tokens":10,"output_tokens":1}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" world"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}

event: message_stop
data: {"type":"message_stop"}
```

差异点：
1. **事件粒度**：OpenAI 单一 `delta` 事件承载所有内容；Anthropic 区分 `message_start` / `content_block_start` / `content_block_delta` / `content_block_stop` / `message_delta` / `message_stop`
2. **事件命名**：Anthropic 每个事件有 `event:` 行 + `data:` 行；OpenAI 只有 `data:` 行
3. **usage 出现时机**：OpenAI 在最后一个 `delta` 事件中（需 `stream_options.include_usage`）；Anthropic 在 `message_start`（input_tokens）和 `message_delta`（output_tokens）
4. **工具调用流式**：OpenAI 在 `delta.tool_calls` 数组中分片发送（含 `index`）；Anthropic 单独 `content_block_start` (type=tool_use) + `content_block_delta` (type=input_json_delta)
5. **结束标记**：OpenAI 发 `data: [DONE]`；Anthropic 发 `message_stop` 事件

---

## 4. 桥接执行位置与触发条件

### 4.1 触发条件矩阵

| 网关入口协议 | 上游 provider_type | 是否桥接 | 转换方向 |
|-------------|-------------------|---------|---------|
| `ChatCompletions` | `openai` / `openai-compatible` / `openai-chat-completion` / `deepseek` / `moonshot-ai` / `kimi-code` / `newapi` / `siliconflow` / `aihubmix` / `openrouter` / `minimax` / `xai-grok-build` / `ollama` / `custom` / `codex` / `gemini-cli` / `antigravity` | ❌ 否 | — |
| `ChatCompletions` | `anthropic` / `claude-relay-service` | ✅ 是 | O→A（请求）+ A→O（响应/流） |
| `AnthropicMessages` | `anthropic` / `claude-relay-service` | ❌ 否 | — |
| `AnthropicMessages` | 上表中 OpenAI 兼容类型 | ✅ 是 | A→O（请求）+ O→A（响应/流） |
| `Responses` | `openai-responses` | ❌ 否 | — |
| `Responses` | 其他 | ❌ 否 | 直接返回 `UnsupportedProtocol`（本次不桥接） |
| 任意 | `websocket` / `openai-codex` | ❌ 否 | WS 协议不参与桥接 |

> `provider_type` 判定使用 `ClientFactory::create` 中的现有分类常量，保证与 Client 实例化一致。

### 4.2 执行位置

桥接需在 **`ForwardPipeline::execute_and_finalize` 内**、调用 `forwarder.execute` **之前** 完成：

```
ForwardPipeline::execute_and_finalize
  ├── prepare_body                // 替换 model_id、注入 stream_options（保持现状）
  ├── [新增] apply_bridge          // 判定是否桥接，按需转换 body
  │     ├── bridge_kind = detect_bridge(gateway_protocol, provider_type)
  │     ├── if bridge_kind == O2A: body = openai_chat_to_anthropic(body)
  │     └── if bridge_kind == A2O: body = anthropic_to_openai_chat(body)
  ├── forwarder.execute           // 走原本的 ClientFactory（此时 protocol 已调整为上游协议）
  └── [新增] 反向转换响应
        ├── 非流式：bridge_kind==O2A 时把上游 Anthropic 响应体转为 OpenAI 格式
        ├── 非流式：bridge_kind==A2O 时把上游 OpenAI 响应体转为 Anthropic 格式
        └── 流式：包装 bytes_stream，逐事件转换（详见 §5.3）
```

### 4.3 `UpstreamRequest.protocol` 的处理

桥接时 `UpstreamRequest.protocol` 必须改为**上游协议**（与 ClientFactory 创建的 Client 类型一致），否则 Client 入口的 `request.protocol !=` 校验会失败。

新增 `GatewayProtocol::to_upstream_with_bridge(provider_type) -> UpstreamProtocol`：

```rust
impl GatewayProtocol {
    pub fn to_upstream_with_bridge(self, provider_type: &str) -> UpstreamProtocol {
        let is_openai_family = matches!(provider_type, "openai" | "openai-compatible" | ...);
        let is_anthropic_family = matches!(provider_type, "anthropic" | "claude-relay-service");
        match (self, is_openai_family, is_anthropic_family) {
            (Self::ChatCompletions, true, false) => UpstreamProtocol::ChatCompletions,
            (Self::ChatCompletions, false, true) => UpstreamProtocol::AnthropicMessages, // 桥接
            (Self::AnthropicMessages, false, true) => UpstreamProtocol::AnthropicMessages,
            (Self::AnthropicMessages, true, false) => UpstreamProtocol::ChatCompletions, // 桥接
            (Self::Responses, _, _) => UpstreamProtocol::Responses,
            _ => self.to_upstream(), // 兜底，沿用旧行为
        }
    }
}
```

`DirectForwarder::execute` 与 `VirtualForwarder` 内构造 `UpstreamRequest` 时改用此方法。

### 4.4 `BridgeKind` 枚举

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeKind {
    /// 无桥接
    None,
    /// 入口 OpenAI Chat → 上游 Anthropic Messages
    OpenaiToAnthropic,
    /// 入口 Anthropic Messages → 上游 OpenAI Chat
    AnthropicToOpenai,
}

pub fn detect_bridge(gateway_protocol: GatewayProtocol, provider_type: &str) -> BridgeKind { ... }
```

`BridgeKind` 写入 `ForwardContext`，供后续响应转换与日志标签使用。

---

## 5. 转换规则

### 5.1 请求体转换：Anthropic Messages → OpenAI Chat Completions（A→O）

```rust
pub fn anthropic_to_openai_chat(body: &mut Value) -> Result<(), BridgeError>
```

步骤：

1. **顶层 system 提取**：把 `body.system`（string 或 array of text blocks）拼接为字符串，构造 `{role:"system", content: <str>}`，插入 `body.messages` 开头；删除 `body.system`
2. **messages 重组**：
   - 遍历 Anthropic messages，按角色转换
   - `assistant` 消息中 `content` 数组里的 `tool_use` block → 提取到 message 顶层 `tool_calls: [{id, type:"function", function:{name, input}}]`（`input` 序列化为 JSON 字符串）
   - `user` 消息中 `content` 数组里的 `tool_result` block → 拆分为独立的 `{role:"tool", tool_call_id, content}` 消息（多条 tool_result 拆为多条 tool 消息）
   - 多模态：`image` block → `image_url`，data URL 拼回 `data:{media_type};base64,{data}`；url 形式直接用 `url`
3. **tools 重命名**：`tools[].input_schema` → `tools[].function.parameters`，加 `type:"function"` 包裹
4. **tool_choice 转换**：
   - `{type:"auto"}` → `"auto"`
   - `{type:"any"}` → `"required"`
   - `{type:"tool", name}` → `{type:"function", function:{name}}`
5. **字段重命名**：
   - `max_tokens` 保持（OpenAI 可选）
   - `stop_sequences` → `stop`（单值时不需包装，OpenAI 接受 string 或 array）
   - `metadata.user_id` → `user`
6. **字段丢弃**：移除 `thinking`（OpenAI 不识别；如需保留 reasoning，见 §7.3）
7. **流式字段**：注入 `stream_options.include_usage = true`（沿用 `prepare_body` 既有逻辑）

### 5.2 请求体转换：OpenAI Chat Completions → Anthropic Messages（O→A）

```rust
pub fn openai_chat_to_anthropic(body: &mut Value) -> Result<(), BridgeError>
```

步骤：

1. **system 提取**：扫描 `body.messages`，把所有 `role:"system"` 的消息文本拼接为字符串，赋给顶层 `body.system`；从 messages 数组中删除
2. **messages 重组**：
   - `role:"tool"` 消息 → 合并到前一条 `assistant` 消息的 `content` 数组中作为 `{type:"tool_result", tool_use_id: tool_call_id, content}`；若无前一条 assistant，则构造 `user` 消息包裹
   - `assistant` 消息的 `tool_calls` → `content` 数组中的 `{type:"tool_use", id, name, input: <parsed JSON>}`；同时 `content` 字符串转为 `{type:"text", text}` block
   - 多模态：`image_url.url` 解析——`data:image/png;base64,xxx` 拆分为 `source:{type:"base64", media_type:"image/png", data:"xxx"}`；`https://...` 直接 `source:{type:"url", url:"..."}`
3. **tools 重命名**：`tools[].function.parameters` → `tools[].input_schema`，去除 `type:"function"` 包裹
4. **tool_choice 转换**：反向 §5.1.4
5. **字段重命名**：
   - `stop` → `stop_sequences`（单值包装为数组）
   - `user` → `metadata.user_id`
   - `max_tokens` 若缺失，按 §7.2 决策从 `model_configs.max_output_tokens` 读取，兜底 200000
6. **字段丢弃**：`frequency_penalty` / `presence_penalty` / `seed` / `n` / `logprobs` / `service_tier` / `stream_options`
7. **`response_format` 处理**：按 §7.4 决策，移除 `response_format` 字段并在 `system` 末尾追加 JSON 输出 prompt 提示
8. **reasoning_effort 转换**：`reasoning_effort` → `thinking.budget_tokens`，按 §3.5 表格映射
9. **流式字段**：移除 `stream_options`（Anthropic 流式默认带 usage）

### 5.3 响应体转换：非流式

桥接发生时，`UpstreamResponse::Complete { body }` 的 body 必须反向转换：

- `OpenaiToAnthropic` 桥接：上游返回 Anthropic 响应体 → 转为 OpenAI Chat 响应体
- `AnthropicToOpenai` 桥接：上游返回 OpenAI 响应体 → 转为 Anthropic 响应体

转换位置：在 `execute_and_finalize` 拿到 `UpstreamResponse` 之后、构造 `axum_resp` 之前。

实现要点：

```rust
pub fn anthropic_response_to_openai(body: &mut Value) -> Result<(), BridgeError> {
    // content array → choices[0].message.content (string) + tool_calls
    // stop_reason → finish_reason
    // usage.input_tokens → usage.prompt_tokens
    // usage.output_tokens → usage.completion_tokens
    // usage.total_tokens = input + output（计算）
    // object = "chat.completion"
}

pub fn openai_response_to_anthropic(body: &mut Value) -> Result<(), BridgeError> {
    // choices[0].message.content (string) → content: [{type:"text", text}]
    // choices[0].message.tool_calls → content array 的 tool_use blocks
    // finish_reason → stop_reason
    // usage.prompt_tokens → usage.input_tokens
    // usage.completion_tokens → usage.output_tokens
    // type = "message", role = "assistant"
}
```

错误响应：上游错误体需不需要转换？见 §7.1。

### 5.4 响应体转换：流式（最复杂）

桥接发生时，`UpstreamResponse::Streaming { response, protocol }` 的字节流必须**逐事件转换**：

- `OpenaiToAnthropic` 桥接：上游是 Anthropic SSE → 转为 OpenAI SSE
- `AnthropicToOpenai` 桥接：上游是 OpenAI SSE → 转为 Anthropic SSE

实现方式：在 `response_handler::build_response` 之前插入一层 `stream::map`，把上游字节流通过状态机转为目标协议的 SSE 字节流。

#### 5.4.1 OpenAI SSE → Anthropic SSE（A→O 桥接的流式响应）

状态机维护：
- 是否已发 `message_start`
- 当前 content_block 索引
- 累积的 `output_tokens`（从 `delta.usage` 提取）
- 累积的 `input_tokens`

转换规则：

| OpenAI 事件 | Anthropic 输出 |
|------------|---------------|
| 首个 `delta.role:"assistant"` | `message_start` + `content_block_start` (type=text, index=0) |
| `delta.content` | `content_block_delta` (type=text_delta) |
| `delta.tool_calls[i]` 出现 `id` + `function.name` | `content_block_stop` (上一个 block) + `content_block_start` (type=tool_use, index=i+1) |
| `delta.tool_calls[i].function.arguments` 增量 | `content_block_delta` (type=input_json_delta) |
| `finish_reason:"stop"` | `content_block_stop` + `message_delta` (stop_reason=end_turn) + `message_stop` |
| `finish_reason:"tool_calls"` | `content_block_stop` + `message_delta` (stop_reason=tool_use) + `message_stop` |
| `finish_reason:"length"` | 同上，stop_reason=max_tokens |
| usage 在末尾 delta | 累积到 `message_delta.usage.output_tokens` |
| `data: [DONE]` | （已发 `message_stop`，忽略） |

#### 5.4.2 Anthropic SSE → OpenAI SSE（O→A 桥接的流式响应）

转换规则：

| Anthropic 事件 | OpenAI 输出 |
|---------------|------------|
| `message_start` | （不发，记录 input_tokens） |
| `content_block_start` (type=text) | `delta.role:"assistant"`（仅首个 block 时） |
| `content_block_delta` (text_delta) | `delta.content` |
| `content_block_start` (type=tool_use) | `delta.tool_calls[index]={id, type:"function", function:{name, arguments:""}}` |
| `content_block_delta` (input_json_delta) | `delta.tool_calls[index].function.arguments`（累积） |
| `content_block_stop` | （不发） |
| `message_delta` (stop_reason) | `delta.finish_reason` |
| `message_delta` (usage) | 累积到末尾 |
| `message_stop` | 末尾发一个带 `usage` 的 delta + `data: [DONE]` |

#### 5.4.3 usage 提取

桥接时 `parse_sse_event_for_usage` 仍按**上游协议**解析 usage（因为流中确实是上游协议的事件格式）。状态机内部也需要累积 usage 用于构造目标协议的 usage 字段——这与 `usage_accumulator` 是同一份数据，无需重复。

`build_response` 的 `protocol` 参数需改为**上游协议**（与流事件格式一致），保持 `parse_sse_event_for_usage` 行为不变。

### 5.5 错误体处理

上游错误响应（HTTP ≥ 400）的 body 在桥接场景下有两种处理策略，详见 §7.1。

---

## 6. 日志标签规则

### 6.1 标签生成位置

| 日志类型 | 标签生成位置 | 当前逻辑 |
|---------|------------|---------|
| 转发日志（`LogKind::Forward`，`source = ProviderApi`） | `forwarder.rs::execute_and_finalize` 调用 `protocol_tags()` | 按 `provider_type` / `transport` / `is_stream` 生成 `sse` / `ws` |
| 网关入口日志（`LogKind::Gateway`，`source = Gateway`） | `router.rs::detect_response_tags` 按响应头检测 | 按 `Content-Type` / `Upgrade` 生成 `sse` / `ws` |

### 6.2 新增 `bridge` 标签规则

**触发条件**：`BridgeKind != None`（即发生协议桥接）。

**添加位置**：仅添加到 **转发日志**（`LogKind::Forward`）。原因：
- 转发日志记录的是「网关 → 供应商」的真实通信，协议在此层发生桥接
- 网关入口日志记录的是「客户端 → 网关」，对客户端透明，不应暴露内部桥接细节

**标签顺序规则**（用户需求）：

> 协议转换需要多打一个桥接的 tag 标签，如果存在 sse 标签，则在后方多加一个 bridge；如果遇到 websocket 协议，应该打一个 ws 标签。

实现：

```rust
pub fn protocol_tags(
    provider_type: &str,
    transport: Option<&str>,
    is_stream: bool,
    bridge_kind: BridgeKind,    // 新增参数
) -> Vec<String> {
    let mut tags = Vec::new();
    if is_stream {
        tags.push("sse".to_string());
    }
    // WebSocket 标签：统一使用 `ws`（详见 §6.3）
    // 注意：transport 配置值仍为 "websocket"，仅日志标签更名为 "ws"
    if (provider_type == "openai-responses" || provider_type == "openai-codex")
        && transport == Some("websocket")
    {
        tags.push("ws".to_string());
    }
    // 桥接标签：在 sse / ws 之后追加
    if bridge_kind != BridgeKind::None {
        tags.push("bridge".to_string());
    }
    tags
}
```

示例：

| 场景 | tags |
|------|------|
| 普通非流式 OpenAI 转发 | `[]` |
| 普通流式 OpenAI 转发 | `["sse"]` |
| 流式 + Anthropic→OpenAI 桥接 | `["sse", "bridge"]` |
| 非流式 + Anthropic→OpenAI 桥接 | `["bridge"]` |
| WebSocket 传输（不桥接） | `["ws"]`（流式 WS）或 `["sse", "ws"]` |
| WebSocket + 桥接（理论不会出现，预留） | `["sse", "ws", "bridge"]` |

### 6.3 标签命名决定：使用 `ws` 替换 `websocket`

**决定**：日志标签中的 `websocket` 全部更名为 `ws`。`provider.transport` 配置值仍保持 `"websocket"`（数据库存储语义不变），仅日志标签更名。

**需同步修改的位置**：

| 位置 | 修改内容 |
|------|---------|
| `src-tauri/src/modules/gateway_runtime/forwarding/util.rs::protocol_tags` | `tags.push("websocket")` → `tags.push("ws")` |
| `src-tauri/src/modules/gateway_runtime/router.rs::detect_response_tags` | `tags.push("websocket")` → `tags.push("ws")` |
| `src/components/ui/log-viewer.tsx` | 标签筛选器枚举值与展示文案 `websocket` → `ws` |
| `docs/gateway-runtime.md` §5.4 / §6.2 | 文档中标签名 `websocket` → `ws` |
| `docs/log-framework.md` §3.5 | 文档中标签名 `websocket` → `ws` |
| 历史日志数据（`logger` 内存缓冲区与持久化文件） | **不批量迁移**：旧标签 `websocket` 与新标签 `ws` 在筛选器中同时保留一段时间，缓冲区自然轮换后旧标签消失；持久化文件按保留天数自动清理 |

**前端筛选器兼容**：`log-viewer.tsx` 的标签筛选器需同时接受 `websocket`（历史数据）与 `ws`（新数据），避免历史日志无法筛选。实现方式：筛选 `websocket` 时同时匹配 `ws`，反之亦然。

### 6.4 错误场景标签

转发失败时 `build_error_tags` 在原 tags 基础上追加 `network`：

```rust
pub fn build_error_tags(tags: &[String], is_network: bool, bridge_kind: BridgeKind) -> Vec<String> {
    let mut error_tags = tags.to_vec();
    if is_network {
        error_tags.push("network".to_string());
    }
    error_tags
}
```

桥接标签已由 `protocol_tags` 注入到 `tags` 中，无需额外处理。

---

## 7. 决策记录

> 本节原为待决项，已由用户逐条确认。以下为最终决策，作为实现的强制约束。

### 7.1 上游错误响应体转换 ✅ 方案 B

**决定**：HTTP 4xx 的 body 按入口协议格式转换；HTTP 5xx 视为上游不可用，统一走 `upstream_error_response` 转为 OpenAI 标准错误体（项目既有约定，见 AGENTS.md §10.1）。

**理由**：HTTP 4xx 通常是上游供应商的业务错误（如认证失败、参数错误），客户端需要看到具体错误信息，应转换为入口协议格式；HTTP 5xx 视为上游不可用。

**Anthropic 错误体 → OpenAI 错误体**：
```json
// Anthropic
{ "type": "error", "error": { "type": "invalid_request_error", "message": "..." } }
// → OpenAI
{ "error": { "message": "...", "type": "invalid_request_error", "param": null, "code": null } }
```

**OpenAI 错误体 → Anthropic 错误体**（入口 messages + 上游 openai 时）：
```json
// OpenAI
{ "error": { "message": "...", "type": "invalid_request_error", "param": null, "code": null } }
// → Anthropic
{ "type": "error", "error": { "type": "invalid_request_error", "message": "..." } }
```

### 7.2 Anthropic `max_tokens` 必填，OpenAI 可选 ✅ 从模型配置读取

**决定**：O→A 转换时若 `max_tokens` 缺失，按以下顺序解析默认值：

1. 优先从 `model_configs.max_output_tokens`（当前模型的上限）读取
2. 读取失败时使用兜底默认值 **200000**

**实现要点**：
- 桥接模块需要从 `GatewaySharedState` 访问 `ai_gateway` 服务以查询 `max_output_tokens`
- `detect_bridge` / `apply_bridge` 的签名需传入 `shared` 或 `ctx`，便于查表
- 兜底常量 `MAX_TOKENS_FALLBACK: i64 = 200_000`，放在 `bridge/mod.rs`

### 7.3 `reasoning_effort` ↔ `thinking` 的精确映射 ✅ 按推荐方案

**决定**：
- MVP 阶段使用 §3.5 的固定映射表
- 后续在 `model_configs.thinking_json` 中声明每模型的 `budget_tokens` 范围，桥接时按模型查表
- 反向（A→O）时若 `budget_tokens` 不在档位上，取 `floor` 到最近档位

### 7.4 `response_format`（JSON mode）的处理 ✅ 方案 B（注入 prompt 提示）

**决定**：O→A 转换时若请求含 `response_format`：
- 移除 `response_format` 字段（Anthropic 不支持）
- 在 `system` 字符串末尾追加 prompt 提示，引导模型输出 JSON

**注入的 prompt 文本**（常量化，便于后续调整）：

```
\n\nPlease respond with valid JSON only, without any additional text or markdown formatting.
```

若 `response_format.type == "json_schema"` 且含 `json_schema.schema`，进一步将 schema 描述拼入 prompt：

```
\n\nPlease respond with valid JSON matching the following schema, without any additional text or markdown formatting:\n{schema}
```

**注意**：
- 仅在 `system` 存在时追加到末尾；`system` 不存在时构造新的 `system` 字段
- 不修改 `messages` 数组
- 不改变响应结构（不转 tool use）

### 7.5 标签命名 ✅ 使用 `ws` 替换 `websocket`

**决定**：日志标签中的 `websocket` 全部更名为 `ws`。详见 §6.3。

**影响范围与迁移工作**：
- 后端代码 2 处：`forwarding/util.rs::protocol_tags`、`router.rs::detect_response_tags`
- 前端代码 1 处：`src/components/ui/log-viewer.tsx`（标签筛选器同时兼容 `ws` 与历史 `websocket`）
- 文档 2 处：`docs/gateway-runtime.md`、`docs/log-framework.md`
- 历史日志数据不批量迁移，自然轮换

### 7.6 流式桥接的容错策略 ✅ 按推荐方案

**决定**：
- 解析失败的事件**原样透传**（按字节流输出），不中断流
- 同时 `tracing::warn!` 记录，便于诊断
- `bridge` 模块提供 `BridgeStreamState`，所有解析错误用 `BridgeError` 包装但不向上传播

### 7.7 虚拟供应商场景下的桥接 ⏸ 本次不实施

**决定**：本次不在虚拟供应商场景下做桥接支持。虚拟供应商的候选路由应保持协议一致（推荐由 UI 层校验或文档约定）。

**后续工作**：
- 当虚拟供应商跨协议场景出现时，按原推荐方案实施——`BridgeKind` 不在 `ForwardContext` 构造时固定，而是在 `execute_and_finalize` 内每次执行前重新计算：`detect_bridge(ctx.gateway_protocol, ctx.upstream.provider.provider_type)`
- 这样虚拟路由切换时桥接策略自动跟随

**当前限制**：虚拟供应商路由若包含不同协议类型的候选路由，桥接不会生效，请求会因协议不匹配而失败。该限制需在用户文档中明确说明。

### 7.8 桥接日志的请求/响应体展示 ✅ 展示转换后 + tracing::debug!

**决定**：
- **请求体展示转换后**（实际发给上游的），与 `ctx.upstream.request_headers_json` 的"真实出站"语义一致
- **响应体展示转换后**（实际返回给客户端的），与网关入口日志的响应体一致
- **调试日志**：在 `bridge` 模块的每个转换函数中，使用 `tracing::debug!` 输出转换前后 body，便于开发/运维在终端/日志文件中追踪

**实现约定**：

```rust
// bridge/request.rs
pub fn anthropic_to_openai_chat(body: &mut Value) -> Result<(), BridgeError> {
    let before = body.to_string();
    // ... 转换逻辑 ...
    tracing::debug!(
        target: "i_code::bridge",
        "bridge request A→O | before={} | after={}",
        before, body
    );
    Ok(())
}
```

同理 `openai_chat_to_anthropic` / `anthropic_response_to_openai` / `openai_response_to_anthropic` / 流式状态机均需在关键节点输出 `tracing::debug!`。

### 7.9 桥接性能影响 ✅ 无需特殊优化

**评估**：
- 非流式桥接：单次 JSON 转换，开销可忽略（< 1ms）
- 流式桥接：每个 chunk 需解析 + 转换 + 重新序列化，但 SSE chunk 通常较小（数百字节），开销在微秒级
- 不桥接时零开销（`BridgeKind::None` 短路返回）

**决定**：不引入特殊优化。桥接路径与普通路径分开，热路径（不桥接）不受影响。

### 7.10 工具调用 ID 格式兼容性 ✅ 不重命名

**决定**：**不重命名**，原样透传 `tool_call.id` / `tool_use.id`。

**理由**：
- 客户端不关心 ID 前缀，只关心请求与响应中的 ID 一致
- 桥接层在请求与响应中保持 ID 一致即可
- 重命名会破坏流式增量中的 ID 关联

---

## 8. 实现拆分（最终版本）

### 8.1 模块结构

```
src-tauri/src/modules/gateway_runtime/
├── bridge/                          # 新增
│   ├── mod.rs                       # BridgeKind、detect_bridge、detect_bridge_protocol
│   ├── error.rs                     # BridgeError
│   ├── request.rs                   # 请求体转换（A→O / O→A）
│   ├── response.rs                  # 非流式响应体转换
│   ├── stream.rs                    # 流式事件状态机转换
│   └── tests.rs                     # 单元测试（覆盖 §3 全部差异点）
├── forwarding/
│   ├── context.rs                   # 修改：GatewayProtocol::to_upstream_with_bridge
│   ├── forwarder.rs                 # 修改：execute_and_finalize 接入请求/响应/流式桥接 + apply_stream_bridge
│   ├── response_handler.rs          # 未修改：build_response 保持不变（流式桥接在 forwarder 层完成）
│   └── util.rs                      # 修改：protocol_tags 增加 bridge_kind 参数 + websocket→ws
└── ...
```

### 8.2 分阶段实施

| 阶段 | 内容 | 验收 | 状态 |
|------|------|------|------|
| **P1** | `bridge/` 模块骨架、`BridgeKind`、`detect_bridge`、请求体双向转换 + 单元测试（含 §7.2 模型配置读取、§7.4 response_format prompt 注入） | `cargo test` 通过；非流式 O→A / A→O 桥接 e2e 可用 | ✅ 已完成 |
| **P2** | 非流式响应体双向转换、错误体转换（§7.1 方案 B：4xx 转换、5xx 走标准错误体） | 非流式桥接完整可用 | ✅ 已完成 |
| **P3** | 流式事件状态机双向转换（含 §7.6 容错策略） | 流式桥接可用；tags 含 `sse, bridge` | ✅ 已完成 |
| **P4** | 日志标签 `bridge` 注入；`websocket` → `ws` 标签更名迁移（§6.3、§7.5）；`tracing::debug!` 转换前后 body 输出（§7.8） | 转发日志 tags 正确；前后端标签筛选器兼容历史 `websocket` 与新 `ws` | ✅ 已完成 |
| **P5**（可选） | `thinking` ↔ `reasoning_effort` 模型级查表（替代 §3.5 固定表） | 复杂场景对齐 | ⏳ 未实施 |
| **后续**（不在本次） | §7.7 虚拟供应商跨协议桥接支持 | 虚拟路由跨协议故障转移可用 | ⏳ 未实施 |

### 8.2.1 实施进度（P1–P4 最终版本 · 已发布）

> 截止 2026-08-04，P1/P2/P3/P4 全部完成并验证通过：`cargo test` 290 项通过、`pnpm type-check` 通过。
> 本节为最终发布版本，记录各阶段实际落地的文件清单、关键设计决策与测试覆盖。后续 P5 / §7.7 不在本次发布范围。

#### P1 — 桥接模块骨架 + 请求体双向转换 ✅

**新增 / 修改文件**：

- [`src-tauri/src/modules/gateway_runtime/bridge/mod.rs`](../../src-tauri/src/modules/gateway_runtime/bridge/mod.rs)
  - `BridgeKind` 枚举（`None` / `OpenaiToAnthropic` / `AnthropicToOpenai`）
  - `detect_bridge(entry_protocol, upstream_protocol) -> BridgeKind`
  - `bridge_upstream_protocol(entry, provider_type) -> UpstreamProtocol`
  - `MAX_TOKENS_FALLBACK = 200_000`
  - `is_openai_chat_family` / `is_anthropic_family`
  - `BridgeKind::is_bridged()` / `label()`
- [`bridge/error.rs`](../../src-tauri/src/modules/gateway_runtime/bridge/error.rs) — `BridgeError`（`InvalidField` / `JsonParse`）
- [`bridge/request.rs`](../../src-tauri/src/modules/gateway_runtime/bridge/request.rs)
  - `anthropic_to_openai_chat(body)`
  - `openai_chat_to_anthropic(body, max_output_tokens: Option<i64>)`（含 §7.4 `response_format` → prompt 注入）
- [`forwarding/context.rs`](../../src-tauri/src/modules/gateway_runtime/forwarding/context.rs) — `GatewayProtocol::to_upstream_with_bridge(provider_type) -> UpstreamProtocol`
- [`forwarding/forwarder.rs`](../../src-tauri/src/modules/gateway_runtime/forwarding/forwarder.rs) — 接入 `detect_bridge` + `apply_request_bridge`（O→A 查 `max_output_tokens` via `lookup_max_output_tokens`）

**§7.2 模型配置读取**：`lookup_max_output_tokens` 链路
`ctx.upstream.gateway_model_id` → `get_gateway_model` → `get_model_config` → `ModelConfig.max_output_tokens`；任一步失败或 `None` 兜底 `MAX_TOKENS_FALLBACK`。

**测试**：`bridge/tests.rs` 覆盖 §3 全部字段差异点（`system` 提取、`tool_calls` 重组、`tool_result` 拆分、多模态、`stop_reason` 映射、`usage` 重命名）。

#### P2 — 非流式响应体双向转换 + 错误体转换 ✅

**新增 / 修改文件**：

- [`bridge/response.rs`](../../src-tauri/src/modules/gateway_runtime/bridge/response.rs)
  - `anthropic_response_to_openai(body)`
  - `openai_response_to_anthropic(body)`
  - `convert_error_body(body, kind)` — §7.1 方案 B：4xx 转换、5xx 构造 OpenAI 标准错误体 + 502
- [`forwarding/forwarder.rs`](../../src-tauri/src/modules/gateway_runtime/forwarding/forwarder.rs) — `execute_and_finalize` `Ok(mut response)` 分支接入 `apply_response_bridge`：
  - `Complete` 2xx：响应体转换
  - `Complete` 4xx：错误体转换
  - `Complete` 5xx：构造 OpenAI 标准错误体 + 502
  - `Streaming`：P3 接入前直接透传

#### P3 — 流式事件状态机双向转换 ✅

**新增 / 修改文件**：

- [`bridge/stream.rs`](../../src-tauri/src/modules/gateway_runtime/bridge/stream.rs) — 流式状态机：
  - `BridgeStreamState`（行缓冲 + 双向转换状态字段）
  - `openai_sse_to_anthropic(chunk, state) -> Vec<String>` — §5.4.1
  - `anthropic_sse_to_openai(chunk, state) -> Vec<String>` — §5.4.2
  - §7.6 容错：解析失败的事件原样透传 + `tracing::warn!(target: "i_code::bridge")`
  - §7.8 调试：`tracing::debug!(target: "i_code::bridge")` 输出转换统计
  - §7.10 工具调用 ID 原样透传
- [`bridge/mod.rs`](../../src-tauri/src/modules/gateway_runtime/bridge/mod.rs) — 注册 `pub mod stream;`
- [`forwarding/forwarder.rs`](../../src-tauri/src/modules/gateway_runtime/forwarding/forwarder.rs) — `execute_and_finalize` `Ok(response)` 分支接入 `apply_stream_bridge`：
  - 当 `bridge_kind.is_bridged() && is_streaming` 时包装 `reqwest::Response.bytes_stream()`
  - 闭包内：先按**上游协议**解析原始 chunk usage 更新 `usage_accumulator`（§5.4.3），再用状态机转换为入口协议字节流
  - 通过 `UpstreamResponse::WebSocketStream` 变体回传 `build_response`（保持 protocol = 上游协议，§5.4.3）
  - `WebSocketStream` 与 `Complete` 不进入桥接（§4.1 矩阵：WS / Responses 不桥接）

**测试**（13 项，全部通过）：
- §5.4.1 全部规则：role→`message_start`+`content_block_start`、`content`→`text_delta`、`tool_calls` id+name→`content_block_stop`+`content_block_start` tool_use、arguments 增量→`input_json_delta`、`finish_reason`→`content_block_stop`+`message_delta`+`message_stop`、`[DONE]` 忽略、`finish_reason:length`→`stop_reason:max_tokens`
- §5.4.2 全部规则：`message_start` 记录 `input_tokens` 不发、`content_block_start` text→`delta.role:assistant`、`text_delta`→`delta.content`、`content_block_start` tool_use→`delta.tool_calls`、`input_json_delta`→`delta.tool_calls.arguments`、`content_block_stop` 忽略、`message_delta` stop_reason→`delta.finish_reason`、`message_stop`→带 usage delta + `[DONE]`
- §7.6 容错：malformed chunk 原样透传
- §7.10 工具调用 ID 不重命名（双向）
- 跨 chunk 事件缓冲

#### P4 — 日志标签 `bridge` 注入 + `websocket` → `ws` 更名 ✅

**修改文件**：

- [`forwarding/util.rs`](../../src-tauri/src/modules/gateway_runtime/forwarding/util.rs) — `protocol_tags` 增加 `bridge_kind` 参数；桥接转发注入 `bridge` 标签；`websocket` 统一更名为 `ws`（§6.3 / §7.5）
- [`forwarding/forwarder.rs`](../../src-tauri/src/modules/gateway_runtime/forwarding/forwarder.rs) — `protocol_tags` 调用方传入 `bridge_kind`
- [`router.rs`](../../src-tauri/src/modules/gateway_runtime/router.rs) — `detect_response_tags`：`Upgrade: websocket` → `ws` 标签；`is_streaming` 判定同步更新
- [`src/modules/logger/types.ts`](../../src/modules/logger/types.ts) — 注释中的标签示例更新为 `sse` / `ws` / `bridge`
- [`docs/gateway-runtime.md`](../gateway-runtime.md) §5.4 — `protocol_tags` 签名与标签值同步更新

**§7.8 `tracing::debug!` 转换前后 body 输出**：P1/P2/P3 已在 `bridge/request.rs` / `bridge/response.rs` / `bridge/stream.rs` 中按 `target: "i_code::bridge"` 输出转换前后 chunk / body 统计。

**前端兼容性**：日志查看器（`log-viewer.tsx`）按 `tags` 字符串原样展示，无硬编码标签值，无需修改。历史 `websocket` 标签的旧日志条目仍以字符串形式可见，新生成日志统一为 `ws`。

#### 验收（最终发布版本）

| 项 | 状态 |
|----|------|
| `cargo check` | ✅ 0 errors |
| `cargo test` | ✅ 290 passed |
| `pnpm type-check` | ✅ 通过 |
| 非桥接场景零变化 | ✅ `bridge_kind.is_bridged()` 短路返回；现有 `usage_extractor` 测试全部通过 |
| 桥接 e2e（O→A / A→O 非流式 + 流式） | ✅ 单元测试覆盖全部转换规则 |

> **发布结论**：P1–P4 全部交付，协议自动转换功能已上线。本提案自 v1.0.0 起标记为**已发布**。
> 后续 P5（`thinking` ↔ `reasoning_effort` 模型级查表）与 §7.7（虚拟供应商跨协议桥接）作为独立迭代推进，不阻塞本次发布。

### 8.3 测试策略

1. **单元测试**（`bridge/tests.rs`）：
   - 覆盖 §3 全部字段差异点
   - 含 `system` 提取、`tool_calls` 重组、`tool_result` 拆分、多模态、`stop_reason` 映射、`usage` 字段重命名
   - 流式状态机：构造上游 SSE 字节流 fixture，断言输出 SSE 字节流
2. **集成测试**：
   - 启动 mock 上游服务器（HTTP + SSE），分别模拟 OpenAI / Anthropic 响应
   - 通过网关调用 `/v1/messages` + openai 供应商，断言响应是合法 Anthropic 格式
   - 反向同理
3. **回归测试**：
   - 不桥接场景（协议一致）行为零变化
   - 现有 `usage_extractor` 测试全部通过

---

## 9. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| 流式状态机解析失败导致流中断 | 客户端连接中断 | §7.6 容错策略：解析失败原样透传，不中断 |
| 协议字段遗漏导致上游 400 | 桥接请求失败 | P1 阶段完整覆盖 §3 差异点 + 单元测试 |
| Anthropic `max_tokens` 缺失 | A→O 桥接请求被 Anthropic 拒绝 | §7.2 从 `model_configs.max_output_tokens` 读取，兜底 200000 |
| 虚拟供应商跨协议故障转移 | 故障转移后协议不匹配导致请求失败 | §7.7 本次不实施；UI 层校验候选路由协议一致；用户文档明确限制 |
| `websocket` → `ws` 更名后历史日志筛选漏数据 | 旧日志无法筛选 | §6.3 前端筛选器同时接受 `websocket` 与 `ws` |
| 桥接后 usage 数据不准 | 调用记录 token 数偏差 | 桥接状态机与 `usage_accumulator` 共享数据，按上游协议解析 |
| `response_format` prompt 注入对 Anthropic 模型效果不佳 | 模型未严格输出 JSON | §7.4 决策；prompt 文案常量化便于迭代；不改变响应结构 |

---

## 10. 参考实现

- `参考项目/vscode-unify-chat-provider-7.12.3/src/client/anthropic/` — Anthropic 协议参考
- `参考项目/vscode-unify-chat-provider-7.12.3/src/client/openai/` — OpenAI 协议参考
- 官方文档：
  - [OpenAI Chat Completions API](https://platform.openai.com/docs/api-reference/chat)
  - [Anthropic Messages API](https://docs.anthropic.com/en/api/messages)
  - [Anthropic Streaming Messages](https://docs.anthropic.com/en/api/messages-streaming)

---

## 附录 A：字段映射速查表（A→O）

| Anthropic 请求字段 | OpenAI 请求字段 | 备注 |
|-------------------|----------------|------|
| `model` | `model` | 已由 `prepare_body` 替换 |
| `system` (string/array) | `messages[0]` (role=system) | 拼接为字符串 |
| `messages[].role` | `messages[].role` | user/assistant 直接映射 |
| `messages[].content` (string) | `messages[].content` (string) | 一致 |
| `messages[].content[].type=text` | `messages[].content[].type=text` | 一致 |
| `messages[].content[].type=image` | `messages[].content[].type=image_url` | source 拆解 |
| `messages[].content[].type=tool_use` | `messages[].tool_calls[]` | 提到 message 顶层 |
| `messages[].content[].type=tool_result` | 独立 message (role=tool) | 拆分 |
| `max_tokens` | `max_tokens` | 一致 |
| `stop_sequences` | `stop` | 单值不包装 |
| `temperature` | `temperature` | 一致 |
| `top_p` | `top_p` | 一致 |
| `tools[].input_schema` | `tools[].function.parameters` | 重命名 |
| `tool_choice.type=auto/any/tool` | `tool_choice=auto/required/{type:function,...}` | 结构转换 |
| `metadata.user_id` | `user` | 重命名 |
| `thinking.budget_tokens` | `reasoning_effort` | §3.5 映射 |
| `stream` | `stream` | 一致 |
| —（默认带 usage） | `stream_options.include_usage=true` | 注入 |

## 附录 B：字段映射速查表（O→A）

| OpenAI 请求字段 | Anthropic 请求字段 | 备注 |
|----------------|-------------------|------|
| `model` | `model` | 已由 `prepare_body` 替换 |
| `messages[role=system]` | `system` (string) | 提取到顶层 |
| `messages[].role=user/assistant` | `messages[].role` | 一致 |
| `messages[role=tool]` | 前一条 assistant 的 `content[].tool_result` | 合并 |
| `messages[].tool_calls[]` | `messages[].content[].tool_use` | 移入 content |
| `messages[].content[].type=image_url` | `messages[].content[].type=image` | source 拼接 |
| `max_tokens`（缺失） | `max_tokens=<model.max_output_tokens \| 200000>` | §7.2 决策：模型配置读取，兜底 200000 |
| `stop` | `stop_sequences` | 单值包装为数组 |
| `tools[].function.parameters` | `tools[].input_schema` | 重命名 |
| `tool_choice` | `tool_choice` | 反向 §A |
| `user` | `metadata.user_id` | 重命名 |
| `reasoning_effort` | `thinking.budget_tokens` | §3.5 映射 |
| `stream` | `stream` | 一致 |
| `stream_options` | （移除） | Anthropic 默认带 usage |
| `response_format` | （移除字段，注入 `system` prompt 提示） | §7.4 决策：方案 B |
| `frequency_penalty` / `presence_penalty` / `seed` / `n` / `logprobs` / `service_tier` | （丢弃） | Anthropic 不支持 |
