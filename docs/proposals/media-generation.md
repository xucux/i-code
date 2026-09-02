# 文生图 / 文生视频（媒体生成）模块整合方案

> 状态：**提案**（待评审）
> 日期：2026-09-02
> 关联模块：`ai-gateway`、`gateway-runtime`、`call-records`、`logger`、新增 `media-generation`
> 关联数据：`data/builtin-providers-vision.json`、`data/builtin-models-vision.json`（已落地）
> 首个参考供应商：日日新 SenseNova U1 Fast（`sensenova-u1-fast`，`sensenova-image-generation` 协议）

---

## 1. 背景与目标

### 1.1 背景

图像 / 视频生成类模型正在成为主流供应商的标准能力（SenseNova U1、OpenAI Images/GPT-Image、Gemini 图像、即梦/可灵等）。其接口形态与聊天模型有本质差异，当前架构无法直接承载：

- **响应模型不同**：文生图返回 `{ url | b64_json }`，同步返回、无流式、无 token 增量；chat 管道的 O↔A 桥接、SSE 透传、token 计数均不适用。
- **视频是异步任务**：典型生命周期为「提交任务 → 轮询状态（分钟级）→ 下载产物」，与 chat 的同步请求 / 流式响应模型根本冲突。
- **产物存储不同**：如 SenseNova 生图 URL **固定 1 小时过期**，必须本地下载缓存；chat 的 JSONL 消息存储不适用于二进制产物。
- **消费方不同**：CLI 工具（Claude Code 等）经由本地网关消费的是 chat/messages 接口，不会通过网关调生图；生图 / 生视频的主要消费方是**应用自身 UI**，以及（本软件作为 **MCP Server** 对外暴露后的）**第三方 agent 生态**。

已完成的前置工作（数据层）：

| 项 | 状态 |
|----|------|
| `ProviderType` 硬编码新增 `sensenova-image-generation` 协议 | ✅（Rust `types.rs` + 前端 `types.ts` + provider-form 协议列表） |
| 内置供应商扩展预设 `builtin-providers-vision.json`（`sensenova-u1`，拷贝自日日新） | ✅ |
| 内置模型扩展预设 `builtin-models-vision.json`（`sensenova-u1-fast`） | ✅ |
| `seed.rs` 合并加载扩展预设 | ✅ |
| gateway-runtime 转发支持 | ❌ 未实现（chat 入口调用该协议返回不支持） |

### 1.2 目标

1. **供应商直连生图**：复用 ai-gateway 的供应商 / 认证 / Secret 体系，新增图像生成客户端与专属 UI（工作台 + 画廊）。
2. **网关暴露**（Phase 4）：`POST /v1/images/generations`（OpenAI Images 兼容），不走 chat 桥接。
3. **文生视频**（Phase 5）：任务状态机 + 轮询 + 进度事件推送。
4. **MCP Server 暴露生图**（Phase 6）：本软件作为 **MCP Server**，借助本地网关端口对外暴露生图工具（如 `generate_image`），供 Claude Code 等第三方 MCP Client 调用；与供应商直连（应用内 UI 消费）构成并列通道。
5. **可观测性**：生图 / 生视频调用接入现有 `logger`（两套日志）与 `call-records` 统计。

---

## 2. 方案对比

### 2.1 候选方案

| 方案 | 描述 |
|------|------|
| **A. 复用供应商 + 聊天通道** | 把生图 / 生视频当作「特殊聊天」：复用 chat 转发管道与消息气泡，协议差异在 chat 层打补丁 |
| **B. 新模块（复用供应商基建）** | 新建 `media-generation` 模块承载 UI 与运行时；供应商 / Key / Secret / 日志 / 统计复用 ai-gateway；不进 chat 桥接 |
| **C. 仅 MCP Server 暴露** | 不做应用内直连 UI，仅以 MCP Server 形式对外暴露生图能力（复用供应商基建） |

### 2.2 对比矩阵

