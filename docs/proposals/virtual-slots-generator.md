# 虚拟供应商一键生成（三模型槽位）提案

> 状态：**草案**（待评审；先不开发，本提案 + 参考 JSON 供评审与数据源准备）  
> 关联：`docs/development.md` §5.16、`src-tauri/src/modules/virtual_provider/`、`src/modules/virtual-provider/ui/virtual-provider-list.tsx`、`src-tauri/data/virtual-slots.json`

---

## 0. 摘要

用户希望「一键生成虚拟供应商 + 三个虚拟模型」，省去手工创建虚拟供应商、虚拟模型并逐个选择子级路由的重复操作。三个虚拟模型槽位固定语义：

| 槽位 | 虚拟模型 ID | 用途 | 选型倾向 |
|------|------------|------|---------|
| Opus | `virtual_opus` | 规划（planning） | 能力最强、上下文最大 |
| Sonnet | `virtual_sonnet` | 写代码（coding） | 编码均衡、工具支持完善 |
| Haiku | `virtual_haiku` | 工具调用（tool_use） | 轻量、快速、廉价 |

实现方式：以一份**数据源 JSON**（`virtual-slots.json`）描述「虚拟供应商元信息 + 每个槽位对应的一批固定实体模型 ID（带优先级）」。程序读取该 JSON，从**已开启显示的模型列表**（`gateway_exposed_models`，即 `is_exposed=1` 且供应商启用的模型）中按优先级规则匹配实体模型，自动生成 1 个虚拟供应商 + 3 个虚拟模型及各自的子级路由。

数据源 JSON 已由用户推送到 GitHub 仓库，程序运行时以该仓库 JSON 文件为匹配数据源（离线时回退本地内置副本）。**数据源 URL 可在设置页配置，用户可替换为自己的 JSON 地址**（详见 §3.4）。

> **本提案不涉及数据库迁移**：生成结果复用现有 `virtual_providers` / `virtual_models` / `virtual_model_routes` 三表与现有 `virtual_provider_create` / `virtual_model_save` 命令，生成后与手工创建的虚拟供应商完全等价、可自由编辑。

---

## 1. 背景与动机

### 1.1 现状

- 创建一套「规划 / 写代码 / 工具调用」三模型虚拟供应商需要：创建虚拟供应商 → 逐个创建 3 个虚拟模型 → 为每个虚拟模型从穿梭框选择多个子级路由并设置优先级，操作繁琐且易遗漏。
- 用户在真实供应商页面已维护「已开启显示的模型」列表（`gateway_models.is_exposed=1` 且 `providers.is_enabled=1`），但虚拟供应商与真实模型之间没有快捷匹配能力。
- 虚拟供应商的典型价值在于「固定对外模型 ID + 后端自动故障转移」：客户端只需固定配置 `{alias}/{model_id}`，无需感知真实供应商增减。一键生成正是把这一价值落地到可复制的「模板化」流程。

### 1.2 目标

1. 提供一个入口（一键按钮），从已开启显示的模型列表中自动匹配并生成虚拟供应商 + 三个虚拟模型。
2. 用一份可维护的数据源 JSON 定义「每个槽位固定对应哪些实体模型 ID、匹配优先级」，用户可直接编辑并推送到 GitHub 仓库作为数据源。
3. 生成结果可回退、可编辑：未匹配到任何模型的槽位明确提示，不产生半成品状态。

### 1.3 非目标（本阶段不做）

- 不做模型能力探测/自动分级（不通过 context window、能力标签推断槽位归属，只用 JSON 显式规则匹配）。
- 不做「应用市场/规则中心」的云端拉取 UI，仅提供「数据源 URL 配置 + 本地内置兜底」。
- 不修改三个槽位之外的虚拟模型生成逻辑。

---

## 2. 整体流程

```
用户点击「一键生成三模型」按钮
        │
        ▼
获取数据源 JSON（优先 GitHub 仓库 URL → 失败回退本地内置 virtual-slots.json）
        │
        ▼
读取「已开启显示的模型列表」（gateway_exposed_models）
        │
        ▼
对每个槽位按 matches 规则（priority 升序）匹配实体模型
        │
        ▼
生成虚拟供应商（若 alias 已存在则提示冲突/复用）
        │
        ▼
对每个槽位调用 virtual_model_save 创建虚拟模型 + 子级路由
        │
        ▼
展示生成结果（每个槽位命中的路由数 / 未命中提示）
```

