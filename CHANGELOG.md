# Changelog

## [Unreleased]

### 新增

### 修复

### 变更

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