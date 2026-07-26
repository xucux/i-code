# Changelog

## [Unreleased]

### 新增

### 修复

### 变更

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