# Changelog

## [Unreleased]

## [0.0.10] - 2026-07-30

### 新增

- **供应商网络连通性检测**：供应商列表工具栏新增「检测」下拉按钮，支持直连/代理两种模式 ping 所有供应商 URL
  - 后端新增 `gateway_provider_ping` Command，逐个检测供应商并实时推送事件
  - 前端点击后立即弹出检测对话框，逐条接收 `provider:ping-result` 事件实时追加表格行
  - 检测完成后接收 `provider:ping-done` 汇总事件，展示成功/失败/总数
  - 每条结果写入自研 logger（source=system），便于日志页面按 system 来源筛选查看
  - 任意 HTTP 响应（含 4xx/5xx）视为可达；仅网络错误（超时、DNS 失败、连接拒绝）视为失败
- **供应商增删改事件广播**：供应商创建/更新/删除后通过 `provider:changed` 事件通知前端，列表自动刷新
- **全局代理配置保存校验**：设置页全局代理开启时，新增保存按钮替代失焦自动保存，避免误触发无效代理配置
  - 保存时写入脱敏代理日志到自研 logger（system 来源）

### 修复

- **翻译 key 不匹配与 i18n 命名空间误用**：修复部分组件翻译键名与命名空间引用不一致导致文案缺失的问题
- **额度快照过滤逻辑**：关闭额度监控后立即隐藏对应数据，不再展示过期的残留快照
- **虚拟路由 hook 渲染时序**：简化依赖管理，修复潜在渲染时序问题

### 变更

- **虚拟模型展示格式优化**：显示完整的 `供应商slug/模型ID` 路径，便于识别模型来源
- **虚拟模型表单布局重构**：拆分路由设置为独立 Tab 页，提升交互体验与表单可读性

## [0.0.9] - 2026-07-29

### 新增

- **日志框架迁移至 tracing**：从 `tauri-plugin-log` 迁移到 `tracing` 生态，统一日志基础设施，支持全链路追踪
  - 新增 `trace_id` 模块：生成与传播唯一追踪 ID（base32 编码），用于跨日志/调用记录关联
  - 新增 `TraceIdLayer`：将 trace_id 注入线程上下文，供转发器、调用记录等复用
  - 新增 `size_aware_appender`：大小感知的日志文件追加器，按文件大小滚动切分
  - 新增 `atomic_filter`：原子级别过滤器，支持运行时动态调整日志级别
  - 新增 `tracing_webview` 模块：将日志事件通过 `CONSOLE_LOG` 事件转发到 WebView 控制台
  - 集成 `tower-http` 的 `TraceLayer`：为 HTTP 网关请求自动注入 trace_id，实现请求级追踪
  - 前端新增 `registerConsoleLogForwarder` 监听器，在 `App` 组件中注册
  - 新增迁移方案文档 `docs/plan/log-migration-tracing.md`
- **额度脚本代理支持**：Rhai 脚本运行时新增代理配置能力，与全局/供应商代理策略对齐
  - 新增 `proxied_http` 模块：自动应用供应商与全局代理配置，支持 GET/POST/通用请求/JSON 解析
  - 新增 `http::set_proxy` host function：支持手动配置脚本代理 URL
  - 向脚本注入 `proxy` 系统变量，提供供应商与全局代理配置信息
  - 新增 3 个内置脚本 snippet 演示代理使用方式
  - 新增脱敏 URL 工具函数，避免代理 URL 中的认证信息泄露

### 变更

- **转发请求 ID 复用 trace_id**：网关 forwarder 复用 `TraceIdSpan` 注入的 trace_id 作为 `request_id`，使转发日志、调用记录与 tracing 日志的 tid 保持一致，便于全链路关联；兜底使用自动生成的 trace id 作为 fallback
- **脚本 host 白名单校验逻辑优化**：区分市场脚本与本地脚本，仅对公共市场脚本强制执行 host 白名单校验，本地脚本不做强制限制
  - 新增 `is_marketplace` 方法判断脚本来源
  - 新增 `enforce_host_whitelist` 参数控制是否强制校验
  - 统一市场脚本 `snippet_id` 前缀常量使用
  - 标记废弃的旧阻塞代理函数

