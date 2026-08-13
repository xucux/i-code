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

数据源 JSON 后续由用户推送到 GitHub 仓库，程序运行时直接以该仓库 JSON 文件为匹配数据源（离线时回退本地内置副本）。

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
  "version": "0.1.0",

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
          "pattern": "^o3(-mini)?$",      // regex 时的正则表达式（大小写不敏感）
          "providerSlugs": ["anthropic"]  // 限定供应商 slug；空数组=不限
        }
      ]
    }
  ]
}
```

### 3.3 匹配规则类型

| type | 说明 | 示例 |
|------|------|------|
| `exact` | `modelId` 与真实模型 `modelId` 完全相等 | `claude-opus-4-1` |
| `prefix` | 真实模型 `modelId` 以 `modelId` 开头（鲁棒，可匹配供应商的版本后缀） | `claude-opus` → `claude-opus-4-1` / `claude-opus-4-5` |
| `regex` | 正则表达式匹配（大小写不敏感），用于不便用前缀表达的场景 | `^o3(-mini)?$` |

匹配时同时受 `providerSlugs` 约束（非空时，真实模型所属供应商 slug 必须在其中）。同一槽位内按 `priority` 升序逐条尝试。

### 3.4 数据源获取方式（后续开发）

用户将 `virtual-slots.json` 推送到 GitHub 仓库后，程序按以下顺序解析数据源：

1. **远程优先**：从「设置」中可配置的数据源 URL（默认指向用户 GitHub 仓库的 raw JSON 地址）拉取；拉取成功则作为当前匹配依据。
2. **本地内置兜底**：远程拉取失败（离线 / 网络错误 / JSON 解析失败）时回退使用内置 `src-tauri/data/virtual-slots.json`。
3. **缓存**：远程拉取成功后按需缓存到本地（例如应用数据目录），供离线重复使用；下次启动可重新拉取校验版本。

> 数据源 URL 属于「外部数据配置」，纳入全局设置管理（`settings` 模块），默认值在提案评审时由用户提供 GitHub 仓库地址后确定。

---

## 4. 匹配算法

输入：数据源 JSON（`provider` + `slots`）、已开启显示模型列表 `exposedModels: ExposedModel[]`（字段见 `ai-gateway/types.ts`：`id = {providerSlug}/{modelId}`、`providerSlug`、`modelId`、`displayName`、`family`）。

对每个 `slot`：

1. 初始化结果集 `matched: Map<"providerId/modelId", {rule, model}>`（按 `targetProviderId + targetModelId` 去重）。
2. 按 `matches` 的 `priority` 升序遍历规则：
   - `exact` / `prefix` / `regex` 分别与每个 `exposedModel.modelId` 做匹配（`regex` 忽略大小写）。
   - 若规则 `providerSlugs` 非空，则仅当 `exposedModel.providerSlug ∈ providerSlugs` 时参与匹配。
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
  - `dataSourceUrl?: string`：数据源 URL（缺省用全局设置默认值；空串或不可用时回退内置 JSON）
  - `strategy` / `maxRetries` / `retryIntervalMs`：可选，覆盖 JSON 中的 `provider` 字段
- 后端 `service::generate_preset(input)` 编排：
  1. 获取数据源 JSON（远程 → 内置兜底）并解析为 `VirtualSlotsConfig`（校验 `schemaVersion`）。
  2. 通过 `ai_gateway` 的 `list_exposed_models()` 获取已开启显示的模型列表（跨模块只读数据通过对方 Service 暴露的接口，符合 §3.3 分层规则）。
  3. 校验 `alias` 唯一性：已存在同名虚拟供应商时返回 `CONFLICT` 并提示（或由前端确认后复用）。
  4. 在事务中：`create_provider` + 对每个 slot 调用 `save_model`（含匹配到的路由）。`save_model` 已是事务内「创建/更新模型 + 重建路由」，可直接复用。
  5. 返回结果：创建的 provider、每个 slot 的虚拟模型与命中路由数、未命中槽位列表。

### 5.2 路由数据源说明

匹配到的是 `ExposedModel`（含 `providerId` 与 `gatewayModelId`），但 `SaveVirtualModelRouteInput` 需要 `targetProviderId` + `targetModelId`，两者均来自 `ExposedModel`，无需额外查询。

### 5.3 幂等/冲突

- `alias` 冲突：返回 `CONFLICT`，由前端提示「已存在同名虚拟供应商」，让用户选择取消或复用现有供应商（复用时不重建路由，仅提示）。
- 重复点击：前端按钮 loading 防抖；后端不要求幂等（一次生成一份数据，重复生成视为用户意图）。

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

---

## 7. 边界与失败处理

| 场景 | 处理 |
|------|------|
| 数据源 URL 拉取失败且内置 JSON 缺失/损坏 | 返回 `VALIDATION` 错误，前端提示「数据源不可用」 |
| JSON `schemaVersion` 不兼容 | 返回 `VALIDATION`，提示升级应用或更换数据源 |
| alias 已存在 | `CONFLICT`，前端提示用户取消或复用 |
| 某槽位未匹配到任何实体模型 | 仍创建虚拟模型（空路由），结果中标记提醒 |
| 已开启显示模型列表为空 | 提示「无已开启显示的模型，请先在供应商管理开启模型显示」 |
| 生成的供应商/模型与既有冲突（同一 provider+model 重复） | 按去重规则处理；不同虚拟模型间允许引用同一实体模型 |

---

## 8. 实施清单（开发阶段参考）

- [ ] `src-tauri/data/virtual-slots.json`：内置兜底数据源（已提供参考版本）
- [ ] 后端 `types.rs`：`GenerateVirtualProviderInput` / `GenerateVirtualProviderResult` / `VirtualSlotsConfig`（slot / match / provider 元信息）
- [ ] 后端 `service.rs`：`generate_preset`（拉取数据源 → 匹配暴露模型 → 事务创建）
- [ ] 后端 `commands.rs` + `main.rs`：注册 `virtual_provider_generate_preset`
- [ ] 前端 `types.ts` 同步 DTO
- [ ] 前端 `virtual-provider-list.tsx`：一键按钮 + 确认弹窗 + 结果 toast
- [ ] `settings` 模块：数据源 URL 配置项（含默认值）
- [ ] i18n 四语同步；`cargo check` / `pnpm type-check` 验证
- [ ] 评审后确定 GitHub 数据源仓库地址并回填默认 URL

---

## 9. 决策点（待评审确认）

1. **数据源仓库/URL**：用户指定的 GitHub 仓库地址与文件路径（当前默认指向内置 JSON，评审后回填）。
2. **alias 命名**：参考 JSON 使用 `assistant`；可按用户习惯改为 `claude` / `ai` 等（注意避免与真实供应商 slug 冲突）。
3. **匹配规则粒度**：当前以 `prefix` 为主兼顾 `exact` / `regex`，模型 ID 随用户真实供应商集合调整。
4. **失败时是否仍创建空模型**：当前方案「创建空模型 + 提示」；可改为「失败槽位不创建，仅提示」。
