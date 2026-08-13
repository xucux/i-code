# 虚拟供应商迭代提案

> 状态：**已确认**（分 6 阶段实施）  
> 关联：`docs/development.md` §5.16、`docs/gateway-runtime.md`、`src-tauri/src/modules/virtual_provider/`、`src-tauri/src/modules/gateway_runtime/forwarding/virtual_forwarder.rs`、`src-tauri/src/modules/scheduler/mod.rs`

---

## 0. 摘要

当前虚拟供应商仅实现了 `fallback` 策略 + 被动降级的最小闭环，存在多处「定义但未落地」的接口（如 `timeout_ms` / `extra_headers_json` / `RouteAttempt` / `HealthCheckStatus`）。本提案规划 6 个独立、可叠加的迭代阶段，按"投入产出比 / 风险"排序，使虚拟供应商达到生产可用状态。

| 阶段 | 主题 | DB 迁移 | 风险 | 价值 |
|------|------|---------|------|------|
| Phase 1 | 路由字段补全（timeout/extraHeaders/extraBody） | 否 | 低 | 高 |
| Phase 2 | 主动健康检查与恢复机制 | 是 | 中 | 高 |
| Phase 3 | `load_balance` / `on_all` 策略 + RouteSelector 抽象 | 是 | 中 | 中 |
| Phase 4 | 路由尝试历史落库与可视化 | 是 | 低 | 中 |
| Phase 5 | UI 交互优化（拖拽排序、tooltip、单条测试） | 否 | 低 | 中 |
| Phase 6 | alias 破坏性变更约束与影响提示 | 否 | 低 | 低 |

---

## 1. 背景与动机

### 1.1 现状

- `VirtualModelStrategy` 枚举 3 种策略，仅 `Fallback` 落地，其余 2 种直接 `NOT_IMPLEMENTED`。
- `VirtualModelRoute` 表已含 `timeout_ms` / `extra_headers_json` / `extra_body_json` 三列，但 `SaveVirtualModelRouteInput` 不接收；`repository::save_model` 把 `timeout_ms` 硬编码 `None`。
- 健康检查只有被动降级（`mark_route_unhealthy`），无主动探活、无恢复、无 `last_healthy_at` 累积。
- `types.ts` 定义了 `RouteAttempt` / `VirtualProviderResolveResult.attempts` / `HealthCheckStatus`，但后端无对应类型或落库。
- 前端路由编辑表单只暴露 `priority` / `maxRetries` / `retryIntervalMs`；路由 `enabled` 在保存时硬编码 `true`，无法单独禁用。
- `VirtualModelGraph` 节点不显示 priority / timeout / 上次健康时间；`quotaPercent` 字段从未被填充。
- `virtual_provider.alias` 可任意修改，但它是 `{alias}/{model_id}` 路由前缀，修改后外部客户端与 CLI profile 中旧前缀失效。

### 1.2 目标

1. 把"已定义未落地"的接口全部落地。
2. 把"被动降级"升级为"主动探活 + 渐进恢复"。
3. 实现剩余 2 种策略，抽出策略选择器抽象便于扩展。
4. 让用户能在 UI 上看到完整路由尝试链、单条路由健康趋势。
5. 防止破坏性变更（alias 修改）悄无声息地影响外部配置。

---

## 2. Phase 1：路由字段补全

### 2.1 目标

让 `VirtualModelRoute` 的 3 个已存在列真正可用，并允许在 UI 上单独禁用某条路由。

### 2.2 数据模型

**不新增迁移**：`virtual_model_routes` 表已含 `timeout_ms` / `extra_headers_json` / `extra_body_json` / `enabled` 列。

### 2.3 后端 DTO 变更