## [0.0.8] - 2026-07-28

### 新增

- **脚本模板市场**：新增脚本模板市场模块，支持从公共 GitHub 仓库拉取模板列表、预览和一键应用为本地草稿
  - 后端新增 `script_template_marketplace_list` / `get_detail` / `apply` 三个 Command，包含缓存、校验与冲突处理
  - 前端新增 `ScriptTemplateMarketplaceDialog` 组件，支持筛选、搜索、预览和一键应用
  - 新增 `useScriptTemplateMarketplace` Hook，封装市场列表拉取与详情查询
  - 新增市场提案文档 `docs/proposals/script-template-marketplace.md`
- **Claude CLI 配置一键应用**：新增 `cli_apply_claude_config` 命令与服务实现，支持一键将当前供应商配置写入 Claude Code 的 `settings.json`
  - 支持自动同步网关/直连模式的 Base URL 与认证信息
  - 支持配置开关、模型映射、兜底模型等完整 CLI 选项
  - 保留原有保存功能的同时新增独立的应用配置入口
  - 新增 `ApplyClaudeConfigInput` / `ApplyClaudeConfigResult` DTO
- **Codex 模型映射**：Codex CLI 面板支持模型映射配置，UI 与后端逻辑调整对齐 Claude CLI 模式

### 修复

- **供应商表单 API Key 显示**：优化 API Key 显示与编辑体验，支持留空不修改原有密钥
- **额度监控展示**：修复额度监控展示逻辑，仅在非 `none` 模式下展示相关 UI

### 变更

- **模型映射编辑器重构**：将通用模型映射编辑能力抽离为公共组件 `ClaudeModelMapping`，供 Claude CLI 和 Codex 复用
- **代码编辑器自适应高度**：为 `CodeEditor` 组件新增 `autoHeight` 属性，实现基于内容的自适应高度；CLI 设置面板和脚本模板预览面板的编辑器高度样式同步调整
- **托盘额度菜单更新逻辑**：重构托盘额度菜单更新逻辑，提取公共函数 `update_tray_balance_items` 复用代码

## [0.0.7] - 2026-07-28

### 新增

- **供应商扩展模板变量**：供应商表单新增「扩展」Tab，支持管理 `key/value/isSecret/label` 的变量列表，运行时以 `variables["key"]` 注入额度脚本
  - 后端新增 `script_variables_json` 字段及迁移 `V002__provider_script_variables.sql`
  - 敏感变量值由 Secret 模块加密后存储为 `$SECRET:{uuid}$` 引用
  - 余额脚本上下文注入 `variables` map，可在脚本中读取扩展变量
  - 前端新增 `ScriptVariablesEditor` 组件，支持增删改、敏感开关、key 格式校验与重复校验
- **京东 JoyAgent 余额脚本 Snippet**：新增 `joyagent-balance` 内置脚本，查询可用积分、积分上限、已用积分、剩余百分比、优惠券/钱包/欠款金额及账户状态
- **字符串转换 Host Functions**：Rhai 脚本运行时新增 `str::to_float` / `str::to_int`（同时提供扁平别名 `str_to_float` / `str_to_int`），用于将接口返回的字符串数值转为数值类型

### 修复

- **供应商表单误提交**：`ScriptVariablesEditor` 中「添加变量」与「删除」按钮未声明 `type="button"`，默认触发表单提交导致弹窗意外关闭并保存；已显式指定 `type="button"`

### 变更

- **余额脚本鉴权方式调整**：内置 JoyAgent / 小米 MiMo 余额查询脚本从直接读取 `api_key` 改为通过 `variables["cookie"]` 读取扩展变量，Cookie 等动态凭证不再占用 API Key 字段，注释同步指引用户在扩展模板变量中配置 `cookie`

## [0.0.6] - 2026-07-27

### 新增