| 维度 | A. 复用聊天 | B. 新模块（推荐） | C. 仅 MCP Server |
|------|------------|------------------|-----------|
| 供应商 / AuthConfig / Secret | ✅ 复用 | ✅ 复用 | ✅ 复用（经 media-generation 后端） |
| 网关转发管道 | ⚠️ 硬塞进 chat bridge | ✅ 独立 image/video client | ✅ 复用网关端口挂载 MCP 端点 |
| 聊天气泡内嵌展示 | ✅ 天然 | ⚠️ 需另做集成点（底层调新模块） | ❌ 不涉及（无应用内 UI） |
| 参数能力（size / n / watermark 等） | ❌ 消息体表达不了 | ✅ 完整暴露供应商原生参数 | ✅ 以 MCP 工具入参暴露 |
| 视频异步任务（提交→轮询） | ❌ 与 SSE 请求响应模型冲突 | ✅ 独立任务状态机 | ✅ 工具内提交 / 查询 |
| 产物本地留存（URL 过期） | ❌ JSONL 消息存储不适配 | ✅ 图片本地缓存 / 下载 | ✅ 本地缓存，工具返回本地路径 |
| 官方协议覆盖（OpenAI Images 等） | 差 | 好 | 好 |
| 实现成本 | 低但技术债重 | 中 | 较低（省应用内 UI） |

### 2.3 决策依据

**选 B（新模块，复用供应商基建）**，理由：

1. **最贵的基建已经存在**：供应商 CRUD、`AuthConfig` 多态认证、`$SECRET:{uuid}$` 引用解析、extra headers、两套日志、调用统计，全部可直接复用；新模块只需新增协议客户端与 UI，边际成本最低。
2. **聊天管道复用是负资产**：异步任务、非流式响应、二进制产物硬塞进 SSE / JSONL 模型，chat 模块与媒体能力两侧都要持续打补丁。
3. **视频的任务生命周期决定了必须引入任务表 / 状态机**，这天然属于独立模块而非 chat 的附属逻辑。
4. **A 与 B 不互斥**：「聊天气泡内嵌生图」保留为 B 的体验层集成点，用户感知与方案 A 相同，但架构上不污染 chat 管道。
5. **C 作为并列通道保留**：本软件以 MCP Server 形式把生图能力暴露给第三方 agent（Claude Code 等），与供应商直连（应用内 UI）互补，不构成替代。

---

## 3. 推荐方案分层设计（B）

### 3.1 分层总览

```
┌─ 前端 ─────────────────────────────────────────────┐
│  modules/media-generation/ui   工作台 / 参数面板 / 画廊   │
│  modules/chat/ui               聊天气泡内嵌生图（集成点）  │
└──────────────┬─────────────────────────────────────┘
               │ invoke / event
┌─ 后端 ───────┴─────────────────────────────────────┐
│  modules/media-generation                          │
│    commands / service / repository / types          │
│    image client（同步） + video 任务状态机（轮询）      │
│    产物存储（本地图片缓存 + 生成历史表）                 │
├────────────────────────────────────────────────────┤
│  复用层：ai-gateway（供应商/认证/Secret 解析）          │
│         logger（两套日志）/ call-records（统计）       │
│         gateway-runtime（二期暴露 /v1/images/...）    │
└────────────────────────────────────────────────────┘
```

### 3.2 复用层（不改架构，只扩展）

| 能力 | 复用方式 |
|------|---------|
| 供应商 / 模型管理 | 直接复用；`ProviderType` 继续硬编码新增协议（`sensenova-image-generation` 已加；后续 `openai-images`、video 类协议同理） |
| 认证与 Secret | 复用 `AuthConfig`（api-key 等）与 `$SECRET:{uuid}$` 后端解析；前端仍只传明文一次 |
| 日志 | 网关 / 供应商调用同写两套：自研 logger（UI 可见、去敏请求头）+ tauri-plugin-log（完整请求/响应体） |
| 调用统计 | 生图调用写入 `call-records`；token 计数对生图无意义，按「次数 + 成本」口径记录 |

### 3.3 供应商「视觉生成」标识与隔离约束（硬性要求）

