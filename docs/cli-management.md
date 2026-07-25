# CLI 管理模块设计

> 状态：v0.2 实施中  
> 关联：`docs/development.md` §5.6、`docs/database.md` §4.18-§4.20、`docs/proposals/cli-workspace-implementation.md`

## 1. 目标

CLI 管理用于配置外部 AI CLI 如何使用 i-code 中已有的供应商与模型，不负责编辑工作区 Prompt、MCP 或 Skill。

首批内置客户端固定为：

| Tab | `slug` / `cli_type` | 主要配置文件 |
|-----|---------------------|--------------|
| Claude CLI | `claude-code` | `~/.claude/settings.json` |
| Codex | `codex` | `~/.codex/config.toml` |
| OpenCode | `opencode` | `~/.config/opencode/opencode.json` |
| 设置 | — | 集中探测、校验和保存上述路径 |

内置客户端档案由后端按 `slug` 幂等创建。用户不能删除或重命名内置档案，但可以启用/停用客户端、修改配置文件路径，并维护供应商绑定与模型映射。

## 2. 页面结构

`/cli` 使用单层 Tabs，默认选中 Claude CLI。三个客户端 Tab 的内容和布局各不相同，每个 Tab 有专属面板组件。

### 2.1 通用规则

- 供应商列表中的名称超长时使用省略号（`truncate`），避免文字溢出。
- 所有滚动区域由父级通过 `useAvailableHeight` 传入明确高度，禁止双层 `ScrollPage`。

### 2.2 Claude CLI Tab

```text
┌ Claude CLI ───────────────────────────────────────────────────────┐
│ ┌ 供应商列表 ──────┐ ┌ 模型映射 + 开关 + 操作 ────────────────┐ │
│ │ 供应商A (已应用) │ │ ┌ 模型映射表 ──────────────────────┐  │ │
│ │ 供应商B          │ │ │ Sonnet → claude-opus-4-8[1M]    │  │ │
│ │ 供应商C (超长… ) │ │ │ Opus   → claude-opus-4-8[1M]    │  │ │
│ │                  │ │ │ Fable  → claude-opus-4-8[1M]    │  │ │
│ │ [+ 添加供应商]   │ │ │ Haiku  → claude-sonnet-4-5…     │  │ │
│ │                  │ │ │ 默认兜底: claude-opus-4-8        │  │ │
│ │                  │ │ └──────────────────────────────────┘  │ │
│ │                  │ │                                       │ │
│ │                  │ │ ┌ JSON 开关项 ──────────────────────┐ │ │
│ │                  │ │ │ 隐藏 AI 署名     [开关]           │ │ │
│ │                  │ │ │ Teammates 模式   [开关]           │ │ │
│ │                  │ │ │ 启用 Tool Search [开关]           │ │ │
│ │                  │ │ │ 最大强度思考     [开关]           │ │ │
│ │                  │ │ │ 禁用自动升级     [开关]           │ │ │
│ │                  │ │ └────────────────────────────────────┘ │ │
│ │                  │ │                                       │ │
│ │                  │ │ [预览 settings.json] [应用该供应商]   │ │
│ └──────────────────┘ └───────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

**Claude CLI 专属功能：**

| 功能 | 说明 |
|------|------|
| 模型映射 | 使用 `ModelMappingEditor` 组件，支持多角色映射（Sonnet/Opus/Fable/Haiku）+ 1M 声明 + 兜底模型 |
| JSON 开关项 | 5 个开关直接控制 `settings.json` 中的 env 字段 |
| 预览 settings.json | 弹窗展示根据当前供应商 + 开关生成的完整 `settings.json` 配置 |
| 应用供应商 | 将当前供应商的模型映射写入 `settings.json`；已应用的供应商在列表中显示"已应用"状态 |

**开关项与 settings.json 字段映射：**

| 开关 | settings.json 字段 | 默认值 |
|------|-------------------|--------|
| 隐藏 AI 署名 | `includeCoAuthoredBy: false` | 关 |
| Teammates 模式 | `env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS: "1"` | 关 |
| 启用 Tool Search | `env.ENABLE_TOOL_SEARCH: "true"` | 关 |
| 最大强度思考 | `env.CLAUDE_CODE_EFFORT_LEVEL: "max"` | 关 |
| 禁用自动升级 | `env.DISABLE_AUTOUPDATER: "1"` | 关 |

### 2.3 Codex Tab

```text
┌ Codex ───────────────────────────────────────────────────────────┐
│ ┌ 供应商列表 ──────┐ ┌ 模型映射 + 操作 ───────────────────────┐ │
│ │ 供应商A (已应用) │ │ CLI 别名 → Gateway 模型              │ │
│ │ 供应商B          │ │ 添加、编辑、删除                     │ │
│ │ [+ 添加供应商]   │ │                                       │ │
│ │                  │ │ [预览 config.toml] [应用该供应商]      │ │
│ └──────────────────┘ └───────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

