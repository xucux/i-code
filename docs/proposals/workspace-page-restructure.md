# 工作区页面重构方案

> 状态：待评审 / 待实现
> 关联：`docs/development.md` §5.6 / §5.7、`docs/database.md` §4.18-§4.25、
> `src/routes/workspaces/index.tsx`、`src/modules/workspace/ui/workspace-list.tsx`

## 背景

当前工作区页面采用「左侧工作区列表 + 右侧详情」的双栏布局。该布局在只维护少量工作区时可用，但随着功能扩展存在以下问题：

1. **工作区与子配置混排**：左侧固定展示工作区列表，右侧需要在「CLI 配置头」与「Prompts / MCP / Skills」之间反复切换，认知负担大。
2. **操作入口分散**：新增 Prompt、MCP、Skill 的入口藏在 Tabs 内部，新增工作区的入口又在左侧标题栏，整体操作链路不统一。
3. **CLI 信息展示不足**：当前仅显示 `cli_profile_id`（UUID），用户无法直观看到每个 CLI 的名称、类型、应用状态。
4. **全局视角缺失**：没有一个统一的视图让用户先看到「当前工作区已应用到哪些 CLI」，再决定进入哪类子配置。

因此需要对工作区页面进行重构，使其与工作区隔离的核心业务模型（Workspace → CLI Config → Prompts / Skills / MCP）更匹配。

## 目标

1. 以 **Tab 页**组织内容，顶层视角从「CLI 概览」切换到「提示词 / 技能 / MCP」。
2. 将 **工作区选择器** 前置到页面最左侧，与新建按钮一起作为全局上下文切换器。
3. 默认展示第一个工作区的数据；没有工作区时展示空状态并引导创建。
4. 选择工作区后，主 Tab 展示已配置 CLI 的聚合信息；切换到子 Tab 展示对应类型的配置列表。
5. 列表上方根据当前 Tab 动态显示新建按钮（主 Tab 显示三个，子 Tab 仅显示对应一个）。
6. 内容列表支持右侧快捷操作：新建、删除、应用、预览。

## 需求拆解

| 编号 | 需求 | 说明 |
|------|------|------|
| R1 | 工作区选择器前置 | 页面顶部最左侧为工作区下拉选择框，紧邻「新建工作区」按钮 |
| R2 | Tab 页签组织 | 选择器右侧依次为「CLI 概览」「提示词」「技能」「MCP」四个 Tab |
| R3 | 默认选中首个工作区 | 加载完成后自动选中第一个工作区；无工作区时展示空状态 |
| R4 | CLI 概览列表 | 主 Tab 下列出当前工作区已配置的所有 CLI 档案，含名称、类型、应用状态 |
| R5 | 子配置列表 | 切换到提示词/技能/MCP Tab 后，列出当前工作区下所有对应子配置（跨 CLI 聚合） |
| R6 | 动态新建按钮 | 主 Tab 上方同时显示「新建 Prompt」「新建 Skill」「新建 MCP」；子 Tab 仅保留对应新建按钮 |
| R7 | 右侧快捷操作 | 列表行内或列表右上角支持新建、删除、应用、预览 |
| R8 | 新建工作区表单 | 弹窗收集名称、slug、目录路径、是否立即激活等基础信息 |

## 页面布局方案

### 整体结构

```
┌──────────────────────────────────────────────────────────────────────┐
│ [Workspace ▼] [+]          │ CLI 概览 │ 提示词 │ 技能 │ MCP │          │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  [新建 Prompt] [新建 Skill] [新建 MCP]          [应用全部] [预览]      │  ← 主 Tab 工具栏
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │ CLI 档案 A                                      [应用] [预览]   │  │
│  │   类型: claude-code   状态: 待应用                                │  │
│  ├────────────────────────────────────────────────────────────────┤  │
│  │ CLI 档案 B                                      [应用] [预览]   │  │
│  │   类型: codex         状态: 已应用                                │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

切换到「提示词」Tab 后：

```
┌──────────────────────────────────────────────────────────────────────┐
│ [Workspace ▼] [+]          │ CLI 概览 │ 提示词 │ 技能 │ MCP │          │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  [新建 Prompt]                                                   ...   │  ← 子 Tab 工具栏
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │ Prompt A                      [编辑] [删除] [预览]             │  │
│  │   所属 CLI: claude-code                                           │  │
│  ├────────────────────────────────────────────────────────────────┤  │
│  │ Prompt B                      [编辑] [删除] [预览]             │  │
│  │   所属 CLI: codex                                                 │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

