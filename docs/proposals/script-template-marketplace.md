# 脚本模板市场（公共 GitHub 仓库）设计提案

> 状态：**实现中**  
> 关联：  
> - `docs/proposals/balance-script-templates.md`（本地脚本模板 CRUD / Rhai 运行时，已实现 MVP）  
> - `docs/database.md` §4.33 `script_templates`  
> - `src/modules/script-template/`、`src-tauri/src/modules/script_template/`  
> - `docs/proxy.md`（市场拉取走全局代理）  
> - `docs/error-handling.md`  
>
> 说明：原 `balance-script-templates.md` §1.3 将「在线社区/云端模板市场同步」列为非目标；**本提案将其拆为独立能力**，不改动已落地的本地模板 CRUD。

---

## 1. 背景与目标

### 1.1 现状

| 能力 | 状态 |
|------|------|
| 本地 `script_templates` 表 CRUD | ✅ 已实现 |
| Rhai 额度脚本运行时 / 试运行 | ✅ 已实现 |
| 网关总览「脚本模板」Tab 列表 + 编辑器 | ✅ 已实现 |
| 内置 snippet（本地硬编码示例） | ✅ 已实现 |
| 公共模板仓库 / 在线市场浏览 | ✅ 已实现 |
| 一键从市场「应用」为本地模板 | ✅ 已实现 |
| 本地模板 `author` / 来源溯源字段 | ❌ 无（仅 name/slug/description…） |

当前用户只能：

1. 空白新建  
2. 从本地内置 snippet 起稿  

无法浏览社区/官方维护的现成脚本，也无法复用他人已验证的额度查询脚本。

### 1.2 目标

| 编号 | 目标 | 说明 |
|------|------|------|
| M1 | 公共 GitHub 仓库 | 独立公开仓，专门存放可复用脚本模板（与 i-code 应用仓解耦） |
| M2 | 稳定目录/清单协议 | 机器可读的 `catalog` + 单模板元数据，支持多 `kind` 扩展 |
| M3 | 应用内市场入口 | 脚本模板列表工具栏增加「市场」按钮 |
| M4 | 浏览与筛选 | 按类型浏览；展示名称、作者、slug、创建/更新时间、简介等 |
| M5 | 一键应用 | 将市场条目映射为 `CreateScriptTemplateInput`，**直接调用现有新建**，写入本地库 |
| M6 | 可演进 | 首期仅 `kind=balance`；协议与 UI 不锁死为单一类型 |

### 1.3 非目标（本期不做）

| 非目标 | 说明 |
|--------|------|
| 用户从应用内向公共仓投稿 / PR | 投稿走 GitHub 流程（Issue / PR），应用只读消费 |
| 市场模板在线直接运行（不落库） | 必须先应用为本地模板，再走既有试运行 / 绑定供应商 |
| 自动跟踪上游更新并覆盖本地 | 可展示「有新版本」提示，但覆盖策略二期再定 |
| 多源市场（自建源 / 第三方镜像） | 首期固定官方仓 + 可选自定义 raw URL（设置项可后置） |
| 评分、评论、下载量排行 | 仓库侧可用 GitHub star；应用内不做社交 |
| 付费 / 鉴权下载 | 仅公开 raw 内容 |
| 脚本类型扩展实现 | 协议预留 `kind`；运行时仍仅 `balance` |

### 1.4 设计原则

1. **应用只读消费，仓库侧人工/CI 治理** —— 降低供应链与安全面。  
2. **市场条目 ≠ 本地实体** —— 市场是只读目录；应用后生成独立本地 `script_templates` 行（新 UUID）。  
3. **复用现有创建 Command** —— 不新增「特殊导入表」；应用 = `script_template_create` + 预填字段。  
4. **协议先于实现** —— 公共仓 schema 稳定后，应用再接；避免仓与客户端互相耦合发布。  
5. **安全默认** —— 应用后默认 `draft`；用户审阅脚本后再 `publish`。  
6. **网络走全局代理** —— 与 `docs/proxy.md` 一致，禁止另起一套 HTTP 客户端策略。

---

## 2. 总体架构

