# MCP Server：对外暴露生图 / 识图能力方案

> 状态：**提案**（待评审）
> 日期：2026-09-02（识图工具扩展：2026-09-03）
> 关联模块：`media-generation`、`gateway-runtime`、新增 `mcp-server`、`ai-gateway`（网关设置扩展）
> 承接文档：`docs/proposals/media-generation.md` §3.7 / §5 Phase 6
> 配套依赖：`rmp-serde`（MessagePack，`mcp-spec` 传输编码）

---

## 1. 背景与目标

### 1.1 背景

视觉生成页（`/vision`）已完成 Phase 3 供应商直连能力：工作台/画廊 UI、图像生成服务（`media_generate_image`）、产物本地化存储、生成历史与调用统计均已落地。按 `media-generation.md` §5 演进路径，Phase 6 是**以本软件作为 MCP Server**，把 `media-generation` 的生图能力以标准 MCP 工具形式暴露给第三方 MCP Client（Claude Code、Codex 等），与应用内 UI 直连形成**并列通道**；并在此之上扩展**识图工具**（`analyze_image`）：走本地网关聊天链路调用用户手动关联的多模态模型，让第三方 agent（其宿主模型可能不支持视觉输入）也能借助本软件完成图片理解。

MCP（Model Context Protocol）2025-03-26 标准化的 **Streamable HTTP 传输** 已获主流客户端支持（Claude Code `--transport http`、`claude mcp add --transport http`、Codex、Cursor 等），其核心要求：

| 要求 | 说明 |
|------|------|
| 传输层 | 单端点 `POST`，支持 **HTTP + SSE 双模式**（初始化握手后的客户端常为流式） |
| 握手 | `initialize` → 服务端返回 capabilities / instructions / serverInfo → 客户端 `notifications/initialized` |
| 消息帧 | **MessagePack 二进制**（`application/json` 为兼容兜底，仅用于 HTTP 单响应模式；SSE 帧内嵌 msgpack） |
| 会话 | 需要 persistent session：`Mcp-Session-Id` 请求头 + 服务端内存会话表（含 SSE 队列） |
| 端点发现 | `GET /mcp` 返回 SSE 响应 + 推送 server instructions；`POST /mcp` 收发 JSON-RPC 消息 |

### 1.2 目标

1. **零新端口**：MCP 端点挂载在本地网关 axum 进程（`POST /mcp` + `GET /mcp`），复用 `127.0.0.1:54321` 与网关认证；网关关闭时 MCP 不可用（与网关生命周期绑定）。
2. **最小协议面**：仅暴露 `generate_image`（生图）+ `analyze_image`（识图）两个工具 + `tools/list` + `tools/call` + `initialize`（含 `resources` / `prompts` 空 capabilities）——**不实现** 采样（sampling）、roots、资源/提示词读取，避免协议面膨胀。
3. **产物可见性**：生图工具返回**本地绝对路径**与元信息（尺寸、耗时、模型）；第三方客户端运行在用户本机，可直接读取/展示。识图工具返回模型对图片的**文字回答**。
4. **识图模型手动关联**：识图走聊天链路（多模态 chat 模型），**不自动选模型**——由用户在设置页从「支持图片输入（`capabilities.imageInput`）且已暴露」的模型中**手动关联**若干个到 MCP 识图工具，第一个为默认；未关联时识图工具不下发。
5. **可观测与合规**：MCP 触发的调用同写两套日志（自研 logger 可见 + tauri-plugin-log 完整体）与 `call-records` 统计，且统一打 **`mcp` 标识**（日志 tag + 调用记录 `source="mcp"`），日志页可按来源筛选审计；产物不入备份、不入导出（沿用 media-generation §6 风险约定）。
6. **前端可管理**：设置页新增「MCP 服务」卡片：开关、只读展示端点地址 / 工具清单 / 会话数、识图模型关联管理、会话清理、等价 `claude mcp add` 命令一键复制。

---

## 2. 方案对比

| 方案 | 描述 | 判定 |
|------|------|------|
| **A. 自研极简实现** | 按 `mcp-spec` 手写：axum 新路由 + MessagePack 编解码（`rmp-serde`）+ JSON-RPC 分发 + SSE 会话表 | ✅ **推荐**：依赖极轻（仅新增 1-2 个 crate）、完全掌控协议面、无 crate 版本爆炸风险；SSE 传输取 2025-03-26 标准子集 |
| **B. 引入 `rmcp`（官方 Rust SDK）** | 官方 SDK 提供 transport / session / server 抽象 | 引入 `rmcp` + `tokio-util` + `tokio-serde` + `rmp-serde` + serde_json 等 10+ 依赖树；axum 0.7 与其 transport 集成需适配层（官方示例多为 axum 0.8），且 Session 生命周期/SSE 队列与现有 `GatewaySharedState` 体系需桥接，收益有限 |
| **C. 独立进程/端口** | 另起一个 axum Server 监听新端口（如 54322） | 违背「端口复用」设计目标，增加防火墙/端口占用/管理复杂度；仍须解决认证与状态共享 |
| **D. 仅 CLI 侧包装** | 不实现 HTTP 端点，改为生成 `claude mcp add --transport stdio` 的本地脚本 | stdio 传输要求子进程存活且需打包可执行脚本，Windows 下体验差；协议面无法复用网关认证；不属于 MCP Server 语义 |