### 布局要点

- 顶部一行固定高度，左侧为工作区选择器 + 新建按钮，右侧为 Tab 列表。
- 下方内容区使用 `useAvailableHeight` 计算可用高度，并传入 `ScrollPage`。
- 每个 Tab 内容区高度 = 页面高度 - 顶部工具栏高度 - 内边距。
- 列表使用 `ScrollPage` 做内部滚动，外层容器固定高度并加 `overflow-hidden`，避免双层滚动。

## 数据模型与命令

### 当前后端能力

当前后端已提供以下命令：

| 命令 | 用途 |
|------|------|
| `workspace_list` | 列出所有工作区 |
| `workspace_create` / `workspace_update` / `workspace_delete` | 工作区 CRUD |
| `workspace_switch` | 切换激活工作区 |
| `workspace_cli_config_list` | 列出某工作区下的 CLI 配置头 |
| `workspace_prompt_list` / `workspace_skill_list` / `workspace_mcp_server_list` | 按 CLI 配置头列出子配置 |
| `workspace_apply` | 将工作区配置应用到 CLI 配置文件 |

### 需要新增/复用的能力

#### 方案 A：前端聚合（推荐 v0.1）

- 前端同时调用 `workspace_list`、`cli_profile_ensure_defaults`、`workspace_cli_config_list`。
- 在 `WorkspaceList` 组件内通过 `cliProfileId` 关联 `CliProfile`，得到 CLI 名称、类型、启用状态。
- 子配置列表需要跨所有 `workspace_cli_config_id` 聚合：前端对每个 config 分别调用 `workspace_prompt_list` 等，再合并展示。

**优点**：
- 不改动后端，改动范围可控。
- 与当前前端「无 Repository/Service 层」的规范一致。

**缺点**：
- CLI 概览 Tab 需要 1 次 `workspace_cli_config_list` + 1 次 `cli_profile_ensure_defaults`。
- 子配置聚合 Tab 需要 N 次子配置查询（N = 当前工作区下 CLI 配置头数量），请求数随 CLI 数量线性增长。
- 每个子配置行需要显示「所属 CLI」，前端需要自行做关联。

#### 方案 B：后端新增聚合命令

新增命令 `workspace_aggregate`，参数 `workspaceId`，返回：

```rust
pub struct WorkspaceAggregate {
    pub workspace: Workspace,
    pub cli_configs: Vec<WorkspaceCliConfigAggregate>,
}

pub struct WorkspaceCliConfigAggregate {
    pub config: WorkspaceCliConfig,
    pub profile: CliProfile,
    pub prompts: Vec<WorkspacePrompt>,
    pub mcp_servers: Vec<WorkspaceMcpServer>,
    pub skills: Vec<WorkspaceSkill>,
}
```

**优点**：
- 一次请求拿到当前工作区所有数据，前端状态简单。
- 后端天然完成 `cli_profile_id` → `CliProfile` 的关联。
- 未来「预览」功能可以直接基于聚合数据生成。

**缺点**：
- 需要新增后端 DTO、Service、Repository 查询、Command 注册。
- 数据量可能较大（所有 prompts / mcp / skills 一次性返回），但工作区级配置通常可控。

使用方案B

## 组件设计

### 页面级组件

重构后 `src/routes/workspaces/index.tsx` 仅负责：

- 页面高度测量（`useAvailableHeight`）。
- 渲染 `WorkspaceManager` 并传入可用高度。

### `WorkspaceManager`

位置：`src/modules/workspace/ui/workspace-manager.tsx`（新建，替换 `workspace-list.tsx`）

职责：

- 维护当前选中的 `workspaceId` 和 `activeTab`。
- 加载工作区列表、CLI 档案列表、当前工作区的 CLI 配置头。
- 根据 Tab 动态渲染工具栏与列表内容。
- 管理所有表单弹窗（工作区新建/编辑、Prompt、Skill、MCP）。
- 处理应用、删除、预览等操作。

Props：

```ts
interface WorkspaceManagerProps {
  height: number
}
```

### 子组件拆分