```text
┌─────────────────────────────────────────────────────────────┐
│  公共 GitHub 仓库（例：owner/i-code-script-templates）         │
│  catalog.json  +  templates/{kind}/{slug}/meta.json + *.rhai │
└────────────────────────────┬────────────────────────────────┘
                             │ HTTPS（raw / API）
                             │ 经全局代理
                             ▼
┌─────────────────────────────────────────────────────────────┐
│  i-code 后端 script-template 扩展                             │
│  marketplace_fetch_catalog / fetch_item / apply               │
│  （可选短时内存或磁盘缓存 + ETag）                              │
└────────────────────────────┬────────────────────────────────┘
                             │ Tauri Command
                             ▼
┌─────────────────────────────────────────────────────────────┐
│  前端 ScriptTemplateList「市场」对话框                         │
│  浏览 → 详情 → 应用 → createScriptTemplate(...)               │
│  → 可选打开 ScriptTemplateEditor 继续编辑                     │
└─────────────────────────────────────────────────────────────┘
```

### 2.1 与现有模块关系

| 模块 | 变化 |
|------|------|
| `script-template` | **主扩展点**：市场 DTO、拉取、应用编排；创建仍走现有 Service |
| `settings` | 可选：市场源 URL / 分支 / 缓存 TTL（首期可硬编码默认源） |
| `balance` | **不改**运行时；市场只负责把脚本正文落到本地模板 |
| `shared` / 代理 | 市场 HTTP 客户端复用全局代理构造 |
| 前端 `script-template/ui` | 列表加「市场」按钮 + `ScriptTemplateMarketplaceDialog` |

**不新建**独立顶层业务模块（避免与本地模板双轨）；市场作为 `script-template` 的「只读远程目录 + 导入适配器」。

### 2.2 数据流：浏览

```text
用户打开市场
  → script_template_marketplace_list({ kind?, keyword? })
  → 后端：读缓存 或 GET catalog.json
  → 校验 schema / 过滤 kind / 排序
  → 返回 MarketplaceItemSummary[]
  → UI 表格/卡片展示
```

### 2.3 数据流：应用（核心）

```text
用户点「应用」
  →（可选）二次确认：将创建本地草稿，不会自动启用
  → script_template_marketplace_apply({ sourceSlug 或 sourceId, options })
  → 后端：
       1. 取条目 meta + script_body（缓存或再拉 raw）
       2. 校验 kind ∈ 客户端支持集合、engine、脚本非空
       3. 解析 slug 冲突 → 按策略改名（见 §6.4）
       4. 组装 CreateScriptTemplateInput
       5. 调用既有 ScriptTemplateService::create
       6. 默认 status = draft（create 本就 draft）
  → 返回本地 ScriptTemplate
  → 前端 toast 成功；刷新列表；可选 openEditor(created)
```

**关键约束：**「应用」= **新建本地副本**，不是绑定远程引用。本地删改不影响仓库；仓库更新不自动改本地。

---

## 3. 公共 GitHub 仓库设计

### 3.1 仓库定位

| 项 | 建议 |
|----|------|
| 名称 | `i-code-script-templates`（示例，最终以实际组织/用户为准） |
| 可见性 | **Public** |
| 与应用仓关系 | **独立仓库**；应用通过可配置 URL 消费，不 git submodule |
| License | 建议 MIT 或 Apache-2.0，与应用仓兼容；单模板可在 meta 声明 |
| 默认分支 | `main` |
| 发布方式 | 以分支/tag 的 raw 文件为准；可选打 `catalog` Release 资产（二期） |

### 3.2 推荐目录结构

```text
i-code-script-templates/
├── README.md
├── LICENSE
├── CONTRIBUTING.md                 # 投稿规范、安全审查清单
├── CODE_OF_CONDUCT.md              # 可选
├── schemas/
│   ├── catalog.schema.json         # 清单 JSON Schema
│   └── template-meta.schema.json   # 单模板 meta schema
├── catalog.json                    # 市场索引（应用首选入口，CI 生成）
├── scripts/
│   └── build-catalog.mjs           # 扫描 templates/**/meta.json 生成 catalog.json
└── templates/
    └── balance/                    # kind = 目录名
        ├── deepseek-balance/
        │   ├── meta.json           # 元数据（权威源之一）
        │   ├── script.rhai         # 脚本正文
        │   └── README.md           # 可选：人类说明、截图、对接注意
        ├── openrouter-balance/
        │   ├── meta.json
        │   └── script.rhai
        └── ...
```

**设计取舍：**

