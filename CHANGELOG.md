# Changelog

## [release-version-tempalte]

## [0.1.3] - 2026-08-04

### 新增

- **Grok Build（xAI 订阅）额度监控**：新增 `grok-build` 余额监控方法，调用 Grok CLI 内部 chat-proxy billing 端点查询周/月额度（`GET /billing?format=credits` / `GET /billing`）
  - 认证分支：`xai-grok-oauth`（OAuth 账号，免费档 / SuperGrok）走 `cli-chat-proxy.grok.com/v1/billing`；API Key 回退 `api.x.ai` 健康探测（`/v1/me` + `/v1/chat/completions`），仅提供可用性状态，无法给出精确剩余额度
  - 请求头对齐 `gateway_runtime/auth_resolver.rs` 的 xAI Grok OAuth 解析（`x-grok-client-version` / `xai-grok-cli` 等）；金额字段单位统一为分，输出转换为美元字符串传输，避免浮点精度问题
  - 方案文档：`docs/proposals/grok-build-billing-monitoring.md`

### 变更

- **余额查询统一走全局代理**：新增 `build_balance_http_client()` 统一构造额度查询 HTTP 客户端，复用应用全局代理配置（`shared::apply_global_proxy`，对齐 `docs/proxy.md`）；全部 12 个内置 balance provider（DeepSeek、Codex、Kimi、MiniMax、Grok Build、OpenRouter 等）从 `reqwest::Client::new()` 切换为统一构造，全局代理未启用时按规范强制直连（不读系统环境变量代理），连接超时 10s、总超时 30s
- **Grok Build 字段解析增强**：金额/对象/百分比字段候选键兼容 camelCase / snake_case（`creditUsagePercent` / `credit_usage_percent`、`currentPeriod` / `current_period`、`productUsage` / `product_usage`、`monthlyLimit` / `monthly_limit` 等）；新增「月度用量百分比」指标（`used / monthlyLimit × 100`，仅额度上限 > 0 时输出）
- **脚本模板重命名**：公益 Grok 监控 snippet 更名「公益Grok监控(第三方)」（id `grok-usage` → `grok-usage-thirdparty`），与官方 Grok Build 订阅监控区分
- **隐藏「测试数据」内置监控方法**：余额配置表单中的 `synthetic`（测试数据）选项不再展示

## [0.1.2] - 2026-08-04

### 新增

- **日语与繁体中文语言支持**：应用语言新增「日本語」「繁體中文」选项，i18n 模块、日期组件（date-fns / react-day-picker）、设置页面语言列表、后端 `Locale` 枚举同步扩展；翻译文件 `ja.json` / `zh-TW.json` 新增
- **转发重试事件双通道日志**：网关转发器（`forwarder`）的重试事件（首次请求、退避重试、状态码判定、重试耗尽、网络错误等）同时写入 `tracing`（tauri-plugin-log，终端/日志文件）和自研内存 logger（应用内「日志」页面可见），便于运行时诊断与开发调试

## [0.1.1] - 2026-08-03

### 新增

- **内置预设自动关联默认模型**：`builtin-providers.json` 新增 `defaultModels` 属性（`modelId` 实际发送给供应商、`matchModelId` 对应 `builtin-models.json` 中的模型 id、`displayName` 展示名），从内置预设创建供应商（如 OpenCode Zen Free、Cline Free）时自动按 `matchModelId` 匹配内置模型并创建 `model_config` + `gateway_model`（`source = builtin`），无需创建后再手动添加
  - `BuiltinProvider` / `CreateProviderInput` 两端新增 `defaultModels` 字段；后端匹配逻辑与前端 `findBuiltinByModelId` 一致（精确 > 前缀 > 包含回退），匹配不到时跳过该条目并告警，不阻断供应商创建

### 修复

- **供应商导出丢失附加请求头**：`gateway_provider_export` 此前将 `extra_headers` 硬编码为 `None` 直接丢弃。现改为读取 `provider_extra_headers` 表并写入导出数据（版本升至 `1.1`，旧 `1.0` 数据导入兼容）
  - 导出（带密钥）：`$SECRET` 引用解析为明文
  - 导出（不带密钥）：含 `$SECRET` 引用的条目跳过，避免导入后悬空引用导致转发失败；普通明文与模板变量占位符（`${uuid()}` 等）原样保留
  - 导入侧逻辑原已就绪（`import_provider` 会将 `extraHeaders` 写入 `provider_extra_headers` 表），本次补齐导出侧后闭环


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