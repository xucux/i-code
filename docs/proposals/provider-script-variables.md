# 供应商「扩展 → 模板变量」设计方案

> 状态：**已确认**（V002 增量迁移；不做日志脱敏）  
> 关联：`docs/proposals/balance-script-templates.md`、`src-tauri/src/modules/balance/script/context.rs`

## 1. 背景与动机

当前额度监控 Rhai 脚本只能拿到系统注入的固定变量：`api_key`、`provider`、`auth`、`template`、`now_ms`。
部分供应商（如京东 JoyAgent、小米 MiMo）的额度接口鉴权用的是 **Cookie / 自定义 Token / 账户 ID**
等额外凭证，并不等于 `api_key`。现状下脚本作者只能把 Cookie 塞进供应商的 `apiKey` 字段，
导致：

- `api_key` 语义被污染（既是模型调用凭证又是 Cookie）；
- 一个供应商只能配一个额外凭证，无法支持「Cookie + project_id + 自定义 header token」并存；
- Cookie 这类短时凭证与长期 API Key 混在同一个 Secret 里，过期更换不便。

**目标**：在供应商新增/编辑弹窗新增「扩展」Tab，提供**模板变量**（自定义 key/value 列表），
运行时把这些变量作为只读系统常量注入 Rhai Scope，脚本按 `variables["cookie"]` 或
`cookie`（扁平别名）取用。

## 2. 数据模型

### 2.1 数据库

`providers` 表新增一列：

```sql
-- V001__init.sql providers 表内追加（fresh install 生效）
script_variables_json TEXT,
```

> **迁移策略见 §7**：现有 DB 需要 `ALTER TABLE providers ADD COLUMN script_variables_json TEXT;`。

### 2.2 存储结构（JSON）

`script_variables_json` 存 `ProviderScriptVariables` 序列化字符串：

```jsonc
{
  "version": 1,
  "items": [
    {
      "key": "cookie",
      "value": "$SECRET:1892347...$",   // 敏感：引用
      "isSecret": true,
      "label": "JoyAgent Cookie",
      "allowedHosts": ["joyagent.jd.com", "agentrs.jd.com"]
    },
    {
      "key": "project_id",
      "value": "my-project",             // 非敏感：明文
      "isSecret": false
    }
  ]
}
```

字段说明：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `version` | int | 是 | 结构版本，当前 `1`，便于后续演进 |
| `items[].key` | string | 是 | 变量名，脚本中取用键；`^[a-zA-Z_][a-zA-Z0-9_]*$`，禁止与系统保留名冲突 |
| `items[].value` | string | 是 | `isSecret=true` 时为 `$SECRET:{id}$` 引用；否则明文 |
| `items[].isSecret` | bool | 否 | 是否走加密存储（默认 `false`） |
| `items[].label` | string | 否 | UI 显示用备注 |
| `items[].allowedHosts` | string[] | 否 | 仅当该变量被脚本用作 URL/凭证时，可选记录允许请求的 host 白名单提示；运行时不强制（host 白名单仍以 `provider.base_url` 为准，见 §5） |

> 保留名（禁止作为 key）：`api_key`、`now_ms`、`provider`、`auth`、`template`、`variables`、`pi`、`e`。

