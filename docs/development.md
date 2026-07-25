# i-code 开发设计文档

> 版本：v0.2.0  
> 对应数据库设计：`docs/database.md`  
> 参考项目：`参考项目/vscode-unify-chat-provider-7.12.3/src`

本文档面向 i-code 的前后端开发人员，定义项目目标、技术栈、分层架构、模块边界、目录结构、核心流程与开发规范。数据库 Schema 细节不再重复，请参阅 `docs/database.md`。

---

## 1. 项目概述与目标

### 1.1 项目定位

i-code 是一款基于 Tauri 2.x 的本地桌面应用，用于：

1. **管理 AI Gateway 供应商**：集中维护多个 LLM API 供应商（OpenAI、Anthropic、Gemini、OpenRouter 等），支持多种协议与认证方式。
2. **提供本地 API Gateway**：将所有已配置模型统一暴露为 `{provider_slug}/{model_id}` 格式，本地监听并代理请求。
3. **管理 CLI 配置**：为 Claude Code、Codex、Gemini CLI 等受管 CLI 维护配置档案，支持直连或路由到本地 Gateway。
4. **工作区隔离**：按工作区（Workspace）隔离 Prompts、MCP、Skill 配置，切换并应用后才写入 CLI 实际配置文件。

### 1.2 核心目标

- **统一入口**：一处配置，多处使用；CLI 通过路由模式复用同一套 Gateway 供应商。
- **安全存储**：API Key、OAuth Token、代理认证等敏感信息目前加密落库，使用aes密钥（界面可配置或快捷生成）加密。
- **模块独立**：像后端 DDD 一样划分模块，各域之间通过显式接口/事件交互，降低耦合。
- **可扩展主题与国际化**：所有 UI 组件默认支持 `dark / light / claude-light / claude-dark / deepseek-light / deepseek-dark` 六种主题，并支持 `zh-CN / en` 两种语言。
- **向后兼容参考项目**：类型、配置字段、内置供应商/模型数据尽量与 `vscode-unify-chat-provider-7.12.3` 对齐，便于未来导入/迁移。

---

## 2. 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri 2.x（Rust + WebView） |
| 前端框架 | React 19 |
| 类型系统 | TypeScript 5（严格模式） |
| 包管理 | pnpm |
| UI 组件库 | shadcn/ui + Tailwind CSS |
| 图标 | Font Awesome / Lucide（fonticon 语义） |
| 国际化 | i18next 或 react-intl（zh-CN / en） |
| 本地存储 | SQLite 3（通过 `tauri-plugin-sql` 或 Rust `rusqlite`） |
| 后端服务 | Rust（Tauri Commands + State + 本地 HTTP Server） |
| 路由/导航 | TanStack Router（文件系统路由、类型安全 loader） |
| 状态管理 | Zustand（前端全局 Store）+ Tauri State（后端全局 State） |
| 表单 | react-hook-form + zod |
| HTTP 客户端 | reqwest（Rust） |
| 类型同步 | ts-rs crate（Rust struct → TypeScript 类型自动生成） |

---

## 3. 架构原则

### 3.1 DDD-like 模块分离

项目按业务域（Bounded Context）切分模块，每个模块拥有独立的 `types / service / repository / ui` 子层：

- `ai-gateway`：供应商、模型、认证、额度、代理。
- `cli-management`：CLI 档案、CLI 供应商绑定、模型映射。
- `workspace`：工作区、Prompts、MCP、Skill。
- `gateway-runtime`：本地 HTTP 网关生命周期与请求路由。
- `virtual-provider`：虚拟供应商、模型路由、故障转移、健康检查。
- `secret`：敏感数据加密与密钥链。
- `settings`：应用全局设置、主题、语言、网关监听地址。
- `backup`：数据库压缩备份与恢复，支持本地目录与 WebDAV。

### 3.2 分层架构

每个模块内部按以下层次组织（前端/后端各有侧重）：

**前端模块层次：**
```
ui/          ← React 组件、页面、hooks（只依赖后端 Commands 与 shared）
types/       ← 类型定义、常量、枚举
```

**后端模块层次：**
```
commands/    ← Tauri Command 声明（参数校验、调用 Service、错误转换）
service/     ← 业务逻辑、编排、状态转换（可调用其他模块 Service 及 Repository）
repository/  ← 数据访问、SQL 映射（仅 backend）
types/       ← 类型定义、DTO
```

跨模块调用规则：

- 前端 UI 层只能通过 Tauri `invoke` 调用后端 Commands，或使用 `core/shared` 中的工具。
- 后端 Service 层可调用其他模块的 Service，但禁止直接访问其他模块的 Repository。
- 后端 Repository 层禁止调用 Service 或发送事件。
- 后端 Repository 直接操作 SQLite；前端通过 Tauri Commands 与后端通信，**前端不存在 Repository 层**。

### 3.3 模块独立与依赖方向

```
core/shared
    ↑
theme / i18n / secret / db / balance / logger / backup
    ↑
settings / ai-gateway / cli-management / workspace / gateway-runtime / virtual-provider
    ↑
frontend (components / pages / hooks)
```

- `core/shared` 位于最底层，提供类型、工具函数、事件总线、错误基类。
- `theme` 与 `i18n` 被所有 UI 模块依赖，但不依赖业务模块。
- `balance` 为独立模块，被 `ai-gateway` 和 `gateway-runtime` 依赖。
- `logger` 为独立模块，被 `gateway-runtime` 依赖（记录请求/响应日志），也可由前端直接查看。
- `backup` 为独立模块，依赖 `db` 和 `secret`，仅由前端通过 Commands 触发，不反向依赖业务模块。
- `virtual-provider` 依赖 `ai-gateway`（读取真实供应商/模型）与 `gateway-runtime`（在转发时执行故障转移），并为 `cli-management` 提供可选的虚拟 CLI 供应商绑定。
- 业务模块之间允许同层调用，但优先通过事件或 Service 接口解耦。
- 禁止循环依赖：若 A 模块 Service 需要 B 模块数据，应通过 B 模块 Service 暴露的只读接口获取，而非直接读表。

---

## 4. 项目目录结构

```
i-code/
├── docs/
│   ├── database.md              # 数据库设计文档（Schema、JSON Schema、流程）
│   └── development.md           # 本文档
├── src/                         # ── 前端（React + TypeScript）──
│   ├── core/                    # 跨域共享层（零业务依赖）
│   │   ├── types.ts             # 全局基础类型（ID、时间戳、Result、Paging）
│   │   ├── errors.ts            # 业务错误基类（IcodeError、ValidationError、NotFoundError）
│   │   ├── events.ts            # 前端事件总线（mitt）与后端 Tauri Event 事件名常量
│   │   ├── utils.ts             # 纯函数工具（deepClone、assertNever、uuid 等）
│   │   └── constants.ts         # 全局常量（含 $SECRET: 前缀常量）
│   ├── hooks/                   # 跨模块通用 hooks
│   │   ├── use-command.ts       # 封装 Tauri invoke + 错误处理 + 加载态
│   │   ├── use-gateway-status.ts
│   │   ├── use-provider-list.ts
│   │   ├── use-model-list.ts
│   │   └── use-workspace-applier.ts
│   ├── components/              # 跨模块通用 UI 组件（shadcn/ui 扩展）
│   │   └── ui/                  # shadcn/ui 基础组件与自定义全局组件
│   │       ├── error-boundary.tsx   # 全局错误边界
│   │       ├── toast.tsx
│   │       ├── title-bar.tsx        # 自定义标题栏（含迷你面板入口）
│   │       ├── title-bar-info.tsx   # 标题栏紧凑信息
│   │       ├── memory-info.tsx      # 标题栏内存占用
│   │       ├── dropdown.tsx         # 基础下拉选择
│   │       ├── dropdown-search.tsx  # 可搜索下拉选择
│   │       ├── log-viewer.tsx       # 日志列表展示
│   │       ├── log-panel.tsx        # 基于编辑器的日志面板（缓冲队列）
│   │       ├── log-rolling-config.tsx  # 日志文件滚动记录配置
│   │       ├── mini-floating-panel.tsx  # 迷你悬浮信息卡片（预览页演示用）
│   │       ├── mini-panel.tsx       # 迷你面板容器与设置（预览页演示用）
│   │       ├── virtual-model-graph.tsx  # 虚拟模型路由关系图（中心-子节点）
│   │       ├── tray-info.tsx        # 托盘信息预览
│   │       └── tray-provider-selector.tsx  # 托盘供应商选择
│   ├── modules/
│   │   ├── theme/               # 主题模块（仅有前端 UI）
│   │   │   ├── types.ts
│   │   │   ├── theme-provider.tsx
│   │   │   ├── use-theme.ts
│   │   │   └── themes/          # dark / light / claude-light / claude-dark / deepseek-light / deepseek-dark CSS 变量
│   │   ├── i18n/                # 国际化模块（仅有前端 UI）
│   │   │   ├── types.ts
│   │   │   ├── i18n.ts
│   │   │   ├── use-translation.ts
│   │   │   └── locales/
│   │   │       ├── zh-CN.json
│   │   │       └── en.json
│   │   ├── settings/            # 应用设置模块
│   │   │   ├── types.ts         # 前端类型定义（与后端 DTO 对齐）
│   │   │   └── ui/              # 设置页面组件
│   │   ├── ai-gateway/          # AI Gateway 模块
│   │   │   ├── types.ts         # Provider、GatewayModel、ModelConfig、ProviderShareConfig 等类型
│   │   │   └── ui/              # 供应商/模型列表页、表单页、内置供应商选择、配置导入/导出
│   │   ├── balance/             # 额度监控模块（独立，被 ai-gateway 引用）
│   │   │   ├── types.ts
│   │   │   └── ui/
│   │   ├── cli-management/      # CLI 管理模块
│   │   │   ├── types.ts
│   │   │   └── ui/
│   │   ├── workspace/           # 工作区模块
│   │   │   ├── types.ts
│   │   │   └── ui/
│   │   ├── gateway-runtime/     # 本地网关运行时（前端仅状态查看）
│   │   │   ├── types.ts
│   │   │   └── ui/
│   │   ├── virtual-provider/    # 虚拟供应商与模型故障转移
│   │   │   ├── types.ts
│   │   │   └── ui/
│   │   │       ├── virtual-provider-form.tsx
│   │   │       ├── virtual-model-form.tsx
│   │   │       ├── virtual-model-route-editor.tsx
│   │   │       └── virtual-provider-list.tsx
│   │   ├── logger/              # 日志控制台（前端日志查看与过滤）
│   │   │   ├── types.ts
│   │   │   └── ui/
│   │   │   └── ui/
│   │   │       └── secret-input.tsx  # 密码输入框，传明文给后端 Command
│   │   ├── logger/              # 日志控制台（前端日志查看与过滤）
│   │   │   ├── types.ts
│   │   │   └── ui/
│   │   │       ├── log-viewer.tsx     # 日志列表（实时滚动）
│   │   │       ├── log-filter.tsx     # 过滤面板（级别/来源/关键词/状态码）
│   │   │       └── log-export.tsx     # 导出按钮（JSON / CSV）
│   │   ├── backup/              # 备份与恢复模块（前端配置入口）
│   │   │   ├── types.ts
│   │   │   └── ui/
│   │   │       ├── backup-settings.tsx  # 备份设置页（本地目录、WebDAV 配置）
│   │   │       ├── backup-list.tsx      # 本地/WebDAV 备份列表
│   │   │       └── restore-confirm.tsx  # 恢复前二次确认弹窗
│   ├── pages/                   # 页面路由入口（TanStack Router 文件路由）
│   │   ├── index.tsx            # 首页/仪表盘
│   │   ├── gateways/
│   │   ├── cli/
│   │   ├── workspaces/
│   │   └── settings.tsx
│   ├── App.tsx
│   └── main.tsx
├── src-tauri/                   # ── 后端（Rust）──
│   ├── src/
│   │   ├── main.rs              # Tauri 入口
│   │   ├── error.rs             # IcodeError 统一错误类型
│   │   ├── modules/             # 按模块组织，与前端 modules/ 一一对应
│   │   │   ├── settings/
│   │   │   │   ├── commands.rs
│   │   │   │   ├── service.rs
│   │   │   │   └── repository.rs
│   │   │   ├── ai-gateway/
│   │   │   │   ├── commands.rs
│   │   │   │   ├── service.rs
│   │   │   │   ├── repository.rs
│   │   │   │   └── models.rs     # 模型覆盖合并、官方模型拉取
│   │   │   ├── balance/
│   │   │   │   ├── commands.rs
│   │   │   │   ├── service.rs
│   │   │   │   └── repository.rs
│   │   │   ├── cli-management/
│   │   │   │   ├── commands.rs
│   │   │   │   ├── service.rs
│   │   │   │   └── repository.rs
│   │   │   ├── workspace/
│   │   │   │   ├── commands.rs
│   │   │   │   ├── service.rs     # 含配置文件写入逻辑
│   │   │   │   └── repository.rs
│   │   │   ├── gateway-runtime/
│   │   │   │   ├── commands.rs
│   │   │   │   ├── service.rs     # HTTP server 生命周期管理
│   │   │   │   ├── router.rs      # HTTP 路由定义与请求转发
│   │   │   │   └── auth-middleware.rs
│   │   │   ├── secret/
│   │   │   │   ├── commands.rs
│   │   │   │   ├── service.rs     # 加密/解密、密钥链读写、$SECRET: 引用解析
│   │   │   │   └── repository.rs
│   │   │   └── logger/
│   │   │       ├── commands.rs
│   │   │       ├── service.rs     # 日志记录、查询、导出
│   │   │       └── repository.rs
│   │   │   └── backup/
│   │   │       ├── commands.rs
│   │   │       ├── service.rs     # 创建/推送/拉取/恢复备份
│   │   │       ├── repository.rs  # 数据库连接关闭/重开、路径查询
│   │   │       └── webdav.rs      # WebDAV HTTP 客户端封装
│   │   ├── db/
│   │   │   ├── connection.rs
│   │   │   ├── schema.rs
│   │   │   └── migrations/      # V{version}__{description}.sql
│   │   └── gateway/             # HTTP server 运行时实现（axum 路由注册）
│   │       ├── mod.rs
│   │       ├── routes/
│   │       └── middleware/
│   └── Cargo.toml
├── scripts/                     # 构建与代码生成脚本
│   ├── sync-builtin-data.ts     # 从参考项目导出供应商/模型 seed SQL
│   └── generate-types.ts        # 通过 ts-rs 生成前端 TypeScript 类型
├── package.json
├── tsconfig.json
├── tailwind.config.ts
└── README.md
```