| 方案 | 优点 | 缺点 | 结论 |
|------|------|------|------|
| A. 仅 `catalog.json` 内联全文 | 一次请求拿齐 | catalog 膨胀；diff 难 | ❌ 不采用 |
| B. 每模板目录 + 汇总 catalog | 单模板 PR 清晰；列表轻量 | 应用详情/应用时可能二次请求 | ✅ **采用** |
| C. 仅靠 GitHub API 列目录 | 少维护 catalog | 速率限制、结构脆弱、无排序字段 | ❌ 不作主路径 |

### 3.3 `meta.json`（单模板权威元数据）

```json
{
  "schemaVersion": 1,
  "slug": "deepseek-balance",
  "name": "DeepSeek 额度查询",
  "kind": "balance",
  "engine": "rhai",
  "author": "i-code",
  "authors": ["i-code", "contributor-name"],
  "description": "调用 DeepSeek 官方余额接口，映射为 amount 指标。",
  "tags": ["deepseek", "official-api", "cny"],
  "homepage": "https://github.com/org/i-code-script-templates/tree/main/templates/balance/deepseek-balance",
  "license": "MIT",
  "defaultTimeoutMs": 15000,
  "allowedHosts": ["api.deepseek.com"],
  "minAppVersion": "0.0.7",
  "scriptFile": "script.rhai",
  "createdAt": "2026-07-28T00:00:00Z",
  "updatedAt": "2026-07-28T00:00:00Z",
  "version": "1.0.0",
  "changelog": "初始版本"
}
```

| 字段 | 必填 | 说明 |
|------|------|------|
| `schemaVersion` | ✅ | 协议版本，首期 `1` |
| `slug` | ✅ | 稳定标识；与目录名一致；字符集对齐本地 `^[A-Za-z0-9\-_.@]{1,64}$` |
| `name` | ✅ | 展示名 |
| `kind` | ✅ | 首期仅 `balance`；目录必须位于 `templates/{kind}/` |
| `engine` | ✅ | 首期仅 `rhai` |
| `author` | ✅ | 主作者展示名（字符串） |
| `authors` | 否 | 多作者；UI 可拼展示 |
| `description` | 建议 | 列表/详情摘要 |
| `tags` | 否 | 筛选辅助 |
| `defaultTimeoutMs` | 否 | 默认 15000 |
| `allowedHosts` | 否 | 写入本地 `allowed_hosts_json` |
| `minAppVersion` | 否 | 客户端可过滤不兼容项 |
| `scriptFile` | 否 | 默认 `script.rhai` |
| `createdAt` / `updatedAt` | ✅ | ISO8601；**市场展示用**，不等于本地 created_at |
| `version` | 建议 | semver；用于二期「有更新」 |
| `license` / `homepage` / `changelog` | 否 | 详情展示 |

**禁止在仓库中存放：** API Key、Cookie、私有 Token、可识别个人账号的配置。

### 3.4 `catalog.json`（市场索引）

由 CI / `scripts/build-catalog.mjs` 从各 `meta.json` 生成，**应用列表只依赖此文件**（减少 N 次请求）。

```json
{
  "schemaVersion": 1,
  "generatedAt": "2026-07-28T12:00:00Z",
  "repo": "https://github.com/org/i-code-script-templates",
  "ref": "main",
  "items": [
    {
      "id": "balance/deepseek-balance",
      "slug": "deepseek-balance",
      "name": "DeepSeek 额度查询",
      "kind": "balance",
      "engine": "rhai",
      "author": "i-code",
      "description": "调用 DeepSeek 官方余额接口…",
      "tags": ["deepseek"],
      "version": "1.0.0",
      "createdAt": "2026-07-28T00:00:00Z",
      "updatedAt": "2026-07-28T00:00:00Z",
      "path": "templates/balance/deepseek-balance",
      "metaPath": "templates/balance/deepseek-balance/meta.json",
      "scriptPath": "templates/balance/deepseek-balance/script.rhai",
      "defaultTimeoutMs": 15000,
      "allowedHosts": ["api.deepseek.com"],
      "minAppVersion": "0.0.7"
    }
  ]
}
```

| 约定 | 说明 |
|------|------|
| `id` | 稳定市场 ID，建议 `{kind}/{slug}`，全局唯一 |
| 列表字段 | 足够渲染表格，**不含** `scriptBody` |
| 排序 | 生成时按 `kind`、`name` 或 `updatedAt` desc；客户端可再排 |
| 校验 | CI 必须：slug 唯一、kind 与路径一致、script 文件存在、JSON Schema 通过 |