- **聊天提示词库**：ChatInput 工具栏新增提示词按钮，点击弹出提示词列表弹窗，支持选择后一键填入输入框
  - 提示词来源：用户配置目录下 `prompt/` 文件夹中的 `.md` 文件，标题取自首个 `# ` 行
  - 列表紧凑布局，标题超宽时自动横向滚动（跑马灯），每行右侧「应用」按钮
  - 超过 125000 字符自动截断并提示
  - 后端新增 `chat_prompt_list` / `chat_prompt_get` 两个 Command

### 修复

- **提示词弹窗无限请求**：`PromptPickerDialog` 中 `useTranslation` 返回的 `t` 每次渲染新建引用，导致 effect 依赖无限循环调用 `chat_prompt_list`；改用 `tRef` 稳定依赖数组

## [0.0.5] - 2026-07-27

### 修复

- **代理策略修正**：全局代理未启用时强制 `no_proxy()` 直连，不再回落到 reqwest 默认行为（读取系统 `HTTP_PROXY` / `HTTPS_PROXY` 环境变量），修复「系统设了代理环境变量但代理不可用时，直连可达的供应商也拉取/转发失败」的问题
- **模型拉取忽略代理配置**：`fetch_official_models` / `fetch_models_by_protocol` 改用 `build_provider_http_client`（含 `apply_provider_proxy`），修复此前使用裸 `reqwest::Client::new()` 导致供应商代理策略全部失效的问题
- **供应商无法切回全局代理**：前端 `provider-form.tsx` 始终序列化 `proxyJson`（含 `global` 模式），修复此前 `global` 时返回 `undefined` 被 Tauri invoke 省略导致后端跳过更新、DB 保留旧代理配置的问题
- **OAuth 代理不一致**：`oauth2.rs::new_for_provider` 改用 `apply_provider_proxy`，修复此前 `Global` 分支不应用全局代理的缺陷

### 变更

- **代理逻辑统一到 `shared` 层**：新增 `apply_provider_proxy`，供 `ai_gateway`（模型拉取 / OAuth）与 `gateway_runtime`（网关转发）共用，保证两条网络路径策略一致；详细设计见 [`docs/proxy.md`](docs/proxy.md)
- **代理日志增强**：代理决策全链路增加 `tauri-plugin-log` 的 `trace` / `error` 级别日志，含策略来源、最终决策、URL（脱敏认证信息）；`error` 日志输出完整 reqwest 错误链（含网络栈），便于排障

## [0.0.4] - 2026-07-26

### 新增

- 全局代理现在应用于所有出站网络请求，包括供应商 API 调用、额度脚本 HTTP 请求、更新检测等

### 修复

- 版本号更新脚本修复正则 `g` 标志导致静态版本引用未同步的问题

### 变更

- **全局代理配置重构**：代理类型从 `direct / custom / system / vscode` 简化为 `direct / system / http / socks`，移除已废弃的 `authorization`、`strictSSL` 字段，HTTP/SOCKS 代理 URL 支持直接包含认证信息（如 `http://user:pass@host:port`）
- **全局代理统一应用**：将 `apply_global_proxy` 从 `update_version` 模块提取到 `shared` 模块，供网关运行时、额度脚本 HTTP 调用、更新检测等所有出站请求复用；新增 `apply_global_proxy_blocking` 供同步阻塞客户端（Rhai 脚本）使用
- 设置页网络卡片 i18n 键名从扁平 `settings.network` 重构为嵌套 `settings.network.title`，全局代理描述文案同步更新

## [0.0.3] - 2026-07-26

### 新增

#### 额度监控脚本模块