---

## 3. 数据源设计

### 3.1 文件位置

- **参考/内置副本**：`src-tauri/data/virtual-slots.json`（与 `builtin-*.json` 同目录，随仓库分发）。
- **正式数据源**：用户将该 JSON 推送到 GitHub 仓库后，程序以该仓库的 JSON 文件为匹配数据源（详见 §3.4）。

### 3.2 JSON 结构（字段说明）

```jsonc
{
  "$comment": "说明性字段，程序忽略",
  "schemaVersion": 1,          // 数据源 schema 版本，用于向后兼容
  "id": "virtual-slots-assistant",
  "name": "智能助手三模型套件", // 展示名（生成时可用于 provider.displayName）
  "description": "…",
  "version": "0.2.0",

  "provider": {                 // 虚拟供应商元信息
    "alias": "assistant",       // 对外路由前缀 {alias}/{modelId}，须唯一
    "name": "智能助手",
    "displayName": "智能助手",
    "strategy": "fallback",     // fallback | on_all | load_balance
    "maxRetries": 3,
    "retryIntervalMs": 1000,
    "isEnabled": true
  },

  "slots": [                    // 三个虚拟模型槽位
    {
      "key": "virtual_opus",    // 槽位标识（程序内部引用，不直接落库）
      "modelId": "virtual_opus",// 虚拟模型对外 ID
      "displayName": "Opus · 规划",
      "role": "planning",       // planning | coding | tool_use
      "description": "…",
      "routeDefaults": {        // 生成路由的默认重试/超时（可选）
        "maxRetries": 3,
        "retryIntervalMs": 1000
      },
      "matches": [              // 实体模型匹配规则，按 priority 升序尝试
        {
          "priority": 1,        // 越小越优先；同时作为生成路由的 priority
          "type": "exact",      // exact | prefix | regex
          "modelId": "claude-opus-4-1",   // exact/prefix 时的目标模型 ID
          "pattern": "^o3(-mini)?$"       // regex 时的正则表达式（大小写不敏感）
        }
      ]
    }
  ]
}
```

### 3.3 匹配规则类型

| type | 说明 | 示例 |
|------|------|------|
| `exact` | `modelId` 与真实模型 `modelId` 完全相等 | `glm-5.2` |
| `prefix` | 真实模型 `modelId` 以 `modelId` 开头（鲁棒，可匹配供应商的版本后缀） | `claude-opus` → `claude-opus-4-1` / `claude-opus-4-5` |
| `regex` | 正则表达式匹配（大小写不敏感），用于不便用前缀表达的场景 | `^o3(-mini)?$` |

**匹配规则约束**：不限定供应商 slug（完全移除了 `providerSlugs` 约束），仅按 `modelId` 匹配，兼容中转站与多变渠道，减少配置维护复杂度。

同一槽位内按 `priority` 升序逐条尝试。

### 3.4 数据源获取方式（用户可配置）

用户将 `virtual-slots.json` 推送到 GitHub 仓库后，程序按以下顺序解析数据源：

1. **远程优先**：从「设置」中可配置的数据源 URL 拉取，默认指向用户 GitHub 仓库的 raw JSON 地址（`https://raw.githubusercontent.com/xucux/i-code/main/src-tauri/data/virtual-slots.json`）。用户可在设置页修改该地址，替换为自己的 JSON 地址（如自建仓库、企业内网 CDN 等）。
2. **本地内置兜底**：远程拉取失败（离线 / 网络错误 / JSON 解析失败 / 用户清空 URL 配置）时，回退使用内置 `src-tauri/data/virtual-slots.json`。
3. **缓存策略**：远程拉取成功后按需缓存到本地（应用数据目录 `virtual-slots-cache.json`），供离线重复使用；下次启动时先尝试远程拉取，远程不可用时使用缓存，缓存也不可用时回退内置。

#### 3.4.1 配置项设计

在 `settings` 模块中新增一个可配置项：

| 配置项 | 存储位置 | 类型 | 默认值 |
|--------|---------|------|--------|
| 虚拟供应商数据源 URL | `global_configs` 表，`group='virtual_provider', key='preset_data_source_url'` | 字符串（URL 或空字符串） | `https://raw.githubusercontent.com/xucux/i-code/main/src-tauri/data/virtual-slots.json` |

