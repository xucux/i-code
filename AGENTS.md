# i-code — Agent 工作指南

> 本文档面向 AI Agent / 开发者，约定项目目标、架构边界与编码规范。  
> 详细设计见 `docs/`，本文只保留**每次任务都需要遵守**的关键约束。

---

## 1. 项目定位

**i-code** 是基于 **Tauri 2.x** 的本地桌面应用，用于：

1. **管理 AI Gateway 供应商**：集中维护多 LLM 供应商（OpenAI、Anthropic、Gemini、OpenRouter 等），支持多协议与认证。
2. **提供本地 API Gateway**：统一暴露模型 ID 为 `{provider_slug}/{model_id}`，本地监听并代理请求。
3. **管理 CLI 配置**：为 Claude Code、Codex、Gemini CLI 等维护配置档案，支持直连或路由到本地 Gateway。
4. **工作区隔离**：按 Workspace 隔离 Prompts / MCP / Skill；**切换并「应用」后**才写入 CLI 实际配置文件。

版本：`0.1.9`  
包管理：`pnpm@11`  
数据库：本地 SQLite（`i-code.db`）  
默认网关：`127.0.0.1:54321`

### 核心业务规则

| 规则 | 说明 |
|------|------|
| 模型路由 ID | 对外 = `{provider_slug}/{model_id}`；网关拆分后路由到真实供应商 |
| CLI 路由模式 | `base_url` 指向本地网关，`model` 字段保留前缀 |
| 敏感数据 | API Key / Token **禁止明文落库**；配置中仅存 `$SECRET:{uuid}$` |
| Secret 边界 | 加解密**仅在 Rust 后端**；前端只传明文一次，不缓存 |
| 工作区应用 | 修改 Prompts/MCP/Skill 后标记 `pending_apply`；用户点「应用」才写 CLI 文件 |

---

## 2. 技术栈（勿随意更换）

| 层级 | 技术 |
|------|------|
| 桌面 | Tauri 2.x（Rust + WebView） |
| 前端 | React 19 + TypeScript 5（严格模式） |
| 路由 | TanStack Router（文件系统路由，`src/routes/`） |
| UI | shadcn/ui + Tailwind CSS + Font Awesome |
| 状态 | Zustand（前端）+ Tauri State（后端） |
| 表单 | react-hook-form + zod |
| 国际化 | i18next（`zh-CN` / `en`） |
| 后端 HTTP 网关 | axum |
| DB | rusqlite + r2d2 |
| 加密 | AES-GCM（本地模式；密钥链模式待迭代） |
| 类型同步 | ts-rs（Rust → TypeScript） |

**不要**引入 React Query / SWR（数据走 Tauri Command 一次性调用 + Event 推送）。  
**不要**在新代码中使用 `lucide-react`（图标统一 Font Awesome）。

---

## 3. 架构与模块边界

### 3.1 依赖方向

```
core/shared（零业务依赖）
    ↑
theme / i18n / secret / db / balance / logger / backup
    ↑
settings / ai-gateway / cli-management / workspace / gateway-runtime / virtual-provider
    ↑
frontend (components / routes / hooks)
```

### 3.2 模块清单

| 模块 | 职责 | 前端 | 后端 |
|------|------|------|------|
| `ai-gateway` | 供应商、模型、认证、代理 | ✅ | ✅ |
| `gateway-runtime` | 本地 HTTP 网关生命周期与转发 | ✅ 状态 | ✅ |
| `virtual-provider` | 虚拟供应商、故障转移路由 | ✅ | ⚠️ 部分 |
| `script-template` | 额度监控 Rhai 脚本模板 CRUD / 试运行 | ✅ | ✅ |
| `cli-management` | CLI 档案、绑定、模型映射 | ⚠️ 类型/骨架 | ⚠️ 骨架 |
| `workspace` | 工作区、Prompts/MCP/Skill | ⚠️ 类型/骨架 | ⚠️ 骨架 |
| `secret` | 敏感数据加密与引用 | 仅输入组件 | ✅ AES 本地 |
| `settings` | 主题/语言/网关地址等 | ✅ | ✅ |
| `balance` | 额度查询（含自定义脚本） | ✅ | ✅ |
| `logger` | 运行时日志 | ✅ | ✅ |
| `call-records` | 模型调用统计 | ✅ | ✅ |
| `backup` | DB 备份/恢复、WebDAV | ✅ | ✅ |
| `theme` / `i18n` | 主题与双语 | ✅ 仅前端 | — |