### 4.1 关键约定

- **前端没有 Repository/Service 层**。前端模块仅包含 `types.ts`（类型定义）和 `ui/`（展示组件），所有业务逻辑和数据访问都通过 Tauri `invoke` 调用后端 Commands。前端 `hooks/use-command.ts` 封装了 invoke 调用、错误处理和加载态。
- **前后端模块一一对应**：`src/modules/{name}/` ↔ `src-tauri/src/modules/{name}/`，同一个业务功能的修改只需在两个同名目录中找对应文件。
- **后端模块内部按 commands/service/repository/types 分四层**，每层各有明确职责。
- `core` 中禁止引入任何业务模块类型；业务模块类型可依赖 `core`。
- 后端 `commands.rs` 只做参数校验与调用 Service；`service.rs` 实现业务逻辑；`repository.rs` 做 SQL 数据访问。
- **类型同步**：Rust struct 通过 `ts-rs` crate 自动生成前端 TypeScript 类型定义（见 `scripts/generate-types.rs`），保持前后端类型一致。

---

## 5. 模块设计

### 5.1 core/shared

职责：提供所有业务模块共享的基础设施，保持零业务依赖。

- **types.ts**：全局 ID 类型（`Uuid`、`Slug`）、时间戳格式、`Result<T, E>`、`PagingParams`、排序方向。前端 TypeScript 版本由 `ts-rs` 自动生成，与 Rust 保持一致。
- **errors.ts**：
  - `IcodeError`：基类，含 `code`、`message`、`details`。
  - `ValidationError`：表单/参数校验失败。
  - `NotFoundError`：资源不存在。
  - `AuthError`：认证失败或过期。
  - `GatewayError`：网关请求转发异常。
- **events.ts**：
  - **前端事件总线**：使用 `mitt`，仅在 `src/hooks/` 和 `src/modules/*/ui/` 内使用，用于模块间 UI 状态同步（如弹窗关闭后刷新列表）。
  - **后端事件（Tauri Event）**：后端通过 `app_handle.emit()` 推送事件到前端。前端通过 `listen()` 接收。后端事件用于推送跨进程状态变化（如 Gateway 启停、工作区应用完成）。
  - **事件流方向**：
    ```
    后端状态变化 → Tauri Event emit → 前端 listen() → Zustand store 更新 → UI 重渲染
                                                          ↕
                                                    mitt 内部事件（模块间通信）
    ```
  - **完整事件清单与使用说明**见 `docs/events.md`，事件名常量统一在 `core/events.ts` 中维护。
- **utils.ts**：不可变操作、uuid 生成、日期格式化、 deepClone、类型守卫、字符串处理（如 `$SECRET:{uuid}$` 前缀常量）。

### 5.2 theme

职责：管理 `dark / light /claude-light / claude-dark / deepseek-light / deepseek-dark` 六种主题，并向全局注入 CSS 变量。

- `theme-provider.tsx`：在根组件挂载主题类名，监听系统主题变化。
- `use-theme.ts`：返回当前主题、切换函数、主题列表。
- `themes/*.css`：每种主题定义 `--background`、`--foreground`、`--card`、`--primary`、`--muted`、`--border`、`--ring` 等 shadcn/ui 变量。
- 所有新增组件必须使用这些 CSS 变量，禁止硬编码颜色。

### 5.2.1 UI 组件规范

本小节约束跨模块通用组件、shadcn/ui 扩展组件以及预览/示例组件的书写方式，保证组件库风格一致、可维护、可测试。

#### 目录与分类

- `src/components/ui/`：基础通用组件（Button、Input、Dialog 等 shadcn/ui 扩展，以及 TitleBar、MemoryInfo、Dropdown、DropdownSearch、LogViewer、MiniFloatingPanel 等全局组件）。
- `src/components/preview/`：组件库预览/示例组件，仅用于 `/preview` 页面展示交互，不携带业务逻辑。
- `src/modules/{domain}/ui/`：业务域专属展示组件，可依赖本域 `types.ts` 与后端 Commands。
- 禁止在 `components/ui/` 中引入业务模块类型或服务。

#### 命名与导出

- 文件名使用 `kebab-case.tsx`，与组件名对应（如 `title-bar.tsx` → `TitleBar`）。
- 组件使用函数声明或 `React.forwardRef`，必须导出 Props 接口（命名 `{ComponentName}Props`）。
- 复杂类型优先在组件文件内定义并导出，跨组件共享类型放入 `src/core/types.ts` 或模块 `types.ts`。

#### 图标

- 统一使用 **Font Awesome Free** 图标字体，通过 `<i className="fa-solid fa-{name} ..." />` 或 `fa-regular` 系列使用。
- 图标尺寸统一使用 `size-3` / `size-3.5` / `size-4` / `size-5` Tailwind 工具类，禁止直接写死 `width` / `height`。
- 不再引入 `lucide-react`；已有 Lucide 图标已全部替换为 Font Awesome。

#### 样式与主题

- 所有颜色必须使用 shadcn/ui CSS 变量：`--background`、`--foreground`、`--primary`、`--muted`、`--border`、`--ring` 等。
- 背景透明度、渐变、阴影等效果应基于 CSS 变量计算，例如 `bg-primary/5`、`hsl(var(--primary) / 0.04)`。
- 新增主题时，在 `src/modules/theme/themes/` 下新增 `{theme-name}.css`，并在 `src/index.css`、`src/core/types.ts` 的主题联合类型、`theme-provider.tsx` 的主题列表、i18n 语言包中同步注册。

#### 字体与排版

本项目不引入额外 Web 字体，统一使用系统字体栈以保证加载速度与跨平台一致性。

**字体族：**

```css
font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", "Helvetica Neue", Arial, sans-serif;
```

- 中文环境优先使用系统自带的 **PingFang SC**（macOS / iOS）或 **Microsoft YaHei**（Windows）。
- 英文与数字回退到 **Segoe UI** / **Helvetica Neue** / **Arial**。
- 代码与日志区域使用等宽字体：`'JetBrains Mono', Consolas, 'Courier New', monospace`（优先 JetBrains Mono，回退 Consolas / Courier New）。
- 金额、token 计数、内存等数值展示应使用 `tabular-nums` 保证对齐。

**字号层级：**

| 层级 | Tailwind 类 | 近似 px | 使用场景 |
|------|-------------|---------|----------|
| 辅助/微型 | `text-[10px]` / `text-[9px]` / `text-[8px]` | 10 / 9 / 8 | 标题栏胶囊、迷你面板、徽章、特性标签、上下文键值 |
| 小号 | `text-xs` | 12 | 表单项说明、次级标签、小按钮、CardDescription |
| 正文 | `text-sm` | 14 | 表单输入、正文段落、菜单项、表格内容 |
| 小标题 | `text-base` | 16 | CardTitle、对话框标题、Section 标题 |
| 标题 | `text-lg` | 18 | 页面标题、重要弹窗标题 |
| 大标题 | `text-xl` | 20 | 仪表盘大数字说明 |
| 展示 | `text-2xl`+ | 24+ | 首页核心数据展示（极少使用） |

**字重规范：**

| 字重 | Tailwind 类 | 使用场景 |
|------|-------------|----------|
| 常规 | `font-normal` | 正文、输入框内容 |
| 中等 | `font-medium` | 按钮文字、菜单项、CardTitle、强调标签 |
| 半粗 | `font-semibold` | 页面标题、数值、重要状态 |
| 粗体 | `font-bold` | 关键指标、警告标题 |

**行高规范：**

- 标题类使用 `leading-tight`（1.25）。
- 正文与表单使用默认行高，或 `leading-relaxed`（1.625）用于多行说明。
- 单行胶囊/标签使用 `leading-none` 避免高度抖动。

**禁忌：**

- 禁止在业务组件中写死 `font-family`、`font-size` px 值。
- 禁止为了"更细"的效果使用 `font-thin` / `font-light`，最小字重为 `font-normal`。
- 避免连续使用超过 3 种字号，保持页面节奏统一。

#### 注释

- 每个通用组件顶部必须包含 JSDoc / 块注释，说明组件职责、主要 Props 用途。
- 复杂逻辑、副作用、事件监听、格式化规则等需添加行内中文注释。
- Rust Command 与 Tray 等基础设施代码同样需要中文注释，说明当前为占位逻辑还是已完成逻辑。

#### 数值与精度

- **内存**：统一由 Rust 返回 KB，前端使用 `formatMemory(kb)` 格式化为 `KB / MB / GB`（边界为 1 MB、1 GB）。
- **计数（token、额度等）**：统一使用 `formatCompactCount(n)` 格式化为 `K / W / B`（1 K = 1,000；1 W = 10,000；1 B = 100,000,000）。
- 格式化函数位于 `src/core/utils.ts`，禁止在组件内重复实现。

**精度注意事项：**

- JavaScript `number` 的精确整数上限为 `Number.MAX_SAFE_INTEGER`（`9,007,199,254,740,991`，约 9e15）。超过此值时，Rust 端返回的 `u64` / `i64` 经 JSON 解析到前端会丢失精度。
- 对于可能超过 9e15 的数值（如累计 token 数、大额余额、某些统计计数），**后端 DTO 中应使用字符串类型**，前端同样以 `string` 接收与存储，仅在展示或计算时按需转换。
- 金额、额度等涉及财务精度的字段，后端统一以最小货币单位（如美分）或字符串形式的定点数存储；禁止在前端使用浮点数进行金额运算，避免 `0.1 + 0.2 !== 0.3` 类误差。
- 需要在前端进行大整数运算时，优先使用 `BigInt`；需要小数精度时，优先引入 `decimal.js` / `big.js` 等库，并在 `core/utils.ts` 中封装统一工具函数。
- Rust 返回给前端的整数类型应尽量与前端安全范围匹配：
  - 确定小于 2^53 的值可用 `u64` / `i64`（前端按 `number` 处理）。
  - 可能超过 2^53 的值应在 Rust 中序列化为字符串，或拆分为多个 `u32` 字段。
- SQLite `INTEGER` 最大可存 64 位有符号整数；若业务需要无符号 64 位最大值（`18,446,744,073,709,551,615`），请在应用层以字符串或 `i128` 映射处理。

#### 标题栏与托盘组件约定

- `TitleBar` 固定定位在页面顶部，通过 `info` 插槽展示 `TitleBarInfo`、`MemoryInfo` 等紧凑信息；左侧提供闪电图标入口，用于切换迷你面板模式。
- `TitleBarInfo` 仅展示纯数据；数值类型自动调用 `formatCompactCount`。
- `MemoryInfo` 通过 `useMemoryUsage` Hook 监听 Rust `memory-usage` 事件并展示本进程内存。
- `TrayInfo` / `TrayProviderSelector` 为纯展示组件，数据与回调由调用方传入；实际托盘菜单与事件由 Rust 侧维护。
- 迷你面板为独立 Tauri 窗口（`decorations: false`、`always_on_top: true`、`skip_taskbar: true`），由标题栏闪电图标触发 `open_mini_panel` 命令打开。迷你窗口路由（`/mini-panel`）在根布局中跳过 TitleBar 渲染，整个页面无标题栏、无滚动条，支持 `data-tauri-drag-region` 拖拽。关闭迷你窗口时自动显示并聚焦主窗口。迷你面板有「正常」与「最小化」两种模式，详见 §5.17。