> 结论：选 **A（自研极简实现）**。核心是「按规范正确实现 Streamable HTTP 子集」，协议面仅 `tools/list` + `tools/call` 两个业务工具方法 + 握手，工作量集中在传输层（msgpack + SSE + 会话），与现有 axum / tokio / 日志体系天然契合。

---

## 3. 模块设计：新增 `mcp-server` 模块

### 3.1 目录与分层（遵循 §3.3 后端分层）

```
src-tauri/src/modules/mcp_server/
├── mod.rs            # 模块声明 + 文档注释
├── types.rs          # MCP JSON-RPC 消息 / 握手 / 工具 DTO（camelCase，ts-rs 可选）
├── protocol.rs       # MessagePack 编解码 + JSON-RPC 分发（initialize / tools/list / tools/call / ping / notifications）
├── tools.rs          # 工具注册表：generate_image / analyze_image 的 schema、参数校验与分发（新工具在此登记）
├── session.rs        # 会话表：Mcp-Session-Id ↔ { SSE 客户端队列, 状态 }
├── sse.rs            # SSE 连接管理（GET /mcp 挂载、消息推送、断线清理、心跳）
├── router.rs         # axum 路由注册（POST/GET /mcp）+ 供 gateway_runtime 挂载
├── service.rs        # 业务编排：生图 → media_generation::MediaGenerationService；识图 → 构造多模态请求体 + ForwardPipeline::run → 双写日志 / call-records（source/logger_tags 打标）
├── commands.rs       # Tauri Command：mcp_config_get / mcp_config_update / mcp_sessions_list / mcp_session_kill / mcp_tools_snapshot / mcp_tool_models_get / mcp_tool_models_update
└── guards.rs         # 认证复用：从 gateway auth 提取 RequestApiKey → MCP 调用上下文
```

### 3.2 依赖（Cargo.toml 新增）

```toml
# MCP Server：Streamable HTTP 传输的 MessagePack 编解码（2025-03-26 规范）
rmp-serde = "1"
```

说明：`bytes` / `http` / `axum` / `futures` / `tokio` 均为已存在依赖，直接复用；不引入 `mcp-spec` / `rmcp`。若后续需要工具入参 JSON Schema 推导，可再评估 `schemars`（当前手工构造 schema 即可，体积小）。

### 3.3 路由注册（挂载点）

