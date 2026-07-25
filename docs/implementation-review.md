# i-code 模块实现与开发文档一致性审查报告

> 审查日期：2026-07-15  
> 对比文档：`docs/development.md`、`docs/database.md`  
> 检查范围：后端 Rust 模块 + 前端 TypeScript 模块

---

## 1. 总体评估

| 维度 | 状态 | 说明 |
|------|------|------|
| 后端模块结构 | ✅ 符合 | DDD 分层清晰，commands/service/repository/types 四层完整 |
| 前端模块结构 | ✅ 符合 | types.ts 定义完整，与后端对齐 |
| 数据库 Schema | ✅ 符合 | V001 迁移覆盖文档定义的所有表 |
| 核心业务流程 | ⚠️ 部分实现 | 部分模块标注"待实现" |

---

## 2. 已实现模块清单

### 2.1 后端模块（Rust）

| 模块 | 状态 | 文件完整性 |
|------|------|-----------|
| `shared` | ✅ 完整 | ProxyConfig / RetryConfig / TimeoutConfig 已实现 |
| `secret` | ✅ 完整 | types / crypto / service / repository / commands |
| `settings` | ✅ 完整 | types / service / repository / commands |
| `ai_gateway` | ✅ 完整 | types / service / repository / commands / seed |
| `balance` | ✅ 完整 | types / service / commands |
| `logger` | ✅ 完整 | types / service / repository / commands |
| `call_records` | ✅ 完整 | types / service / repository / commands |
| `backup` | ✅ 完整 | types / service / commands |
| `gateway_runtime` | ⚠️ 部分 | types / router / auth / service / upstream（路由不完整） |
| `virtual_provider` | ⚠️ 部分 | types / service / repository / commands（策略枚举不一致） |

### 2.2 前端模块（TypeScript）

| 模块 | 状态 | 说明 |
|------|------|------|
| `core` | ✅ 完整 | types.ts / errors.ts / events.ts / utils.ts / constants.ts |
| `hooks` | ✅ 完整 | use-command.ts / use-gateway-status.ts 等 |
| `ai-gateway` | ✅ 完整 | types.ts 定义完整 |
| `settings` | ✅ 完整 | types.ts 定义完整 |
| `theme` | ✅ 完整 | 6 种主题 CSS 变量已定义 |
| `i18n` | ✅ 完整 | zh-CN / en 语言包 |
| `cli-management` | ⚠️ 待实现 | 目录存在但无实质内容 |
| `workspace` | ⚠️ 待实现 | 目录存在但无实质内容 |

---

## 3. 发现的不一致问题

### 3.1 虚拟供应商策略枚举不一致 ❌

**文档定义**（development.md §5.16.1）：
```typescript
FailoverStrategy = 'on_error' | 'on_quota_exceeded' | 'on_timeout' | 'on_all'
```

**代码实现**（`virtual_provider/types.rs`）：
```rust
pub enum VirtualProviderStrategy {
    OnAll,       // 对应文档 on_all
    Fallback,    // 文档中无此值
    LoadBalance, // 文档中无此值
}
```

**数据库默认值**（`V001__init.sql`）：
```sql
strategy TEXT NOT NULL DEFAULT 'on_all'
```

**问题**：
1. 代码中的 `Fallback` 和 `LoadBalance` 在文档中无定义
2. 文档中的 `on_error`、`on_quota_exceeded`、`on_timeout` 在代码中缺失
3. 枚举命名风格不一致（文档用 `on_xxx`，代码用 `Fallback`/`LoadBalance`）

**建议**：统一为文档定义的四值枚举，或更新文档说明实际实现策略。

---

### 3.2 Gateway Runtime 路由不完整 ⚠️

**文档定义**（development.md §5.8.3）：
- `GET /health` ✅
- `GET /readyz` ✅
- `GET /v1/models` ✅
- `POST /v1/chat/completions` ✅
- `POST /v1/responses` ❌ 未实现
- `POST /v1/embeddings`（可选）❌ 未实现

**代码实现**（`gateway_runtime/router.rs`）：
```rust
.route("/health", get(health))
.route("/readyz", get(readyz))
.route("/v1/models", get(list_models))
.route("/v1/chat/completions", post(chat_completions))
.route("/v1/messages", post(anthropic_messages))  // 文档未提及此路由
```

**问题**：
1. `/v1/responses` 路由缺失（OpenAI Responses API）
2. `/v1/messages` 路由存在但文档未说明（Anthropic 兼容）

**建议**：在文档中补充 `/v1/messages` 路由说明，或在代码中添加 `/v1/responses` 路由。

---

### 3.3 CLI Management 和 Workspace 模块未实现 ⚠️

**文档定义**：development.md §5.6 和 §5.7 详细定义了这两个模块的职责和流程。

**代码状态**：
- 后端 `modules/mod.rs` 注释标注：`cli_management` 和 `workspace` 为"待实现"
- 前端目录 `src/modules/cli-management/` 和 `src/modules/workspace/` 存在但无实质内容