| 组件 | 文件 | 职责 |
|------|------|------|
| `WorkspaceSelector` | `workspace-selector.tsx` | 工作区下拉 + 新建按钮 |
| `CliOverviewList` | `cli-overview-list.tsx` | CLI 概览列表 |
| `PromptList` | `prompt-list.tsx` | 提示词列表（跨 CLI 聚合） |
| `SkillList` | `skill-list.tsx` | 技能列表（跨 CLI 聚合） |
| `McpServerList` | `mcp-server-list.tsx` | MCP 列表（跨 CLI 聚合） |
| `WorkspaceEmptyState` | `workspace-empty-state.tsx` | 无工作区时的空状态 |

### 表单弹窗复用

- `WorkspaceForm`：已存在，仅保留名称、slug、目录、是否立即激活字段。
- `PromptForm` / `SkillForm` / `McpServerForm`：已存在，复用。

**注意**：当前子配置创建需要指定 `workspaceCliConfigId`。在新设计中，用户从主 Tab 点击「新建 Prompt」时，需要明确关联到哪个 CLI 配置头。建议：

- 主 Tab 工具栏的三个新建按钮，点击后先弹出「选择 CLI 配置头」的二次确认，再打开对应表单。
- 子 Tab 列表中的「新建」按钮同样需要先选择 CLI 配置头。
- 另一种方案：默认选择第一个 CLI 配置头，在表单中提供可切换的下拉框。

### 预览功能

需求中的「预览」指生成当前工作区将要写入 CLI 配置文件的内容。实现方式：

- 复用 `workspace_apply` 的逻辑，但改为只读生成不写入文件；或
- 前端基于已有聚合数据，按统一 JSON 格式生成预览内容。

**建议**：新增一个只读预览弹窗 `WorkspacePreviewDialog`，内容格式与后端 `apply_single_cli_config` 输出对齐。v0.1 可先在前端用模拟格式生成，后续与后端统一。

## 状态管理

### 局部状态（`useState`）

- `selectedWorkspaceId`：当前选中的工作区 ID。
- `activeTab`：当前激活的 Tab，枚举 `'overview' | 'prompts' | 'skills' | 'mcp'`。
- 各表单弹窗的 open 状态与编辑对象。
- `deleteTarget`：待删除对象。
- `previewWorkspaceId` / `previewCliProfileId`：预览弹窗上下文。

### 数据获取（自定义 hooks）

复用已有 hooks：

- `useWorkspaces`：加载工作区列表。
- `useCliProfiles`：加载 CLI 档案列表。
- `useWorkspaceCliConfigs`：加载当前工作区下的 CLI 配置头。
- `useWorkspacePrompts` / `useWorkspaceSkills` / `useWorkspaceMcpServers`：按配置头加载子配置。

新增 hook：

- `useWorkspaceAggregate(selectedWorkspaceId)`：内部调用上述 hooks，返回聚合后的 `{ configs, prompts, skills, mcpServers, loading }`。

## 交互流程

### 初始化

1. 页面加载时调用 `workspace_list` 和 `cli_profile_ensure_defaults`。
2. 工作区列表返回后，自动选中第一个工作区（`selectedWorkspaceId = workspaces[0].id`）。
3. 选中工作区变化后，加载 `workspace_cli_config_list`。
4. 配置头返回后，分别加载每个配置头下的 prompts / skills / mcp_servers。

### 切换工作区

1. 用户从下拉框选择其他工作区。
2. `selectedWorkspaceId` 更新，所有子配置 hooks 重新加载。
3. `activeTab` 保持不变（例如仍停留在「提示词」Tab）。

### 切换 Tab

1. 用户点击「CLI 概览 / 提示词 / 技能 / MCP」。
2. `activeTab` 更新，工具栏按钮和列表内容同步切换。
3. 当前 Tab 数据已在初始化时预加载（或按需加载）。

### 新建工作区

1. 点击顶部「+」按钮，打开 `WorkspaceForm` 弹窗。
2. 填写名称、slug、目录、是否立即激活，提交 `workspace_create`。
3. 成功后刷新工作区列表，并自动选中新创建的工作区。

### 新建子配置

1. 点击工具栏「新建 Prompt / Skill / MCP」按钮。
2. 如果当前工作区有多个 CLI 配置头，弹出「选择 CLI」确认；如果只有一个，直接选中。
3. 打开对应表单，提交后刷新子配置列表并标记对应 CLI 配置头为 `pending_apply`。

### 应用与预览