#### 主窗口关闭与系统托盘行为

- **关闭 → 隐藏到托盘**：主窗口右上角 × 按钮、Alt+F4 或系统菜单关闭均触发 `on_window_event` 拦截，调用 `api.prevent_close()` + `window.hide()` 隐藏到托盘，**不退出进程**。
- **恢复主窗口**：右键托盘图标 → 点击「显示主界面」→ `window.show()` + `window.unminimize()` + `window.set_focus()`。
- **彻底退出**：右键托盘图标 → 点击「退出」→ `app.exit(0)`。
- **开机自启时自动隐藏**：若启动参数包含 `--autostart`（由 `tauri-plugin-autostart` 注入），`setup` 阶段自动 `window.hide()` 隐藏主窗口。详见 §5.4.1。

#### 自定义组件补充约定

- **Dropdown / DropdownSearch**：基于 Radix Popover 封装的单选下拉组件，使用 Font Awesome 图标；`DropdownSearch` 内置输入框并支持实时过滤。两者均通过 `value/onChange` 受控，选项类型支持 `value`、`label`、`icon`、`disabled`。
- **LogViewer**：纯展示型日志列表，支持 `debug / info / warn / error` 级别颜色区分、时间格式化、行点击高亮。过滤、导出、实时追加由调用方控制；日志数据通过 `LogEntry[]` 传入。
- **LogPanel**：基于 `CodeEditor` 的日志面板，仅展示缓冲队列中的最新日志，超出 `bufferSize` 的旧日志自动丢弃。适合需要复制、搜索、主题融合的日志展示场景。
- **LogRollingConfig**：日志滚动记录配置组件，用于配置内存缓冲队列大小、是否启用本地日志文件、单文件大小上限、保留文件数量与保留天数。当前为纯 UI，后端 Service 按此配置执行文件写入与清理。
- **MiniFloatingPanel**：迷你悬浮信息卡片（预览页演示用），以紧凑形式展示当前供应商、模型、额度进度、网关状态与模型消耗趋势。提供「面积图模式」与「数字模式」两种展示；当未提供 `chartData` 时自动 fallback 为数字模式。所有数值通过 `formatCompactCount` 格式化。
- **MiniPanel**：迷你面板容器（预览页演示用），通过标题栏闪电图标触发后覆盖主界面。内部组合 `MiniFloatingPanel` 与设置卡片，支持调整展示形式（compact / normal / expanded）、信息展示量（minimal / normal / full），并提供「切换主界面」入口。
- **MiniPanelPage**（`routes/mini-panel.tsx`）：迷你面板独立窗口页面，为实际运行时使用的组件。不使用组件库 Card/Panel 容器，直接以原生元素紧凑排列。详见 §5.17。

### 5.3 i18n

职责：提供 `zh-CN / en` 双语支持。

- `i18n.ts`：初始化 i18next，加载语言包，默认语言从 `app_settings.locale` 读取。
- `use-translation.ts`：封装 `useTranslation`，支持命名空间（如 `t('provider.form.title')`）。
- `locales/*.json`：按模块命名空间组织键名，例如 `ai-gateway.provider.name`、`workspace.prompt.empty`。
- 所有用户可见字符串必须走 i18n；键名采用 `模块.页面.元素` 三段式。

### 5.4 settings/app

职责：管理应用全局设置、主题、语言、网络超时与重试、配置密钥。

- **类型**：`AppSettings`（对应 `app_settings` 表）。
- **Service**：
  - `getSettings()` / `updateSettings(partial)`
  - `setTheme(theme)` / `setLocale(locale)`
- **Repository**：读写 `app_settings` 单例行，维护 `updated_at`。
- **UI**：设置页面，包含主题切换、语言切换、全局代理、配置密钥、超时重试折叠面板。

#### 5.4.1 开机自启（Auto-start）

i-code 支持开机自启功能，使用 `tauri-plugin-autostart` 跨平台插件实现。

**插件注册**（`src-tauri/src/main.rs`）：

```rust
.plugin(tauri_plugin_autostart::Builder::new()
    .args(["--autostart"])
    .build())
```

关键设计点：

| 项目 | 说明 |
|------|------|
| 自启参数 | 插件注册时传入 `--autostart`，启动时通过 `std::env::args()` 检测是否包含该参数 |
| 自启时隐藏窗口 | 检测到 `--autostart` 参数后，自动隐藏主窗口到系统托盘（`window.hide()`），不显示界面 |
| 自启时恢复网关 | 若上次网关处于运行状态（`gateway_last_running=true`），自启时通过事件机制自动启动网关 |
| 自启开关持久化 | `auto_start_enabled` 存储在 `app_settings` 表，前端切换开关时同步调用插件 `enable/disable` |
| 系统注册 | 插件自动在各平台注册自启入口：Windows（注册表 `HKCU\...\Run`）、macOS（LaunchAgent）、Linux（XDG autostart） |
| 日志记录 | 自启启用/关闭/失败均通过 `log::info!` / `log::error!` 输出系统日志 |

**前端调用**（`src/routes/settings.tsx`）：

```typescript
import { enable as autostartEnable, disable as autostartDisable } from '@tauri-apps/plugin-autostart'

// 开启
autostartEnable().catch((e) => toast.error(`开机自启启用失败: ${e}`))
// 关闭
autostartDisable().catch((e) => toast.error(`开机自启关闭失败: ${e}`))
```

**托盘菜单切换**（Rust 侧 `on_menu_event`）：

```rust
"auto-start" => {
    let autolaunch = app.autolaunch();  // 需导入 ManagerExt trait
    if new_val { autolaunch.enable()? } else { autolaunch.disable()? }
}
```

**自启启动流程**：

```
系统启动 → 调用 i-code --autostart
    → main.rs setup 检测 args 包含 "--autostart"
    → window.hide() 隐藏主窗口到托盘
    → 若 gateway_last_running=true → emit("gateway:toggle-request") 启动网关
    → 用户通过托盘「显示主界面」恢复窗口
```

### 5.5 ai-gateway

职责：维护 AI Gateway 供应商、模型、认证、额度与代理配置。

#### 5.5.1 核心类型

- `Provider`：对应 `providers` 表，包含 `slug`（路由标识）、`display_name`、`provider_type`、`base_url`、认证、代理等。
- `GatewayModel`：对应 `gateway_models` 表，暴露给外部的模型，ID 格式为 `{provider_slug}/{model_id}`。
- `ModelConfig`：对应 `model_configs` 表，保存完整模型参数。
- `BuiltinProvider` / `BuiltinModel`：内置种子数据。
- `AuthConfig`：多态联合类型（`none / api-key / oauth2 / google-vertex-ai-auth / ...`）。
- `ProxyConfig` / `TimeoutConfig` / `RetryConfig`。
- `ProviderShareConfig`：用于分享导出的供应商配置 DTO，可被序列化为 base64 JSON。结构包含 `version`、`provider`（Provider + extra headers/body）、`models`（GatewayModel[] + 关联 ModelConfig）、`missingSecrets`（可选，标记缺失的 Secret 引用）。

```typescript
// 分享配置 DTO 示例
interface ProviderShareConfig {
  version: '1.0';
  provider: Provider & {
    extraHeaders: Record<string, string>;
    extraBody: Record<string, unknown>;
  };
  models: Array<{
    gatewayModel: GatewayModel;
    modelConfig: ModelConfig;
    extraHeaders?: Record<string, string>;
    extraBody?: Record<string, unknown>;
  }>;
  missingSecrets?: string[];
}
```

#### 5.5.2 Service 职责

- **Provider CRUD**：创建、更新、删除、启用/禁用供应商；校验 `slug` 全局唯一。
- **Secret 引用处理**：在保存/读取供应商时，将 API Key 等敏感值交给 `secret` 模块加密，配置中仅存 `$SECRET:{uuid}$`。
- **从内置供应商列表添加**：选择 `builtin_providers` 预设，复制其默认配置（base_url、provider_type、auth_types、默认 auth/balance、extra headers/body 等）生成新 `Provider` 草稿，用户补充名称/凭证后保存。
- **从配置导入（base64 JSON）**：解析 base64 编码的 `ProviderShareConfig`，校验 schema 与 slug 唯一性，将敏感字段交给 `secret` 模块加密后落地；支持导入单条或多条供应商配置。
- **导出/分享配置**：将指定供应商（含关联的 `provider_extra_headers`、`provider_extra_body`、`gateway_models` / `model_configs`）序列化为 `ProviderShareConfig`，base64 编码后供用户复制；默认不导出 Secret 明文，仅导出引用或提示缺失。
- **模型管理**：
  - 手动添加模型：用户填写模型 ID 与参数。
  - 从内置列表添加：选择 `builtin_providers` 推荐的 `builtin_models`，按供应商类型合并 `builtin_model_overrides`。
  - 从官方列表添加：通过供应商 API 拉取 `official_model_cache`，匹配 `builtin_model_aliases` 做映射。
- **额度刷新**：调用 `balance` 模块 Service 按 `BalanceConfig` 获取余额，更新 `cli_providers.balance_json`。
- **模型暴露策略**：网关 `/v1/models` 仅返回 `providers.is_enabled = 1` 且 `gateway_models.is_exposed = 1` 的模型。
- **官方模型拉取**：通过后端 Rust Service（`ai-gateway/models.rs`）调用供应商 API 拉取模型列表，写入 `official_model_cache`；前端只触发拉取命令并展示结果。

#### 5.5.3 Repository 职责

- `providers` 表及 `provider_extra_headers / provider_extra_body` 关联表的读写。
- `model_configs`、`gateway_models`、`official_model_cache`、`builtin_*` 表的读写。
- 在更新/删除供应商时处理级联与外键约束（如 `gateway_models` CASCADE、`cli_providers.provider_id` SET NULL）。

#### 5.5.4 UI 职责

- 供应商列表页、供应商表单页（参考参考项目 `provider-form-screen`）。
- **添加供应商入口**：右上角「+」下拉菜单提供：
  - 添加供应商（空白表单）
  - 从内置供应商列表添加（网格/列表选择 `builtin_providers`）
  - 从配置导入（粘贴 base64 JSON，预览后确认）
- 供应商卡片操作：编辑、删除、复制、导出 base64 配置。
- 模型列表页、模型表单页、添加模型来源选择弹窗（内置/官方/手动）。
- 认证表单动态渲染：根据 `auth.method` 切换 `api-key-input`、`oauth-callback`、`vertex-ai-auth` 等子组件。

### 5.6 cli-management

职责：管理受管 CLI 的配置档案、供应商绑定与模型映射。

#### 5.6.1 核心类型

- `CliProfile`：对应 `cli_profiles` 表，`cli_type` 支持 `claude-code / codex / gemini-cli / cursor-agent / custom`。
- `CliProvider`：对应 `cli_providers` 表，绑定 Gateway 供应商或直连地址，支持 `route_mode`。
- `CliModelMapping`：对应 `cli_model_mappings` 表，`cli_model_alias` 为 CLI 内使用的模型名。

#### 5.6.2 Service 职责

- **CLI Profile CRUD**：增删改查 CLI 档案。
- **绑定 Gateway 供应商**：从 `ai-gateway` 选择已启用供应商，生成 `CliProvider`。
- **路由模式**：当 `route_mode = 1` 时，CLI base_url 指向本地 Gateway，`model` 字段保持 `{provider_slug}/{model_id}`；网关层拆分后路由。
- **模型映射**：支持从列表选择（填充 `gateway_model_id`）或手动输入（填充 `raw_model_id`）。
- **额度展示**：读取 `CliProvider.balance_json` 并渲染。
- **配置写入**：在「应用」工作区时，将当前工作区对应 CLI 的配置写入 `cli_profiles.config_file_path`。

#### 5.6.3 UI 职责

- CLI 列表页、CLI 档案表单。
- CLI 供应商绑定弹窗（选择 Gateway Provider + 是否路由模式）。
- 模型映射表格（CLI 模型别名 ↔ Gateway 模型/真实模型）。

### 5.7 workspace

职责：按工作区隔离 Prompts、MCP、Skill，并在切换/应用时才修改 CLI 实际配置文件。

#### 5.7.1 核心类型