复用供应商基建时，视觉生成类供应商与普通聊天供应商**必须显式区分并隔离**，避免混入既有网关与虚拟供应商链路。

#### 3.3.1 供应商新增「是否视觉生成」标识

| 项 | 设计 |
|----|------|
| 字段 | `gateway_providers` 表新增布尔字段 `is_media_generation`（是否视觉生成，默认 `false`） |
| DTO 同步 | Rust `Provider` / `CreateProviderInput` / `UpdateProviderInput` 与前端 `types.ts` 同步新增该字段（`#[serde(rename_all = "camelCase")]` → `isMediaGeneration`） |
| 内置预设 | 视觉生成的供应商预设与图片模型预设**仅来自** `builtin-providers-vision.json` 与 `builtin-models-vision.json`，**不合并**进通用内置列表（`gateway_builtin_providers_list` / `gateway_builtin_models_list` 不返回视觉生成条目）；vision 条目携带 `isMediaGeneration: true`，经专用 Command（`gateway_builtin_media_providers_list` / `gateway_builtin_media_models_list`）获取；`find_builtin_*` 与按协议筛选仍可命中视觉生成条目（供创建供应商/模型时一键填充） |
| 迁移 | 随 media 迭代的新增迁移文件落地，遵循「同一迭代合并迁移」约定（不碎片化） |
| 判定兜底 | 网关 / 虚拟供应商侧的运行时过滤以 `MEDIA_GENERATION_FAMILY` 协议族常量集中判定（与 `OPENAI_CHAT_FAMILY` / `ANTHROPIC_FAMILY` 同模式，见 `bridge/mod.rs`），字段标识用于展示与用户自定义，两者不一致时以协议族判定为准，避免用户误改字段导致隔离失效 |

#### 3.3.2 列表 tag 展示

| 位置 | 展示 |
|------|------|
| 供应商列表 | 视觉生成供应商在名称旁展示 tag（如「视觉生成」），使用 `text-[10px]` 级小徽章与主题 CSS 变量配色，风格对齐现有列表徽章 |
| 内置供应商列表（从内置预设添加） | 携带该标识的预设条目同样展示 tag，提示用户该供应商的用途与限制 |
| 内置模型列表 | 视觉生成模型（`providerTypes` 含媒体协议）在类型筛选中可见，但不参与聊天模型选择器 |
| i18n | tag 文案走 i18n（`zh-CN`：视觉生成 / `en`：Media Gen），键名 `模块.列表.元素` 规范 |

#### 3.3.3 隔离约束（三条硬性边界）

| # | 约束 | 实现要点 |
|---|------|---------|
| 1 | **不进入原网关转发逻辑** | `/v1/chat/completions`、`/v1/messages`、`/v1/responses` 路由到视觉生成供应商的模型时，返回 OpenAI 标准化错误体（`{ error: { message, type, param, code } }`，如 `model not found` 语义），**不路由、不桥接、不透传**；判定基于协议族常量，在路由解析 model 阶段前置拦截 |
| 2 | **不进入虚拟供应商逻辑** | virtual-provider 的故障转移选路（父级模型 → 子模型映射）**排除**视觉生成供应商的模型；保存 / 应用虚拟模型时若命中媒体协议族直接校验拒绝（`IcodeError::validation`），而非运行时静默跳过 |
| 3 | **模型不进入 `/v1/models`** | `GET /v1/models` 聚合真实供应商与虚拟供应商模型时，**过滤**视觉生成供应商的全部模型；外部 CLI 看到的模型列表保持纯聊天语义 |

> 设计动机：本地网关与虚拟供应商的现有消费方是 CLI 工具（Claude Code / Codex 等），其协议语义是聊天补全。视觉生成模型的请求/响应结构完全不同，混入 `/models` 会让 CLI 侧出现「可见但不可用」的模型，且误调用会产生难以理解的错误；从源头隔离是成本最低、语义最清晰的方案。

### 3.4 新增层：media-generation 模块

**后端**（与前端模块同名对应，遵循 commands / service / repository / types 分层）：