- 空字符串或清空后，程序仅使用本地内置兜底。
- 该配置项通过 `global_configs` 键值表存储，无需新增迁移或表结构变更。
- 前端通过新增 `settings_virtual_slots_data_source_url` / `settings_set_virtual_slots_data_source_url` 两个命令读写（或复用 `global_configs` 的通用读写命令）。

#### 3.4.2 设置页 UI

在 `settings.tsx` 的「AI Gateway」或「虚拟供应商」相关 Card 中新增一个输入行：

- 标签 i18n：`settings.virtualSlotsDataSourceUrl`（「虚拟供应商数据源 URL」）
- Input 控件：`<Input type="url" />`，placeholder 显示默认值
- 右侧含「重置默认」按钮（清空用户输入，回退默认值）
- 保存时调用 `updateSettings` 或专用命令写入 `global_configs`
- 保存后即生效（下次「一键生成」时使用新 URL）

#### 3.4.3 数据源获取流程

```text
[一键生成] 按钮点击
    │
    ▼
读取 global_configs.virtual_provider.preset_data_source_url
    │
    ├── 非空 → 尝试远程 HTTP GET 拉取
    │       ├── 成功 → 解析 JSON → 校验 schemaVersion → 使用
    │       └── 失败 → 读本地缓存 (virtual-slots-cache.json)
    │               ├── 成功 → 使用缓存数据
    │               └── 失败 → 回退内置 JSON
    │
    └── 空字符串 → 直接回退内置 JSON
```

- 远程拉取使用 `reqwest`（已存在于项目依赖），超时 10s。
- 缓存文件存储于 `app_config_dir`（与 `i-code.db` 同目录），文件名 `virtual-slots-cache.json`。
- 远程拉取成功后自动覆盖本地缓存。

---

## 4. 匹配算法

输入：数据源 JSON（`provider` + `slots`）、已开启显示模型列表 `exposedModels: ExposedModel[]`（字段见 `ai-gateway/types.ts`：`id = {providerSlug}/{modelId}`、`providerSlug`、`modelId`、`displayName`、`family`）。

对每个 `slot`：

1. 初始化结果集 `matched: Map<"providerId/modelId", {rule, model}>`（按 `targetProviderId + targetModelId` 去重）。
2. 按 `matches` 的 `priority` 升序遍历规则：
   - `exact` / `prefix` / `regex` 分别与每个 `exposedModel.modelId` 做匹配（`regex` 忽略大小写）。
   - 不再限定供应商 slug（任意供应商均可匹配）。
3. 命中后记录：路由 `target_provider_id = exposedModel.providerId`、`target_model_id = exposedModel.modelId`、`priority = 规则 priority`、`enabled = true`、`is_healthy = true`，并套用 `routeDefaults`（未命中则用虚拟供应商级默认值）。
4. 同一槽位内生成的路由按 `priority ASC, target_model_id ASC` 排序。

**去重规则**：同一 `providerId + modelId` 只生成一条路由（多条规则命中同模型时取 priority 最小的规则）。

**未命中处理**：某槽位 `matched` 为空时——仍创建该虚拟模型但不生成任何路由，并在结果中标记「未匹配到实体模型」提醒用户手动补充；不阻断其它槽位。

---

## 5. 后端设计

> 原则：**最小侵入**。复用现有命令与事务，新增一个「编排型」命令完成一次生成。

### 5.1 新增命令

`virtual_provider_generate_preset(input: GenerateVirtualProviderInput) -> GenerateVirtualProviderResult`

- `GenerateVirtualProviderInput`：
  - `dataSourceUrl?: string`：可选，覆盖设置中的数据源 URL；传空或省略时从 `global_configs` 读取已配置的 URL（配置为空时回退内置 JSON）
  - `strategy` / `maxRetries` / `retryIntervalMs`：可选，覆盖 JSON 中的 `provider` 字段