`src-tauri/src/modules/virtual_provider/types.rs`：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveVirtualModelRouteInput {
    pub target_provider_id: String,
    pub target_model_id: String,
    pub priority: i64,
    pub enabled: bool,           // 原硬编码 true，改为由前端传入
    pub is_healthy: bool,
    pub max_retries: i64,
    pub retry_interval_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_headers: Option<serde_json::Value>,  // 反序列化为 JSON 对象
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_body: Option<serde_json::Value>,
}
```

### 2.4 Repository 改造

`repository::save_model` 写入路由时：

- `timeout_ms` 取 `route.timeout_ms`（原 `None::<i64>`）
- `extra_headers_json` = `route.extra_headers.as_ref().map(|v| v.to_string())`
- `extra_body_json` = `route.extra_body.as_ref().map(|v| v.to_string())`
- `enabled` 取 `route.enabled`

### 2.5 网关运行时透传

`route_resolver::build_virtual_context` 在构造 `ForwardContext` 时：

- 将 `route.extra_headers_json` 反序列化后合并进 `ctx.extra_headers`（路由级覆盖供应商级）
- 将 `route.extra_body_json` 合并进 `ctx.extra_body`（路由级覆盖）
- `route.timeout_ms` 设入 `ctx.timeout_ms`，覆盖供应商级 `timeout_ms`

### 2.6 前端 UI

- `SaveVirtualModelRouteInput`（`types.ts`）补全对应字段。
- `RouteSettingsList`（`model-transfer-list.tsx`）每行增加：
  - 启用开关（`enabled`）
  - 超时（毫秒）Input，留空表示继承供应商级
  - 「高级」折叠区：JSON 编辑器编辑 `extraHeaders` / `extraBody`（复用 balance 模块的 `CodeEditor` 组件，预设 `json` 高亮）
- `virtual-model-form.tsx` handleSubmit 时把 `routeDetails` 中的 `enabled` / `timeoutMs` / `extraHeaders` / `extraBody` 一并提交。

### 2.7 校验

- `extra_headers` / `extra_body` 在 Service 层验证为 JSON 对象（非数组、非原始值）。
- `timeout_ms` ≥ 0。

---

## 3. Phase 2：主动健康检查与恢复

### 3.1 目标

让降级后的路由能在上游恢复后自动恢复健康；提供 UI 可见的连续失败次数 / 上次错误 / 上次检查耗时。

### 3.2 数据模型

> **迁移合并说明**：Phase 2/3/4 三处 schema 变更（健康检查元数据、路由权重、路由尝试历史表）均属本次迭代且尚未发布，统一合并为一个迁移文件 `V007__virtual_route_iteration.sql` 生成，不拆分为多个版本号。

第 1 块（健康检查元数据）——并入 `V007__virtual_route_iteration.sql`：

```sql
-- 在 virtual_model_routes 增列：连续失败次数 / 上次错误 / 上次检查耗时
ALTER TABLE virtual_model_routes ADD COLUMN consecutive_failures INTEGER NOT NULL DEFAULT 0;
ALTER TABLE virtual_model_routes ADD COLUMN last_error_text TEXT;
ALTER TABLE virtual_model_routes ADD COLUMN last_check_duration_ms INTEGER;
ALTER TABLE virtual_model_routes ADD COLUMN last_check_at TEXT;

-- 索引：调度器快速取出待探活路由
CREATE INDEX IF NOT EXISTS idx_virtual_routes_health_check
  ON virtual_model_routes(is_healthy, consecutive_failures);
```

> 注意：原 `last_healthy_at` 列已存在；本次只新增探活元数据，不修改既有列语义。

### 3.3 后端模块

#### 3.3.1 Service 新增方法

`virtual_provider/service.rs`：

```rust
/// 探活成功：置健康，重置失败计数，更新 last_healthy_at
pub fn mark_route_healthy(&self, route_id: &str, check_duration_ms: u64) -> IcodeResult<()>;

/// 探活失败：递增 consecutive_failures，记录 last_error_text / last_check_duration_ms / last_check_at
pub fn mark_route_check_failed(&self, route_id: &str, err: &str, check_duration_ms: u64) -> IcodeResult<()>;