### 3.5 原始内容 URL 约定

默认（可配置）：

```text
BASE = https://raw.githubusercontent.com/{owner}/{repo}/{ref}
CATALOG = {BASE}/catalog.json
ITEM_META = {BASE}/{metaPath}
ITEM_SCRIPT = {BASE}/{scriptPath}
```

备选（二期）：

- GitHub Contents API（带 ETag / 更高可控性）  
- jsDelivr / 自建 CDN 镜像（国内可达性）

**首期实现建议：** raw + 可选自定义 base URL；超时与代理走全局设置。

### 3.6 仓库治理与投稿

`CONTRIBUTING.md` 建议强制：

1. 一 PR 一模板（或同 kind 小批量相关模板）  
2. 必须含 `meta.json` + `script.rhai`  
3. 脚本仅使用文档公开的 host functions / 系统变量  
4. 不得硬编码密钥；示例用 `api_key` 等系统变量  
5. 维护者审查：HTTP 目标 host、有无危险逻辑、返回结构是否符合 `BalanceSnapshot` 约定  
6. Merge 后 CI 重建 `catalog.json`

---

## 4. 应用内产品设计

### 4.1 入口

位置：网关总览 →「脚本模板」Tab → 工具栏（与「新建」并列）。

```text
[状态筛选] [搜索]          [市场] [新建]
                              ↑
                     fa-store / fa-globe
```

- 文案 i18n：`scriptTemplate.marketplace`（zh: 市场 / en: Marketplace）  
- 紧凑按钮：`h-7 text-xs`，符合 900×700 密度

### 4.2 市场对话框（建议）

组件：`ScriptTemplateMarketplaceDialog`（`modules/script-template/ui/`）

**布局（紧凑）：**

```text
┌─ 脚本模板市场 ─────────────────────────────────────┐
│ [类型: 全部|额度监控] [关键词] [刷新]     源: official │
├────────────────────────────────────────────────────┤
│ 表格（ScrollableTable + useAvailableHeight）        │
│ 名称 | 作者 | slug | 类型 | 版本 | 更新时间 | 操作   │
├────────────────────────────────────────────────────┤
│ 选中行详情：描述 / tags / allowedHosts / 仓库链接     │
│                              [查看脚本] [应用]      │
└────────────────────────────────────────────────────┘
```

| 列 | 来源 |
|----|------|
| 名称 | `name` |
| 作者 | `author` |
| slug | `slug` |
| 类型 | `kind` → i18n |
| 版本 | `version` |
| 创建时间 | `createdAt`（可默认隐藏，详情里显示） |
| 更新时间 | `updatedAt` |
| 操作 | 应用 |

**交互：**

| 动作 | 行为 |
|------|------|
| 打开市场 | 拉 catalog；失败展示可重试错误（网络/代理/404） |
| 刷新 | 强制绕过缓存 |
| 类型筛选 | 首期：`all` / `balance`；未来随 kind 扩展 |
| 关键词 | 匹配 name / slug / author / tags / description |
| 查看脚本 | 拉 script raw，只读 CodeEditor 预览（不落库） |
| 应用 | 调用 apply Command → 本地 create |
| 应用成功 | toast；关闭或保持市场；父列表 refetch；**建议**直接打开编辑器审阅 |

### 4.3 应用确认与冲突

确认对话框摘要：

- 将创建：**名称 / slug / 作者（写入 description 或溯源字段）**  
- 状态：**草稿 draft**（需手动启用）  
- 若本地 slug 已存在：展示自动改名结果，例如 `deepseek-balance-1`

| 冲突策略（首期固定一种，可设置化后置） | 行为 |
|----------------------------------------|------|
| `rename`（推荐默认） | slug 冲突时追加 `-2` `-3`… |
| `fail` | 返回 `CONFLICT`，UI 提示改名或放弃 |
| `overwrite` | **不做**（避免静默覆盖用户脚本） |

名称冲突：本地无唯一约束，允许同名不同 slug。

### 4.4 与「新建」编辑器的关系

| 路径 | 说明 |
|------|------|
| 空白新建 | 现有 `openCreate()` |
| 市场应用 | `create` 成功后 `openEdit(created)`，正文已预填 |
| 不强制改编辑器 Props | 应用走 Command；编辑器仍只认本地 `ScriptTemplate` |