前后端模块**同名对应**：`src/modules/{name}/` ↔ `src-tauri/src/modules/{name}/`。

### 3.3 分层规则（必须遵守）

**前端模块：**
- 仅 `types.ts` + `ui/`（+ 必要时 hooks 放 `src/hooks/`）
- **无** Repository / Service 层
- 数据访问一律 `invoke` → 后端 Command（用 `use-command` 封装）

**后端模块：**
```
commands.rs  → 参数校验、调 Service、错误转换
service.rs   → 业务逻辑与编排（可调其他模块 Service）
repository.rs → 仅 SQL / 数据访问（禁止调 Service、禁止发事件）
types.rs     → DTO / 领域类型
```

- Service **禁止**直接访问其他模块的 Repository
- Repository **禁止**调用 Service
- 跨模块只读数据：通过对方 Service 暴露的接口

### 3.4 详细文档（按需查阅，勿全文复制进回复）

| 文档 | 内容 |
|------|------|
| `docs/development.md` | 完整架构、模块设计、UI 规范、Commands 清单 |
| `docs/database.md` | Schema、JSON 字段约定、业务流程 |
| `docs/gateway-runtime.md` | 网关运行时生命周期、路由转发、Provider 适配器 |
| `docs/chat-module.md` | 聊天界面布局、发送/流式/错误气泡、JSONL 存储与 Command |
| `docs/events.md` | 前后端事件总线（含 `chat:stream-*`） |
| `docs/proxy.md` | 网络代理两层配置体系、核心函数、网络路径与日志约定 |
| `docs/log-framework.md` | 日志框架两套机制约定、网关日志格式、级别控制 |
| `docs/error-handling.md` | 错误处理体系、`IcodeError` 结构、前后端转换规范 |
| `docs/cli-management.md` | CLI 档案管理、绑定、模型映射设计 |
| `docs/proposals/balance-script-templates.md` | 额度监控脚本模板（Rhai）CRUD、运行时、编辑体验 |
| `docs/proposals/` | 待实现/演进提案 |
| `docs/fix-bug.md` | 已知问题与修复记录 |
| `参考项目/vscode-unify-chat-provider-7.12.3/` | 类型与 well-known 数据参考源 |

---

## 4. 目录速查

```
i-code/
├── docs/                      # 设计与提案
├── src/                       # 前端 React
│   ├── core/                  # types / errors / events / utils / constants
│   ├── hooks/                 # 跨模块 hooks（use-command 等）
│   ├── components/
│   │   ├── ui/                # shadcn + 全局通用组件（禁止业务类型）
│   │   ├── layout/            # 侧栏布局等
│   │   └── preview/           # /preview 演示用，无业务逻辑
│   ├── modules/{domain}/      # types.ts + ui/
│   ├── routes/                # TanStack 文件路由
│   ├── main.tsx
│   └── index.css
├── src-tauri/                 # 后端 Rust
│   ├── src/
│   │   ├── main.rs            # 入口、托盘、迷你窗、command 注册
│   │   ├── error.rs
│   │   ├── db/                # 连接、schema、migrations/
│   │   └── modules/           # 与前端 modules 一一对应
│   ├── data/                  # builtin-models/providers JSON
│   ├── tauri.conf.json
│   └── Cargo.toml
├── scripts/                   # 内置数据转换等
└── package.json
```

### 主要前端路由

| 路径 | 说明 |
|------|------|
| `/` | 仪表盘 |
| `/gateways`、`/gateways/providers`、`/gateways/models`、`/gateways/settings` | AI Gateway |
| `/cli` | CLI 管理 |
| `/workspaces` | 工作区 |
| `/logs` | 日志 |
| `/settings` | 设置 |
| `/preview` | 组件预览（开发用） |
| `/mini-panel` | 迷你悬浮窗（无 TitleBar / 无侧栏） |

---

## 5. GUI 与 UI 硬约束

### 5.1 窗口

主窗口固定设计尺寸 **900×700**（`tauri.conf.json`，`decorations: false`）：

- 布局优先**紧凑**，避免大片空白
- 理论左右布局在此宽度下可能变成上下布局——做响应式时用小断点验证
- 优先使用 shadcn 组件库已有组件

### 5.2 主题与样式