/// 列出待探活路由：is_healthy=0 OR consecutive_failures > 0
pub fn list_routes_for_health_check(&self) -> IcodeResult<Vec<VirtualModelRoute>>;
```

#### 3.3.2 调度器扩展

`scheduler/mod.rs` 增加 `health_check_loop`：

- 间隔：60s（可调，常量 `HEALTH_CHECK_INTERVAL_SECS = 60`）
- 对每条候选路由构造轻量探活请求：
  - 优先用 `GET {provider_base_url}/v1/models`，若上游不支持则降级为 `POST /v1/chat/completions`，body = `{"model":"{target_model_id}","messages":[{"role":"user","content":"ping"}],"max_tokens":1}`
  - 超时：5s
  - 成功条件：HTTP 2xx；401/403 视为"路由配置错误"而非"上游不可用"，**不**计入失败次数，记录 `last_error_text` 后跳过降级
- 恢复策略：连续 N=2 次探活成功才置 `is_healthy=1`，避免抖动
- 失败升级：连续失败 ≥ 3 次时调高下次探活间隔（指数退避，最长 10min）

#### 3.3.3 配置

`gateway_settings` 不引入新字段；探活开关放在 `app_settings` 或新增 `virtual_provider_settings` 单例表（暂用 `global_configs` 键值表，键 `virtual_provider.health_check.enabled` / `.interval_secs`）。

### 3.4 前端 UI

- `VirtualModelRoute` 类型补全 `consecutiveFailures` / `lastErrorText?` / `lastCheckDurationMs?` / `lastCheckAt?`
- `VirtualModelGraph` 节点 tooltip 显示：priority / timeout / maxRetries / 上次健康时间 / 上次检查时间 / 连续失败次数 / 上次错误
- 虚拟供应商详情页新增「健康检查」Card：总路由数 / 健康数 / 降级数 / 探活开关

---

## 4. Phase 3：策略补全与 RouteSelector 抽象

### 4.1 目标

实现 `load_balance` 与 `on_all` 策略；抽出策略选择器 trait，便于后续扩展（如基于权重的灰度）。

### 4.2 数据模型

第 2 块（负载均衡权重）——并入 `V007__virtual_route_iteration.sql`：

```sql
-- load_balance 策略使用的权重，0 表示禁用参与轮询
ALTER TABLE virtual_model_routes ADD COLUMN weight INTEGER NOT NULL DEFAULT 1;
```

`virtual_providers.strategy` 字段不变；新增策略值 `load_balance`（已存在但未实现）。

### 4.3 RouteSelector 抽象

`virtual_provider/service.rs`：

```rust
/// 路由选择器：按策略从候选路由中选择一条或多条
pub trait RouteSelector: Send + Sync {
    /// 选择最终要执行的路由（fallback/load_balance 返回 1 条；on_all 返回全部）
    fn select(&self, routes: &[VirtualModelRoute]) -> Vec<&VirtualModelRoute>;
}

pub struct FallbackSelector;       // 现有逻辑迁移过来
pub struct LoadBalanceSelector;    // 加权随机
pub struct OnAllSelector;          // 全部返回