在 [gateway_runtime/router.rs](file:///d:/ProjectApp/i-code/src-tauri/src/modules/gateway_runtime/router.rs#L54-L79) 的 `build_router` 中追加（受 `mcp_enabled` 开关控制；默认关闭）：

```rust
let router = Router::new()
    .route("/health", get(health))
    .route("/readyz", get(readyz))
    .route("/v1/models", get(list_models))
    .route("/v1/chat/completions", post(chat_completions))
    .route("/v1/responses", post(responses))
    .route("/v1/messages", post(anthropic_messages))
    // MCP Server：Streamable HTTP（双注册；路由实际由 mcp::router::build_mcp_router 提供）
    .merge(mcp_server::router::build_mcp_router(shared.clone()))
    .with_state(shared)
    .layer(...); // 认证中间件：/mcp 自动纳管（不在 EXEMPT_PATHS 中）
```

要点：

- `mcp_enabled = false` 时 **不注册** `/mcp` 路由 → 端点完全不可达（404），避免「组件存在但关闭」的模糊状态。
- 认证：`EXEMPT_PATHS` 仅含 `/health`、`/readyz`，因此 `/mcp` 自动进入 [auth_middleware](file:///d:/ProjectApp/i-code/src-tauri/src/modules/gateway_runtime/auth.rs#L59-L74) 校验流程（开放模式 `auth_enabled=false` 时放行），认证通过后 `RequestApiKey` 注入请求扩展，MCP handler 读取并写入调用记录。
- 路由响应 `Content-Type: text/event-stream` 与 `Cache-Control: no-cache`（对齐 §10.1 SSE 硬约束）。

---

## 4. 传输层设计（Streamable HTTP 子集）

### 4.1 消息格式

| 场景 | Content-Type | 编码 |
|------|--------------|------|
| 客户端 → 服务（POST 请求体） | `application/json` **或** `application/msgpack` | 握手后客户端未来消息优先 msgpack；服务端两态都接受 |
| 服务 → 客户端（HTTP 单响应） | 与请求 Content-Type 对齐 | 与请求一致（`Accept` 为 `application/json, text/event-stream` 时走 SSE） |
| 服务 → 客户端（SSE） | `text/event-stream` | **每帧 `data:` 内嵌 msgpack 字节**（非 JSON） |

消息结构（JSON-RPC 2.0 子集）：

```jsonc
// 服务端 → 客户端（SSE 帧负载）
{
  "jsonrpc": "2.0",
  "method": "notifications/message",   // | responses/{id} | notifications/tools/list_changed
  "params": {
    "sessionId": "…",                   // 仅 initialize 响应后带回
    "message": { /* 嵌套 JSON-RPC 消息 */ }
  },
  "id": "…"                              // 仅响应帧存在
}
```

### 4.2 会话生命周期

| 环节 | 实现 |
|------|------|
| 会话创建 | `POST /mcp` 收到 `initialize` → 生成 `Mcp-Session-Id`（`core::id::generate_id()` 雪花 ID）→ 登记会话表 → `initialize` 响应带 `Mcp-Session-Id` 响应头 |
| 会话关联 | 后续请求必须携带 `Mcp-Session-Id` 头；缺失 / 未知 → 400 `{"jsonrpc":"2.0","error":{"code":-32001,"message":"Session not found"}}` |
| 信道绑定 | 会话表记录该会话的 SSE 客户端队列（`tokio::sync::mpsc`）与协议偏好（`Accept`/`Content-Type`）；SSE 连接断开 → 标记 `sse_disconnected`，会话保留存活（服务端可推送给同一会话内的其他连接）；再建立新 SSE 连接时复用会话 |
| 超时回收 | 会话空闲超过 30 分钟自动清理（定时任务 / 惰性检查），前端「会话列表」可手动 kill |
| 关闭推送 | `notifications/initialized` 后，若网关 `stop()` 或应用退出，向所有会话推送 `notifications/closed` 后销毁 |

### 4.3 SSE 连接（GET /mcp）

- 响应头：`Content-Type: text/event-stream`；`Cache-Control: no-cache`；`Connection: keep-alive`。
- 连接建立后立即推送一条 `notification`（`server instructions`：说明端点、认证、工具清单获取方式）。
- 后续服务端消息（`notification` / `response`）经会话队列推送；每帧格式：`event: message\ndata: <msgpack bytes>\n\n`。
- 心跳：每 30s 空注释帧 `: ping`；客户端断线（`Connection: close` / TCP 断开）→ 清理队列、标记 `sse_disconnected`。
- **不做** `Mcp-Session-Id` 的 SSE 事件分片（每帧独立完整 msgpack，避免客户端解析负担）。

### 4.4 双模式响应策略

| 客户端 `Accept` | 行为 |
|----------------|------|
| 仅 `application/json` | 返回**非流式** JSON 单响应（`Content-Type: application/json`） |
| `text/event-stream`（或 `application/json, text/event-stream`） | 返回 **SSE 单响应**（`Content-Type: text/event-stream`，先 `event: message` 帧 + `end` 帧），并持续推送后续通知 |
| `application/msgpack`（单响应场景） | 返回 msgpack 编码的 JSON-RPC 响应 |

> 兼容策略：**优先按 `Accept` 协商，缺省回落 SSE**（主流 MCP Client 均声明 `text/event-stream`）。codec 统一封装 `encode_message` / `decode_message`，内部根据会话协议偏好选择 msgpack / json。

---

## 5. 工具设计（生图 + 识图）

### 5.1 工具清单

| 工具名 | 用途 | 描述 |
|--------|------|------|
| `generate_image` | 文生图 | 参数：`prompt`（必填）、`model`（选填，缺省用默认视觉生成模型）、`size`（选填，缺省供应商默认）、`n`（选填，1-4）、`watermark`（选填 bool）；返回：本地产物**绝对路径列表** + 元信息 |
| `analyze_image` | 识图（图片理解） | 参数：`image`（必填）、`question`（必填）、`model`（选填，缺省用**关联列表第一个**识图模型，仅可取已关联模型）、`detail`（选填，`auto`/`low`/`high`）；返回：模型对图片的**文字回答**。走聊天链路（多模态 chat 模型），调用方为第三方 agent 的场景如「看一眼这张截图里报错是什么」。 |

> 生图与识图的模型选择策略不同：生图沿用「首个启用且 `is_media_generation`」的**自动兜底**（该类供应商本就由视觉生成预设专用，无歧义）；识图涉及用户全部聊天模型，必须**手动关联**（§5.5），避免把用户聊天流量误路由到未预期的模型。

### 5.2 入参 Schema（`inputSchema`，JSON Schema Draft 2020-12 子集）

**`generate_image`**：

```jsonc
{
  "type": "object",
  "properties": {
    "prompt":     { "type": "string",  "description": "图像描述文本" },
    "model":      { "type": "string",  "description": "模型 ID，缺省使用默认视觉生成模型" },
    "size":       { "type": "string",  "description": "图像尺寸，如 2752x1536" },
    "n":          { "type": "integer", "minimum": 1, "maximum": 4, "description": "生成数量" },
    "watermark":  { "type": "boolean", "description": "是否添加水印" }
  },
  "required": ["prompt"]
}
```

**`analyze_image`**（`model` 的 `enum` 由关联列表动态生成，见 §5.5）：

```jsonc
{
  "type": "object",
  "properties": {
    "image":    { "type": "string",  "description": "图片来源：http(s) URL / data URL（base64）/ 本地绝对路径（限图片扩展名）" },
    "question": { "type": "string",  "description": "对图片的提问或处理指令，如「描述这张图」「提取图中的表格」" },
    "model":    { "type": "string",  "enum": ["<provider_slug>/<model_id>", "…"], "description": "识图模型（网关路由 ID），缺省用关联列表第一个" },
    "detail":   { "type": "string",  "enum": ["auto", "low", "high"], "description": "图像识别精度（OpenAI vision 约定；非 OpenAI 协议供应商忽略）" }
  },
  "required": ["image", "question"]
}
```

### 5.3 调用链路

**生图链路**（复用 media-generation 直连实现）：

```
MCP Client
  → POST /mcp (tools/call{name:"generate_image", arguments})
  → mcp_server::tools::handle_generate_image
      ├─ 校验 arguments（必填 prompt；n 范围 1-4；其他委托 GenerateImageInput）
      ├─ 解析默认视觉生成模型：ai_gateway 查首个启用且 is_media_generation 的供应商/模型
      │    （无可用视觉供应商 → IcodeError::validation("未配置可用的视觉生成供应商")）
      ├─ MediaGenerationService::generate_image(ai_gateway, input, source="mcp")
      │    // generate_image 增加 source 参数：UI 直连传 "internal"，MCP 传 "mcp"（call-records 来源区分）
      │    （产物本地化、失败历史、token 留空等沿用现有实现）
      ├─ 双写日志（自研 logger 条目附 tags=["mcp"]；tauri-plugin-log 完整参数 + 结果路径）
      └─ 返回 {
            content: [ { type:"text", text: JSON 字符串 } ],
            structuredContent: {
              assetPaths: [绝对路径…], modelId, providerSlug,
              size, n, watermark, durationMs, createdAt
            },
            isError: false
          }
```

**识图链路**（进程内复用网关转发管道，不经 HTTP 自调用）：

```
MCP Client
  → POST /mcp (tools/call{name:"analyze_image", arguments})
  → mcp_server::tools::handle_analyze_image
      ├─ 校验 arguments（image / question 必填；model 必须命中关联列表，否则
      │    IcodeError::validation("模型未关联到 MCP 识图工具")；image 大小/扩展名校验 §6）
      ├─ 解析模型路由 ID → resolve_route(shared, "provider_slug/model_id", ChatCompletions)
      ├─ 构造 OpenAI ChatCompletions 多模态请求体（stream=false）：
      │    messages: [{ role:"user", content: [
      │        { type:"image_url", image_url:{ url, detail? } },
      │        { type:"text", text: question } ] }],
      │    max_tokens: 4096（防上游无限输出）
      ├─ ForwardPipeline::run(shared, ForwardRequest{ source:"mcp", logger_tags:["mcp"], … })
      │    // 复用网关既有的：协议桥接（ChatCompletions→Anthropic 的 image_url→image 转换已实现）、
      │    // 重试、去敏请求头快照、两套日志、call-records（source 透传为 "mcp"）
      ├─ 解析非流式 JSON 响应 choices[0].message.content（拒答/空 content → 视为失败）
      └─ 返回 {
            content: [ { type:"text", text: 模型回答 } ],
            structuredContent: { modelId, providerSlug, durationMs,
                                 usage: { promptTokens, completionTokens, totalTokens } },
            isError: false
          }
```

> 识图响应的 token 用量直接来自网关 usage 拦截结果，`call-records` 照常记录——识图与普通聊天一样**有 token 成本**，统计口径完全一致，仅 `source="mcp"` 区分。

### 5.4 失败语义

- 上游失败 / 校验失败 → `isError: true` + `content[0].text` 承载 `IcodeError.message`（不暴露堆栈 / SQL / Secret）；**protocol 层**返回 `-32603`（内部错误）或 `-32000`（服务器错误）。
- `tools/call` 未知工具名 → `-32602`（Invalid params：`Unknown tool`）。
- 识图特有：`model` 未关联 → `-32602`（Invalid params）；`image` 读取失败（路径不存在 / 扩展名不支持 / 超 20MB）→ `-32602`；上游拒绝图片输入（供应商不支持多模态）→ `-32603` 并把上游错误摘要回传。
- 方法论一致性：**错误体与 `errorMessage` 不含明文 Secret**（复用 `IcodeError.message` 规范，§12.2）。

### 5.5 识图模型关联（手动关联多模态模型）

识图走聊天链路，模型范围由用户**手动圈定**，不做自动推断：

| 项 | 设计 |
|----|------|
| 候选池 | 网关模型列表中 `is_exposed=true` 且 `capabilities_json.imageInput=true` 的模型（路由 ID = `{provider_slug}/{model_id}`）；`imageInput` 标记来自内置模型预设与用户编辑，候选池为「声明支持图片输入」的模型，不猜测实际能力 |
| 关联存储 | 新表 `mcp_tool_models`（§8，`tool_name="analyze_image"` 维度，按 `sort_order` 排序；通用设计为 M3 视频工具预留） |
| 默认模型 | `sort_order` 最小者；MCP `model` 参数缺省时使用，设置页可上移/下移调整 |
| `tools/list` 动态化 | **未关联任何模型 → `analyze_image` 不出现在 `tools/list`**（避免客户端调用必然失败）；关联后 `model` 参数 `enum` = 关联列表 + `default` = 第一项 |
| 关联变更推送 | `mcp_tool_models_update` 成功后向所有在线会话推送 `notifications/tools/list_changed`，支持动态刷新的客户端即时感知（不支持的客户端下次会话生效） |
| Command | `mcp_tool_models_get()` → 关联列表（含候选池）；`mcp_tool_models_update({ models: ["slug/model", …] })` 全量替换（事务覆盖删除+插入） |
| 一致性兜底 | 调用时若 `model` 指向已失效模型（供应商禁用 / 模型删除），返回 `IcodeError::validation` 并在错误信息中提示重新关联；tools/call 不做静默降级，避免用户困惑 |

---

## 6. 认证与安全

| 面 | 设计 |
|----|------|
| 认证 | 复用网关认证中间件：外部 Client 需 `Authorization: Bearer {gateway_key}` 或 `X-API-Key`；`gateway_auth_keys` 反查 / 默认 Key 回退逻辑原样生效；`auth_enabled=false` 时为开放模式（与网关其余端点一致）。**不提供** MCP 专属免认证模式 |
| 会话劫持 | `Mcp-Session-Id` 仅作为会话标识（非凭证）；鉴权每请求重放（不走「首次认证后免鉴」） |
| 路径安全 | 工具返回的产物路径为 `asset_store::absolute_path` 结果，仅暴露 `media/` 目录内文件；MCP 不提供任意文件读取工具 |
| 识图图片输入 | `analyze_image.image` 支持三种形态：http(s) URL（原样透传给上游，服务端不下载）、data URL（base64，服务端原样透传）、本地绝对路径（**服务端读取**，仅允许 `.png/.jpg/.jpeg/.webp/.gif/.bmp` 扩展名 + 上限 20MB，读取后转 data URL）。不支持相对路径与目录，避免任意文件读取面扩大 |
| 识图上下文成本 | 图片按上游 token 计价，`max_tokens` 固定 4096 防止失控输出；大图成本在 call-records 照常计入，用户可在调用统计页审计 |
| 资源约束 | 单会话队列容量 64（背压饱和丢弃 + 日志告警）；`tools/call` 并发经 semaphore 限流（默认 2），防生图刷爆上游配额 |
| 审计 | MCP 调用同写 `call-records`（`source="mcp"`）与两套日志，供日志页筛选与审计 |
| 数据合规 | 产物不随备份 / WebDAV 导出（沿用 media-generation §6 约定；生成历史仍只存相对路径） |

---

## 7. 前端（设置页「MCP 服务」卡片）

### 7.1 入口与布局

- 位置：设置页（[settings.tsx](file:///d:/ProjectApp/i-code/src/routes/settings.tsx)）新增「MCP 服务」Card（`fa-plug` 图标），置于「本地网络」之后；滚动布局沿用 `ScrollPage` + 内容区高度实测（避免溢出窗口）。
- 内容区设计（900×700 紧凑）：

```
┌─ MCP 服务 ────────────────────────────────────────┐
│ [开关] 启用 MCP Server（供外部 AI 客户端调用生图/识图）│
│ 状态：未运行（网关未启动） | 运行中 · 端点地址        │
│ 端点：http://127.0.0.1:54321/mcp         [复制]    │
│ 工具：generate_image · analyze_image（2 个）        │
│ 会话：2 活动 · 最近活动 12:03                [清理]  │
│ ────────────────────────────────────────────────  │
│ 识图模型关联（第一个为默认）：                        │
│ [✔ provider-a/gpt-4.1] [✔ provider-b/claude-...]   │
│ [✘ volcano/doubao-vision]          [+ 添加模型]     │
│ （候选池 = 支持图片输入且已暴露的模型；未关联时        │
│   analyze_image 工具不下发，客户端看不到）            │
│ ────────────────────────────────────────────────  │
│ 接入命令（复制到终端）：                             │
│ claude mcp add --transport http i-code-media        │
│   http://127.0.0.1:54321/mcp                        │
│   --header "Authorization: Bearer {gateway_key}"    │
│                                    [复制命令] [查看 Key]│
└────────────────────────────────────────────────────┘
```

识图模型关联区块交互：弹层（Dialog）内多选候选池模型（搜索 + 按供应商分组），已选列表支持上移/下移（第一项即默认），保存调用 `mcp_tool_models_update`；候选池为空时展示引导文案「暂无支持图片输入的已暴露模型，前往模型管理开启 `imageInput` 或添加多模态模型」。

### 7.2 交互与数据流

| 元素 | 行为 | 数据来源 |
|------|------|---------|
| 开关 | `mcp_config_update({ enabled })` 持久化；重启网关后生效（前端提示「重启网关后生效」） | `mcp_config_get` |
| 端点地址 | 由 `gateway_settings.gateway_host/port` 拼接展示（**运行时只读**，不新增存储字段） | `gateway_settings_get` + `mcp_config_get` |
| 识图模型关联 | 弹层多选候选池 + 排序（§7.1）；保存即全量替换并触发 `notifications/tools/list_changed`；默认模型打徽章 | `mcp_tool_models_get` / `mcp_tool_models_update` |
| 会话列表 | `mcp_sessions_list`：会话 ID 短码、会话创建时间、最后活动时间、是否 SSE 在线；支持单条 kill | `mcp_session_kill` |
| 接入命令 | 展示 `claude mcp add --transport http` 模板；`{gateway_key}` 经 `gateway_resolve_default_key`（**仅按需在后端解析，前端不回显明文**，点击「查看 Key」时才临时显示并 3s 后自动隐藏） | `gateway_resolve_default_key` |
| 工具清单 | `mcp_tools_snapshot`：只读展示工具名与参数摘要（`analyze_image` 标注「未关联模型，不下发」状态） | 后端静态 schema |
| 空视觉供应商引导 | 无可用视觉生成供应商时展示引导文案「前往供应商页添加视觉生成预设」（`/gateways/providers`） | `provider:changed` 事件联动 |
| 识图候选池联动 | 模型能力 / 暴露状态变更（`model:changed`、`provider:changed`）后刷新候选池；已关联模型失效时在卡片上标红提示 | 事件订阅 |

### 7.3 i18n 键规划（zh-CN / en / ja / zh-TW）

`settings.mcp.*`：`title` / `enableLabel` / `enableDescription` / `statusRunning` / `statusStopped` / `endpointLabel` / `toolsLabel` / `sessionsLabel` / `cleanup` / `connectCommand` / `copyCommand` / `viewKey` / `keyHint` / `needGateway` / `restartToApply` / `noMediaProvider` 等；识图关联区块另需 `visionModelsTitle` / `visionModelsHint` / `visionModelsEmpty` / `visionDefaultBadge` / `addModel` / `saveAssociation` 等。按 §6.4「用户可见文案必须 i18n + `模块.页面.元素`」执行。

---

## 8. 开关持久化与数据库

**单一迁移 `V011__mcp_server.sql`**：新增 `mcp_enabled` 列 + 识图模型关联表（「同一迭代合并迁移」，不碎片化）：

| 项 | 方案 |
|----|------|
| MCP 开关 | `gateway_settings` 新增 `mcp_enabled` INTEGER 列（默认 0） |
| 识图模型关联 | 新表 `mcp_tool_models`：`id TEXT PK`、`tool_name TEXT NOT NULL`（当前仅 `analyze_image`，通用设计为 M3 视频工具预留）、`gateway_model_id TEXT NOT NULL`（网关路由 ID `{provider_slug}/{model_id}`）、`sort_order INTEGER NOT NULL DEFAULT 0`（0 为默认模型，升序）、`created_at TEXT NOT NULL`、`updated_at TEXT NOT NULL`；`UNIQUE(tool_name, gateway_model_id)` 防重复关联 |
| 迁移注册 | §6.3 三步注册：`migrations.rs` 常量 + `BUILTIN_MIGRATIONS` 追加 `(11, "mcp_server", …)` + `SCHEMA_VERSION` → 11 |
| DTO | `GatewaySettings` / `UpdateGatewaySettingsInput`（[types.rs](file:///d:/ProjectApp/i-code/src-tauri/src/modules/ai_gateway/types.rs#L1228-L1253)）新增 `mcp_enabled: bool`；`GATEWAY_SETTINGS_SELECT_SQL` 与 `gateway_settings_row_mapper`（[repository.rs](file:///d:/ProjectApp/i-code/src-tauri/src/modules/ai_gateway/repository.rs#L1303-L1327)）同步扩展；`mcp_tool_models` 的 DTO 放 `mcp_server/types.rs`，前端 `modules/mcp-server/types.ts` 手工同步（§12.4） |
| 生效时机 | 网关 `start()` 构建 Router 时读取 `mcp_enabled`；开关变更后**需重启网关**生效（前端提示）。识图模型关联**即时生效**（tools/list 动态组装 + `tools/list_changed` 推送），无需重启 |

> 迁移文件与 schema 注册遵循 AGENTS.md §6.3「3 步注册」硬约束。

---

## 9. 可观测性（两套日志 + 统计，统一 `mcp` 标识）

MCP 触发的所有调用（生图 / 识图）在日志与统计中**统一打 `mcp` 标识**，日志页 / 调用统计页可按来源筛选审计：

| 观测面 | 实现 |
|--------|------|
| 自研 logger（日志页可见） | MCP 调用事件：`MCP tools/call generate_image / analyze_image 开始/完成(耗时ms)/失败(原因)`，条目附 **`tags=["mcp"]`**（请求头去敏 JSON 对齐 §3.5 去敏规则）。`LogEntry.tags` 字段与 DB 存储 / 导出均已支持，需新增：① logger 模块带 tag 写入接口（如 `Log::info_with_tags(&["mcp"], msg)`，现有 `Log::info` 保持不变）；② 网关日志记录器（`LogRecordBuilder`）支持外部注入 tags |
| tauri-plugin-log（终端/文件） | 完整参数、上游请求/响应概要、会话生命周期（创建 / 断线 / 清理）、SSE 推送异常；级别 `info`/`warn`/`error`（此通道按 log-framework §2 约定不打业务 tag，靠消息前缀 `MCP` 区分） |
| call-records | `source` 字段新增取值 **`"mcp"`**（现有 `cli`/`gateway`/`internal` 不变）；识图调用 token 计数照常记录，生图 token 留空 |
| 转发链路打标（关键改动） | `ForwardRequest` 新增 `source: Option<String>`（默认 `"gateway"`）与 `logger_tags: Vec<String>`（默认空）；`start_call_log` 的 `source` 从硬编码 `"gateway"` 改为透传；`LogRecordBuilder` 构造的 gateway / provider-api 日志附带 `logger_tags`。识图链路传 `source="mcp"` + `logger_tags=["mcp"]`，虚拟路由转发（`VirtualForwarder`）同步透传。生图链路 `MediaGenerationService::generate_image` 新增 `source` 参数（UI 传 `internal`、MCP 传 `mcp`），其自研 logger 条目在 MCP 入口侧补打 tag |
| 日志页展示 | 日志页（logger UI）当前未展示 tags：新增 tag 徽章展示（`mcp` / 协议族 / `sse` 等既有 tag 一并受益）+ 按 tag 筛选；调用统计页 `source` 筛选下拉补 `mcp` 选项 |
| 网关响应日志 | `/mcp` 走 `log_gateway_response`（复用 [router.rs](file:///d:/ProjectApp/i-code/src-tauri/src/modules/gateway_runtime/router.rs#L436-L556) 的 LogPipeline）；SSE 响应仅记录状态行（不落请求/响应大 body，避免 msgpack 二进制污染日志） |

> 新增 Command（`mcp_config_get/update`、`mcp_sessions_list/kill`、`mcp_tools_snapshot`、`mcp_tool_models_get/update`）必须在 `main.rs` `invoke_handler` 注册（§12.1）。

---

## 10. 演进路径与里程碑

| 阶段 | 内容 | 依赖 |
|------|------|------|
| **M1（本次提案）** | GatewaySettings 扩展（`mcp_enabled` + `mcp_tool_models` 表 + 迁移 V011）→ `mcp-server` 模块（protocol / tools / session / sse / router / service / commands）→ 路由挂载 → 认证复用 → 生图 + 识图两条工具链路（含 `ForwardRequest.source`/`logger_tags` 打标改造与 `generate_image` source 参数）→ 双写日志 + call-records → 日志页 tag 徽章与筛选 → 设置页 MCP 卡片（含识图模型关联）+ i18n 四语 | Phase 3 视觉直连能力 |
| **M2（可选）** | 接入真实 MCP Client 验证矩阵：Claude Code `--transport http` / Codex / Cursor / 协议细节（msgpack 握手、`Accept` 协商、会话复用、`tools/list_changed`）；识图专项：多模态消息跨协议桥接（ChatCompletions→Anthropic 的 `image_url→image` 已有实现，需实测）、openai-responses 上游对 image content 的兼容性、大图（>5MB）行为 | M1 |
| **M3（可选）** | 视频任务工具 `generate_video_submit` / `generate_video_query`（任务状态机复用 media_video_tasks 预留表 + 进度事件；`mcp_tool_models` 表以 `tool_name` 维度直接复用） | Phase 5 视频任务落地后 |
| **M4（可选）** | 网关端 `/v1/images/generations`（OpenAI Images 兼容，media-generation.md §3.5 Phase 4）；聊天气泡内嵌生图 | M1 |

> 注：`media-generation.md` §5 Phase 6 原写「挂载网关端口 + 复用网关认证」，本提案将其细化为 M1 落地；Phase 4（网关端点）与 M3（视频）相互独立，可按需先行。

---

## 11. 风险与开放问题

| # | 风险 / 问题 | 对策 |
|---|------------|------|
| 1 | **MCP 规范面持续演进**（2025-11-25 版新增多传输、可配置资源/提示等） | 仅实现 2025-03-26 Streamable HTTP 稳定子集；协议面收敛为「握手 + tools/list + tools/call」；`serverInfo` 标注 `version`，后续升级只扩展不破坏 |
| 2 | **axum 0.7 + SSE 手动实现** | 现有 `bridges/stream.rs` 已有 SSE 透传先例可参考；队列用 `mpsc` + `futures::stream::unfold` 组装，避免引入额外 SSE crate |
| 3 | **msgpack 与 JSON 双编解码** | codec 封装在 `protocol.rs` 单点；会话表记录 `protocol_pref`，杜绝「响应格式混乱」 |
| 4 | **会话泄漏 / 断线残留** | 30 分钟空闲回收 + 手动 kill + 网关停止时全量推送 `notifications/closed` 并销毁 |
| 5 | **生图并发打爆上游配额** | `tools/call` 侧 semaphore 限流（默认并发 2）；入参 `n` 上限 4（与服务端一致） |
| 6 | **`{gateway_key}` 明文展示** | 仅按需临时解析并自动隐藏；日志 / 导出不含 Key 明文（§12.3） |
| 7 | **第三方客户端兼容面** | M2 阶段用真实 Client 验证；`serverInfo` / instructions 帧与 `Accept` / `Content-Type` 协商优先兼容 Claude Code 行为 |
| 8 | **识图多模态消息的跨协议兼容** | ChatCompletions→Anthropic 桥接的 `image_url→image` block 转换已有实现（bridge/request.rs），M2 实测；openai-responses 上游（`input_image` 格式）若无转换则该类供应商识图返回错误——错误信息中明示「该供应商协议暂不支持识图」，不静默失败 |
| 9 | **识图上下文成本失控** | 图片计入 prompt tokens（上游计价），`max_tokens` 固定 4096；call-records 照常记录 token 与费用，可在统计页审计；本地路径图片设 20MB 上限 |
| 10 | **关联模型失效 / tools/list 刷新差异** | 调用时校验模型有效性（失效返回校验错误并提示重新关联，不静默降级）；`tools/list_changed` 推送仅对支持的客户端生效，不支持的客户端下次会话生效——设置页保存时提示「部分客户端需重开会话」 |

---

## 12. 结论

以**自研极简 MCP Server**（`mcp-server` 模块 + `rmp-serde`）挂载本地网关 `POST/GET /mcp`，复用网关端口与认证，向第三方 MCP Client 暴露两个工具：`generate_image`（复用 `media-generation` 直连链路，返回本地产物路径）与 `analyze_image`（进程内复用 `ForwardPipeline` 走多模态聊天链路，返回文字回答）。识图模型由用户**手动关联**（新表 `mcp_tool_models`，候选池为 `imageInput` 已暴露模型，第一个为默认，未关联不下发工具）。MCP 触发的调用在两套日志与 call-records 中统一打 `mcp` 标识（`ForwardRequest.source`/`logger_tags` 透传 + 日志页 tag 徽章筛选），保证可观测与审计。设置页提供开关、识图模型关联管理、端点/命令展示与会话管理。数据库为单一迁移 V011（`gateway_settings.mcp_enabled` 列 + `mcp_tool_models` 表）。