- 主题：`light` / `dark` / `claude-light` / `claude-dark` / `deepseek-light` / `deepseek-dark` / `nvidia-light` / `nvidia-dark`
- **禁止硬编码颜色**；使用 CSS 变量（`--background`、`--primary`、`--muted` 等）
- 图标：`<i className="fa-solid fa-{name}" />`，尺寸用 `size-3` / `size-3.5` / `size-4` / `size-5`
- 字号以 `text-xs` / `text-sm` 为主，标题最多 `text-lg`；数值用 `tabular-nums`
- 用户可见文案必须走 i18n（`zh-CN` / `en`），键名 `模块.页面.元素`

### 5.3 组件分类

| 目录 | 用途 |
|------|------|
| `components/ui/` | 通用组件，**禁止**引入业务模块类型 |
| `modules/{domain}/ui/` | 业务展示组件 |
| `routes/` | 页面组合与路由参数 |
| `components/preview/` | 仅演示 |

格式化：内存用 `formatMemory`，计数用 `formatCompactCount`（均在 `src/core/utils.ts`），禁止组件内重复实现。

### 5.4 迷你面板

- 独立窗口 `label=mini-panel`：`always_on_top`、`skip_taskbar`、`decorations: false`
- 路由 `/mini-panel` 在 `__root.tsx` 中跳过 TitleBar 与侧栏
- 由 `open_mini_panel` / `close_mini_panel` Command 控制；关闭时聚焦主窗

### 5.5 滚动布局规范

页面内需要滚动的区域，必须使用 `useAvailableHeight`（`src/hooks/use-available-height.ts`）计算实际可用像素高度，再通过 `style={{ height }}` 传入滚动容器或内容组件——**禁止依赖 CSS `h-full` / `flex-1` 自动撑满**，因为多层 flex 嵌套下浏览器无法正确推导视口高度。

典型模式：

```tsx
// 1. 测量页面总高度
const [pageHeight, pageRef] = useAvailableHeight()
// 2. 测量固定表头/工具栏高度
const [headerHeight, headerRef] = useAvailableHeight()
// 3. 计算内容区可用高度
const contentHeight = Math.max(0, pageHeight - headerHeight - padding)
// 4. 传入滚动容器
<ScrollPage style={{ height: contentHeight || undefined }} variant="borderless" />
// 或直接传入内容组件（组件需接受 style prop）
<LogViewer style={{ height: contentHeight || undefined }} />
```

关键规则：

| 规则 | 说明 |
|------|------|
| 使用 `useAvailableHeight` | 禁止用 `h-full` / `flex-1` 猜测高度；必须实测后传入 |
| 滚动容器用 `ScrollPage` | 统一使用 `components/ui/scroll-page`，支持 `variant` / `scrollbarVisible` 等定制 |
| flex 子项加 `min-h-0` | flex 子项内含 ScrollArea / ScrollPage 时必须加 `min-h-0`，否则内容溢出不触发滚动 |
| 组件接受 `style` prop | 需要精确高度的组件（如 LogViewer）应声明 `style?: React.CSSProperties`，由父级传入计算值 |
| TabsContent 内的滚动 | Tab 页内容区高度 = 页面高度 - Tab 栏 - 筛选栏 - 内边距；不同 Tab 可共享同一计算值 |
| 禁止双层滚动容器 | 已使用 `useAvailableHeight` + `ScrollableTable` 做内部滚动的组件，**禁止**外层再包 `ScrollPage`；Radix ScrollArea 会注入 `display: table; min-width: 100%` 的包裹 div，导致内容按固有尺寸撑开、固定高度失效 |

**典型反例**：`ModelList` 已内部管理滚动，若外层再套 `<ScrollPage>`，Card 会被 Radix 的 table 包裹层撑开，表格无法滚动。正确做法是父级容器直接给固定高度并加 `overflow-hidden`。

---

## 6. 编码规范

### 6.1 通用

- 文件名：`kebab-case`；组件名：`PascalCase`；导出 Props：`{Name}Props`
- 注释：通用组件顶部 JSDoc；复杂逻辑、Rust Command、托盘等用**中文**说明职责
- 修改文件时保持原编码、换行风格，**禁止**无关全文件格式化
- 默认 UTF-8

### 6.2 前端

- 路径别名 `@/` → `src/`
- 列表/变更：用现有 hooks（`use-provider-list`、`use-cli-mutation` 等），勿直接散落 `invoke`
- 错误：走 `IcodeError` 体系与 toast，勿吞错
- 大整数/金额：可能超 `Number.MAX_SAFE_INTEGER` 时后端用 string；金额勿用浮点运算

