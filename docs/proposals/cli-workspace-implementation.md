# CLI 管理与工作区模块实现方案

> 状态：v0.1 已采用方案 A（统一 JSON 应用），待后续迭代选择是否切换。
> 关联：`docs/development.md` §5.6 / §5.7、`docs/database.md` §4.18-§4.25

## 背景

应用需要同时管理两类核心配置：

1. **CLI 管理**：维护受管 CLI（Claude Code、Codex、Gemini CLI 等）的档案、与 Gateway 供应商的绑定关系、以及 CLI 内部模型别名到真实模型的映射。
2. **工作区**：按项目/目录隔离 Prompts、MCP Servers、Skills 配置，并支持一键将当前工作区配置写入 CLI 实际配置文件。

`cli_profiles.config_file_path` 与 `workspaces.root_path` 预留了配置落地入口，
`workspace_cli_configs` 表作为工作区与 CLI 档案之间的「配置头」，承载应用状态与待应用标记。

## v0.1 已实现范围

- CLI 档案 CRUD（`cli_profiles`）
- CLI 供应商绑定 CRUD（`cli_providers`）
- CLI 模型映射 CRUD（`cli_model_mappings`）
- 工作区 CRUD 与激活切换（`workspaces`）
- 工作区 Prompts / MCP Servers / Skills CRUD
- 创建/修改工作区子配置后自动标记 `workspace_cli_configs.pending_apply = 1`
- `workspace_apply` 命令：将工作区配置写入 CLI 配置文件

## 不确定点：工作区配置如何写入 CLI 实际配置文件

`workspace_apply` 的核心是将数据库中的结构化配置（prompts / mcp_servers / skills）转换为 CLI 可识别的文件内容。
不同 CLI 对配置格式、字段命名、嵌套结构的期望各不相同，存在多种实现策略。

## 方案 A：统一 JSON 结构（v0.1 已实现）

**实现位置**：`src-tauri/src/modules/workspace/service.rs` 的 `apply_single_cli_config`

### 设计要点

- 无论目标 CLI 类型如何，统一生成如下 JSON 结构：

  ```json
  {
    "generated_at": "2026-07-15T10:00:00Z",
    "workspace_id": "workspace-1",
    "cli_profile_id": "profile-1",
    "prompts": [
      { "name": "代码审查", "content": "...", "sort_order": 0 }
    ],
    "mcp_servers": [
      { "name": "filesystem", "transport": "stdio", "config": { ... } }
    ],
    "skills": [
      { "name": "React 组件开发", "source_path": null, "content": "..." }
    ]
  }
  ```

- 直接写入 `cli_profiles.config_file_path`
- CLI 侧负责读取并转换为自己需要的格式

### 优点

- 实现简单，后端无需了解每种 CLI 的私有格式
- 数据结构与数据库表字段一一对应，调试方便
- 新增 CLI 类型时后端改动小

### 缺点

- 每种 CLI 都需要自己写转换层
- 对最终用户不透明，CLI 配置文件不能直接生效
- 无法利用 CLI 原生的配置文件结构（如 Claude Code 的 JSON、Codex 的 YAML）

### 适用场景

- v0.1 快速验证阶段
- 受管 CLI 由项目团队自行维护或统一包装的场景

## 方案 B：按 CLI 类型生成原生配置文件

### 设计要点

- 在 `CliProfile` 中通过 `cli_type` 区分目标 CLI
- 后端为每种 CLI 实现独立的配置渲染器：
  - `claude-code`：生成 `claude.settings.json` 格式
  - `codex`：生成 Codex 配置文件（JSON 或 YAML）
  - `gemini-cli`：生成 Gemini CLI 配置
  - `cursor-agent`：生成 Cursor 规则文件
- 渲染器读取 `prompts` / `mcp_servers` / `skills`，映射为 CLI 原生字段

### 优点

- 生成的配置文件可直接被 CLI 识别，无需额外转换层
- 用户体验好，`config_file_path` 指向的文件就是 CLI 实际读取的文件
- 对最终用户透明，便于手动排查

### 缺点

- 后端需要持续跟进每种 CLI 的配置格式变化
- 新增 CLI 类型需要新增渲染器
- MCP / Skill 等概念在不同 CLI 中的支持程度不同，可能需要降级或忽略