| 组件 | 职责 |
|------|------|
| `image_client` | 图像生成客户端：`POST {base_url}/images/generations`，注入认证头与 extra headers；解析 `{ created, data: [{ url }] }` |
| `video_task` | 视频任务状态机：`submitted → running → succeeded / failed`；任务表 + 定时轮询 + 进度事件（`video-task-updated`，kebab-case） |
| `asset_store` | 产物存储：生成成功后**立即下载**图片到应用数据目录（URL 1 小时过期），DB 只存本地相对路径 |
| 生成历史表 | 记录 provider_id / model_id / prompt / 参数（size、n、watermark 等 JSON）/ 产物路径 / 状态 / 错误 / 耗时 |
| commands | `media_generate_image`、`media_list_history`、`media_delete_history`、`media_video_submit`、`media_video_query` 等（`模块_动作` 命名，注册于 `invoke_handler`） |

**前端**：

| 组件 | 职责 |
|------|------|
| 工作台页面 | 提示词输入 + 模型选择（复用 `{provider_slug}/{model_id}` 路由 ID）+ 参数面板（SenseNova：11 种尺寸枚举、n、watermark） |
| 画廊 / 历史 | 缩略图网格、点击大图、重新生成、删除；滚动布局遵循 `useAvailableHeight` + `ScrollPage` 规范 |
| 聊天内嵌（集成点） | 聊天中选用生图模型 → 底层调 `media_generate_image` → 结果以图片气泡呈现；**不经过** chat 转发管道与 O↔A 桥接 |

#### 3.5.1 视觉生成页面界面设计

**导航入口**：侧栏在「聊天」之后新增菜单项（`app-layout.tsx` 的 navItems 追加）：

```ts
{ to: '/vision', icon: 'fa-solid fa-wand-magic-sparkles', labelKey: 'nav.visionGeneration' }
```

**页面整体结构**（左右分栏，工具栏 + 参数面板 + 画布区）：

```
┌─ 侧栏 ─┬────────────────────────────────────────────────┐
│        │ 工具栏：[模型选择 ▾ sensenova-u1/sensenova-u1-fast]  [Tab: 工作台|画廊] │
│        ├──────────────┬─────────────────────────────────┤
│        │ 参数面板 260px │ 画布区（flex-1, min-h-0）          │
│        │ ┌──────────┐ │  ┌─────────────────────────┐    │
│        │ │提示词      │ │  │                         │    │
│        │ │textarea   │ │  │   图片预览区              │    │
│        │ │(计数/4096) │ │  │   object-contain 居中    │    │
│        │ ├──────────┤ │  │   棋盘格/暗色底           │    │
│        │ │尺寸 ▾      │ │  │                         │    │
│        │ │11种比例分组 │ │  └─────────────────────────┘    │
│        │ │数量 n 1-4  │ │  [下载][复制路径][重新生成][删除]   │
│        │ │水印 ⚑ 开关  │ │                                 │
│        │ │  (注:去水印 │ │                                 │
│        │ │  公测免费)  │ │                                 │
│        │ ├──────────┤ │                                 │
│        │ │[生成 ✨]    │ │                                 │
│        │ │ loading+计时│ │                                 │
│        │ └──────────┘ │                                 │
└────────┴──────────────┴─────────────────────────────────┘
```

**关键设计点**：