- `Workspace`：对应 `workspaces` 表，`is_active` 标识当前激活工作区。
- `WorkspaceCliConfig`：对应 `workspace_cli_configs` 表，每个工作区 × 每个 CLI 一条记录。
- `WorkspacePrompt` / `WorkspaceMcpServer` / `WorkspaceSkill`：三类子配置。

#### 5.7.2 Service 职责

- **Workspace CRUD**：增删改查工作区，切换激活状态。
- **子配置隔离**：为每个 `WorkspaceCliConfig` 维护独立的 Prompts、MCP、Skill 列表。
- **应用工作区**：
  1. 读取当前激活工作区下所有 `workspace_cli_configs`。
  2. 按 CLI 类型生成对应配置文件内容（JSON/YAML）。
  3. 写入 `cli_profiles.config_file_path`。
  4. 更新 `is_applied = 1`、`pending_apply = 0`、`last_applied_at`。
- **待应用提示**：当用户修改工作区子配置后，设置 `pending_apply = 1`，UI 显示「有未应用变更」。

#### 5.7.3 UI 职责

- 工作区侧边栏/下拉切换。
- 工作区编辑表单。
- 每个 CLI 的 Prompts / MCP / Skill 编辑页。
- 「应用」按钮与待应用状态提示。

### 5.8 gateway-runtime

职责：启动本地 HTTP 服务，接收来自内部 CLI 或外部客户端的 OpenAI 兼容请求，并路由到真实供应商。

#### 5.8.1 核心类型

- `GatewayRuntimeState`：运行时状态（**仅在内存中维护**，不持久化到数据库），包含 `is_running`、`bound_host`、`bound_port`、`started_at`、`last_error`。
- `GatewayRequest` / `GatewayResponse`：标准化请求/响应封装。

#### 5.8.2 Service 职责

- **生命周期**：根据 `gateway_settings.gateway_host / gateway_port` 启动/停止本地 HTTP Server（默认 `127.0.0.1:54321`，支持 `0.0.0.0`）。状态存储在 Tauri State 中，不写库。
- **模型列表接口**：实现 `/v1/models`，仅返回已暴露模型。
- **聊天补全/响应接口**：实现 `/v1/chat/completions`、`/v1/responses`。
- **健康检查**：实现 `GET /health`（存活检查）和 `GET /readyz`（就绪检查，验证数据库与上游供应商可达性）。
- **模型 ID 解析**：从请求体 `model` 字段拆分 `provider_slug` 与 `model_id`，查询 `providers` 与 `gateway_models`。
- **认证豁免**：识别来自内部 CLI 的请求（如特定 Header 或来源 IP 白名单），可跳过 Gateway API Key 校验。
- **转发与流式响应**：调用 `ai-gateway` Service 获取目标供应商配置，使用对应协议客户端转发请求，并流式返回 SSE 数据给客户端。流式错误处理：
  - 上游断开：向客户端发送最后一个有效的 SSE chunk + `error` event，不阻塞。
  - 客户端取消：通过 HTTP `request.abort()` 传播 cancellation 到上游请求。
  - 超时：按 `TimeoutConfig` 控制连接超时和响应超时，超时后返回 504。

#### 5.8.3 Router 职责

- 注册路由：
  - `GET /health`
  - `GET /readyz`
  - `GET /v1/models`
  - `POST /v1/chat/completions`
  - `POST /v1/responses`
  - `POST /v1/embeddings`（可选）
- 中间件：API Key 校验、请求日志、超时控制、CORS。

#### 5.8.4 网关概览页图表

`src/routes/gateways/index.tsx` 的「网关」标签页通过三张图表展示运行态流量：

| 图表 | 组件 | 默认窗口 | 默认粒度 | 展示维度 |
|---|---|---|---|---|
| 请求流量 | `GatewayTrafficChart` | 1 小时（可拖动滑块调整 1-24 小时） | 按窗口动态变化 | 总请求数（不区分模型） |
| 请求趋势 | `GatewayTrendChart` | 1 小时 | 10 分钟 | 每个模型一条折线 |
| Token 消耗 | `GatewayTokenChart` | 1 小时 | 10 分钟 | 每个模型一条堆叠面积曲线 |

三张图均通过 `useAggregatedStats` hook 调用后端 `call_stats_aggregated` 命令获取数据，默认自动刷新间隔 5 秒（用户可通过 `AutoRefreshSelect` 调整或关闭）。

**请求流量图时间窗口与聚合粒度映射**

`GatewayTrafficChart` 在 `src/modules/gateway-runtime/ui/gateway-traffic-chart.tsx` 中根据窗口小时数选择最合适的聚合粒度，并在前端按桶宽二次汇总为总请求数：

| 窗口范围 | 聚合粒度 | 桶宽 |
|---|---|---|
| 1 小时 | `thirtySeconds` | 30 秒 |
| 2-6 小时 | `oneMinute` | 1 分钟 |
| 6-12 小时 | `tenMinutes` | 10 分钟 |
| 12-24 小时 | `thirtyMinutes` | 30 分钟 |

**后端聚合策略**

`call_stats_aggregated` 根据 `StatsGranularity` 选择数据源：

- `hourly` / `daily`：从预聚合表 `model_call_stats_hourly` / `model_call_stats_daily` 读取。
- `thirtySeconds` / `oneMinute` / `tenMinutes` / `thirtyMinutes`：从 `model_call_logs` 明细表实时 `GROUP BY` 动态桶宽聚合，桶宽通过 `strftime('%s', requested_at) / bucket_secs * bucket_secs` 计算。

该策略兼顾短窗口的细粒度与长窗口的查询性能，避免为临时粒度维护额外预聚合表。

**前端桶序列与补 0 逻辑**

`GatewayTrafficChart` 生成桶序列时：

1. 结束时间对齐到当前桶宽边界。
2. 向前生成覆盖整个窗口的连续桶。
3. 后端返回数据按桶时间戳汇总；无数据的桶保持 `requests = 0`。

这样即使某些时段没有请求，时间轴也能保持连续，图表不会出现断点。

**趋势图与 Token 图**

- 两者默认展示最近 1 小时、按 10 分钟桶聚合的数据。
- 每条模型曲线使用 HSL 色相环生成独立颜色，保证视觉可区分。
- `GatewayTrendChart` 使用折线图展示请求次数；`GatewayTokenChart` 使用堆叠面积图展示 `total_tokens` 消耗。

### 5.9 secret

职责：安全存储 API Key、OAuth Token、代理认证、Gateway Key 等敏感数据。

> **架构约束**：所有加密/解密、密钥链读写、`$SECRET:{uuid}$` 引用解析**仅在后端进行**。前端只有一个 `secret-input.tsx` 密码输入框组件，将用户输入的明文传给后端 Command 后立即丢弃，从不保留明文。

#### 5.9.1 核心类型

- `SecretKind`：`api-key / oauth-token / proxy-auth / gateway-key`。
- `SecretRef`：`$SECRET:{uuid}$` 引用。
- `SecretStorageMode`：`keychain`（系统密钥链）或 `encrypted`（本地 AES-GCM 加密）。

#### 5.9.2 Service 职责

- **保存 Secret**：接收明文，生成 uuid，按 `app_settings.store_secrets_in_keychain` 决定存储位置；返回 Secret 引用。
- **读取 Secret**：根据 uuid 从密钥链或本地加密存储读取明文。
- **解析引用**：扫描配置对象中所有 `$SECRET:{uuid}$` 字符串并替换为明文（仅在后端进行）。
- **清理未引用 Secret**：定期扫描数据库中仍在使用的 Secret ID，删除孤立记录。

#### 5.9.3 安全约束

- 明文永不离开后端；前端只显示掩码或引用占位符。
- 本地加密密钥由操作系统密钥链保护（Tauri `stronghold` 或 `keytar`）。

### 5.10 balance（独立模块）

职责：额度监控与余额查询，被 `ai-gateway` 和 `gateway-runtime` 依赖。

#### 5.10.1 核心类型

- `BalanceMethod`：多态联合，继承参考项目 `balance/types.ts`，支持 `none / moonshot-ai / kimi-code / newapi / deepseek / openrouter / siliconflow / aihubmix / claude-relay-service / antigravity / gemini-cli / codex / synthetic / minimax` 等。
- `BalanceConfig`：各方法的参数配置 JSON。
- `BalanceSnapshot`：额度快照，包含 `amount`、`currency`、`fetched_at`、`is_expired`。

#### 5.10.2 Service 职责

- **查询余额**：按 `BalanceConfig.method` 选择对应 Provider 实现，调用供应商 API 获取余额。
- **缓存快照**：将余额结果写入 `cli_providers.balance_json`，避免频繁请求。
- **额度警告**：低于阈值时产生 `BalanceWarning` 事件，UI 层展示提示。

#### 5.10.3 Repository 职责

- 读写 `cli_providers.balance_json`（通过 `cli-management` 模块的 Repository 间接访问，避免直接依赖）。

### 5.11 logger（日志控制台）

职责：记录所有 HTTP 请求/响应日志（包括网关转发和供应商 API 调用），供运维诊断和开发者调试。

> 与审计日志不同，logger 模块聚焦于**运行时诊断**——记录请求的 URL、状态码、耗时、Token 用量、错误信息等。数据存储在后端内存环形缓冲区（Ring Buffer），可按需持久化到文件。

#### 5.11.1 核心类型

- `LogEntry`：`id`、`timestamp`、`level`（`DEBUG` / `INFO` / `WARN` / `ERROR`）、`source`（`gateway` / `provider-api` / `system`）、`method`（HTTP 方法）、`url`、`status_code`、`duration_ms`、`prompt_tokens`、`completion_tokens`、`total_tokens`、`cached_tokens`、`error_message`、`request_id`。
- `LogFilter`：`levels`、`sources`、`status_codes`、`keyword`、`time_range`。

#### 5.11.2 Service 职责

- **日志写入**：`gateway-runtime` 的拦截器在处理完请求后调用 `logger.service.write()` 写入日志。**写入为异步非阻塞操作**——日志先写入内存环形缓冲区，再由后台线程批量 flush 到文件（可选），不阻塞响应返回。
- **日志查询**：支持按 `LogFilter` 过滤查询，返回分页结果。
- **实时推送**：通过 Tauri Event `log:new-entry` 推送新日志到前端，前端自动更新列表。
- **日志导出**：支持按过滤条件导出为 JSON 或 CSV 格式。
- **自动清理**：内存环形缓冲区默认保留最近 10000 条；文件持久化按天滚动，保留期可配置。

#### 5.11.3 Repository 职责

- 内存环形缓冲区读写（主存储）。
- 可选的文件持久化（按天滚动日志文件）。
- 导出时读取缓冲区或文件。

### 5.12 call-records（模型调用记录）

职责：记录每个模型每次被调用的详细信息，包括 Token 用量、缓存命中率、耗时、状态码，为用量统计和成本分析提供数据。

#### 5.12.1 核心类型

- `ModelCallLog`：对应 `model_call_logs` 表，详见 `database.md §4.26`。
- `CreateModelCallLogInput` / `UpdateModelCallLogInput`：创建/更新调用记录的输入参数。
- `ModelCallStatsInput` / `ModelCallStatsRow`：按供应商 + 模型 ID + 入口聚合的统计查询参数与结果，字段覆盖请求数、成功率、Token、缓存命中率、成本、耗时、首字延迟、输出速率等。

#### 5.12.2 Service 职责

- **记录写入**：`gateway-runtime` 的响应拦截器在请求完成后调用 `call-records.service.write()` 写入记录。**写入为异步非阻塞操作**——`model_call_logs` 的 INSERT 通过后台队列批量提交，不阻塞响应返回给客户端。
- **统计查询**：`aggregate_call_stats(input)` 按 `provider_id`、`model_id`、`source`、时间范围分组聚合，计算成功率、缓存命中率、平均耗时、平均首字延迟、平均输出速率、费用占比等指标。
- **缓存命中分析**：从响应头或响应体中提取 `cached_tokens` 与 `total_tokens`，计算缓存命中率。

#### 5.12.3 Repository 职责

- 读写 `model_call_logs` 表。
- `aggregate_call_stats`：执行分组聚合 SQL，返回 `ModelCallStatsRow` 列表。
- 列表与详情查询（按 `provider_id`、`model_id`、时间范围过滤）。

### 5.13 gateway-runtime 拦截器链

`gateway-runtime` 在处理每个请求时经过以下拦截器链，用于采集日志、记录调用、注入追踪信息：

```
请求进入 → [Auth 中间件] → [请求拦截器] → 转发到真实供应商 → [响应拦截器] → 返回客户端
```

**请求拦截器（Request Interceptor）职责**：
1. 生成唯一 `request_id`（UUID）。
2. 记录请求方法、URL、请求体大小（不记录完整 body 以保护隐私）。
3. 注入 `X-Request-ID` header 到上游请求。
4. 启动计时器。

