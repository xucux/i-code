# Changelog

## [Unreleased]

### 新增

### 修复

### 变更

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