可选增强（非必须）：编辑器支持「初始草稿 props」而不先 create——**不推荐**，与「应用=调用新建」产品语义不一致。

### 4.5 已安装态（可选，MVP+）

若引入溯源字段（§5），列表可标记「来自市场 / 版本」，市场行可显示「已安装」。  
**MVP 可不做**，仅依赖 slug 模糊提示。

---

## 5. 本地数据模型影响

### 5.1 MVP：零迁移方案（推荐先做）

不改 `script_templates` 表。应用时：

| 市场字段 | 写入本地 |
|----------|----------|
| `name` | `name` |
| `slug`（可能改名） | `slug` |
| `kind` | `kind` |
| `description` + 作者/来源摘要 | `description`（拼接一行来源，如 `Author: x · Market: balance/foo@1.0.0`） |
| `script` 正文 | `script_body` |
| `defaultTimeoutMs` | `default_timeout_ms` |
| `allowedHosts` | `allowed_hosts_json` |
| — | `status = draft`，`engine = rhai` |

优点：无 migration、可马上落地。  
缺点：无法可靠做「已安装 / 检查更新」。

### 5.2 增强：溯源字段（建议紧随 MVP 或同迭代）

新增可空列（迁移 `V00x__script_template_marketplace_source.sql`）：

| 列 | 类型 | 说明 |
|----|------|------|
| `source_origin` | TEXT | 如 `marketplace` / `local` / `snippet` |
| `source_id` | TEXT | 市场 `id`（`balance/deepseek-balance`） |
| `source_version` | TEXT | 应用时的 version |
| `source_repo` | TEXT | 源仓标识或 base URL |
| `source_checksum` | TEXT | 脚本正文 sha256（可选） |

应用/更新策略二期可基于 `source_id` + `source_version` 比较。

**作者字段：**  
- 方案 A：不进表，仅 description  
- 方案 B：增加 `author TEXT`（仅展示元数据）  

建议：**MVP 用 A；若产品强依赖作者列筛选，再加 B**。市场 UI 的作者来自远程 catalog，不依赖本地列。

---

## 6. 后端设计

### 6.1 扩展点（仍在 `script_template` 模块）

```text
src-tauri/src/modules/script_template/
├── ...
├── marketplace/
│   ├── mod.rs
│   ├── types.rs          # MarketplaceCatalog, MarketplaceItemSummary, ApplyOptions...
│   ├── client.rs         # HTTP 拉 raw + 代理 + 超时
│   ├── cache.rs          # 内存缓存 + ETag/TTL
│   ├── validate.rs       # schemaVersion / kind / engine / slug
│   └── apply.rs          # 映射 CreateScriptTemplateInput + slug 冲突
├── service.rs            # 增加 marketplace_* 方法
└── commands.rs           # 注册 Command
```

### 6.2 Command 草案

| Command | 入参 | 返回 | 说明 |
|---------|------|------|------|
| `script_template_marketplace_list` | `{ kind?, keyword?, forceRefresh? }` | `MarketplaceListResult` | 拉/缓存 catalog 并过滤 |
| `script_template_marketplace_get` | `{ id }` 或 `{ kind, slug }` | `MarketplaceItemDetail` | meta + 可选 scriptBody |
| `script_template_marketplace_preview_script` | `{ id }` | `{ scriptBody, version, ... }` | 只读预览 |
| `script_template_marketplace_apply` | `MarketplaceApplyInput` | `ScriptTemplate` | 核心：远程 → create |

```ts
interface MarketplaceApplyInput {
  /** catalog 项 id，如 balance/deepseek-balance */
  id: string
  /** 覆盖本地 slug；默认用市场 slug（冲突则 rename） */
  slugOverride?: string
  /** 覆盖显示名 */
  nameOverride?: string
  /** 默认 rename */
  conflictStrategy?: 'rename' | 'fail'
  /** 应用后是否直接 publish；默认 false（安全） */
  publishAfterCreate?: boolean
}
```

**安全默认：** `publishAfterCreate = false`。若产品坚持「应用即启用」，须在 UI 明确风险，且仍建议先预览。

### 6.3 HTTP 与缓存

| 项 | 约定 |
|----|------|
| 客户端 | `reqwest`，`shared` 全局代理 |
| 超时 | 例如 15s（可配置） |
| 缓存 | 进程内：`catalog` TTL 默认 10–30 min；`forceRefresh` 清缓存 |
| 条件请求 | 支持 `ETag` / `If-None-Match` 更佳 |
| 错误 | 转为 `IcodeError`：`NETWORK` / `VALIDATION` / `NOT_FOUND`；**禁止**把 raw HTML/堆栈丢给 UI |
| 大小限制 | catalog / 单脚本最大字节数（防异常大文件） |