### 2.3 后端 DTO（`ai_gateway/types.rs`）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderScriptVariables {
    pub version: i32,
    pub items: Vec<ProviderScriptVariable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderScriptVariable {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub is_secret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_hosts: Vec<String>,
}
```

- `Provider` DTO 新增 `script_variables_json: Option<String>`。
- `CreateProviderInput` / `UpdateProviderInput` 新增 `script_variables_json: Option<String>`
  （`UpdateProviderInput` 用 `Option<Option<String>>` 语义以区分「未传」与「清空」）。

### 2.4 前端类型（`src/modules/ai-gateway/types.ts`）

```ts
export interface ProviderScriptVariables {
  version: number
  items: ProviderScriptVariable[]
}
export interface ProviderScriptVariable {
  key: string
  value: string
  isSecret?: boolean
  label?: string
  allowedHosts?: string[]
}
// Provider 新增
scriptVariablesJson?: string
```

> 该类型手工维护（与 `virtual-provider` 一致），不进 ts-rs 自动生成范围。

## 3. 脚本运行时注入

### 3.1 解密流程

`build_balance_refresh_input`（`ai_gateway/service.rs`）目前只解密 `auth_json` 得到 `api_key`。
扩展为：同时解析 `provider.script_variables_json`，对 `isSecret=true` 的项调用
`secret_handle.service().resolve_ref(&item.value)` 解密明文，得到
`Vec<(key, plaintext_value)>`，放入 `BalanceRefreshInput::script_variables`。

### 3.2 `BalanceRefreshInput` 扩展

```rust
// balance/provider.rs
pub struct BalanceRefreshInput {
    // ... 既有字段 ...
    /// 已解密的模板变量（key, 明文 value）
    pub script_variables: Vec<(String, String)>,
}
```

### 3.3 `ScriptContext` 注入

`context.rs` 在 `inject_into_scope` 末尾追加：

```rust
// 注入 variables map（key → 明文 value）
let mut vars = Map::new();
for (k, v) in &self.script_variables {
    vars.insert(k.clone().into(), Dynamic::from(v.clone()));
}
scope.push_constant("variables", Dynamic::from_map(vars));

// 同时注入扁平别名（每个 key 直接作为顶层常量，便于脚本直接写 cookie）
for (k, v) in &self.script_variables {
    // 跳过保留名冲突（理论上 §2.2 已禁止，二次保险）
    if !is_reserved_name(k) {
        scope.push_constant(k.clone(), v.clone());
    }
}
```

> 同时提供 `variables["cookie"]`（map 访问）与 `cookie`（顶层常量）两种写法，
> 与 `balance-script-prompt-guide.md` 文档对齐。**保留名冲突时跳过扁平注入**，仅 `variables[...]` 可用。

### 3.4 脚本使用示例

```rhai
// 优先用 variables.cookie；顶层别名 cookie 也可直接用
let cookie = variables["cookie"];
let headers = #{
    "Cookie": cookie,
    "Referer": "https://joyagent.jd.com/",
    "User-Agent": "Mozilla/5.0 ..."
};
```

## 4. Secret 处理

### 4.1 新增 SecretKind

`secret/types.rs` 新增：

```rust
pub enum SecretKind {
    // ... 既有 ...
    /// 供应商扩展模板变量
    ScriptVariable,
}
// as_str => "script-variable"
```

### 4.2 保存路径（复用既有加密管线）

`ai_gateway/service.rs::create_provider` / `update_provider` 在序列化
`script_variables_json` 前，调用新增的 `process_script_variables_for_save`：

- 对 `isSecret=true` 且 `value` 为明文（非 `$SECRET:` 引用）的项 →
  `maybe_encrypt_secret(&Some(value), SecretKind::ScriptVariable)` → 替换为引用；
- 已是引用 / `isSecret=false` → 原样保留；
- `isSecret=true` 但 value 为空 → 视为未设置，保留空串。

> 与 `auth` 加密路径**完全一致**，前端只传明文一次，后端加密后只存引用。

### 4.3 Secret 引用清理

删除供应商 / 删除某个变量项时，旧 Secret 引用成为孤儿。复用既有
`secret::service::scan_references` + `garbage_collect`（如已实现）；
若未实现，先标注 TODO，至少保证删除供应商时级联清理（参考 `auth_json` 现有清理逻辑）。

## 5. 前端 UI

### 5.1 Tab 结构

`provider-form.tsx` 的 `TabsList` 在 `advanced` 右侧追加：

```tsx
<TabsTrigger value="extension" className="text-xs">
  {t('aiGateway.providerForm.tabs.extension')}