| 区域 | 设计 |
|------|------|
| 模型选择器 | 复用聊天界面的轻量 dropdown（无边框 ghost 风格）；仅列出视觉生成供应商的模型，名称旁带「视觉生成」tag（呼应 §3.3.2 隔离约束）；无可用模型时空状态引导「从内置供应商添加 sensenova-image」 |
| 参数面板 | 固定 260px 宽；参数默认值从 builtin model 预设填充；尺寸下拉按 aspect ratio 分组（1:1 / 16:9 / 9:16 / 4:3…11 项）；水印用 Switch 并附 tooltip 说明去水印公测政策 |
| 生成按钮 | 底部固定 primary；生图耗时数十秒 → loading 态显示 spinner + 已耗时（`tabular-nums`）；生成中可切换页面，后台继续（Command 异步 + `image-generated` 事件推送，对齐 §12.5 事件规范） |
| 画布区 | n>1 时 2×2 网格；单张底部操作条：下载（产物已本地化，直接 save）、复制路径、重新生成（带入原 prompt + 参数）、删除；失败显示错误气泡（`IcodeError` message + 重试，禁止 `[object Object]`） |
| 画廊 Tab | 筛选栏（模型 / 日期 / prompt 搜索）+ 缩略图网格（4–5 列）；点击进入大图 lightbox——复用社区模块已有的缩放模式（0.5x–4x、0.25x 步进、z-10 按钮组、遮罩 `overflow-hidden`）；源 URL 过期的历史条目显示「源链接已过期，查看本地缓存」角标 |
| 滚动布局 | 页面高度用 `useAvailableHeight` 实测后传入：内容区高度 = 页面高度 − 工具栏；参数面板与画廊内部各自 `ScrollPage`，画布区不滚动（图片自适应）；遵守「禁止双层滚动容器」约束 |

**i18n 键规划**：`nav.visionGeneration`；`vision.workbench.*`（prompt / size / count / watermark / generate…）；`vision.gallery.*`（filter / expiredNotice / emptyState…）；`zh-CN` / `en` 同步。

### 3.5 网关暴露（Phase 4，可选）

- 新增 `POST /v1/images/generations`（OpenAI Images 兼容格式），复用现有 API Key 认证中间件（`EXEMPT_PATHS` 之外自动纳管）与模型路由（`{provider_slug}/{model_id}` 拆分）。
- **不进入** `OPENAI_CHAT_FAMILY` / `ANTHROPIC_FAMILY` 桥接体系；错误体同样遵循 OpenAI 标准格式 `{ error: { message, type, param, code } }`，不暴露内部错误码。
- 定位说明：该端点主要服务于**应用外部工具的统一入口**，非核心消费路径（核心是应用 UI 自身）。

### 3.7 MCP Server：对外暴露生图能力（Phase 6，并列通道）

**方向定位**：本软件**不是** MCP Client（不接入第三方生图 server），而是作为 **MCP Server**，把 `media-generation` 的生图能力以标准 MCP 工具形式暴露给第三方 MCP Client（Claude Code、Codex 及其他支持 MCP 的客户端 / IDE）。

| 设计点 | 方案 |
|--------|------|
| 端口复用 | MCP 端点**挂载在本地网关进程**（axum，默认 `127.0.0.1:54321`）之上，如 `POST /mcp`（Streamable HTTP 传输），**不新增监听端口**；网关未运行时 MCP 能力不可用（与网关生命周期绑定） |
| 认证 | 复用网关现有认证体系：外部 Client 使用 `Authorization: Bearer {gateway_key}`；不提供免认证模式 |
| 工具设计 | `generate_image`（入参：prompt / size / n / watermark / model 等）、后续 `generate_video_submit` + `generate_video_query`（提交 / 查询任务）；工具返回本地产物路径与元信息（尺寸、耗时、模型） |
| 产物可见性 | 工具返回本地文件路径；第三方客户端在用户本机运行，可直接读取该路径展示 / 引用图片 |
| 第三方接入示例 | `claude mcp add --transport http i-code-media http://127.0.0.1:54321/mcp --header "Authorization: Bearer {gateway_key}"` |
| 与供应商直连的关系 | **互补而非替代**——直连服务应用内 UI（参数完整性、工作台体验），MCP Server 服务外部 agent 生态（agent 自主调用生图） |

---

## 4. 关键设计点与开放问题

