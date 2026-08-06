# Changelog

## [release-version-tempalte]

## 新增

## 修复

## 变更

## [0.1.6] - 2026-08-06

### 新增

- **模型调用统计展示总 token**：模型统计页面的统计描述新增「总 token」展示（随当前「明细 / 汇总」Tab 联动，取当前视图数据求和）
  - 新增 `formatTokenKMB` token 格式化工具函数，支持 K（千）/ M（百万）/ B（十亿）西式紧凑单位转换；非法输入兜底返回 `0`
  - `ModelList` 新增 `activeTab` 状态追踪当前激活 Tab，据此计算对应视图的 `totalTokens` 并参与统计描述 `totalTokens` 占位符渲染

### 修复

- **模型列表默认视图模式改为滚动模式**：`ModelList` 表格视图模式默认由 `compact`（自适应换行）改为 `scroll`（固定列宽横向滚动），避免列宽撑开导致布局溢出
- **日志 URL 与实际请求 URL 不一致**：重构 `build_log_url`（`forwarding/util.rs`），改为通过 `bridge_upstream_protocol` 计算桥接后的**上游协议**再选路径，与 `AnthropicClient` / `OpenAiChatClient` 内部 `build_upstream_url` 的入参保持一致，解决桥接场景下入口协议（如 `ChatCompletions`）与上游协议（如 `AnthropicMessages`）不同导致日志展示路径误导排查的问题
- **流式桥接转换函数调用方向错误**：修正 `forwarder` 流式桥接（`apply_stream_bridge`）中两种桥接模式下的 SSE 转换函数调用方向——`OpenaiToAnthropic`（入口 O → 上游 A）响应需转换为入口 OpenAI SSE（`anthropic_sse_to_openai`），`AnthropicToOpenai`（入口 A → 上游 O）响应需转换为入口 Anthropic SSE（`openai_sse_to_anthropic`），即响应转换方向与请求转换方向**相反**，与 `apply_response_bridge` / `convert_error_body` 保持一致；同步更新注释说明转换逻辑细节

### 变更

- **Anthropic Client 凭证双写 `Authorization` 头**：配置 `ApiKey` 时，除写入 `x-api-key` 外，同步写入 `Authorization: Bearer {key}`，兼容需要双重认证的中转网关（如小米 token-plan 等）；官方 Anthropic API 只认 `x-api-key`，多出的 `Authorization` 头会被忽略无副作用，`extra_headers` 仍可在最后覆盖 `x-api-key` / `Authorization` / `anthropic-version`

## [0.1.5] - 2026-08-06

### 新增

- **协议桥接模块（Anthropic ↔ OpenAI Chat 双向转换）**：当网关入口协议与上游供应商协议不一致时，在转发前对请求 / 响应 / 流式事件做双向转换，支持 Anthropic Messages 与 OpenAI Chat Completions 协议互转
  - 新增 `gateway_runtime/bridge` 模块：`BridgeKind` 枚举与 `detect_bridge` 触发判定、`anthropic_to_openai_chat` / `openai_chat_to_anthropic` 请求体转换、非流式响应体双向转换、流式事件状态机双向转换（容错：畸形 chunk 透传并告警）
  - 关键约束：`max_tokens` 缺失时从 `model_configs.max_output_tokens` 读取，兜底 `MAX_TOKENS_FALLBACK = 200000`；O→A 时移除 `response_format` 并在 `system` 末尾追加提示；工具调用 ID 原样透传不重命名
  - `GatewayProtocol::to_upstream_with_bridge` 按桥接判定返回上游 Client 实际协议；`forwarder` 在 `execute_and_finalize` / `apply_stream_bridge` 集成桥接，非桥接场景零开销
  - 方案文档：`docs/proposals/protocol-bridge.md`（P1–P4 全部落地，v1.0.0 released）
- **脚本模板变量依赖声明（`varList`）**：公共仓 `catalog.json` / `meta.json` 新增 `varList` 字段，显式声明脚本依赖的系统变量（`api_key` / `provider.base_url` 等）与供应商「扩展模板变量」（`variables["cookie"]` 等）
  - 后端 `marketplace/types.rs` 新增 `VarDef` / `VarSource`，`RemoteCatalogItem` 与 `MarketplaceItemSummary` 透传 `var_list`
  - 前端 `MarketplaceItemSummary` 新增 `varList`；脚本模板市场详情面板移除 tag 标签展示，改为渲染变量列表（变量名 / 来源徽章 system=蓝 custom=琥珀 / 必填徽章 / 描述），无变量时显示占位