</TabsTrigger>
```

`TabsContent value="extension"` 内放 `ScriptVariablesEditor` 组件。

### 5.2 `ScriptVariablesEditor` 组件

新建 `src/modules/ai-gateway/ui/script-variables-editor.tsx`：

- 顶部说明：变量将作为只读系统常量注入额度脚本（`variables["key"]` 或顶层 `key`）。
- 列表：每行三列 + 操作
  - Key：`<Input>`，`onBlur` 校验 `^[a-zA-Z_][a-zA-Z0-9_]*$`，重复/保留名红字提示
  - Value：
    - `isSecret=false` → 普通 `<Input>`
    - `isSecret=true` → `<SecretInput>`（password 框 + 「已保存，重新输入以替换」占位），
      编辑态若 provider 已有引用则显示 `••••••••` 占位，留空表示不修改
  - Label（可选）：`<Input>` 简短备注
  - 删除按钮：`<Button variant="ghost" size="icon"><i className="fa-solid fa-trash" /></Button>`
- 底部「+ 添加变量」按钮；「敏感」勾选框（Switch）切换明文/加密存储
- 整体紧凑（900×700 窗口）：单行 `grid-cols-[120px_1fr_120px_28px]`，字号 `text-xs`

### 5.3 表单状态管理

与既有高级设置一致，不入 zod schema，用 `useState` 管理：

```ts
const [scriptVariables, setScriptVariables] = useState<ProviderScriptVariable[]>([])
```

- 编辑回填：`JSON.parse(provider.scriptVariablesJson ?? '{"version":1,"items":[]}')`
  - 对 `isSecret=true` 项，value 保留 `$SECRET:...$` 引用原样，UI 显示为占位
- 提交：`handleSubmit` 中 `buildScriptVariablesJson(scriptVariables)`：
  - `isSecret=true` 且 value 为空 / 仍为 `$SECRET:...$` 占位 → 保留原引用（编辑时未改）
  - `isSecret=true` 且 value 为新明文 → 原样发给后端（后端加密）
  - `isSecret=false` → 明文原样
  - 过滤掉 key 为空或重复的项
  - items 为空 → `undefined`（不传该字段，后端不覆盖）

### 5.4 i18n

`zh-CN` / `en` 同步新增键（`aiGateway.providerForm.scriptVariables.*`）：

| key | zh-CN | en |
|-----|-------|-----|
| `tabs.extension` | 扩展 | Extension |
| `scriptVariables.title` | 模板变量 | Template Variables |
| `scriptVariables.description` | 这些变量将作为只读常量注入额度监控脚本 | These variables are injected as read-only constants into balance scripts |
| `scriptVariables.keyPlaceholder` | 变量名 | Variable name |
| `scriptVariables.valuePlaceholder` | 变量值 | Value |
| `scriptVariables.labelPlaceholder` | 备注 | Label |
| `scriptVariables.add` | 添加变量 | Add variable |
| `scriptVariables.isSecret` | 敏感（加密） | Secret |
| `scriptVariables.keyInvalid` | 变量名只能含字母/数字/下划线，且不以数字开头 | Name must match `^[a-zA-Z_][a-zA-Z0-9_]*$` |
| `scriptVariables.keyReserved` | 该名称为系统保留 | Reserved system name |
| `scriptVariables.keyDuplicate` | 变量名重复 | Duplicate name |

## 6. 校验规则

| 校验点 | 规则 |
|--------|------|
| key 格式 | `^[a-zA-Z_][a-zA-Z0-9_]*$` |
| key 唯一 | 同一供应商内 key 不可重复（不区分大小写） |
| key 保留名 | 禁止 `api_key`/`now_ms`/`provider`/`auth`/`template`/`variables`/`pi`/`e` |
| key 长度 | 1–64 字符 |
| value 长度 | 明文 ≤ 4096 字符（超长走 Secret 引用同样限制明文长度） |
| items 数量 | ≤ 32 项 |
| allowedHosts | 每项 ≤ 64 字符，最多 8 项（仅提示，不强制 host 白名单） |

后端 `process_script_variables_for_save` 与 `build_balance_refresh_input` 都做这些校验，
前端 Editor 做即时反馈。

## 7. 迁移与兼容

### 7.1 问题

`src-tauri/src/db/migrations.rs` 当前为「基线重置模式」：仅 V001，启动时清空
`schema_migrations` 重跑 V001，但 V001 用 `CREATE TABLE IF NOT EXISTS`，**不会给已存在的表
追加新列**。所以直接改 V001 只对 fresh install 生效，存量 DB 不会出现新列。

### 7.2 方案

新增增量迁移 `V002__provider_script_variables.sql`：

```sql
ALTER TABLE providers ADD COLUMN script_variables_json TEXT;
```

并在 `migrations.rs::BUILTIN_MIGRATIONS` 追加 `(2, "provider_script_variables", V002__PROVIDER_SCRIPT_VARIABLES)`。
此时 `is_baseline_only()` 返回 `false`，不再清空 `schema_migrations`，存量 DB 按 `version > current`
执行 V002 一次，正确加上新列。

同时在 V001__init.sql 的 `providers` 表定义中加入 `script_variables_json TEXT,`，保证
fresh install 也有该列（V002 在 fresh install 会因列已存在而报错，需用幂等写法，见下）。

### 7.3 V002 幂等性

SQLite 的 `ALTER TABLE ADD COLUMN` 不支持 `IF NOT EXISTS`。为兼容「fresh install 已有列」
与「存量 DB 加列」两种场景，V002 用 PL/SQL 风格的列存在性检查：

```sql
-- 仅当列不存在时添加（SQLite 不支持 ADD COLUMN IF NOT EXISTS）
INSERT INTO schema_migrations(version, applied_at)
SELECT 2, datetime('now')
WHERE 0;  -- 占位，真实加列逻辑见下
```

实际实现：在 `migrations.rs` 的 V002 SQL 中用一条带 `PRAGMA table_info` 的过程不可行（SQL 层无法
条件 DDL）。推荐两种之一：

- **方案 A（推荐）**：fresh install 走 V001（已含列），存量 DB 走 V002；V002 SQL 包裹在
  `migrations.rs` 的 Rust 逻辑里（检测列是否存在再 `ALTER`），而非纯 SQL。
  即把 V002 实现为 Rust 函数而非 `include_str!` 的纯 SQL。
- **方案 B**：V002 直接 `ALTER TABLE ... ADD COLUMN`，并在 V001 中**不**加该列；
  fresh install 先跑 V001（无该列）再跑 V002（加列），两条迁移都跑，自然幂等。
  → **更简单，推荐**。V001 保持纯净（不含 script_variables_json），新列只由 V002 引入。

> **已确认**：采用方案 B：V001 不动，新增 V002 引入列。这与 AGENTS.md §6.3「只追加不改历史」一致，
> 并修正 `migrations.rs` 中「不再追加 V002+」的过时注释。

### 7.4 向后兼容

- 旧 `script_variables_json` 为 NULL → `items` 视为空数组，脚本无 `variables` 常量。
- 旧脚本继续用 `api_key` 取 Cookie 不受影响（平滑迁移，不强制改写 snippet）。
- 后续逐步把 JoyAgent / MiMo 等 snippet 改为读 `variables["cookie"]`，并在文档标注。

## 8. 改动清单

### 后端 (`src-tauri/`)

| 文件 | 改动 |
|------|------|
| `src/db/migrations/V002__provider_script_variables.sql` | 新增：`ALTER TABLE providers ADD COLUMN script_variables_json TEXT;` |
| `src/db/migrations.rs` | 注册 V002；更新注释 |
| `src/modules/ai_gateway/types.rs` | `Provider` / `CreateProviderInput` / `UpdateProviderInput` 加 `script_variables_json` |
| `src/modules/ai_gateway/repository.rs` | insert/update/list SQL 加列 |
| `src/modules/ai_gateway/service.rs` | `process_script_variables_for_save`（加密）；`build_balance_refresh_input` 解密注入 |
| `src/modules/balance/provider.rs` | `BalanceRefreshInput::script_variables` |
| `src/modules/balance/script/context.rs` | `ScriptContext` 携带 variables；`inject_into_scope` 注入 `variables` map + 扁平别名 |
| `src/modules/secret/types.rs` | 新增 `SecretKind::ScriptVariable` |
| `src/modules/backup/service.rs` | 导出/导入 providers 时包含新字段 |

### 前端 (`src/`)

| 文件 | 改动 |
|------|------|
| `src/modules/ai-gateway/types.ts` | `ProviderScriptVariables` / `ProviderScriptVariable`；`Provider.scriptVariablesJson` |
| `src/modules/ai-gateway/ui/script-variables-editor.tsx` | 新建：key/value 列表编辑器 |
| `src/modules/ai-gateway/ui/provider-form.tsx` | TabsList 加 `extension`；TabsContent 内渲染 Editor；state / 回填 / 提交 |
| `src/hooks/use-ai-gateway-mutation.ts` | create/update 入参加 `scriptVariablesJson` |
| `src/locales/*/ai-gateway.json` | zh-CN / en 同步新增 §5.4 键 |

### 文档

| 文件 | 改动 |
|------|------|
| `docs/database.md` | `providers` 表加 `script_variables_json` 列说明 |
| `docs/proposals/balance-script-prompt-guide.md` | 系统变量章节加 `variables` 与扁平别名说明 |
| `docs/proposals/balance-script-templates.md` | 关联本提案 |
| `AGENTS.md` §3.2 | `ai-gateway` 模块职责补「模板变量」 |

## 9. 安全与边界

- **明文不落库**：`isSecret=true` 的 value 经后端加密后只存 `$SECRET:{id}$` 引用，符合 AGENTS.md §1 核心规则。
- **日志脱敏**：不在本次范围。模板变量中的敏感值（如 Cookie）在脚本日志中不做自动脱敏，与当前 `api_key` 在脚本 `log::info` 中可能明文出现的行为一致。
- **导出/备份**：`backup/service.rs` 导出 providers 时，`script_variables_json` 中的 `$SECRET` 引用应与 `auth_json` 一致——默认导出引用（不含明文），导入时重新加密（复用既有 Secret 引用映射机制）。
- **host 白名单不变**：模板变量只提供「值」，不改变 `host_http.rs` 的 host 白名单逻辑（仍以 `provider.base_url` host + `script_template` 的 `allowed_hosts` 为准）。`allowedHosts` 字段仅作 UI 提示，不参与运行时强制，避免引入越权风险。
- **脚本只读**：注入为 `push_constant`，脚本不可修改，与现有 `api_key` 一致。
- **保留名二次保险**：`inject_into_scope` 对保留名跳过扁平注入，仅 `variables[...]` 可用。

## 10. 不在本次范围

- 不引入「变量值类型」（int/bool）：统一字符串，脚本内自行 `str::to_float` 转换（已注册）。
- 不做日志脱敏扩展（模板变量中的敏感值在脚本日志中不自动脱敏）。
- 不做工作区级模板变量（仅供应商级）。
- 不改写既有 `mimo_balance` / `joyagent_balance` snippet 强制迁移；提供新能力后逐步切换。

## 11. 验收

- [ ] 新建供应商 → 扩展 Tab 添加 `cookie`（敏感）+ `project_id`（明文）→ 保存 → DB `script_variables_json` 含 1 个 `$SECRET` 引用 + 1 个明文。
- [ ] 试运行额度脚本：脚本中 `log::info(variables["cookie"])` 输出明文。
- [ ] 删除 `cookie` 变量后保存 → 旧 Secret 引用被清理（或标注 TODO）。
- [ ] 保留名 / 重复名 / 非法格式前端红字提示且无法保存。
- [ ] 存量 DB（v1）启动后自动执行 V002，`providers` 出现 `script_variables_json` 列，既有数据不丢。