**影响**：
- CLI 档案管理、供应商绑定、模型映射功能不可用
- 工作区隔离、Prompts/MCP/Skill 配置功能不可用

**建议**：这是 v0.1 范围外的功能，文档应明确标注为"后续迭代"。

---

### 3.4 Secret 模块仅实现本地加密模式 ⚠️

**文档定义**（development.md §5.9.2）：
- 支持两种存储模式：`keychain`（系统密钥链）或 `encrypted`（本地 AES-GCM 加密）

**代码实现**（`secret/service.rs` 注释）：
```rust
//! ## v0.1 限制
//!
//! - 仅实现「本地 AES-GCM 加密」模式
//! - 系统密钥链模式需引入 `tauri-plugin-stronghold`，待后续迭代
```

**建议**：文档应明确标注 v0.1 仅支持本地加密模式。

---

### 3.5 路由模式字段类型不一致 ⚠️

**文档定义**（database.md §4.26）：
```sql
route_mode INTEGER NOT NULL DEFAULT 1  -- 1=路由模式，0=直连
```

**代码实现**（`call_records/types.rs`）：
```rust
pub enum RouteMode {
    Direct = 1,          // 直接请求真实供应商
    VirtualFallback = 2, // 虚拟供应商故障转移
}
```

**问题**：
1. 文档定义 `route_mode=0` 为直连，代码定义 `Direct=1`
2. 文档未定义 `VirtualFallback=2` 这个值

**建议**：统一枚举值定义，更新文档或代码。

---

### 3.6 内置数据种子脚本未执行 ⚠️

**文档定义**（database.md §7.2）：
- `builtin_*` 表的种子数据应由构建脚本从参考项目导出

**代码状态**（`V001__init.sql` 注释）：
```sql
-- ===== 内置数据（builtin_* 表）=====
-- 占位：实际内置数据由 scripts/sync-builtin-data.ts 脚本
-- 从参考项目 well-known/models.ts 与 well-known/providers.ts 导出生成迁移文件。
```

**问题**：种子数据 SQL 被注释，表结构存在但无数据。

**建议**：执行 `scripts/sync-builtin-data.ts` 生成 V002 种子数据迁移。

---

## 4. 前后端类型对齐检查

### 4.1 已对齐的类型 ✅

| 类型 | 后端文件 | 前端文件 | 状态 |
|------|---------|---------|------|
| ProviderType | ai_gateway/types.rs | ai-gateway/types.ts | ✅ 一致 |
| AuthConfig | ai_gateway/types.rs | ai-gateway/types.ts | ✅ 一致 |
| AppSettings | settings/types.rs | settings/types.ts | ✅ 一致 |
| ProxyConfig | shared/mod.rs | ai-gateway/types.ts | ✅ 一致 |
| RetryConfig | shared/mod.rs | ai-gateway/types.ts | ✅ 一致 |
| TimeoutConfig | shared/mod.rs | ai-gateway/types.ts | ✅ 一致 |
| SecretKind | secret/types.rs | secret/types.ts | ✅ 一致 |
| BalanceMethod | balance/types.rs | balance/types.ts | ✅ 一致 |

### 4.2 需要检查的类型 ⚠️

| 类型 | 问题描述 |
|------|---------|
| VirtualProviderStrategy | 枚举值不一致（见 §3.1） |
| RouteMode | 枚举值不一致（见 §3.5） |
| FailoverStrategy | 文档定义但代码中无独立枚举 |

---

## 5. 数据库 Schema 一致性检查

### 5.1 已正确实现的表 ✅

| 表名 | 文档章节 | 状态 |
|------|---------|------|
| `app_settings` | §4.1 | ✅ |
| `secrets` | §4.2 | ✅ |
| `gateway_settings` | §4.27 | ✅ |
| `gateway_auth_keys` | §4.28 | ✅ |
| `providers` | §4.3 | ✅ |
| `provider_extra_headers` | §4.4 | ✅ |
| `provider_extra_body` | §4.5 | ✅ |
| `model_configs` | §4.6 | ✅ |
| `model_config_extra_headers` | §4.7 | ✅ |
| `model_config_extra_body` | §4.8 | ✅ |
| `gateway_models` | §4.9 | ✅ |
| `official_model_cache` | §4.10 | ✅ |
| `builtin_models` | §4.11 | ✅ |
| `builtin_model_providers` | §4.12 | ✅ |
| `builtin_model_overrides` | §4.13 | ✅ |
| `builtin_providers` | §4.14 | ✅ |
| `builtin_provider_auth_types` | §4.15 | ✅ |
| `builtin_provider_models` | §4.16 | ✅ |
| `builtin_model_aliases` | §4.17 | ✅ |
| `cli_profiles` | §4.18 | ✅ |
| `cli_providers` | §4.19 | ✅ |
| `cli_model_mappings` | §4.20 | ✅ |
| `workspaces` | §4.21 | ✅ |
| `workspace_cli_configs` | §4.22 | ✅ |
| `workspace_prompts` | §4.23 | ✅ |
| `workspace_mcp_servers` | §4.24 | ✅ |
| `workspace_skills` | §4.25 | ✅ |
| `model_call_logs` | §4.26 | ✅ |
| `virtual_providers` | §5.16 | ✅ |
| `virtual_models` | §5.16 | ✅ |
| `virtual_model_routes` | §5.16 | ✅ |