- **数据库**：新增 `script_templates` 表，支持模板名称、slug、类型、状态（draft/active/disabled）、脚本正文、引擎、超时、host 白名单、试运行记录等字段
- **后端 CRUD**：完整 10 个 Command（`script_template_list`、`get`、`create`、`update`、`delete`、`set_status`、`test`、`list_active_for_select`、`list_snippets`、`list_refs`），全部注册在 `main.rs`
- **状态机**：`draft → active → disabled` 三态迁移，`publish`/`disable`/`revert_to_draft`，启用前校验脚本非空，删除时检查供应商引用
- **Rhai 运行时**：纯 Rust 脚本引擎，每请求新建 Engine + Scope 避免状态泄漏，`spawn_blocking` 避免阻塞 tokio
- **系统变量注入**：`api_key`（已解密）、`provider`（id/slug/name/base_url/type/is_enabled）、`auth`（method/project_id/account_id 白名单）、`now_ms`、`template`（id/name/kind）
- **Host Functions**：
  - `http.get/post/request/get_json` — 基于 reqwest，host 白名单校验、超时（默认 15s，上限 30s）、响应 body 2MB 上限
  - `json.parse/stringify/stringify_pretty` — 字符串与 Dynamic 互转
  - `log.info/warn/error` — 自研 logger，自动脱敏 API Key
  - `error(msg)` — 中止执行并转为业务错误
  - `str.contains/replace`、`url.join` — 工具函数
- **沙箱策略**：禁止文件/进程/环境变量访问，最大执行步数 100_000，脚本正文上限 64 KiB，并行脚本数信号量控制
- **Dynamic → BalanceSnapshot 映射**：校验返回结构合法性，`updatedAt` 缺省时自动补 `now_ms`
- **内置 Snippet**（6 个）：余额 GET + Bearer、返回 items 骨架、Bearer 请求头、小米 MiMo 按量计费、小米 MiMo TokenPlan、Grok Usage
- **BalanceMethod 扩展**：新增 `Script` 方法，`BalanceConfig::Script` 含 `scriptTemplateId`、`timeoutMs`、`allowedHosts`
- **额度刷新适配**：`dispatch_refresh` 中识别 Script 分支，加载 active 模板后执行脚本，失败返回明确错误信息
- **前端类型**：`ScriptTemplate`、`ScriptTemplateStatus`、`CreateScriptTemplateInput`、`UpdateScriptTemplateInput`、`ScriptTemplateTestResult`、`ScriptSnippet` 等完整 DTO
- **前端 Hooks**：`useScriptTemplateList`（含 kind/status/keyword 筛选）、`useActiveScriptTemplates`（仅 active 模板）、`useScriptSnippets`（内置 snippet 列表）、`useScriptTemplateMutation`（CRUD + 状态迁移 + 试运行 + 引用查询）
- **前端组件**：
  - `ScriptTemplateList` — 列表页，支持类型/状态/搜索筛选、紧凑 900×700 布局、空状态引导
  - `ScriptTemplateEditor` — 全功能编辑对话框，元数据字段、CodeMirror 编辑器（JS 高亮近似 Rhai）、系统全屏切换、状态徽章、发布/禁用/恢复草稿
  - `ScriptSidebarDocs` — 右侧文档面板，系统变量/函数/Snippet/返回结构/示例 五个 Tab，点击插入编辑器
  - `ScriptTestPanel` — 试运行面板，供应商 Select 下拉选择、执行结果展示（snapshot / 错误 / 耗时 / 日志）
  - `ScriptTemplateStatusBadge` — 状态徽章（草稿/启用/禁用）
  - `BalanceConfigForm` — 供应商额度配置表单下拉分组「内置」+「自定义脚本」，选择脚本模板后支持 timeoutMs 覆盖，模板禁用时显示警告
- **安全设计**：API Key 仅内存注入，禁止明文落库或写入日志，host 白名单默认跟随 `base_url`，日志脱敏，导出备份不含密钥

### 修复

- 修复全屏模式下 Select 弹出层被 Radix aria-hidden 阻断的问题：改用 `document.documentElement.requestFullscreen()` 全屏整个文档而非 DialogContent，确保 portal 渲染的弹出层可见 

### 变更

- 供应商额度监控支持使用自定义额度监控脚本
- 供应商列表优化额度信息展示


## [0.0.2] - 2026-07-26

### 新增

- 版本检测

### 修复