### 6.4 应用映射伪代码

```text
fn apply(id, opts):
  item = load_item(id)            # meta + script
  validate(item)                  # kind/engine/body
  slug = opts.slug_override ?? item.slug
  slug = resolve_conflict(slug, opts.conflict_strategy)
  input = CreateScriptTemplateInput {
    name: opts.name_override ?? item.name,
    slug,
    kind: item.kind,
    description: merge_description(item),
    script_body: item.script_body,
    default_timeout_ms: item.default_timeout_ms,
    allowed_hosts_json: item.allowed_hosts,
    snippet_id: None,             # 或 marketplace:{id}
    sort_order: 0,
  }
  created = service.create(input)
  if opts.publish_after_create:
    created = service.set_status(created.id, publish)
  // 若有溯源列：update source_* 
  return created
```

### 6.5 配置

| 配置项 | 默认 | 存放 |
|--------|------|------|
| `marketplace_base_url` | `https://raw.githubusercontent.com/{owner}/{repo}/main` | 首期常量；二期 `app_settings` |
| `marketplace_enabled` | `true` | 可关 |
| `marketplace_cache_ttl_secs` | `900` | 可选 |

国内 GitHub raw 可达性风险：文档中写明可改 base 为镜像；**不在首期做多源 UI**，仅预留配置位。

---

## 7. 前端设计

### 7.1 文件

```text
src/modules/script-template/
├── types.ts                         # + Marketplace* 类型
├── ui/
│   ├── script-template-list.tsx     # + 市场按钮
│   ├── script-template-marketplace-dialog.tsx
│   └── script-template-marketplace-detail.tsx  # 可选拆分
src/hooks/
├── use-script-template-marketplace.ts   # list/refresh
└── use-script-template-mutation.ts      # + applyMarketplaceTemplate
```

### 7.2 类型（与后端 camelCase 对齐）

```ts
export interface MarketplaceItemSummary {
  id: string
  slug: string
  name: string
  kind: string
  engine: string
  author: string
  description?: string
  tags?: string[]
  version?: string
  createdAt: string
  updatedAt: string
  minAppVersion?: string
}

export interface MarketplaceListResult {
  source: string
  generatedAt?: string
  items: MarketplaceItemSummary[]
  fetchedAt: string
  fromCache: boolean
}
```

### 7.3 i18n 键（示例）

```text
scriptTemplate.marketplace
scriptTemplate.marketplaceTitle
scriptTemplate.marketplaceRefresh
scriptTemplate.marketplaceApply
scriptTemplate.marketplaceApplySuccess
scriptTemplate.marketplaceApplyConfirm
scriptTemplate.marketplaceEmpty
scriptTemplate.marketplaceLoadFailed
scriptTemplate.marketplaceColumns.name|author|slug|kind|version|updatedAt
scriptTemplate.kind.balance
```

中英同步：`zh-CN.json` / `en.json`。

### 7.4 UI 规范对齐

- 使用 shadcn Dialog / Table / Button / Select  
- 图标 Font Awesome（禁止 lucide）  
- 高度：`useAvailableHeight` + `ScrollableTable`，禁止双层 ScrollPage  
- 颜色仅 CSS 变量  

---

## 8. 安全与信任模型

| 风险 | 缓解 |
|------|------|
| 恶意脚本（SSRF、刷接口） | 既有 Rhai 沙箱：禁文件 IO、HTTP host 白名单、超时；应用默认 draft |
| 供应链投毒 | 仅信任配置的官方仓；CI 审查；可选 checksum |
| 过大 payload | 响应体积上限 |
| 密钥泄漏 | 仓库禁止密钥；应用日志脱敏（沿用现有） |
| 中间人 | HTTPS；不关闭证书校验 |
| 用户误启用 | UI 文案强调「请先试运行再启用」 |

**明确：** 市场脚本与用户自写脚本**同一运行时权限**，不因「官方」而放宽 host 或 IO。

---

## 9. 多类型扩展预留

首期 UI/协议：