### 5.2 虚拟供应商表字段差异 ⚠️

**文档定义**（development.md §5.16.2）：
```
VirtualProvider:
  - strategy: FailoverStrategy  // 'on_error' | 'on_quota_exceeded' | 'on_timeout' | 'on_all'
```

**代码实现**（`V001__init.sql`）：
```sql
strategy TEXT NOT NULL DEFAULT 'on_all'  -- 接受 'on_all' | 'fallback' | 'load_balance'
```

**建议**：统一策略枚举值。

---

## 6. 业务流程实现检查

### 6.1 已实现的流程 ✅

| 流程 | 文档章节 | 实现状态 |
|------|---------|---------|
| 供应商 CRUD | §9.1 | ✅ 完整 |
| Secret 加密/解密 | §5.9 | ✅ 完整 |
| 应用设置管理 | §5.4 | ✅ 完整 |
| 网关健康检查 | §5.8 | ✅ 完整 |
| 模型列表接口 | §5.8 | ✅ 完整 |
| 认证中间件 | §5.8 | ✅ 完整 |
| 虚拟供应商 CRUD | §5.16 | ✅ 完整 |
| 额度监控配置 | §5.10 | ✅ 完整 |
| 日志记录 | §5.11 | ✅ 完整 |
| 调用记录 | §5.12 | ✅ 完整 |
| 备份配置 | §5.15 | ✅ 完整 |

### 6.2 部分实现的流程 ⚠️

| 流程 | 文档章节 | 实现状态 | 缺失部分 |
|------|---------|---------|---------|
| 网关请求转发 | §9.10 | ⚠️ 部分 | `/v1/responses` 路由缺失 |
| 故障转移 | §5.16.3 | ⚠️ 部分 | 仅实现 fallback 策略 |
| 内置模型添加 | §9.5 | ⚠️ 部分 | 种子数据未填充 |
| 官方模型拉取 | §9.6 | ⚠️ 部分 | 注释标注待实现 |

### 6.3 未实现的流程 ❌

| 流程 | 文档章节 | 说明 |
|------|---------|------|
| CLI 档案管理 | §5.6 | 模块标注"待实现" |
| 工作区隔离 | §5.7 | 模块标注"待实现" |
| 配置导入/导出 | §9.3 | ProviderShareConfig 未实现 |
| WebDAV 备份 | §5.15 | 仅本地备份已实现 |

---

## 7. 建议修复项

### 7.1 高优先级（影响功能正确性）

1. **统一虚拟供应商策略枚举**
   - 更新 `virtual_provider/types.rs` 使用文档定义的枚举值
   - 或更新 `development.md` §5.16.1 说明实际实现

2. **补充 `/v1/responses` 路由**
   - 在 `gateway_runtime/router.rs` 添加路由
   - 或在文档中标注为可选

3. **统一 RouteMode 枚举值**
   - 更新 `call_records/types.rs` 使用 `0=Direct` / `1=VirtualFallback`
   - 或更新文档说明实际值

### 7.2 中优先级（影响完整性）

4. **执行种子数据脚本**
   - 运行 `scripts/sync-builtin-data.ts` 生成 V002 迁移
   - 填充 `builtin_*` 表数据

5. **补充文档标注**
   - 在 development.md 中明确标注 v0.1 不包含的功能：
     - CLI Management（§5.6）
     - Workspace（§5.7）
     - 系统密钥链模式（§5.9）
     - WebDAV 备份（§5.15）
     - 配置导入/导出（§9.3）

### 7.3 低优先级（改进项）

6. **补充前端页面路由**
   - 当前仅有 `/`、`/preview`、`/mini-panel` 三个路由
   - 文档 §6.2 定义的页面路由（`/gateways`、`/cli`、`/workspaces`、`/settings`）未实现

7. **完善 gateway_runtime 上游转发**
   - `upstream.rs` 中的 `forward_chat_completions` 需要实际对接供应商 API
   - 当前可能为 stub 实现

---

## 8. 结论

**总体评估**：后端架构设计与文档高度一致，模块分层清晰，类型定义完整。主要差异集中在：

1. **虚拟供应商策略枚举**：代码实现与文档定义不一致
2. **部分路由缺失**：`/v1/responses` 未实现
3. **未实现模块**：CLI Management 和 Workspace 标注为"待实现"
4. **种子数据缺失**：内置供应商/模型数据未填充

**建议**：
- 短期：修复枚举不一致问题，更新文档标注
- 中期：执行种子数据脚本，补充网关路由
- 长期：实现 CLI Management 和 Workspace 模块