**响应拦截器（Response Interceptor）职责**：
1. 停止计时器，计算 `duration_ms`。
2. 从响应头中提取 Token 用量（如 `x-ratelimit-remaining-tokens`、`x-request-id`）或从响应体解析 `usage` 字段。
3. 从响应头/体中提取缓存 Token 数（如 `x-cache-hit`、`usage.prompt_tokens_details.cached_tokens`）。
4. 流式响应记录首 chunk 到达时间，计算 `time_to_first_token_ms`。
5. 根据调用上下文写入 `source`：`cli`（CLI 配置文件发起）、`gateway`（外部客户端走本地 HTTP Gateway）、`internal`（应用内部直接调用）。
6. 根据当前模型定价快照写入 `price_per_1m_tokens`（USD / 1M tokens），用于后续成本统计。
7. 捕获状态码和错误信息（如有）。
8. **异步调用** `logger.service.write()` 写入日志。
9. **异步调用** `call-records.service.write()` 写入调用记录。

> 所有拦截器的日志/记录写入操作均为**异步非阻塞**，确保网关响应延迟不受数据库写入影响。若写入失败（如队列满），降级为丢弃该条记录并记录内部错误计数，不影响客户端响应。

### 5.14 db/persistence

职责：管理 SQLite 连接、迁移与事务。

- **连接管理**：单例连接池，数据库文件位于应用数据目录 `i-code.db`。
- **迁移**：Rust 启动时按 `schema_migrations` 表版本执行 `src-tauri/src/db/migrations/V{version}__*.sql`。
- **事务**：跨表操作（如保存供应商 + 附加头 + 模型）使用事务，失败回滚。
- **种子数据**：`builtin_*` 表初始数据由构建脚本从参考项目 `well-known/models.ts`、`well-known/providers.ts` 导出生成。

### 5.16 virtual-provider（虚拟供应商与模型故障转移）

职责：允许用户创建一个「虚拟供应商」，并为该虚拟供应商定义一个或多个「虚拟模型 ID」。每个虚拟模型 ID 背后关联一组真实供应商的真实模型，按优先级与故障转移策略自动选择可用模型，从而在外部 Agent 客户端无需感知真实供应商变化。

> **典型场景**：用户同时拥有两个 OpenAI 渠道、NVIDIA、DeepSeek、硅基流动、Opus 4.8 中转站、OpenCodeGo 套餐等多个供应商。当某个渠道额度耗尽或网络不稳定时，虚拟供应商自动将请求切换到下一个可用模型，Agent 客户端只需固定配置 AI Gateway 地址、API Key 与虚拟模型 ID。

#### 5.16.1 核心概念

- **虚拟供应商（Virtual Provider）**：对外表现为一个普通供应商，拥有自己的 `provider_id`、名称、Base URL、API Key。客户端看到的供应商列表中，虚拟供应商与其他真实供应商并列。
- **虚拟模型 ID（Virtual Model ID）**：虚拟供应商对外暴露的模型标识，例如 `smart-failover-gpt`。该 ID 在 AI Gateway 内部被解析为一组真实模型路由。
- **模型路由（Model Route）**：一条从虚拟模型 ID 到真实模型实例的映射，包含：
  - `target_provider_id`：目标真实供应商 ID。
  - `target_model_id`：目标真实模型 ID。
  - `priority`：优先级，数值越小优先级越高。
  - `enabled`：是否启用该路由。
  - `extra_headers` / `extra_body`：该路由专属的额外请求头/体（可选，覆盖目标供应商级别配置）。
- **故障转移策略（Failover Strategy）**：决定何时切换到下一个路由：
  - `on_error`：仅当请求返回 HTTP 错误（5xx / 4xx）时切换。
  - `on_quota_exceeded`：当上游返回额度不足、余额耗尽、限流配额用完时切换。
  - `on_timeout`：当请求超时或网络不可达时切换。
  - `on_all`：上述任意一种失败均触发切换。
- **健康检查（Health Check）**：可选的轻量级探测，定期调用目标模型的一个低成本接口（如 `/models` 或一次短 completion），标记路由是否健康。健康检查失败的路由会被临时跳过，直到恢复。

#### 5.16.2 核心类型

- `VirtualProvider`：对应数据库表 `virtual_providers`，字段包括 `id`、`name`、`alias`、`is_enabled`、`strategy`。
- `VirtualModel`：对应数据库表 `virtual_models`，字段包括 `id`、`virtual_provider_id`、`model_id`（即对外虚拟模型 ID）、`is_enabled`。
- `VirtualModelRoute`：对应数据库表 `virtual_model_routes`，字段包括 `id`、`virtual_model_id`、`target_provider_id`、`target_model_id`、`priority`、`enabled`、`max_retries`、`timeout_ms`、`is_healthy`、`last_healthy_at`。
- `FailoverStrategy`：枚举 `on_error | on_quota_exceeded | on_timeout | on_all`。
- `VirtualProviderResolveResult`：解析结果，包含最终选中的 `provider_id`、`model_id`、`actual_request_url`、`headers`、以及本次路由尝试历史 `attempts`。
- `VirtualModelMappingGraph`：前端展示组件的数据结构，见 §5.16.6。

#### 5.16.3 解析与转发流程

```
客户端请求：provider=virtual-openai, model=smart-failover-gpt
                ↓
        gateway-runtime 识别 provider 为虚拟供应商
                ↓
        查询 virtual_models 找到虚拟模型 smart-failover-gpt
                ↓
        按 priority ASC 加载所有 enabled 且 healthy 的 ModelRoute
                ↓
        依次尝试转发到 target_provider_id / target_model_id
                ↓
        若当前路由失败且命中 FailoverStrategy，则记录 attempt 并尝试下一路由
                ↓
        若全部路由失败，返回聚合错误（包含每条路由的失败原因）
```

**路由选择细节**：

1. 只选择 `enabled = true` 的路由。
2. 若启用健康检查，只选择 `is_healthy = true` 的路由；未启用健康检查的路由默认视为健康。
3. 同优先级路由按创建时间排序，保证稳定。
4. 每次请求独立选择，失败不修改数据库优先级，仅更新内存中的健康状态与计数。

**失败判定**：

- HTTP 状态码 ≥ 500 或不可恢复 4xx（如 401 / 403 视为凭证错误，是否切换可配置）。
- 上游返回明确额度相关错误码或消息（如 `insufficient_quota`、`rate_limit_exceeded`）。
- 请求耗时超过 `timeout_ms`。
- 网络层错误（DNS、TCP、TLS 失败）。

**成功判定**：

- 上游返回 2xx 且响应体可被正常解析（流式响应以首个有效 chunk 为准）。

#### 5.16.4 Service 职责

- **创建/编辑虚拟供应商**：校验 `name`/`alias` 唯一性，保存到 `virtual_providers`。
- **创建/编辑虚拟模型**：校验同一虚拟供应商内 `model_id` 唯一性，保存到 `virtual_models`。
- **管理模型路由**：增删改 `virtual_model_routes`，校验目标供应商与目标模型存在性。
- **解析路由**：根据请求中的虚拟供应商与虚拟模型 ID，返回按优先级排序的路由列表。
- **执行故障转移**：在 `gateway-runtime` 转发失败时，由 Service 判断是否继续尝试下一路由，并汇总尝试结果。
- **健康检查调度器**：后台任务按配置周期对健康检查启用的路由执行探测，更新 `is_healthy` 与 `last_healthy_at`。

#### 5.16.5 Repository 职责

- 读写 `virtual_providers`、`virtual_models`、`virtual_model_routes` 表。
- 提供按虚拟供应商 + 虚拟模型查询全部有效路由的方法。
- 在事务中保存虚拟供应商、虚拟模型与多条路由，保证数据一致性。

#### 5.16.6 前端组件

- **VirtualProviderForm**：创建/编辑虚拟供应商表单。
- **VirtualModelForm**：创建/编辑虚拟模型与模型 ID。
- **VirtualModelRouteEditor**：拖拽或表单方式维护路由列表（优先级排序、启用/禁用、删除）。
- **VirtualModelGraph**：虚拟模型关系图组件。中心节点为「虚拟模型 ID」，通过连线指向多个「真实供应商模型」子节点，子节点可展示优先级、健康状态、额度进度。该组件为纯 UI，数据由调用方传入，见 `src/components/ui/virtual-model-graph.tsx`。

#### 5.16.7 Commands

- `list_virtual_providers() -> VirtualProvider[]`。
- `create_virtual_provider(data: VirtualProviderInput) -> VirtualProvider`。
- `update_virtual_provider(id: string, data: VirtualProviderInput) -> VirtualProvider`。
- `delete_virtual_provider(id: string) -> Result<void, Error>`。
- `list_virtual_models(provider_id: string) -> VirtualModel[]`。
- `create_virtual_model(data: VirtualModelInput) -> VirtualModel`。
- `update_virtual_model(id: string, data: VirtualModelInput) -> VirtualModel`。
- `delete_virtual_model(id: string) -> Result<void, Error>`。
- `list_virtual_model_routes(virtual_model_id: string) -> VirtualModelRoute[]`。
- `save_virtual_model_routes(virtual_model_id: string, routes: VirtualModelRouteInput[]) -> VirtualModelRoute[]`。
- `resolve_virtual_model(provider_id: string, model_id: string) -> VirtualProviderResolveResult`（调试用途）。

#### 5.16.8 与 gateway-runtime 集成

- `gateway-runtime` 在收到请求时，先判断 `provider_id` 是否为虚拟供应商。
- 若是，调用 `virtual-provider.service.resolve()` 获取路由列表，并进入故障转移循环。
- 每尝试一条路由，记录 `attempt`（目标供应商、目标模型、失败原因、耗时）。
- 最终无论成功或失败，都通过 `logger` 记录完整转发链路，便于排查。

#### 5.16.9 注意事项

- 虚拟供应商的 API Key 可以是任意值（由客户端配置），网关内部会根据目标路由替换为对应真实供应商的 API Key。
- 流式响应的故障转移较为复杂：若首个 chunk 已成功返回，则不再切换；若在首个 chunk 前失败，可安全切换。
- 健康检查应避免对上游产生显著费用，建议使用 `/models` 列表接口或极短 temperature=0 的 completion。
- 额度耗尽判定依赖上游错误码/消息，不同供应商实现可能不一致，需维护一个可扩展的「额度相关错误模式」列表。

### 5.15 backup（备份与恢复）

职责：将应用 SQLite 数据库及相关配置压缩打包，支持备份到本地目录或远程 WebDAV，并支持以覆盖方式恢复/同步备份。

> **覆盖原则**：备份的「推送」与「恢复」均采用**覆盖式**同步——目标端（本地备份目录或 WebDAV）只保留最新一份同名备份；恢复时当前数据库被备份文件完全覆盖，不做增量合并。该原则简化冲突处理，但要求用户在恢复前明确知晓当前数据将被替换。

#### 5.15.1 核心类型

- `BackupTarget`：枚举，`local`（本地磁盘） / `webdav`（远程 WebDAV）。
- `BackupFormat`：枚举，`zip`（默认，跨平台友好） / `tar-gz`。
- `BackupMeta`：备份元数据，包含：
  - `version`：备份格式版本，例如 `"1.0"`。
  - `app_version`：生成备份时的应用版本号。
  - `created_at`：ISO 8601 时间戳。
  - `database_schema_version`：数据库迁移版本号，用于恢复前兼容性校验。
  - `checksum`：数据库文件及关键配置的 SHA-256 校验和。
- `WebDavConfig`：WebDAV 连接配置，包含 `url`、`username`、`password`（由 `secret` 模块加密存储，配置中仅存 `$SECRET:{uuid}$`）、`remote_path`（可选，远程目录）。
- `BackupResult`：操作结果，包含 `success`、`backup_id`、`target`、`size_bytes`、`path`、`created_at`、`error`。
- `BackupListItem`：备份列表项，包含 `id`、`target`、`path`、`created_at`、`size_bytes`、`app_version`。

#### 5.15.2 备份范围

单次备份应包含以下文件，打包为单个压缩包：

1. **数据库文件**：`i-code.db`（主 SQLite 文件）。
2. **数据库辅助文件**（若存在）：`i-code.db-wal`、`i-code.db-shm`（WAL 模式下的日志与共享内存文件）。
3. **元数据文件**：`backup.json`，即序列化后的 `BackupMeta`。
4. **应用设置快照**（可选）：`app_settings.json`，导出 `app_settings` 表中非敏感字段，便于跨设备恢复后快速还原偏好（主题、语言、网关地址等）。
5. **密钥链引用清单**（可选）：`secret_manifest.json`，列出本次备份依赖的 Secret UUID，**不包含明文**。