**Codex 专属功能：**

| 功能 | 说明 |
|------|------|
| 模型映射 | 使用现有 `CliModelMapping` 列表，支持 select/manual 两种输入模式 |
| 预览 config.toml | 弹窗展示根据当前供应商生成的 `config.toml` 配置 |
| 应用供应商 | 将当前供应商的模型映射写入 `config.toml`；已应用的供应商在列表中显示"已应用"状态 |

### 2.4 OpenCode Tab

```text
┌ OpenCode ────────────────────────────────────────────────────────┐
│ ┌ Provider 配置 ──────────────────────────────────────────────┐  │
│ │ 读取 opencode.json，结构化编辑 provider 模块              │  │
│ │ 每个 provider: id / name / npm / baseURL / apiKey / models │  │
│ │ 支持：添加、编辑、删除 provider 和 model                  │  │
│ │ MCP 模块属于工作区，暂不在本 Tab 编辑                     │  │
│ └─────────────────────────────────────────────────────────────┘  │
│ ┌ Oh-My-OpenAgent 配置管理 ──────────────────────────────────┐  │
│ │ 配置列表：名称 / Agent 数 / 已应用状态                    │  │
│ │ 操作：添加、编辑、复制、删除、应用/取消应用               │  │
│ │ 参考：ai-toolbox opencode 模块                            │  │
│ └─────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

**OpenCode 专属功能：**

| 功能 | 说明 |
|------|------|
| Provider 编辑 | 直接读取/写入 `opencode.json` 的 `provider` 字段，结构化表单编辑 |
| Model 管理 | 每个 provider 下的 models 支持 CRUD、设置主模型 |
| Oh-My-OpenAgent | 配置预设管理（创建/编辑/应用/删除），参考 ai-toolbox 参考项目 |

### 2.5 设置 Tab

设置 Tab 按客户端展示配置文件路径、探测结果、文件格式和语法状态。路径保存后只更新 `cli_profiles.config_file_path`，本轮不覆盖客户端配置文件。

## 3. 数据模型

继续使用现有三表，不新增迁移：

- `cli_profiles`：每个内置客户端一条档案，保存类型、启用状态和配置文件路径。
- `cli_providers`：客户端可用的供应商绑定；`route_mode=1` 表示经过本地 Gateway。
- `cli_model_mappings`：绑定下的模型别名映射。

约束：

- 内置档案 `slug` 与 `cli_type` 固定且一一对应。
- `route_mode` 只能是 `0` 或 `1`。
- 路由模式必须绑定 Gateway 供应商，映射值为 `{provider_slug}/{model_id}`。
- 直连模式必须填写 `direct_base_url`，映射值为上游原始模型 ID。
- 同一 `cli_provider_id` 下 `cli_model_alias` 唯一。

旧类型 `gemini-cli`、`cursor-agent`、`custom` 暂保留用于兼容已有数据，但不在 v0.2 默认 Tabs 中展示。

## 4. 配置文件探测

后端提供只读 Command `cli_config_inspect`，输入 `cliType` 与可选 `configuredPath`，返回：

- 建议路径与最终检查路径；
- 文件是否存在、是否为普通文件、是否可读；
- 识别出的 `json` / `jsonc` / `toml` 格式；
- `missing` / `valid` / `invalid` 解析状态。

探测顺序：

1. 用户已保存的 `configuredPath`；
2. 对应客户端的已知默认路径中第一个存在的文件；
3. 对应平台的首选默认路径。

安全边界：

- 前端不接收配置文件正文。
- 解析错误只返回通用语法状态，不回显可能包含 Token 的原文。
- 探测不创建目录、不修改文件、不解析或解密 Secret。
- 后续生成客户端配置时必须使用结构化解析与字段级合并，禁止字符串替换和无提示全量覆盖。

## 5. Commands

沿用现有 CRUD，并新增：

| Command | 职责 |
|---------|------|
| `cli_profile_ensure_defaults` | 幂等创建并返回 Claude CLI、Codex、OpenCode 三个内置档案 |
| `cli_config_inspect` | 探测配置路径并验证 JSON/JSONC/TOML 语法，不返回文件内容 |

前端参数继续使用 camelCase；Tauri 标量参数映射为 Rust snake_case。

## 6. 前端组件边界

```text
routes/cli/index.tsx
  ├─ ClaudeCliPanel              (Claude CLI 专属面板)
  │    ├─ ProviderBindingForm    (供应商绑定表单，复用)
  │    ├─ ModelMappingEditor     (模型映射编辑器，来自 components/ui)
  │    ├─ ClaudeCliSwitches      (5 个 JSON 开关)
  │    └─ SettingsJsonPreview    (settings.json 预览弹窗)
  ├─ CodexPanel                  (Codex 专属面板)
  │    ├─ ProviderBindingForm    (供应商绑定表单，复用)
  │    ├─ ModelMappingForm       (模型映射表单，复用)
  │    └─ ConfigTomlPreview      (config.toml 预览弹窗)
  ├─ OpenCodePanel               (OpenCode 专属面板)
  │    ├─ OpenCodeProviderEditor (opencode.json provider 结构化编辑)
  │    └─ OhMyOpenAgentManager   (Oh-My-OpenAgent 配置管理)
  └─ CliSettingsPanel            (设置面板，复用)
