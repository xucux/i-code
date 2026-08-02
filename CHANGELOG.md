# Changelog

## [0.1.0] - 2026-08-03

### 新增

- **网关支持 OpenAI Responses API（`POST /v1/responses`）**：本地网关新增对外端点与完整转发链路，支持 Agent 场景的 Responses 协议调用（SSE 事件流 / 非流式）
  - 新增 `responses` handler（`router.rs`），错误体沿用 OpenAI 标准 `{error:{message,type,param,code}}` 格式，自动纳入 API Key 认证与网关日志
  - `GatewayProtocol` / `UpstreamProtocol` 新增 `Responses` 变体，`openai-responses` 供应商类型从 WebSocket 占位改为真实可用
  - 新增 `OpenAiResponsesClient`（`client/openai_responses_client.rs`）：默认走 HTTP/SSE 透传（路径 `/responses`），认证复用 OpenAI 兼容解析（Bearer + extra headers）
  - usage 提取支持 Responses 格式：非流式兼容 `input_tokens` / `input_tokens_details.cached_tokens`；流式解析 `response.completed` 事件中的 `response.usage`
  - `estimate_prompt_tokens` 支持 Responses 请求体 `input` 字段（字符串 / item 数组），调用记录 token 估算不再缺失
- **Responses WebSocket 传输**：`openai-responses` 供应商配置 `transport = websocket` 时，网关以 WebSocket 连接上游（`wss://{base}/responses`），支持 `response.create` 事件与热事件流
  - 引入 `tokio-tungstenite` 依赖（`rustls-tls-native-roots`，与 reqwest 一致读取系统证书库）
  - `UpstreamResponse` 新增 `WebSocketStream` 变体：Client 将 WS 文本帧转换为 SSE 格式字节流（`data: {json}\n\n`），收到 `response.completed` / `response.failed` / `response.incomplete` / `error` 终止事件后发送 Close 帧
  - `response_handler` 重构 `build_sse_from_stream` 统一 SSE 构造入口，`Streaming` 与 `WebSocketStream` 共用字节流透传与 usage 拦截逻辑
- **供应商传输方式配置**：`CreateProviderInput` / `UpdateProviderInput` 新增 `transport` 字段（`auto` / `sse` / `websocket`）并贯通 repository 读写；供应商表单新增「传输方式」下拉（仅 `openai-responses` 协议显示），`auto` / `sse` 走 HTTP+SSE，`websocket` 走 WebSocket
- **网关接口文档更新**：`GatewayApiDocsDialog` 新增 `POST /v1/responses` 端点条目，i18n（zh-CN / en）同步补充

### 变更

- **日志协议标签修正**：`protocol_tags` 增加 `transport` 参数，仅当供应商显式配置 `transport = websocket` 时才打 `websocket` 标签，HTTP 透传场景统一标记为 `sse`，不再对 `openai-responses` 一律误标
- **WebSocket 客户端占位范围收窄**：`openai-responses` 已由真实实现接管，`WebSocketClient` 占位仅保留服务 `openai-codex` / `websocket` 类型

### 测试

- `usage_extractor` 新增 5 个单元测试：Responses 非流式 usage、Chat Completions 回归、Responses 流式 `response.completed`、中途事件不污染、缺失 usage 兜底

### 备注

- 有状态 API（`previous_response_id`、`GET /v1/responses/{id}` 等）仅透传上游，网关不维护本地会话状态；WebSocket 传输暂不应用代理配置
- 方案文档：`docs/proposals/responses-api-support.md`；`docs/gateway-runtime.md` 已同步路由清单、协议标签与实现状态

## [0.0.16] - 2026-08-02

### 新增

- **供应商附加请求头编辑与管理**：供应商表单「高级」页签新增「附加请求头」编辑器，支持增删改请求头，网关转发到上游时注入且可覆盖默认头
  - 值支持模板变量：`${uuid()}`（每次请求随机）、`${uuid_by_day()}`（当天固定）、`${variables["key"]}`（供应商扩展变量）；`$SECRET` 引用原样保存、转发时自动解密
  - 新增 `gateway_provider_extra_headers_list` Command，编辑供应商时加载并回填现有请求头
  - 创建供应商时自动回填内置预设的默认附加请求头（如 OpenCode Zen Free 的 `Authorization: Bearer public` 与 `x-opencode-*` 系列）
- **OAuth Token 续期原地更新**：OAuth 续期成功时新 token 原地更新原 Secret 引用（保留 id），避免每次续期产生孤儿 Secret 记录
- **TLS 信任系统证书库**：reqwest 切换为 `rustls-tls-native-roots`，TLS 校验读取系统证书存储（Windows 证书库），支持信任用户安装的代理 MITM 根证书（如 Proxypin CA）

### 修复

- **附加请求头创建时丢失**：修复从内置预设创建供应商时 `defaultExtraHeaders` 未回填表单导致附加请求头被丢弃、网关无头可注入的问题
- **附加请求头无法编辑**：修复编辑供应商时 `UpdateProviderInput` 缺少 `extra_headers` 字段导致附加请求头保存被忽略的问题

### 变更

- `UpdateProviderInput` 新增三态 `extra_headers` 字段（传对象=全量替换、传 null=清空、不传=不修改）