> **注意**：备份包中的 `i-code.db` 已包含 `secrets` 表。根据存储模式不同：
> - **本地 AES-GCM 加密模式**（`store_secrets_in_keychain = 0`）：`secrets.encrypted_value` 中存储的是加密后的密文，**随数据库一起备份**，恢复后可直接使用，无需重新输入凭证。
> - **系统密钥链模式**（`store_secrets_in_keychain = 1`）：`secrets` 表仅存储密钥链句柄索引，实际值保留在 OS 密钥链中，**不随备份包迁移**。跨设备恢复时，这些 Secret 引用将失效，需用户重新配置凭证。`secret_manifest.json` 用于列出这些失效的引用，供 UI 提示用户补充。

#### 5.15.3 Service 职责

**创建备份（Create Backup）**

- 在备份前执行 `CHECKPOINT` 强制合并 WAL，确保数据库文件完整。
- 生成 `BackupMeta`，计算数据库文件 SHA-256 校验和。
- 按 `BackupFormat` 压缩上述文件到临时目录，文件命名：`i-code-backup-{yyyyMMdd-HHmmss}.{zip|tar.gz}`。
- 返回 `BackupResult`，包含临时路径与元数据。

**推送备份（Push Backup）**

- **本地备份**：将压缩包保存到用户配置的本地备份目录（`BackupSettings.local_directory`），未配置时回退到程序运行目录下的 `backup/`；按配置保留份数自动清理旧备份（`0` 表示不限制）。文件命名：`i-code-backup-{yyyyMMdd-HHmmss}.zip`。
- **WebDAV 备份**：
  1. 解析 `WebDavConfig`，从 `secret` 模块读取密码明文。
  2. 本地创建 zip 压缩包，可选使用 `app_settings.config_key` 经 AES-256-GCM 加密。
  3. 使用 HTTP `PUT` 或 `MKCOL` + `PUT` 将压缩包上传到远程路径。
  4. 上传完成后按配置保留份数清理远程旧备份（`0` 表示不限制）。
  5. 上传完成后可选调用 `PROPFIND` 校验文件大小与服务器返回的 ETag/Last-Modified。
- 推送成功后清理本地临时压缩包；失败时保留临时文件并返回错误详情。

**列出备份（List Backups）**

- **本地备份**：读取本地备份目录，按文件名解析 `BackupMeta` 或直接读取压缩包内的 `backup.json`，返回 `BackupListItem[]`。
- **WebDAV 备份**：调用 `PROPFIND` 远程目录，过滤出备份文件，解析文件名中的时间戳与版本信息；为减少网络开销，列表展示仅读取文件名与 `getcontentlength`，不下载完整压缩包。

**下载/拉取备份（Pull Backup）**

- **本地**：直接返回本地压缩包路径。
- **WebDAV**：使用 HTTP `GET` 下载远程压缩包到临时目录，下载完成后校验文件大小与 `BackupMeta.checksum`（可选，解压后校验）。

**恢复备份（Restore Backup）——覆盖式同步**

1. **前置校验**：
   - 校验压缩包完整性（解压测试、checksum）。
   - 读取 `BackupMeta.database_schema_version`，与当前应用的数据库迁移版本比较。若备份版本高于当前应用版本，禁止恢复并提示升级应用；若低于当前版本，应用应能自动迁移（SQLite 迁移机制保证前向兼容）。
2. **保护当前数据**：
   - 在覆盖前自动对当前数据库做一次「紧急本地备份」（`auto-restore-safety-{timestamp}.zip`），保留最近 3 份，防止误操作导致数据丢失。
3. **覆盖恢复**：
   - 解压备份包，用其中的 `i-code.db[|-wal|-shm]` 替换当前数据库文件。
   - 校验替换后数据库文件的 SHA-256 与 `BackupMeta.checksum` 一致。
   - 清理解压临时目录。
   - **恢复成功后返回 `needs_restart: true`，前端弹窗提示用户手动重启应用**，避免热加载导致数据库连接异常。
4. **Secret 引用修复**：
   - 恢复后扫描数据库中的 `$SECRET:{uuid}$` 引用，根据当前 `app_settings.store_secrets_in_keychain` 值判断：
     - **本地 AES-GCM 加密模式**：`secrets` 表已随数据库完整恢复，`encrypted_value` 可直接解密，无需额外操作。
     - **系统密钥链模式**：若对应 Secret 在当前设备密钥链中不存在，标记为「待重新输入」，UI 中相关供应商/CLI 显示红色警告，提示用户补充凭证。
5. **应用设置还原**（可选）：
   - 若备份包含 `app_settings.json`，将非敏感字段写回 `app_settings` 表；敏感字段（如加密密钥存储位置）不覆盖，避免跨设备配置冲突。

**删除备份（Delete Backup）**

- 本地：直接删除文件。
- WebDAV：发送 HTTP `DELETE` 请求删除远程文件；删除前要求用户确认。

#### 5.15.4 Repository 职责

- 提供数据库文件路径查询，供 Service 读取/复制。
- 在恢复流程中配合 Service 关闭与重新初始化连接。
- 不直接处理压缩、WebDAV 网络或文件系统操作。

#### 5.15.5 Commands（前端调用）

- `create_backup(format: BackupFormat) -> BackupResult`：创建临时备份。
- `push_backup_to_local(backup_id: string, directory?: string) -> BackupResult`：推送到本地目录。
- `push_backup_to_webdav(backup_id: string, config: WebDavConfig) -> BackupResult`：推送到 WebDAV。
- `list_local_backups(directory?: string) -> BackupListItem[]`。
- `list_webdav_backups(config: WebDavConfig) -> BackupListItem[]`。
- `download_backup_from_webdav(id: string, config: WebDavConfig) -> BackupResult`。
- `restore_backup(path: string) -> RestoreBackupResult`：恢复本地或已下载的备份；成功时 `needs_restart` 为 `true`，前端需提示用户重启应用。
- `delete_backup(target: BackupTarget, id: string, config?: WebDavConfig) -> Result<void, Error>`。

#### 5.15.6 UI 职责

- **设置页 → 备份与恢复**：
  - 本地备份目录选择。
  - WebDAV 连接配置（URL、用户名、密码、远程目录）。
  - 「立即备份」按钮，下拉选择备份到本地或 WebDAV。
  - 备份列表（本地 / WebDAV 切换标签），展示时间、大小、版本、操作（下载、恢复、删除）。
  - 恢复前二次确认弹窗，明确提示「当前数据将被完全覆盖」。
- **恢复后提示**：展示恢复结果、自动生成的安全备份路径、缺失的 Secret 引用清单；若 `needs_restart` 为 `true`，弹窗提示用户手动重启应用（仅一个确定按钮）。

#### 5.15.7 安全与隐私

- WebDAV 连接配置保存在 `webdav_configs` 表，密码按当前业务需求**以明文存储**，与项目其他 Secret 加密原则不同。
- WebDAV 远端备份文件可选 AES-256-GCM 加密，密钥由 `app_settings.config_key` 经 SHA-256 派生；该通用密码同时用于 Secret 模块加密 API Key。
- 本地备份文件默认不加密；临时备份文件在操作完成后必须清理。

#### 5.15.8 错误处理

- `BackupError` 细分：
  - `DatabaseLocked`：备份时数据库被锁定，提示用户稍后再试。
  - `ChecksumMismatch`：备份包校验失败，文件损坏。
  - `SchemaVersionTooNew`：备份版本高于当前应用，提示升级。
  - `WebDavAuthFailed` / `WebDavNetworkError` / `WebDavQuotaExceeded`：WebDAV 相关错误。
  - `RestoreSafetyBackupFailed`：恢复前自动备份失败，禁止继续恢复。
- 所有错误通过 `IcodeError` 统一封装，前端展示本地化错误信息。

---

### 5.17 mini-panel（迷你面板）

职责：以独立悬浮小窗口的形式，在用户操作其他应用时仍能快速查看当前供应商、模型、额度、网关状态等关键信息。支持正常模式与最小化（竖长条）模式切换，以及展开回主窗口。

#### 5.17.1 窗口配置（Rust 侧）

迷你面板为独立 Tauri 窗口，通过 `open_mini_panel` / `close_mini_panel` 两个 Command 管理：

| 属性 | 值 | 说明 |
|------|-----|------|
| `decorations` | `false` | 无系统标题栏，自定义拖拽与按钮 |
| `always_on_top` | `true` | 始终悬浮在其他窗口之上 |
| `skip_taskbar` | `true` | 不在任务栏显示 |
| `resizable` | `true` | 允许用户手动调整尺寸 |
| `min_inner_size` | `48 × 80` | 支持最小化竖长条尺寸 |
| `max_inner_size` | `480 × 400` | 正常模式最大尺寸 |
| 默认尺寸 | `320 × 220` | 首次打开的初始尺寸 |

- **`open_mini_panel`**：若迷你窗口已存在则显示并聚焦，否则创建新窗口。初始尺寸从前端 localStorage 持久化设置中读取（Rust 侧仅作后备默认值）。
- **`close_mini_panel`**：关闭迷你窗口，同时显示并聚焦主窗口。

#### 5.17.2 前端路由与布局

- 路由路径：`/mini-panel`，对应 `src/routes/mini-panel.tsx`。
- 根布局（`__root.tsx`）检测到 `/mini-panel` 路径时，跳过 `TitleBar` 渲染和 `pt-9` 顶部间距，整个页面填充窗口。
- 整个窗口 `overflow-hidden`，无滚动条。

#### 5.17.3 拖拽

- 所有非按钮区域添加 `data-tauri-drag-region`，长按可拖拽窗口。
- 按钮区域通过 `WebkitAppRegion: 'no-drag'` 排除拖拽。

#### 5.17.4 正常模式

顶部操作行包含 4 个原生 `<button>` 元素（非 shadcn Button）：

| 图标 | Font Awesome 类名 | 功能 |
|------|-------------------|------|
| 齿轮 | `fa-solid fa-gear` | 展开/收起设置面板 |
| 减号 | `fa-solid fa-minus` | 切换到最小化模式 |
| 方框 | `fa-regular fa-square` | 关闭迷你窗口，展示主窗口 |
| 叉号 | `fa-solid fa-xmark` | 关闭迷你窗口，展示主窗口 |

主体信息区纵向紧凑排列，各信息项可独立控制显隐：

- **供应商**：图标 + 名称（最大 8rem 截断）
- **当前模型**：图标 + 模型名（截断）
- **额度进度**：标签 + 已用/总计 + 原生 div 进度条（h-1）
- **模型消耗**：标签 + 紧凑数字 + 原生 SVG 面积图（底部）
- **网关状态**：状态圆点 + 地址

不使用组件库 Card/Panel/Progress/Slider 等容器，直接用原生 div/span/SVG/input 元素，字号 `text-[8px]~text-[10px]`，间距 `gap-1`。

设置面板（齿轮按钮展开）：
- 宽度/高度调节：原生 `<input type="range">` + `.mini-slider` CSS 样式
- 信息项开关：3 列 grid 原生 `<button>` 标签切换

#### 5.17.5 最小化模式（竖长条）

点击减号按钮后，窗口收缩为竖长条（52 × 180 px），信息区变为纵向居中排列：

- 供应商名：截取前 4 字符
- 模型名：截取前 5 字符
- 网关状态：仅显示状态圆点
- 额度进度：竖向进度条（h-8 w-1.5），从底部向上填充
- 消耗数字：紧凑数字

底部仅保留一个还原按钮（`fa-solid fa-plus`），点击后恢复到最小化前的尺寸。

#### 5.17.6 持久化设置

设置通过 `localStorage`（key: `i-code:mini-panel-settings`）保存，包含：

```typescript
interface MiniPanelSettings {
  width: number        // 正常模式窗口宽度，默认 320
  height: number       // 正常模式窗口高度，默认 220
  visibleFields: {     // 各信息项显隐
    provider: boolean
    currentModel: boolean
    quota: boolean
    modelConsumption: boolean
    gateway: boolean
    chart: boolean
  }
}
```

最小化模式下的尺寸（`MINIMIZED_WIDTH = 52`、`MINIMIZED_HEIGHT = 180`）为硬编码常量，不持久化。最小化前的尺寸通过 `useRef` 临时保存，还原时恢复。

---

## 6. 前端层

### 6.1 组件分层

- **基础组件**（`components/ui/*`）：基于 shadcn/ui 的 Button、Input、Select、Dialog、Table、Form、Tabs 等，保持业务无关。
- **模块组件**（`modules/*/ui/*`）：由业务模块自身维护，例如 `ProviderCard`、`ModelMappingRow`。
- **页面组件**（`pages/*`）：组合模块组件，处理路由参数与页面级状态。

### 6.2 页面规划