- 后端 `service::generate_preset(input)` 编排：
  1. 确定数据源 URL：`input.dataSourceUrl` → `global_configs` 中已配置的 URL → 内置 JSON（按 §3.4.3 流程）。
  2. 获取数据源 JSON（远程 → 缓存 → 内置）并解析为 `VirtualSlotsConfig`（校验 `schemaVersion`）。
  3. 通过 `ai_gateway` 的 `list_exposed_models()` 获取已开启显示的模型列表（跨模块只读数据通过对方 Service 暴露的接口，符合 §3.3 分层规则）。
  4. 校验 `alias` 唯一性：已存在同名虚拟供应商时返回 `CONFLICT` 并提示（或由前端确认后复用）。
  5. 在事务中：`create_provider` + 对每个 slot 调用 `save_model`（含匹配到的路由）。`save_model` 已是事务内「创建/更新模型 + 重建路由」，可直接复用。
  6. 返回结果：创建的 provider、每个 slot 的虚拟模型与命中路由数、未命中槽位列表。

### 5.2 路由数据源说明

匹配到的是 `ExposedModel`（含 `providerId` 与 `gatewayModelId`），但 `SaveVirtualModelRouteInput` 需要 `targetProviderId` + `targetModelId`，两者均来自 `ExposedModel`，无需额外查询。

### 5.3 幂等/冲突

- `alias` 冲突：返回 `CONFLICT`，由前端提示「已存在同名虚拟供应商」，让用户选择取消或复用现有供应商（复用时不重建路由，仅提示）。
- 重复点击：前端按钮 loading 防抖；后端不要求幂等（一次生成一份数据，重复生成视为用户意图）。

### 5.4 数据源 URL 配置命令

数据源 URL 存于 `global_configs`（`group='virtual_provider'`, `key='preset_data_source_url'`），新增两个读写命令（归属 `settings` 或 `virtual_provider` 模块均可，建议归属 `virtual_provider` 以保持特性内聚）：

- `virtual_slots_config_get() -> VirtualSlotsConfigDto`：读取当前数据源配置
  - 字段：`dataSourceUrl`（已保存的用户值，可能为空）、`defaultUrl`（内置默认）、`effectiveUrl`（实际生效 URL，即 `dataSourceUrl || defaultUrl`）、`useDefault`（是否回退默认/内置）
- `virtual_slots_config_set(input: { dataSourceUrl?: string })`：保存用户自定义数据源 URL（写 `global_configs`）；传空字符串表示清空、恢复默认
- 校验：`dataSourceUrl` 非空时必须为合法 URL（`http(s)` 或 `file://`），否则返回 `VALIDATION`

---

## 6. 前端 UI 设计

### 6.1 入口

在 `virtual-provider-list.tsx` 顶部工具栏（与「新建/编辑供应商」按钮同行）新增按钮：

- 图标：`fa-solid fa-wand-magic-sparkles`（或 `fa-wand-magic`）
- 文案 i18n：`virtualProvider.generatePreset`（「一键生成三模型」）
- 900×700 窗口内保持紧凑，`text-xs` / 常规按钮尺寸

### 6.2 交互

1. 点击后打开确认弹窗（复用 Dialog）：展示将生成的虚拟供应商（alias / 名称）与三个槽位及其用途简述。
2. 确认后调用 `virtual_provider_generate_preset`，按钮进入 loading。
3. 完成后 toast 汇总结果；在虚拟供应商列表中自动选中新生成的供应商，刷新模型图。

### 6.3 结果展示

- 成功：toast「已生成虚拟供应商 assistant，3 个虚拟模型，共 N 条路由」。
- 部分失败：toast 提示「virtual_opus 未匹配到实体模型，已创建空模型，请手动添加路由」。
- `alias` 冲突：toast 提示并跳转到现有供应商。

### 6.4 i18n

新增键（`virtualProvider` 命名空间，同步 `zh-CN` / `en` / `zh-TW` / `ja`）：
`generatePreset` / `generatePresetDesc` / `generatePresetConfirm` / `generatePresetSuccess` / `generatePresetSlotEmpty` / `generatePresetAliasConflict` 等。

### 6.5 设置页：数据源 URL 配置

在「设置」页（`src/routes/settings.tsx`）新增一个 Card（或并入「AI Gateway」分组）：