| # | 设计点 | 说明 / 开放问题 |
|---|--------|----------------|
| 1 | 产物必须本地化 | SenseNova URL 固定 1 小时过期；生成成功后立即下载，失败时历史记录标注「源链接已过期」 |
| 2 | 视频轮询节流 | 分钟级任务：轮询间隔 3–5s 起步、指数退避；应用退出期间任务状态如何续接（建议：重启后查询一次未终态任务） |
| 3 | 非流式但耗时 | 生图单次可达数十秒：Command 异步 + 前端 loading 态 + 超时上限可配 |
| 4 | 多模态输入 | 部分图像模型支持图生图（imageInput）；`capabilities.imageInput` 已有字段，一期仅透传参考图 URL/base64，编辑能力后续迭代 |
| 5 | 成本口径 | 生图无 token 概念，`call-records` 按次数与供应商返回的计费信息（若有）记录；`token_count_multiplier` 对该类模型置 1 且不参与展示 |
| 6 | 数据表归属 | 新增 `media_generations` / `media_video_tasks` 表；`gateway_providers` 新增 `is_media_generation` 字段；同一迭代合并为一个迁移文件 |
| 7 | i18n / UI 规范 | 遵循现有硬约束：CSS 变量、Font Awesome、900×700 紧凑布局、`zh-CN` / `en` 双语 |
| 8 | 隔离判定一致性 | 隔离以 `MEDIA_GENERATION_FAMILY` 协议族常量为运行时唯一判定源；`is_media_generation` 字段仅用于展示与预设填充，避免双源判定漂移（见 §3.3.1） |

---

## 5. 演进路径

| 阶段 | 内容 | 依赖 |
|------|------|------|
| **Phase 1**（已部分完成） | 数据层：vision 扩展预设 + `sensenova-image-generation` 协议硬编码 | ✅ 已落地 |
| **Phase 2** | **标识与隔离**：供应商新增 `is_media_generation` 字段（含迁移、DTO 同步）+ `MEDIA_GENERATION_FAMILY` 协议族判定 + 列表 tag + 三条隔离约束落地（网关拦截 / 虚拟供应商排除 / `/v1/models` 过滤）；vision 预设独立加载（仅来自 vision JSON，不合并通用列表，专用 Command 暴露）——**已落地** | Phase 1 |
| **Phase 3** | 文生图供应商直连：`media-generation` 后端（image client + 生成历史 + 产物本地化）+ 工作台 / 画廊 UI；接入 logger / call-records | Phase 2 |
| **Phase 4** | 网关暴露 `POST /v1/images/generations` + 聊天气泡内嵌生图 | Phase 3 |
| **Phase 5** | 文生视频：任务状态机 + 轮询 + 进度事件 + 视频产物存储 | Phase 3 |
| **Phase 6** | 本软件以 MCP Server 形式对外暴露生图工具（挂载网关端口，复用网关认证）；图生图 / 编辑能力 | Phase 3 稳定 |

---

## 6. 风险

1. **供应商协议碎片化**：各家生图 API 参数与响应差异大（OpenAI `b64_json`、SenseNova `url`、视频任务各不相同）；image client 按「协议枚举 → 适配器」组织，与 `ClientFactory` 模式一致，避免 if-else 膨胀。
2. **磁盘占用**：生成图片本地缓存无上限增长；需提供历史清理策略（手动删除 + 可选保留时长设置）。
3. **敏感内容与合规**：生图产物属用户数据，导出 / 备份（含 WebDAV）默认不包含媒体产物，避免备份体积与合规风险。
4. **MCP Server 暴露面**：网关端口挂载 MCP 端点后，持有 gateway_key 的第三方可消耗生图配额；沿用网关认证、不提供免认证模式，MCP 调用同写两套日志与 call-records 以便审计。

---

## 7. 结论

以**新模块 `media-generation`** 承载文生图 / 文生视频的运行时与 UI，**最大程度复用 ai-gateway 的供应商、认证、Secret、日志与统计基建**，不进入聊天转发管道；聊天内嵌生图作为体验层集成点，**本软件以 MCP Server 形式（挂载网关端口）对外暴露生图能力**作为面向第三方 agent 的并列通道。

**复用边界（硬性要求）**：供应商新增「是否视觉生成」标识并在列表以 tag 展示；视觉生成供应商**不进入原网关转发逻辑**、**不进入虚拟供应商逻辑**，其模型**不进入 `GET /v1/models`**，与聊天链路从源头隔离（详见 §3.3）。

按 Phase 1→6 分阶段演进，Phase 1 数据层已落地。