| 页面 | 路径 | 说明 |
|------|------|------|
| 首页/仪表盘 | `/` | 展示 Gateway 运行状态、已配置 CLI、激活工作区 |
| AI Gateways | `/gateways` | 供应商列表（布局见 §8.1 ASCII 图） |
| 供应商表单 | `/gateways/new`、`/gateways/:id/edit` | 新增/编辑供应商 |
| 内置供应商选择 | `/gateways/builtin`（或弹窗） | 从 `builtin_providers` 网格选择预设 |
| 配置导入 | `/gateways/import`（或弹窗） | 粘贴/上传 base64 JSON 配置 |
| 模型管理 | `/gateways/:providerId/models` | 该供应商下的模型列表 |
| CLI 管理 | `/cli` | CLI 档案列表 |
| CLI 表单 | `/cli/new`、`/cli/:id/edit` | CLI 档案配置 |
| 工作区 | `/workspaces` | 工作区列表与切换 |
| 工作区配置 | `/workspaces/:id/:cliSlug` | Prompts / MCP / Skill 编辑 |
| 设置 | `/settings` | 主题、语言、Gateway 地址、代理 |
| 日志控制台 | `/logs` | 日志查看、过滤、导出、实时推送 |
| 迷你面板 | `/mini-panel` | 独立悬浮小窗口，紧凑展示供应商/模型/额度/网关状态，支持最小化竖长条模式 |

### 6.3 Hooks

- `use-theme`：主题切换。
- `use-translation`：国际化。
- `use-command`：封装 `invoke` 与错误处理、加载态、超时重试。
- `use-gateway-status`：通过 Tauri Event 监听或轮询 Gateway 运行状态。
- `use-provider-list` / `use-model-list`：按模块缓存列表数据。
- `use-workspace-applier`：封装「应用工作区」流程与待应用状态。
- `use-logger`：日志实时订阅、过滤条件管理、导出触发。

### 6.4 状态管理

- **Zustand 全局 Store**：
  - `settingsStore`：主题、语言、Gateway 地址。
  - `gatewayStore`：供应商列表、模型列表、运行时状态。
  - `workspaceStore`：当前工作区、待应用状态。
- **本地组件状态**：表单、弹窗、折叠面板等使用 `useState` / `useReducer`。

> 架构决策：不使用 React Query / SWR。数据获取走 Tauri Command 的一次性调用模式（非 REST 轮询），无需客户端缓存框架。Gateway 运行时状态通过 Tauri Event 推送更新。

### 6.5 表单处理

- 使用 `react-hook-form` + `zod` 做校验。
- 复杂表单（如供应商、模型）拆分为多个字段组组件（Basic、Auth、Proxy、Advanced）。
- 保存时先调用 `validateProviderForm`（参考 `ui/form-utils.ts`），再调用 Tauri Command。

---

## 7. 后端/Tauri 层

### 7.1 模块化 Commands

后端每个模块在 `src-tauri/src/modules/{name}/commands.rs` 中定义 Tauri Commands，只负责：

1. 接收前端参数。
2. 参数基础校验（非空、类型、合法性）。
3. 调用本模块 Service。
4. 捕获错误并转换为 `IcodeError`。
5. 返回结果。

> 关键操作（增删改、启停）的日志由 `gateway-runtime` 的拦截器链自动记录到 `logger` 模块，无需 Commands 手动调用。

示例命令：

- `settings_get`、`settings_update`
- `gateway_provider_list`、`gateway_provider_create`、`gateway_provider_update`、`gateway_provider_delete`
- `gateway_model_list`、`gateway_model_create`、`gateway_model_delete`、`gateway_model_add_from_builtin`、`gateway_model_add_from_official`
- `cli_profile_list`、`cli_profile_create`、`cli_profile_update`、`cli_profile_delete`
- `cli_provider_bind_gateway`、`cli_model_mapping_create`
- `workspace_list`、`workspace_create`、`workspace_switch`、`workspace_apply`
- `gateway_runtime_start`、`gateway_runtime_stop`、`gateway_runtime_status`
- `secret_save`、`secret_read`、`secret_delete`
- `balance_refresh`、`balance_query`
- `log_list`、`log_export`
- `call_record_list`、`call_record_stats`

### 7.2 Services

Rust Service 层实现业务规则，不直接执行 SQL。职责包括：

- 编排多个 Repository。
- 调用其他模块 Service（如 `ai-gateway` 保存时调用 `secret` 加密）。
- 处理事务边界。
- 构建返回 DTO。

### 7.3 Repositories

- 每个业务表一个 Repository 结构体。
- 使用 `rusqlite` 或 `sqlx` 执行参数化 SQL，防止 SQL 注入。
- 将数据库行映射为 Rust Model，再由 Service 转换为 DTO 返回前端。
- JSON 字段在 Repository 层做序列化/反序列化，失败时返回 `ValidationError`。

### 7.4 Migrations

- 迁移文件命名：`V{version}__{description}.sql`。
- 初始迁移 `V1__init.sql` 包含 `docs/database.md` 中所有建表语句、索引与默认数据插入。
- 后续迁移遵循：
  1. 新增表/列。
  2. 迁移旧数据。
  3. 删除废弃列/表。
  4. 更新 `schema_migrations`。

---

## 8. UI/UX 设计

### 8.1 整体布局

参考 `image.assets/提示词语/gateways.png`：

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│  ●  ●  ●                                                        i-code      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌───┐  ┌──────────────────────────────────────────────────────────────┐   │
│   │ ⚡│  │  Claude Code │ Codex │ Gemini │ OpenCode │ [AI Gateways]     │   │
│   ├───┤  ├──────────────────────────────────────────────────────────────┤   │
│   │ ◷ │  │                                                              │   │
│   ├───┤  │  [⚡] AI Router                                             │   │
│   │ ☆ │  │       https://api.airouter.org                             │   │
│   ├───┤  │                                                              │   │
│   │ ◯ │  │  [✳] Claude Code                                 [Activate]│   │
│   ├───┤  │                                                              │   │
│   │ ⊢ │  │  [◎] Codex                              [In use — turn off] │   │
│   ├───┤  │                                                              │   │
│   │ ⚙ │  │  [◆] Gemini                                      [Activate]│   │
│   └───┘  │                                                              │   │
│          │  [■] OpenCode                                  ✓ [Set as default]│
│          │       gpt-5.5                                               │   │
│          │                                                              │   │
│          └──────────────────────────────────────────────────────────────┘   │
│                                                                    [ + ]    │
│                                                                             │
│  ─────────────────────────────────────────────────────────────────────────  │
│  Gateway: Running ●    Synced just now                                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

- **左侧导航栏**：固定宽度图标 + 文字导航，包含 AI Gateways、CLI、Workspaces、Logger、Settings。
- **顶部标签栏**：在 AI Gateways 页面内切换 Claude Code / Codex / Gemini / OpenCode / AI Gateways 等视图（类似浏览器标签）。
- **主内容区**：卡片列表或表单。
- **右上角主操作**：蓝色「+」按钮用于新增当前视图主实体。
- **状态栏底部**：显示 Gateway 运行状态、同步时间、待应用变更提示。

### 8.2 供应商列表

- 每条供应商显示：图标（根据 provider_type）、名称、base_url、是否启用开关、编辑/删除/导出按钮。
- 右上角「+」按钮为下拉菜单：
  - 添加供应商
  - 从内置供应商列表添加
  - 从配置导入
- 顶部提供搜索框与排序（最近修改优先）。
- 空状态提示「暂无供应商，点击 + 添加」。

### 8.3 从内置供应商列表添加

参考第二张参考图（img260714.png）：

- 弹出网格/列表选择器，展示 `builtin_providers` 预设。
- 每个卡片显示：供应商图标、名称、provider_type、认证方式标签、推荐模型数量。
- 可按 category（General / Experimental 等）分组或搜索过滤。
- 选择后进入供应商表单，预填充 base_url、provider_type、auth_types、默认 auth/balance、extra headers/body 等字段，用户仅需修改 display_name / slug 并填写凭证。

### 8.4 从配置导入

- 提供文本框粘贴 base64 JSON。
- 解析后展示预览：供应商名称、slug、provider_type、模型数量、敏感字段缺失提示。
- 用户确认后执行导入；若 slug 冲突，提示覆盖或重命名。

### 8.5 模型列表

参考 `image-20260713105719729.png`：

- 右上角操作菜单：
  - 从内置模型列表添加...
  - 从官方模型列表添加...
  - 从配置导入...
  - 自动拉取官方模型（开关）
- 每条模型显示：模型 ID、展示名称、输入/输出 Token、能力标签（工具、图像、编辑）、来源标签（manual/builtin/official）。
- 支持启用/禁用暴露（`is_exposed`）。

### 8.6 表单设计

- 使用卡片分组：基本信息、认证、代理、超时重试、附加头/体、高级。
- 认证区域根据所选 `method` 动态展示字段。
- 附加头/体使用键值对表格，支持增删改。
- 保存按钮常驻底部，取消返回上级页面。

### 8.7 CLI 绑定 Gateway

- 选择 Gateway Provider 下拉框。
- 开关「路由模式」：开启后自动填充本地 Gateway base_url；关闭后允许填写直连地址。
- 模型映射表格：每行包含 CLI 模型别名、选择/输入切换、目标模型。

### 8.8 工作区编辑

- 左侧工作区列表，点击切换激活。
- 右侧按 CLI 分 Tab（Prompts / MCP / Skill）。
- 每个 Tab 内为列表编辑，支持新增、删除、排序、启用/禁用。
- 顶部显示「有未应用变更」badge 与「应用」按钮。

---

## 9. 关键工作流程

### 9.1 新增/编辑/删除供应商

供应商新增有三个入口，编辑/删除流程相同：

**入口**
- 空白表单：用户完全手动填写。
- 从内置供应商列表添加：见 §9.2。
- 从配置导入：见 §9.3。

**编辑/保存流程**
```
1. 打开供应商表单（空白或预填充）。
2. 填写基本信息（slug、display_name、provider_type、base_url）。
3. 选择认证方式并填写凭据 → 保存时 secret Service 加密并替换为 $SECRET:{uuid}$。
4. 填写代理、超时、重试、附加头/体。
5. Service 校验 slug 全局唯一，开启事务：
   - 写入 providers 表。
   - 写入 provider_extra_headers / provider_extra_body。
6. 删除时：Repository 执行 DELETE，数据库级联删除关联模型与附加头/体；cli_providers.provider_id 置 NULL。
```

### 9.2 从内置供应商列表添加供应商

```
1. 用户在供应商列表点击「+」→ 选择「从内置供应商列表添加」。
2. UI 展示 builtin_providers 网格/列表，按 category 分组，支持搜索过滤。
3. 用户选择 builtin_provider（如 Open AI）。
4. Service 生成 Provider 草稿：
   - 复制 builtin_providers 的 display_name、provider_type、base_url、use_raw_base_url。
   - 复制 default_auth_json / default_balance_provider_json / extra_headers_json / extra_body_json。
   - 根据 builtin_provider_auth_types 生成默认认证方式列表。
5. UI 进入供应商表单，预填充草稿，用户修改 slug / display_name 并填写凭证。
6. 保存流程同 §9.1。
7. （可选）保存后提示是否立即从内置模型列表添加该供应商推荐的模型。
```

### 9.3 从配置导入/导出供应商

**导入**
```
1. 用户在供应商列表点击「+」→ 选择「从配置导入」。
2. 用户粘贴 base64 JSON（ProviderShareConfig 或 ProviderShareConfig[]）。
3. Service base64 解码并 zod 校验 schema。
4. UI 展示预览：每条供应商的 slug、provider_type、模型数量、敏感字段缺失提示。
5. 若 slug 冲突，提示「覆盖 / 重命名 / 跳过」。
6. Service 将敏感字段交给 secret 模块加密，替换为 $SECRET:{uuid}$。
7. 事务写入 providers、provider_extra_headers / provider_extra_body、model_configs、gateway_models。
```

**导出/分享**
```
1. 用户在供应商卡片点击「导出配置」。
2. Service 读取供应商及其关联的 extra headers/body、gateway_models（含 model_configs）。
3. 序列化为 ProviderShareConfig：
   - 默认保留 $SECRET:{uuid}$ 引用，不导出明文。
   - 若 Secret 无法读取（如跨设备），在配置中标记 missingSecrets 列表。
4. base64 编码后写入剪贴板，用户可粘贴分享。
```

### 9.5 从内置模型列表添加模型

```
1. 用户选择 builtin_provider（如 Open AI）。
2. 查询 builtin_provider_models 关联的 builtin_models。
3. 用户勾选模型。
4. 对每个选中模型：
   - 复制 builtin_models 标量列与 JSON 列到 model_configs。
   - 按 provider_type 匹配 builtin_model_overrides 并合并。
   - 使用 declared_model_id（如有）作为 gateway_models.model_id。
   - source = 'builtin'。
5. 批量插入 model_configs 与 gateway_models，提交事务。
```