- 标题 i18n：`settings.virtualSlotsDataSource`（「虚拟供应商数据源」）
- 内容：
  - 说明文案：数据源 JSON 用于一键生成虚拟供应商时匹配实体模型；默认使用内置 GitHub 仓库地址，可替换为自己的 JSON 地址。
  - URL 输入框：`<Input type="url" />`，`value` 绑定已保存的 `dataSourceUrl`，`placeholder` 显示默认 URL；输入 blob 地址（`/blob/main/...`）时程序自动转 raw。
  - 「恢复默认」按钮：清空输入回到默认 URL。
  - 保存调用 `virtual_slots_config_set`，成功后 toast「数据源已保存」。
- 保存即生效：下次「一键生成三模型」使用新 URL（§3.4.3 流程）。

---

## 7. 边界与失败处理

| 场景 | 处理 |
|------|------|
| 用户配置的数据源 URL 非法（非 `http(s)` / `file://`） | 保存时 `VALIDATION` 拒绝，提示正确格式，保留原值 |
| 远程 URL 拉取失败（离线 / 网络错误 / 非 200） | 依次回退本地缓存 → 内置 JSON；生成仍可进行，结果提示「使用本地/内置数据源」 |
| 远程与本地内置均不可用 | 返回 `VALIDATION` 错误，前端提示「数据源不可用」 |
| 远程 JSON `schemaVersion` 不兼容 | 回退本地缓存 / 内置 JSON；均不兼容则 `VALIDATION` 提示升级应用或更换数据源 |
| alias 已存在 | `CONFLICT`，前端提示用户取消或复用 |
| 某槽位未匹配到任何实体模型 | 仍创建虚拟模型（空路由），结果中标记提醒 |
| 已开启显示模型列表为空 | 提示「无已开启显示的模型，请先在供应商管理开启模型显示」 |
| 生成的供应商/模型与既有冲突（同一 provider+model 重复） | 按去重规则处理；不同虚拟模型间允许引用同一实体模型 |

---

## 8. 实施清单（开发阶段参考）

- [x] `src-tauri/data/virtual-slots.json`：内置兜底数据源（已提供并同步 GitHub 仓库）
- [ ] 后端 `types.rs`：`GenerateVirtualProviderInput` / `GenerateVirtualProviderResult` / `VirtualSlotsConfig`（slot / match / provider 元信息）/ `VirtualSlotsConfigDto`
- [ ] 后端 `service.rs`：`generate_preset`（拉取数据源 → 匹配暴露模型 → 事务创建）+ 数据源 URL 读取 / 缓存写入
- [ ] 后端 `commands.rs` + `main.rs`：注册 `virtual_provider_generate_preset` / `virtual_slots_config_get` / `virtual_slots_config_set`
- [ ] 后端：数据源 URL 读写 `global_configs`（`group='virtual_provider', key='preset_data_source_url'`）+ blob→raw URL 归一化
- [ ] 前端 `types.ts` 同步 DTO
- [ ] 前端 `virtual-provider-list.tsx`：一键按钮 + 确认弹窗 + 结果 toast
- [ ] 前端 `settings.tsx`：数据源 URL Card（输入 + 恢复默认 + 保存）
- [ ] i18n 四语同步；`cargo check` / `pnpm type-check` 验证

---

## 9. 决策点（待评审确认）

1. **数据源仓库/URL**：**已确认**——用户仓库 `https://github.com/xucux/i-code`，默认数据源 URL = `https://raw.githubusercontent.com/xucux/i-code/main/src-tauri/data/virtual-slots.json`；**可在设置页替换为任意用户自有的 JSON 地址**（详见 §3.4）。
2. **alias 命名**：参考 JSON 使用 `assistant`；可按用户习惯改为 `claude` / `ai` 等（注意避免与真实供应商 slug 冲突）。
3. **匹配规则粒度**：用户指定模型 ID 以 `exact` 置顶优先，兜底保留 `prefix` / `exact` / `regex` 通用规则；**不限定供应商 slug**（已移除 `providerSlugs`，兼容中转站与多变渠道），模型 ID 随用户真实供应商集合调整。
4. **失败时是否仍创建空模型**：当前方案「创建空模型 + 提示」；可改为「失败槽位不创建，仅提示」。
5. **数据源 URL 存储位置**：当前方案存 `global_configs`（免迁移）；备选方案为 `app_settings` 新增列（需迁移，不采用）。
6. **远程数据源缓存**：当前方案支持本地缓存（`virtual-slots-cache.json`）；可简化去掉缓存、仅「远程失败→内置」两级回退。