### 6.3 后端

- 新 Command 在对应 `modules/*/commands.rs` 定义，并在 `main.rs` 的 `invoke_handler` 注册
- 错误统一 `IcodeError`（`error.rs`）
- 跨表写操作使用事务
- 迁移：`src-tauri/src/db/migrations/V{nnn}__{desc}.sql`，只追加不改历史。**新增迁移文件后必须完成 3 步注册**，否则迁移不会执行：
  1. `src-tauri/src/db/migrations.rs`：添加 `const V{nnn}__{NAME}: &str = include_str!("./migrations/V{nnn}__{desc}.sql");`
  2. `src-tauri/src/db/migrations.rs`：在 `BUILTIN_MIGRATIONS` 数组末尾追加 `(n, "desc", V{nnn}__{NAME})`
  3. `src-tauri/src/db/schema.rs`：将 `SCHEMA_VERSION` 常量更新为最新版本号
- Secret 引用扫描与解析仅后端；配置 JSON 中保留 `$SECRET:{uuid}$`

### 6.4 与参考项目对齐

- 类型字段、认证多态、内置供应商/模型尽量对齐  
  `参考项目/vscode-unify-chat-provider-7.12.3/`
- 内置数据：`src-tauri/data/builtin-*.json` + `scripts/convert-well-known.cjs`

---

## 7. 常用命令

```bash
pnpm install          # 安装依赖
pnpm dev              # 仅前端 Vite
pnpm tauri:dev        # 桌面端开发（推荐）
pnpm tauri:build      # 打包
pnpm type-check       # tsc --noEmit
pnpm lint             # eslint
pnpm build            # 前端生产构建
```

Rust 侧在 `src-tauri/` 下用 `cargo check` / `cargo test`（按需）。

---

## 8. 实现状态（改功能前先对齐）

| 能力 | 状态 |
|------|------|
| 供应商/模型 CRUD、设置、Secret 本地加密 | 已实现 |
| 网关运行时（health/models/chat/messages） | 部分（`/v1/responses` 等可能缺失） |
| 虚拟供应商 | 部分（策略枚举可能与文档不一致，改前查代码） |
| 额度脚本模板（Rhai） | 已实现 CRUD / 试运行 / 刷新分发；编辑器高亮用 JS 近似 |
| 脚本模板市场（公共仓浏览 / 一键应用） | 已实现 Phase 1 MVP（列表/筛选/预览/应用/draft） |
| CLI / Workspace 业务 | 类型与骨架为主，完整流程待迭代 |
| 系统密钥链 Secret | 未实现（仅 AES 本地） |
| 内置 seed 数据 | 以 `data/builtin-*.json` 与迁移为准 |

**以代码为准**；文档冲突时优先代码，并在合适时回写 `docs/`。  
审查结论见 `docs/implementation-review.md`。

---

## 9. Agent 行为约定

1. **先读后改**：相关 `types.ts` / `types.rs`、现有 UI 模式、对应 Command 名称先查再写。
2. **小步修改**：不做无关重构；不整文件重写。
3. **前后端同步**：改 DTO 时同时改 Rust 与 TS；新增 Command 必须注册。
4. **UI 紧凑**：900×700 窗口内验证密度，避免大留白与超大字号。
5. **安全**：不把 Secret 明文写进日志、导出配置默认不含明文 Key。
6. **i18n**：新增用户可见字符串同步 `zh-CN` 与 `en`。
7. **技能优先**：
   - UI 视觉与布局：`skills/anthropics-skills-frontend-design`
   - shadcn 组件：`skills/shadcn-ui-shadcn`
   - Vite 配置：`skills/antfu-skills-vite`（若涉及构建）
8. **详细设计不重复造**：架构/表结构/拦截器链以 `docs/development.md`、`docs/database.md` 为准。
9. **Changelog 写入规范**（`CHANGELOG.md`）：
   - **不写 i18n 变更**：翻译文件（`zh-CN` / `en` / `ja` / `zh-TW` 等）的改动不写入变更文档。
   - **不写敏感信息**：禁止将 API Key / Token / Secret 明文、内部路径、SQL 详情等敏感信息写入变更文档；涉及敏感信息时以笼统描述代替。
   - **格式**：使用下述固定结构，按版本追加在 `[release-version-tempalte]` 之后：
     ```markdown
     ## [release-version]

     > [!IMPORTANT]
     > 一般提示应保持简短。

     ### 🚀新增

     ### 🐞修复

     ### 🔄变更
     ```
   - `> [!IMPORTANT]` 提示仅在**数据库结构发生变更**时才需要添加，其余情况省略。
   - `> [!WARNING]` 提示仅在**可能包含未完善功能**时才需要添加，其余情况省略。
   - `> [!NOTE]` 一般提示，无特殊要求，可省略。