- 修复 CICD

## [0.0.1] - 2026-07-25


### 新增

#### AI Gateway 供应商管理
- 支持多供应商（OpenAI、Anthropic、Gemini、OpenRouter 等）的集中管理
- 供应商 CRUD：名称、Base URL、认证方式（API Key / Bearer Token / 自定义 Header）
- 供应商级别的代理配置（ProxyConfig）、重试策略（RetryConfig）、超时设置（TimeoutConfig）
- 模型 CRUD：每个供应商下可管理多个模型，支持模型 ID、名称、上下文窗口、最大输出等基本信息
- 认证方式多态类型，对齐参考项目 `vscode-unify-chat-provider` 的 well-known 数据类型

#### Secret 加密存储
- 基于 AES-GCM 的本地加密方案，API Key / Token 等敏感数据禁止明文落库
- 加密后以 `$SECRET:{uuid}$` 引用形式存入数据库和配置 JSON
- 明文仅在 Rust 后端处理，前端仅输入时接触一次，不缓存
- Secret 引用扫描与解析仅在后端完成

#### 本地 HTTP 网关运行时（部分实现）
- 基于 axum 的本地 HTTP 网关，默认监听 `127.0.0.1:54321`
- 模型路由 ID 格式：`{provider_slug}/{model_id}`
- 支持 `/v1/chat/completions`、`/v1/messages`、`/v1/models` 等接口
- SSE 流式响应原样透传，禁止二次包装
- 错误响应标准化为 OpenAI 格式 `{ error: { message, type, param, code } }`
- 网关认证中间件
- 响应拦截器异步写入 Logger 和 Call Records

#### 额度查询（Balance）
- 支持供应商级别额度查询
- 额度快照管理，支持时间范围查询
- 金额使用 string 类型避免浮点精度问题

#### 调用记录（Call Records）
- 模型调用统计与明细记录
- 聚合统计：按供应商、模型、时间维度
- 支持调用日志查看与筛选

#### 运行时日志（Logger）
- 自研内存环形缓冲区日志，可在应用内「日志」页面查看
- 同时集成 `tauri-plugin-log` 输出到终端和控制台
- 日志级别控制：全局设置影响 tauri-plugin-log，日志页面设置影响自研 logger
- 日志支持按来源、级别、时间筛选，可导出

#### 数据库备份与恢复
- 支持本地 SQLite 数据库备份与恢复
- 支持 WebDAV 远程备份
- 备份任务管理

#### 系统设置
- 主题设置：`light` / `dark` / `claude-light` / `claude-dark` / `deepseek-light` / `deepseek-dark`
- 国际化：`zh-CN` / `en` 双语支持（基于 i18next）
- 网关地址、日志级别等全局设置
- 所有颜色使用 CSS 变量，禁止硬编码

#### 内置数据
- 内置供应商与模型 seed 数据（`src-tauri/data/builtin-*.json`）
- well-known 数据转换脚本

### 技术栈

| 层级 | 技术 |
|------|------|
| 桌面 | Tauri 2.x（Rust + WebView） |
| 前端 | React 19 + TypeScript 5（严格模式） |
| 路由 | TanStack Router（文件系统路由） |
| UI | shadcn/ui + Tailwind CSS + Font Awesome |
| 状态 | Zustand（前端）+ Tauri State（后端） |
| 表单 | react-hook-form + zod |
| 国际化 | i18next（zh-CN / en） |
| 后端 HTTP 网关 | axum |
| 数据库 | rusqlite + r2d2（SQLite） |
| 加密 | AES-GCM（本地模式） |
| 类型同步 | ts-rs（Rust → TypeScript） |
| 包管理 | pnpm@11 |

### 部分实现 / 待迭代

- **网关运行时**：路由不完整，`/v1/responses` 等端点可能缺失
- **虚拟供应商**：策略枚举与文档可能不一致
- **CLI 管理**：目录与类型骨架存在，完整流程待实现
- **工作区管理**：Prompts/MCP/Skill 的编辑与应用流程待实现