### 适用场景

- 中长期主方案
- 受管 CLI 均为成熟第三方 CLI，原生配置格式稳定的场景

## 方案 C：模板引擎渲染配置文件

### 设计要点

- 在 `CliProfile` 或 `cli_type` 层面维护一组模板文件
- 模板使用 Handlebars / Tera / MiniJinja 等引擎
- 工作区数据作为上下文传入模板，渲染后写入 `config_file_path`
- 用户可在 UI 中自定义模板（高级模式）

### 优点

- 灵活性最高，支持用户自定义输出格式
- 新增 CLI 类型只需新增模板，无需修改后端代码
- 模板与业务逻辑解耦，便于版本管理

### 缺点

- 引入模板引擎依赖，增加复杂度
- 用户自定义模板可能导致渲染失败，需要校验与回退
- 调试模板错误对非技术用户不友好

### 适用场景

- 需要支持多种未预定义 CLI 的场景
- 高级用户需要自定义配置结构的场景

## 方案 D：配置合并策略（Merge & Patch）

### 设计要点

- CLI 配置文件本身可能已经存在用户手动编辑的内容
- `workspace_apply` 不是全量覆盖，而是：
  1. 读取现有 CLI 配置文件
  2. 解析为结构化对象
  3. 仅替换工作区管理的字段（prompts / mcp_servers / skills）
  4. 保留用户手动添加的其他字段
  5. 写回文件

### 优点

- 尊重用户已有配置，避免全量覆盖导致数据丢失
- 兼容 CLI 的其他配置项（如主题、快捷键）

### 缺点

- 需要针对每种 CLI 实现解析与合并逻辑
- 合并冲突时需要定义清晰的优先级规则
- 实现复杂度最高

### 适用场景

- 用户已有大量 CLI 配置需要迁移或共存的场景
- 需要与 CLI 官方配置长期保持双向同步的场景

## 选型建议

| 维度 | 方案 A | 方案 B | 方案 C | 方案 D |
|------|--------|--------|--------|--------|
| 实现成本 | 低 ✓ | 中 | 中高 | 高 |
| 用户体验 | 一般 | 好 ✓ | 好 | 最好 |
| 可维护性 | 高 ✓ | 中 | 高 | 低 |
| 灵活性 | 低 | 中 | 高 ✓ | 中 |
| 对现有配置兼容性 | 无 | 低 | 低 | 高 ✓ |
| 新增 CLI 类型成本 | 低 ✓ | 高 | 低 ✓ | 高 |

### v0.1 决策

采用**方案 A**：统一 JSON 结构，快速验证工作区→CLI 应用链路。
后端只负责生成统一结构，CLI 转换层留待后续迭代。

### v0.2+ 演进建议

- **优先方案 B**：为 `claude-code`、`codex`、`gemini-cli` 等主流 CLI 实现原生配置渲染器，
  让 `workspace_apply` 生成的文件可直接生效。
- **辅助方案 C**：在方案 B 基础上引入模板引擎，作为「自定义 CLI 类型」的后备方案。
- **谨慎采用方案 D**：仅在用户反馈强烈需要保留 CLI 其他配置时，再引入配置合并策略。

## 实现差异点

切换到方案 B/C/D 时需修改的代码点：

1. `src-tauri/src/modules/workspace/service.rs`：
   - `apply_single_cli_config` 不再直接生成统一 JSON
   - 新增 `CliConfigRenderer` trait 与具体实现
2. `src-tauri/src/modules/cli_management/types.rs`：
   - 为 `CliType` 扩展原生配置格式元数据（如 `file_format: json | yaml`）
3. 数据库：
   - 可新增 `cli_profiles.config_template_id`（方案 C）或 `cli_profiles.merge_strategy`（方案 D）
4. 前端：
   - 在 CLI 档案编辑页增加「渲染器/模板」选择器
   - 在 `workspace_apply` 前增加预览能力

## 相关代码

- CLI 管理后端：`src-tauri/src/modules/cli_management/`
- 工作区后端：`src-tauri/src/modules/workspace/`
- 前端类型：`src/modules/cli-management/types.ts`、`src/modules/workspace/types.ts`
- 组件演示：`src/components/preview/cli-management-demo.tsx`、`src/components/preview/workspace-demo.tsx`