- **供应商协议类型桥接帮助提示**：供应商表单「协议类型」标签新增 `HelpIcon` 帮助气泡（`popover` / `side=top`），说明协议桥接行为；i18n 补充 `providerTypeBridge` 帮助文案

### 变更

- **日志协议标签 `websocket` 更名为 `ws`**：`protocol_tags` / `detect_response_tags` / `LogEntry` 注释统一将 `websocket` 标签更名为 `ws`，前端日志筛选器与展示同步迁移
- **桥接转发新增 `bridge` 日志标签**：`protocol_tags` 签名新增 `BridgeKind` 参数，桥接转发额外标记 `bridge`，便于日志按桥接场景筛选


### 测试

- 协议桥接模块新增 13 个单元测试（`bridge/tests.rs`，共 1773 行）：覆盖请求 / 响应 / 流式转换规则、容错策略、工具调用 ID 透传等；全部 290 个测试通过

### 备注

- 方案文档：`docs/proposals/protocol-bridge.md`；`docs/gateway-runtime.md` 已同步桥接模块说明与实现状态


## [0.1.4] - 2026-08-05

### 新增

- **自研 logger 请求头展示（去敏）**：网关（inbound 入站请求头）与供应商 API（outbound 出站请求头，缺失时回退到入站头）日志展示请求头 JSON（敏感头值替换为 `***`），位于「模型 ID」下方一行，随导出写入 CSV / JSON
  - 新增 `request_headers_to_json` 去敏序列化（`logging/headers.rs`）：将 `HeaderMap` 序列化为有序 JSON；头名称（不区分大小写、子串匹配）命中 `authorization / api-key / token / secret / credential / cookie / auth` 任一敏感片段时值替换为 `***`；非 UTF-8 二进制值序列化为 `<binary>`；无请求头时返回 `None`
  - `UpstreamClient::execute` 签名改为 `&mut UpstreamContext`，各 Client（openai_chat / anthropic / openai_responses / websocket）在发送前对真实出站请求头做去敏快照写入 `UpstreamContext.request_headers_json`，供转发（provider-api）日志展示
  - 网关四个对外 handler（`chat` / `responses` / `messages` / `models`）捕获入站 `HeaderMap` 并透传到网关日志与转发日志
  - `LogRecord` / `LogEntry` 新增 `request_headers: Option<String>` 字段；CSV 导出新增 `requestHeaders` 列
  - 前端 `LogViewer` 以 JSON 缩进格式展示请求头，i18n（zh-CN / en / ja / zh-TW）补充 `requestHeaders` 文案
- **SSE chunk 专属日志文件（按小时滚动）**：SSE chunk 日志通过独立 target `i_code::sse` 写入单独按小时滚动的 `i-code-sse.*.log`（前缀 `i-code-sse`），不混入主日志文件，避免高频 chunk 刷屏
  - SSE 专属 fmt layer 使用 `TraceIdFormat::without_location()`，文件内不打印 target 与 file:line
  - 常规主日志过滤器（`MainLogFilter`）排除 `i_code::sse` target，确保 chunk 只进专属文件

### 变更

- **日志分段大小写入加固**（`SizeAwareFileAppender`）：写入超限时不只判断滚动，改为按 `max_size` 拆分 buffer——先写满当前文件、滚动到新分段后继续写剩余部分，保证每个分段不超过 `max_size`（修复单块大写入出现 ~40MB 分段的隐患）；使用 `create_new`（O_EXCL）独占创建分段并跳过已被占用的序号，从根上避免多进程 / 多次重启把同一分段追加撑大或序号复用

### 修复

- **复制的网关 API 地址缺少 `/v1` 路径**：网关 API 文档弹窗与网关首页的「复制 API 地址」按钮此前只复制 `http://host:port`，现统一补充 `/v1`，复制完整的 OpenAI 兼容基础地址


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