---

## 10. 网关与数据流（速记）

```
CLI / 外部客户端
    → 本地 Gateway (axum)
    → 解析 model = provider_slug/model_id
    → 若虚拟供应商：virtual-provider 故障转移选路
    → 解析 $SECRET: 后转发真实上游
    → 响应拦截器异步写 logger + call-records
```

工作区：

```
编辑 Prompts/MCP/Skill → pending_apply=1 → 用户「应用」。
    → 按 CLI 类型生成配置 → 写入 cli_profiles.config_file_path
```

### 10.1 网关响应格式硬约束

- **SSE 流式透传**：上游供应商已返回 `text/event-stream` 字节流时，网关必须**原样透传**，禁止再用 `axum::response::sse::Event` 二次包装为 `data: data: ...`。响应头必须显式设置 `Content-Type: text/event-stream` 与 `Cache-Control: no-cache`。
- **错误响应标准化**：网关对外接口（`/v1/chat/completions`、`/v1/messages`、`/v1/models` 等）返回的错误体必须符合 OpenAI 标准格式 `{ error: { message, type, param, code } }`，不得将 `DATABASE` / `INTERNAL` 等内部错误码或 SQL/堆栈详情暴露给外部客户端。

---

## 11. 日志框架选择（必须按场景选择）

项目同时存在两套日志机制，**互不干扰**，开发/需求阶段必须明确使用哪一套。

| 场景 | 应使用 | 不要混淆为 |
|------|--------|------------|
| 开发调试、Rust 后端运行时追踪、需要输出到终端 / WebView 控制台 / 日志文件 | **[tauri-plugin-log 日志]**<br>后端用 `log::info!` / `log::warn!` / `log::error!` / `log::debug!` | 自研内存 logger |
| 业务运行时诊断、需要出现在应用内「日志」页面、供用户/运维查看（网关请求、系统事件、Command 调用） | **[自研内存 logger]**<br>后端用 `crate::modules::logger::Log::info` 等；前端用 `src/modules/logger/logger.ts` 的 `logger.info` 等 | tauri-plugin-log |

### 11.1 tauri-plugin-log 日志

- **定位**：开发/运行时通用日志通道。
- **后端写法**：`log::info!("...")`、`log::warn!("...")` 等。
- **输出目标**：终端、WebView 控制台、应用日志目录文件。
- **级别控制**：`app_settings.log_level`（全局设置 → 日志级别）。
- **何时使用**：模块启动信息、循环调试、错误堆栈、临时追踪。

### 11.2 自研内存 logger

- **定位**：业务/运维可见的诊断日志，聚焦网关请求、供应商 API 调用、系统事件。
- **后端写法**：
  ```rust
  use crate::modules::logger::Log;
  Log::info("模块启动完成");
  Log::error_with_loc("发生错误", file!(), line!());
  ```
- **前端写法**：
  ```ts
  import { logger } from '@/modules/logger/logger'
  await logger.info('用户执行了导出操作')
  ```
- **输出目标**：内存环形缓冲区（应用内「日志」页面），可选按天滚动文件。
- **级别控制**：`log_settings` 表中的各项开关（转发日志、Command 日志、文件阈值等）。
- **何时使用**：需要在「日志」页面展示、需要按来源/级别/时间筛选、需要导出给运维分析。

### 11.3 关键边界