### 9.6 从官方模型列表添加模型

```
1. 用户点击「从官方模型列表添加...」。
2. Service 检查 official_model_cache：
   - 若缓存有效（未过期），直接返回缓存。
   - 若无效或不存在，使用 provider 的 auth 调用供应商 API（如 /v1/models），拉取模型列表并写入缓存。
3. UI 展示官方模型列表，使用 builtin_model_aliases 做名称匹配高亮。
4. 用户选择模型后，复制 builtin_models 基础配置或生成空 model_configs。
5. 写入 gateway_models，source = 'official'。
```

### 9.7 配置 CLI 档案

```
1. 用户创建 CLI Profile，选择 cli_type。
2. 填写 config_file_path（可自动检测常见 CLI 默认路径）。
3. 配置 CLI 级代理 ProxyConfig。
4. 保存 cli_profiles 记录。
```

### 9.8 绑定 Gateway Provider 到 CLI

```
1. 在 CLI 详情页点击「绑定 Gateway Provider」。
2. 选择已启用的 Gateway Provider。
3. 设置 display_name、是否默认、排序。
4. 开启/关闭 route_mode：
   - route_mode = 1：gateway_base_url 可选，为空时运行时从 gateway_settings.gateway_host:gateway_port 动态拼接。
   - route_mode = 0：必须填写 direct_base_url，否则报校验错误。
5. 保存 cli_providers 记录。
```

> 注意：`gateway_base_url` 仅作为缓存/覆盖。当 `app_settings` 中的网关地址变更时，所有 `gateway_base_url` 为空的 CLI 自动使用新地址。

### 9.9 工作区切换与应用

```
1. 用户切换 workspace → Service 开启事务：
   - 将所有 workspaces.is_active 置 0。
   - 将目标 workspace.is_active 置 1。
   - 提交事务（保证原子性，避免出现多个激活工作区）。
2. 如果在切换时 Gateway 有活跃请求，切换操作记录 pending，等待请求完成后执行。
3. 用户编辑 Prompts/MCP/Skill → 仅写 workspace_* 表，设置 workspace_cli_configs.pending_apply = 1。
4. 用户点击「应用」：
   - 后端 workspace/commands.rs 接收「应用」命令。
   - workspace/service.rs 读取当前工作区下所有 workspace_cli_configs。
   - 对每个 cli_profile，按 cli_type 生成配置文件（如 claude-code 的 JSON、codex 的 yaml）。
   - 通过 Rust 文件系统 API 写入 cli_profiles.config_file_path。
   - 更新 is_applied = 1、pending_apply = 0、last_applied_at。
   - 写入 model_call_logs 记录本次应用操作的调用详情。
5. 发送 Tauri Event `workspace:applied`，前端 UI 刷新待应用状态。
```

### 9.10 Gateway 请求路由

```
1. 客户端请求本地 Gateway：POST /v1/chat/completions，Header Authorization: Bearer {gateway_key}（内部 CLI 可豁免）。
2. Auth Middleware 校验 Gateway Key 或来源白名单。
3. Router 解析请求体 model 字段：{provider_slug}/{model_id}。
4. 查询 providers（slug = provider_slug）与 gateway_models（model_id）。
5. 加载 model_configs，合并 provider_extra_headers / model_config_extra_headers、provider_extra_body / model_config_extra_body。
6. Secret Service 解析配置中的 $SECRET:{uuid}$。
7. 根据 provider_type 选择协议客户端（OpenAI、Anthropic、Gemini 等）。
8. 转发请求到真实供应商，以 SSE 流式返回数据给客户端。
9. 流式错误处理：
   - 上游断开连接：发送 ERROR event（含部分已完成数据）后关闭 SSE 连接，不阻塞。
   - 客户端取消请求：通过 HTTP request.abort() 传播 cancellation 到上游。
   - 超时：按 TimeoutConfig 返回 504 Gateway Timeout。
   - 非流式请求同样支持，直接返回完整 JSON 响应。
```

---

## 10. 安全

### 10.1 敏感数据存储

- API Key、OAuth Token、代理认证、Gateway Key 禁止以明文存入 SQLite。
- 统一使用 `$SECRET:{uuid}$` 引用，实际值由 `secret` 模块管理。
- 支持两种存储模式：
  - **系统密钥链**（默认）：利用 OS 原生安全存储。
  - **本地加密**：AES-GCM 加密，加密密钥由系统密钥链保护。

### 10.2 API Key 处理

- 前端表单中 API Key 输入框使用密码类型，仅展示掩码。
- 保存时前端将明文传给后端 Command，后端立即加密并丢弃明文。
- 导出配置时，默认不导出 Secret 明文；如需导出应显式提示风险。

### 10.3 内部 CLI 豁免

- Gateway 对内部 CLI 的通信代理可忽略 API Key 认证。
- 识别方式：请求来源 IP 为 `127.0.0.1` / `::1`，或包含特定内部 Header（如 `X-iCode-Internal: {token}`）。
- 内部 Token 由应用生成并写入 CLI 配置，避免被外部伪造。

### 10.4 配置导入/导出安全

- **导出**：默认使用 `$SECRET:{uuid}$` 引用，不暴露明文；跨设备分享时标记 `missingSecrets`，接收方导入后补充缺失凭证。
- **导入**：对 base64 JSON 做 schema 校验，拒绝未知字段与非法类型；导入前展示预览，避免覆盖用户已有供应商时造成配置丢失。
- **分享风险**：若用户选择「导出明文」高级选项，需二次确认并提示凭证泄露风险。

### 10.5 网络安全

- Gateway 默认监听 `127.0.0.1`；仅在用户明确设置后才监听 `0.0.0.0`。
- 所有外部请求强制校验 Gateway Key。
- 代理配置中的 `authorization` 同样走 Secret 引用。

---

## 11. 开发规范

### 11.1 代码注释

- 所有公共函数、类型、Service 方法必须写 JSDoc/TSDoc 注释，说明职责、参数、返回值、可能抛出的错误。
- 复杂业务逻辑（如额度计算、模型覆盖合并、请求转发）需添加行内注释解释「为什么」。
- Rust 函数使用 `///` 文档注释，复杂 unsafe 或并发逻辑必须说明前提条件。

### 11.2 模块边界

- 禁止 UI 直接调用 Repository 或执行 SQL。
- 禁止 Repository 调用 Service 或发送事件。
- 禁止跨模块直接读写数据库表（应通过目标模块 Service）。
- 新增模块前，先在 `docs/development.md` 中补充模块定位、类型、Service 职责。

### 11.3 类型与校验

- 前后端类型通过 `ts-rs` crate 从 Rust struct 自动生成 TypeScript 类型定义（见 `scripts/generate-types.rs`），避免手写同步。
- JSON 字段（如 `auth_json`、`capabilities_json`）写入前使用 zod 校验，读取失败时按 `ValidationError` 处理。
- 使用 `assertNever` 或类似函数处理联合类型 exhaustiveness check。
- Rust DTO 与数据库 Row 在 Repository 层做序列化/反序列化，失败时返回 `ValidationError`。

### 11.4 错误处理

- 后端错误统一封装为 `IcodeError { code, message, details }`，前端按 code 做 Toast/表单提示。
- 禁止将后端堆栈直接暴露给前端。
- 网络请求失败需区分：用户取消、超时、认证失败、服务端错误。
- 前端 `use-command` hook 统一处理错误状态，提供 `error`、`loading`、`retry` 三个输出。

### 11.5 测试策略

- **单元测试**：
  - `core/utils.ts` 中的纯函数。
  - `secret` 模块的加密/解密、引用解析。
  - `ai-gateway` 的模型覆盖合并、slug 校验。
  - `gateway-runtime` 的模型 ID 解析、请求体重写。
- **集成测试**：
  - Repository 层使用内存 SQLite 测试迁移与 CRUD。
  - Service 层测试工作区应用、供应商保存事务。
- **E2E 测试**：
  - 使用 Playwright 测试新增供应商、添加模型、切换工作区流程。

### 11.6 性能建议

- 启动时预加载：
  - `builtin_*` 全表载入内存。
  - `app_settings`、当前激活 `workspace`、Gateway 状态。
- 运行时缓存：
  - `gateway_models` 路由映射缓存（LRU）。
  - `secrets` LRU 缓存（明文仅在后端内存中短暂驻留）。
- 数据库查询避免 N+1：批量读取关联的 extra_headers / extra_body。

### 11.6 性能建议

#### 前端核心依赖（package.json）

| 包名 | 版本范围 | 用途 |
|------|----------|------|
| `react` | `^19.0.0` | UI 框架 |
| `react-dom` | `^19.0.0` | DOM 渲染 |
| `@tauri-apps/api` | `^2.0.0` | Tauri 前端 API |
| `@tauri-apps/plugin-autostart` | `^2.0.0` | 开机自启前端绑定 |
| `@tanstack/react-router` | `^1.0.0` | 路由/导航 |
| `zustand` | `^5.0.0` | 全局状态管理 |
| `react-hook-form` | `^7.0.0` | 表单处理 |
| `zod` | `^3.0.0` | 校验 |
| `@hookform/resolvers` | `^3.0.0` | react-hook-form + zod 桥接 |
| `i18next` | `^24.0.0` | 国际化核心 |
| `react-i18next` | `^15.0.0` | React 绑定 |
| `mitt` | `^3.0.0` | 前端事件总线 |
| `tailwindcss` | `^4.0.0` | CSS 工具库 |
| `lucide-react` | `^0.400.0` | 图标库 |
| `class-variance-authority` | `^0.7.0` | shadcn/ui 样式变体 |
| `clsx` | `^2.0.0` | 条件 class 拼接 |
| `tailwind-merge` | `^2.0.0` | Tailwind class 合并 |

#### 后端核心依赖（Cargo.toml）

| crate | 特性 | 用途 |
|-------|------|------|
| `tauri` | `v2` | Tauri 框架 |
| `tauri-plugin-*` | 按需 | Tauri 插件 |
| `tauri-plugin-autostart` | `2.0.0` | 开机自启（跨平台） |
| `rusqlite` | `bundled` | SQLite 驱动 |
| `serde` + `serde_json` | `derive` | 序列化/反序列化 |
| `ts-rs` | `default` | Rust → TypeScript 类型生成 |
| `reqwest` | `json`, `stream` | HTTP 客户端（代理转发） |
| `axum` | `default` | 本地 HTTP 网关 Server |
| `tokio` | `full` | 异步运行时 |
| `aes-gcm` | `default` | 敏感数据加密 |
| `uuid` | `v4` | UUID 生成 |
| `chrono` | `serde` | 时间戳处理 |
| `tower-http` | `cors` | HTTP 中间件（CORS、日志） |

### 11.9 拦截器与调用记录

- `gateway-runtime` 必须为每个传入请求添加请求拦截器和响应拦截器。
- 请求拦截器：生成 `request_id`、记录请求元信息、启动计时器。
- 响应拦截器：记录状态码、耗时、从响应头/体提取 Token 用量（`prompt_tokens`、`completion_tokens`、`cached_tokens`）、写入 `model_call_logs` 表。
- Token 提取来源优先级：
  1. 响应头中的 `x-ratelimit-*` 或 `x-cache-hit`。
  2. 响应体 JSON 中的 `usage` 字段（OpenAI 兼容格式）。
  3. 供应商特有头（如 Anthropic 的 `x-request-usage-*`）。
- `model_call_logs` 表用于用量统计和成本分析，禁止手动删除。

### 11.10 `provider_type` 同步

- `provider_type` 枚举种子数据从参考项目 `client/definitions.ts` 的 `ProviderType` 类型通过 `scripts/sync-builtin-data.ts` 脚本自动导出为 SQL seed。
- 参考项目新增 `ProviderType` 时，运行脚本重新生成迁移文件。
- 禁止手动在 SQL 或代码中添加 `provider_type` 值。

### 11.11 提交与代码审查

- 每次提交聚焦一个模块或一个功能点。
- 禁止在代码中提交 API Key、测试凭据。
- 审查时重点检查：Secret 是否明文出现、模块边界是否被破坏、事务是否完整、拦截器是否覆盖所有请求。

---

## 12. 参考与关联文档

- 数据库设计：`docs/database.md`
- 参考项目架构：`参考项目/vscode-unify-chat-provider-7.12.3/src`
- UI 参考图：`image.assets/提示词语/gateways.png`、`image.assets/提示词语/image-20260713105719729.png`
- 技术文档：
  - Tauri 2.x：https://v2.tauri.app/
  - shadcn/ui：https://ui.shadcn.com/docs
  - React 19：https://react.dev/