```

- Route：负责 Tabs、可用高度计算与固定客户端档案匹配。
- `ClaudeCliPanel`：Claude CLI 专属，供应商 + ModelMappingEditor + 开关 + 预览/应用。
- `CodexPanel`：Codex 专属，供应商 + 模型映射 + 预览/应用。
- `OpenCodePanel`：OpenCode 专属，provider 结构化编辑 + Oh-My-OpenAgent 配置管理。
- `CliSettingsPanel`：负责探测、编辑和保存配置文件路径。
- 表单组件只收集输入，不直接调用 Tauri Command。
- 所有滚动区域由父级通过 `useAvailableHeight` 传入明确高度，禁止双层 `ScrollPage`。

## 7. 与 Workspace 的边界

CLI 管理定义“客户端连接到哪些供应商、模型别名如何路由”；Workspace 定义“当前工作区有哪些 Prompt、MCP、Skill，并在用户点击应用后写入客户端”。

两者共享 `cli_profiles`，但页面与写入时机分离：

- 修改 CLI 供应商或模型映射：立即写数据库，不触发 Workspace 应用。
- 修改 Workspace 子配置：标记 `pending_apply`，用户点击“应用”后才写客户端文件。
- 设置 Tab 修改配置文件路径：只改变未来应用/生成的目标路径。

## 8. 本轮验收标准

- `/cli` 默认显示 Claude CLI，并提供 Claude CLI、Codex、OpenCode、设置四个 Tab。
- 三个客户端 Tab 布局和功能各不相同，不再共用同一个 `CliClientPanel`。
- 供应商名称超长时正确显示省略号。
- **Claude CLI**：供应商 + ModelMappingEditor + 5 个开关 + 预览 settings.json + 应用供应商 + 已应用状态。
- **Codex**：供应商 + 模型映射 + 预览 config.toml + 应用供应商 + 已应用状态。
- **OpenCode**：opencode.json provider 结构化编辑 + Oh-My-OpenAgent 配置管理。
- 三个客户端首次打开时都有稳定档案，不要求用户先创建 Profile。
- Gateway 模型映射从已暴露模型中选择，值保持 `{provider_slug}/{model_id}`。
- 设置页可自动探测默认路径、显示存在性与语法状态，并保存自定义路径。
- 中文与英文文案同步；`pnpm type-check`、`cargo check` 通过。

## 9. 后续迭代

1. 为每个客户端实现原生配置渲染器与字段级 Merge/Patch。
2. 写入前提供 diff 预览、备份与失败回滚。
3. 支持从现有配置导入供应商和模型映射，但 Secret 只进入 Rust Secret 模块。
4. 根据稳定性评估是否把 Gemini CLI 或自定义客户端加入一级 Tab。