- 两套日志**不共享级别控制**：修改全局设置的「日志级别」只影响 `tauri-plugin-log`；修改「日志」页面的设置只影响自研内存 logger。
- **不要把 Secret 明文写入任何一套日志**。
- 新增需求若涉及"记日志"，必须在需求/设计阶段明确写入哪一套；若无法确定，优先使用 `tauri-plugin-log`，因为自研 logger 有 UI 可见性要求。
- **网关与供应商 API 同时写入两套日志**：自研 logger 提供 UI 可见、可筛选的运行时诊断；`tauri-plugin-log` 输出完整、未截断的请求/响应体与流式 chunk，供开发/运维在终端/日志文件追踪。SSE chunk 使用独立 target `i_code::sse` 写入单独按小时滚动的 `i-code-sse.*.log`（不混入主日志、不打印 target/file:line）。具体格式与级别约定见 [`docs/log-framework.md` §2.5](docs/log-framework.md)。
- **自研 logger 展示请求头（去敏）**：网关（inbound 入站头）与供应商 API（outbound 出站头，缺失回退 inbound）日志展示请求头 JSON（敏感头值替换为 `***`），位于「模型 ID」下方一行，随导出写入 CSV/JSON。去敏规则见 [`docs/log-framework.md` §3.5](docs/log-framework.md)。

---

## 12. 前后端交互规范

### 12.1 Command 调用

- 前端统一通过 `invokeCommand`（`src/hooks/use-command.ts`）调用 Tauri Command；业务组件禁止直接调用 `invoke`。
- 命令名使用 `snake_case`，按 `模块_动作` 命名，例如 `virtual_provider_create`、`virtual_model_save`。
- 标量参数：Rust 侧使用 `snake_case` 形参；Tauri 默认会把前端 camelCase 键自动映射为 snake_case，因此前端传 `{ virtualProviderId, virtualModelId }` 即可对应 Rust 的 `virtual_provider_id`、`virtual_model_id`。
- 复杂输入对象：统一封装为 Rust DTO，并标注 `#[serde(rename_all = "camelCase")]`；前端对象字段使用 camelCase，例如 `SaveVirtualModelInput`。
- 返回 DTO 同样使用 `#[serde(rename_all = "camelCase")]`，前端类型定义保持 camelCase。
- 新增 Command 必须在 `src-tauri/src/main.rs` 的 `invoke_handler` 中注册，并在对应模块的 `commands.rs` 中声明。

### 12.2 错误结构

- 后端所有 Command 返回 `IcodeResult<T>`，错误类型统一为 `IcodeError`（`src-tauri/src/error.rs`）。
- 错误对象序列化后结构固定为 `{ code, message, details? }`，其中 `code` 为 `SCREAMING_SNAKE_CASE`（`VALIDATION`、`NOT_FOUND`、`DATABASE` 等）。
- 禁止把堆栈、内部路径、SQL 原文或敏感信息放进 `message`；数据库等底层异常应在 Service / Repository 层转换为 `IcodeError`。
- 前端通过 `toIcodeError`（`src/core/errors.ts`）解析，统一转换为 `IcodeError` 实例后再交给 toast / 表单错误处理。
- UI 层禁止直接把捕获的异常 `String(e)` 展示给用户，避免出现 `[object Object]`。

### 12.3 Secret 传输

- API Key / Token 等敏感字段禁止明文落库或写入日志。
- 前端仅在输入时接触明文，通过 Command 一次性传往后端；后端加密后返回 `$SECRET:{uuid}$` 引用。
- 配置 JSON、数据库中只存 Secret 引用；明文解析仅在 Rust 后端完成。

### 12.4 类型同步

- Rust DTO 优先使用 `#[serde(rename_all = "camelCase")]` 向前端对齐。
- 关键类型通过 `ts-rs` 自动生成 TypeScript 定义（如 `ai-gateway`）；手工维护的类型（如 `virtual-provider`）修改时必须前后端同步。
- 新增 / 修改 DTO 后必须执行 `pnpm type-check` 与 `cargo check`，确保两端字段一致。

### 12.5 事件

- 后端主动通知使用 `app.emit` / `window.emit`；前端通过 `listen` 接收。
- 事件名使用 `kebab-case`，例如 `memory-usage`、`provider-changed`。

---

## 13. 相关技能路径

项目内技能（相对仓库根或 `.agents/skills`）：

- `anthropics-skills-frontend-design` — 有意的视觉设计，避免模板感
- `shadcn-ui-shadcn` — shadcn 组件增删改与组合
- `antfu-skills-vite` — Vite 配置与插件
- `wshobson-agents-typescript-advanced-types` — 复杂 TS 类型

---

*维护说明：架构或模块边界变更时，同步更新本文 §3、§8；日志框架约定变更时同步更新本文 §11；前后端交互约定变更时同步更新本文 §12。细节仍写入 `docs/`，保持本文简短可执行。*