impl VirtualProviderService {
    pub fn selector_for(&self, strategy: &VirtualProviderStrategy) -> Box<dyn RouteSelector> {
        match strategy {
            VirtualProviderStrategy::Fallback => Box::new(FallbackSelector),
            VirtualProviderStrategy::LoadBalance => Box::new(LoadBalanceSelector),
            VirtualProviderStrategy::OnAll => Box::new(OnAllSelector),
        }
    }
}
```

### 4.4 策略实现

#### 4.4.1 LoadBalanceSelector

- 输入：`routes.filter(enabled && healthy)`
- 权重：每条路由 `weight`（默认 1，0 表示跳过）
- 算法：加权随机（`rand::seq::SliceRandom::choose_weighted`）
- 返回：单条路由

#### 4.4.2 OnAllSelector

- 输入：`routes.filter(enabled && healthy)`
- 返回：全部（顺序不重要，由 forwarder 并发处理）
- 若全部 unhealthy：返回空数组，由上层报错

### 4.5 网关运行时改造

`virtual_forwarder.rs::VirtualForwarder::run` 拆为两条路径：

- **单路由路径**（Fallback / LoadBalance）：保留现有逐条尝试 + 降级逻辑
- **并发路径**（OnAll）：

  ```rust
  let results = futures::future::join_all(
      routes.iter().map(|route| async move {
          // 构造 ctx，调用 ForwardPipeline::execute_and_finalize
          // 返回 (route_id, Result<Response, IcodeError>)
      })
  ).await;
  // 取首个 2xx 响应；记录其余失败原因到 attempts
  ```

  - 取消：拿到首个成功响应后，通过 `CancellationToken` 取消其他 in-flight 请求
  - 全失败：返回 502，body 包含每条路由的失败原因

### 4.6 前端 UI

- `VirtualModelGraphNode` 增加 `weight?: number`
- `RouteSettingsList` 在策略为 `load_balance` 时显示 weight 输入框，否则隐藏
- `VirtualModelGraph` 节点在 load_balance 模式下显示权重标签
- 策略下拉框新增 `on_all` 描述："同时请求所有可用路由，取首个成功"

---

## 5. Phase 4：路由尝试历史

### 5.1 目标

把每次请求的路由尝试链落库，供用户在 UI 上查看「最近 N 次」「每条路由的成功率/平均耗时」。

### 5.2 数据模型

第 3 块（路由尝试历史表）——并入 `V007__virtual_route_iteration.sql`：

```sql
CREATE TABLE IF NOT EXISTS virtual_route_attempts (
  id TEXT PRIMARY KEY,
  virtual_route_id TEXT NOT NULL REFERENCES virtual_model_routes(id) ON DELETE CASCADE,
  virtual_provider_id TEXT NOT NULL,
  request_id TEXT NOT NULL,           -- 关联 call_records
  attempt_index INTEGER NOT NULL,      -- 第几条尝试（0-based）
  success INTEGER NOT NULL,            -- 0/1
  status_code INTEGER,
  error_message TEXT,
  duration_ms INTEGER NOT NULL,
  attempted_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_attempts_route_time
  ON virtual_route_attempts(virtual_route_id, attempted_at DESC);
CREATE INDEX IF NOT EXISTS idx_attempts_provider_time
  ON virtual_route_attempts(virtual_provider_id, attempted_at DESC);
```

### 5.3 后端写入

`virtual_forwarder.rs::VirtualForwarder::run` 在每条路由尝试结束后追加一条 `virtual_route_attempts` 记录；通过 `tauri::async_runtime::spawn` 异步写入，不阻塞响应返回。

### 5.4 前端 UI

- 虚拟供应商详情页新增「路由历史」Tab：
  - 时间轴视图：最近 20 次请求，每次请求展开显示尝试链
  - 路由维度统计：每条路由 7 天成功率 / 平均耗时 / 最近一次失败原因
- 复用 `call-records` 模块的图表组件（折线图：成功率趋势）

### 5.5 数据治理

- `virtual_route_attempts` 表按 30 天自动清理（调度器每天清理一次）
- 备份模块（`backup`）需要把该表加入备份列表

---

## 6. Phase 5：UI 交互优化

### 6.1 拖拽排序

- `RouteSettingsList` 引入 `dnd-kit`（项目暂未引入，需评估体积；备选：`@hello-pangea/dnd`）
- 拖拽结束后自动重算 `priority`（0, 1, 2, ...），保留原 priority 作为参考但不提交
- 策略为 `load_balance` 时拖拽不影响 `weight`，只影响展示顺序

### 6.2 节点 Tooltip

- `VirtualModelGraph` 节点 hover 显示 Radix Tooltip：priority / maxRetries / timeout / 上次健康 / 连续失败 / 上次错误
- 颜色编码：health=emerald / unhealthy=red / no_data=muted

### 6.3 单条路由测试

- `RouteSettingsList` 每行新增「测试」按钮（fa-bolt 图标）
- 点击后调用新增 Command `virtual_provider_route_test`：
  - 后端构造一次最小请求（同 Phase 2 探活逻辑），返回 `{ success, statusCode, durationMs, errorMessage }`
  - 不写入 `virtual_route_attempts`（避免污染统计）
  - 结果用 toast 展示

### 6.4 quotaPercent 接入

- `VirtualModelGraphNode.quotaPercent` 字段已存在但未填充
- 在 `virtual-provider-list.tsx::graphTargets` 中读取 `balance` 模块最新一次该 `targetProviderId + targetModelId` 的额度查询结果，映射为 0-100
- 节点底部显示进度条

---

## 7. Phase 6：alias 破坏性变更约束

### 7.1 目标

防止用户修改 `virtual_provider.alias` 后悄无声息地破坏外部客户端与 CLI profile 配置。

### 7.2 影响范围查询

`virtual_provider/service.rs::update_provider` 在 `input.alias` 与 `existing.alias` 不一致时：

1. 查询 `cli_providers` 中 `provider_id` 指向该虚拟供应商的记录（CLI 绑定）
2. 查询 `cli_model_mappings` 中 `gateway_model_id` 以 `{existing.alias}/` 开头的记录
3. 查询 `workspace_cli_configs` 中引用该前缀的配置
4. 返回受影响记录数给前端

### 7.3 前端交互

- `virtual-provider-form.tsx` 中 `alias` 字段变更时：
  - 立即调用 `virtual_provider_check_alias_impact` Command
  - 在字段下方以黄色提示框列出影响：「此变更将影响 N 个 CLI 绑定、M 个模型映射」
  - 用户确认后才允许提交

### 7.4 备选方案：禁止修改 alias

- 在 `update_provider` 中直接拒绝 `input.alias` 的修改，返回 `VALIDATION` 错误
- 创建后 alias 不可改，UI 字段置灰
- 优点：零风险；缺点：灵活性差

**决策**：采用方案 7.3（影响提示 + 用户确认），保留灵活性但显式警告。

---

## 8. 比较矩阵与决策依据

| 阶段 | 实施成本 | 用户价值 | 风险 | 决策 |
|------|----------|----------|------|------|
| Phase 1 | 低（仅 DTO + UI） | 高（释放已定义能力） | 低 | **必做** |
| Phase 2 | 中（新增调度任务） | 高（核心可用性） | 中（探活请求成本） | **必做** |
| Phase 3 | 中（抽象 + 并发） | 中（on_all 场景较少） | 中（并发取消复杂） | **推荐** |
| Phase 4 | 中（新表 + UI） | 中（诊断价值高） | 低 | **推荐** |
| Phase 5 | 低（纯 UI） | 中（体验提升） | 低 | **推荐** |
| Phase 6 | 低（查询 + 提示） | 低（防呆） | 低 | **可选** |

**Phase 排序理由**：1 → 2 → 3 → 4 是依赖链（字段补全后才能做探活；探活后才能做策略；策略后才能完整记录 attempts）；5 / 6 相对独立，可穿插在任意 Phase 后实施。

---

## 9. 演进路径

```
Phase 1 ─┐
         ├─► Phase 2 ─► Phase 3 ─► Phase 4
         │
Phase 5 ─┤（可与 1-4 并行）
Phase 6 ─┘（可与 1-4 并行）
```

**MVP 切分**：Phase 1 + 2 完成后即可作为"生产可用"基线；Phase 3-6 作为增量优化。

---

## 10. 测试策略

每个 Phase 必须满足：

- Rust 侧：`cargo check` 通过，新增逻辑有单元测试（探活 mock、策略选择器）
- 前端：`pnpm type-check` + `pnpm lint` 通过
- 迁移：在空库与已有 V006 数据库上各跑一次，验证幂等
- 集成：手动验证 fallback → 降级 → 探活恢复 → 再次 fallback 的完整闭环

---

## 11. 风险与回滚

| 风险 | 缓解 |
|------|------|
| 探活请求对上游产生负担 | 间隔 60s 起步；指数退避；401/403 跳过；可在 `global_configs` 关闭 |
| on_all 并发请求成本翻倍 | 默认禁用，需用户主动选择；UI 显式提示 |
| `virtual_route_attempts` 表膨胀 | 30 天自动清理；可关闭记录 |
| alias 变更仍可能被用户忽略提示 | Phase 6 完成后，考虑在后续版本强制锁 alias |

回滚：每个 Phase 独立迁移，删除对应迁移文件并回退 `SCHEMA_VERSION` 即可回滚数据库；代码层通过 git revert。

---

## 12. 实施清单（按 Phase）

### Phase 1
- [ ] `types.rs::SaveVirtualModelRouteInput` 补字段
- [ ] `repository.rs::save_model` 透传新字段
- [ ] `route_resolver::build_virtual_context` 合并路由级 headers/body/timeout
- [ ] `types.ts::SaveVirtualModelRouteInput` 同步
- [ ] `model-transfer-list.tsx::RouteSettingsList` UI 补字段
- [ ] `virtual-model-form.tsx` handleSubmit 提交新字段

### Phase 2
- [ ] 迁移 `V007__virtual_route_iteration.sql`（合并 Phase 2/3/4 三块 DDL）+ 注册 + `SCHEMA_VERSION=7`
- [ ] `repository.rs` 新增 `mark_route_healthy` / `mark_route_check_failed` / `list_routes_for_health_check`
- [ ] `service.rs` 新增对应方法
- [ ] `scheduler/mod.rs` 新增 `health_check_loop`
- [ ] `global_configs` 增加 3 个键（enabled / interval_secs / recover_threshold）
- [ ] 前端类型 + Graph tooltip + 健康检查 Card

### Phase 3
- [ ] 迁移 weight 列（已并入 `V007__virtual_route_iteration.sql`，不再单独 V008）
- [ ] `service.rs` 抽出 `RouteSelector` trait + 3 个实现
- [ ] `virtual_forwarder.rs::run` 拆分单路由 / 并发路径
- [ ] `on_all` 路径引入 `CancellationToken`
- [ ] 前端 weight 输入 + 策略描述

### Phase 4
- [x] 迁移 `virtual_route_attempts` 表（已并入 `V007__virtual_route_iteration.sql`，不再单独 V009）
- [x] `repository.rs` 新增 attempts 写入 / 查询 / 清理
- [x] `virtual_forwarder.rs::run` 异步写入 attempts
- [x] `backup` 模块加入新表（备份为整库文件拷贝，新表自动包含，无需改动）
- [x] 前端「路由历史」Tab（路由维度统计 + 选中路由最近 50 次尝试明细）

### Phase 5
- [ ] 引入 dnd 库，`RouteSettingsList` 拖拽排序（延后：需评估依赖体积）
- [x] `VirtualModelGraph` Tooltip（Radix Tooltip 结构化展示优先级/权重/健康/探活/失败原因）
- [x] `virtual_provider_route_test` Command（探活请求 + toast 展示，不写入 attempts）
- [ ] `quotaPercent` 接入（延后：需集成 balance 模块额度数据）

### Phase 6
- [x] `virtual_provider_check_alias_impact` Command（统计 cli_model_mappings 中以旧 alias 为前缀的记录数）
- [x] `virtual-provider-create-dialog.tsx` 影响提示 UI（防抖 300ms 检查 + 黄色警告提示）
- [x] alias 变更后允许编辑（原 disabled=isEdit 移除）

---

## 13. CHANGELOG 写入约定

按 AGENTS.md §9：

- 每个阶段完成时写入对应版本块
- 数据库结构变更（Phase 2 / 3 / 4）需要 `> [!IMPORTANT]` 提示
- 不写 i18n 变更；不写敏感信息