```ts
type ScriptTemplateKind = 'balance' // 本地运行时
// 市场 catalog.kind 为 string，客户端白名单过滤
const SUPPORTED_MARKETPLACE_KINDS = ['balance'] as const
```

未来新增 `kind`（如 `request-interceptor`）时：

1. 仓库增加 `templates/{new-kind}/`  
2. 客户端白名单与运行时模块就绪后，市场筛选自动多一项  
3. **不支持的 kind**：列表可显示但「应用」禁用，或默认隐藏  

---

## 10. 分阶段落地

### Phase 0 — 协议与空仓（可先于应用发版）

- [ ] 创建公共仓、LICENSE、README、CONTRIBUTING  
- [ ] 落地 `schemas/*`、`catalog.json` 生成脚本  
- [ ] 迁入 1～N 个已验证 `balance` 模板（可从现有 snippet 提炼）  
- [ ] CI：schema 校验 + build catalog  

### Phase 1 — 应用 MVP（本提案核心）

- [x] 后端 marketplace client + list/get/apply Commands
- [x] 前端市场按钮 + 对话框 + 应用
- [x] slug 冲突 rename；默认 draft
- [x] i18n、错误提示、全局代理
- [x] 文档：本提案状态改为「实现中/已实现」；`AGENTS.md` 实现状态一行

- [ ] 脚本只读预览  
- [ ] 溯源字段 + 已安装标记  
- [ ] 设置页配置 marketplace base URL  
- [ ] 检查更新（不自动覆盖）  

### Phase 3 — 生态

- [ ] 镜像源 / 多源  
- [ ] 从本地模板「导出为投稿包」zip/目录  
- [ ] 版本 changelog 展示  

---

## 11. 测试要点

| 用例 | 期望 |
|------|------|
| catalog 正常 | 列表展示 name/author/slug/时间 |
| 网络失败 / 超时 | 友好错误 + 重试；不崩溃 |
| 代理开启 | 经全局代理可拉取（与 proxy 文档一致） |
| 应用成功 | 本地多一行 draft，正文一致 |
| slug 冲突 | 自动 `slug-2`，仍成功 |
| 空脚本 / 错误 kind | VALIDATION，不落库 |
| 应用后试运行 | 与手建模板行为一致 |
| 不支持的 kind | 不可应用或已过滤 |

---

## 12. 文档与仓库交叉引用

实现前后应维护：

| 文档 | 动作 |
|------|------|
| 本文件 `docs/proposals/script-template-marketplace.md` | 主设计；状态随实现更新 |
| `docs/proposals/balance-script-templates.md` | §1.3 非目标改为「见市场提案」；§G2 区分「本地管理 Tab」与「在线市场」 |
| `docs/database.md` | 若加溯源列则补 §4.33 |
| `docs/development.md` / `AGENTS.md` | Command 清单与实现状态 |
| 公共仓 README | 链回本设计的 schema 说明（可精简版） |

---

## 13. 开放问题（实现前建议拍板）

| # | 问题 | 建议默认 |
|---|------|----------|
| Q1 | 公共仓 owner/名称？ | 由项目维护者创建后，客户端常量写入 |
| Q2 | 应用后是否默认打开编辑器？ | **是**，便于审阅 |
| Q3 | 应用后是否允许直接 publish？ | **否**，默认 draft |
| Q4 | MVP 是否做 DB 溯源列？ | **否**，description 拼来源；Phase 2 再加 |
| Q5 | 是否允许用户改市场源 URL？ | MVP 硬编码；设置项 Phase 2 |
| Q6 | raw.githubusercontent.com 在目标用户网络是否稳定？ | 文档说明；预留 base URL；必要时 jsDelivr |
| Q7 | catalog 是否签名？ | 首期否；信任 HTTPS + 固定 owner |
| Q8 | 「创建时间」展示仓库 `createdAt` 还是 git 历史？ | **meta.createdAt**（作者维护）；CI 可校验 |

---

## 14. 结论（给实现的一句话）

**单独建公共 GitHub 仓，用 `catalog.json` + `templates/{kind}/{slug}` 协议分发元数据与 Rhai 脚本；i-code 在脚本模板 Tab 提供只读「市场」浏览，应用时映射为现有 `script_template_create` 生成本地 draft，不把远程市场当成运行时依赖。**

---

*维护说明：协议字段变更时递增 `schemaVersion` 并保留向后兼容策略；实现开始时把本文状态改为「实现中」，并在 Phase 节点回写检查清单。*