- CLI 概览列表中，每个 CLI 行右侧显示「应用」「预览」按钮。
- 「应用」调用 `workspace_apply`，对整个工作区应用；或未来支持对单个 CLI 配置头应用。
- 「预览」打开弹窗，展示该 CLI 配置头将要生成的配置文件内容。

## i18n 规划

新增 / 调整的键（`zh-CN` / `en` 同步）：

```json
{
  "workspace": {
    "title": "工作区",
    "tabs": {
      "overview": "CLI 概览",
      "prompts": "提示词",
      "skills": "技能",
      "mcp": "MCP"
    },
    "selector": {
      "placeholder": "选择工作区",
      "new": "新建工作区"
    },
    "empty": {
      "noWorkspace": "暂无工作区，请先创建一个工作区",
      "noCliConfig": "当前工作区下暂无 CLI 配置",
      "noPrompts": "暂无提示词",
      "noSkills": "暂无技能",
      "noMcp": "暂无 MCP 配置"
    },
    "actions": {
      "newPrompt": "新建提示词",
      "newSkill": "新建技能",
      "newMcp": "新建 MCP",
      "apply": "应用",
      "applyAll": "应用全部",
      "preview": "预览",
      "edit": "编辑",
      "delete": "删除",
      "belongsTo": "所属 CLI"
    },
    "status": {
      "applied": "已应用",
      "pending": "待应用"
    }
  }
}
```

## 实现计划

### 第一阶段：文档与组件骨架

1. 评审并确认本方案。
2. 新增 / 调整 i18n 键。
3. 创建 `WorkspaceManager` 组件骨架与相关子组件文件。

### 第二阶段：核心交互实现

1. 实现 `WorkspaceSelector`（下拉 + 新建按钮）。
2. 实现 Tab 切换与工具栏动态按钮。
3. 实现 CLI 概览列表、Prompt 列表、Skill 列表、MCP 列表。
4. 实现空状态、加载状态。

### 第三阶段：操作闭环

1. 集成工作区新建 / 编辑 / 删除。
2. 集成 Prompt / Skill / MCP 的新建 / 编辑 / 删除。
3. 集成「应用」按钮与结果提示。
4. 实现预览弹窗（只读）。

### 第四阶段：验证

1. `pnpm type-check`
2. `pnpm lint`
3. 手动验证：
   - 无工作区时的空状态。
   - 创建第一个工作区后自动选中。
   - 切换 Tab 与切换工作区。
   - 新建、编辑、删除、应用、预览各操作。

## 不确定点与待决策

### D1：子配置新建时如何选择 CLI 配置头

- **选项 A**：在主 Tab 点击「新建 Prompt」时，先弹出「选择 CLI」对话框，选择后再打开 Prompt 表单。
- **选项 B**：在 Prompt 表单内部增加「所属 CLI」下拉框，默认选中第一个。
- **选项 C**：如果只有一个 CLI 配置头，直接选中；多个时弹出选择。

**建议**：选项 A

### D2：应用操作的作用域

- **选项 A**：「应用」始终对整个工作区生效（复用现有 `workspace_apply`）。
- **选项 B**：CLI 概览列表中每个 CLI 行的「应用」仅应用该 CLI 配置头；工具栏「应用全部」应用整个工作区。

**建议**：选项B

### D3：预览的数据来源

- **选项 A**：前端基于已加载数据自行渲染统一 JSON 预览。
- **选项 B**：后端新增 `workspace_preview` 命令，返回将要写入 CLI 配置文件的文本内容。

**建议**：选项 B。

### D4：是否一次性聚合加载所有子配置

- **选项 A**：进入工作区页面即加载所有 prompts / skills / mcp_servers，Tab 切换无额外请求。
- **选项 B**：只在切换到对应 Tab 时加载该类型子配置。

**建议**：选项 A，因为工作区级子配置数量通常不大，预加载可提升 Tab 切换体验；若性能出现瓶颈再改为选项 B。

## 演进路径

| 阶段 | 目标 | 关键动作 |
|------|------|----------|
| v0.1（当前重构） | 页面交互重构 | 前端聚合、Tab 布局、工具栏、预览弹窗 |
| v0.2 | 后端聚合命令 | 新增 `workspace_aggregate`，减少前端请求数 |
| v0.3 | 单 CLI 应用 | 支持对单个 CLI 配置头应用与预览 |
| v0.4 | 原生配置预览 | 预览按 CLI 类型生成对应原生格式（JSON / YAML / Cursor 规则等） |
