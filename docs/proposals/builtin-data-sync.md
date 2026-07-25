# 内置数据（builtin_*）同步方案对比

> 状态：v0.1 已采用方案 A（构建脚本生成 SQL 迁移），待评估是否切换到方案 C。
> 关联：`docs/development.md` §5.5 ai-gateway、§5.14 db/persistence
> 涉及表：`builtin_models`、`builtin_model_providers`、`builtin_model_overrides`、
> `builtin_providers`、`builtin_provider_auth_types`、`builtin_provider_models`、
> `builtin_model_aliases`

## 背景

应用需要一份「内置供应商与模型清单」作为种子数据，用于：

- 用户添加供应商时从 `builtin_providers` 选择预设（如 OpenAI / Anthropic / Gemini）
- 用户添加模型时从 `builtin_models` 选择推荐模型
- 模型 ID 自动映射（通过 `builtin_model_aliases`）

数据来源：

- 参考项目 `well-known/models.ts`（约 200+ 条模型定义）
- 参考项目 `well-known/providers.ts`（约 50+ 条供应商预设）
- 参考项目 `client/definitions.ts` 的 `ProviderType` 枚举

约束：

- 应用启动后数据库已包含最新内置数据
- 用户可修改 / 删除内置项的衍生数据（`providers`、`gateway_models`），
  但 `builtin_*` 表本身应保持可覆盖更新
- 离线场景必须可用，不依赖远程 API
- 跨版本升级时能合并新增的内置项

## 方案 A：构建脚本生成 SQL 迁移文件（v0.1 已采用）

**实现位置**：`scripts/sync-builtin-data.ts`（待实现）

### 设计要点

- Node.js 脚本从参考项目 TS 文件解析数据
- 生成 `V002__seed_builtin_data.sql` 迁移文件，包含 `INSERT` 语句
- 迁移机制：复用现有 `schema_migrations` 表的版本化执行器
- 升级方式：每次数据更新新增 `V00X__update_builtin_*.sql`，使用 `INSERT OR REPLACE`

### 优点

- **零运行时开销**：数据嵌入在编译时迁移文件中
- **离线可用**：无需网络请求
- **版本化**：与 schema 迁移统一管理
- **事务安全**：每次迁移在事务内执行，失败回滚

### 缺点

- **用户自定义丢失风险**：用户对 `builtin_*` 的修改在下次迁移时被覆盖
  - 缓解策略：明确 `builtin_*` 表对用户只读，修改应在 `providers` 层进行
- **二进制体积**：迁移文件 `include_str!` 嵌入二进制，大量数据增大体积
- **更新节奏受限**：必须随版本发布，无法热更新
- 脚本依赖参考项目的源码结构与路径

### 适用场景

- v0.1 快速验证
- 内置数据更新频率低（季度 / 版本周期）
- 离线优先

## 方案 B：启动期从 JSON 文件加载

**实现位置**：`src-tauri/src/modules/ai_gateway/seed.rs`（待实现）

### 设计要点

- 构建时将内置数据导出为 `builtin_data.json`，通过 `include_str!` 嵌入二进制
- 应用启动时检查 `builtin_*` 表的 `data_version` 标记
- 若版本低于嵌入 JSON 的版本，执行全量覆盖（DELETE + INSERT）
- 提供 `reload_builtin_data` Tauri Command 手动触发

### 优点

- 数据与代码分离，更新无需改 SQL
- 启动期同步，单次写入
- 可扩展为从远程拉取 JSON

### 缺点

- **启动延迟**：每次启动都要检查与可能写入，影响冷启动速度
- **覆盖策略粗暴**：DELETE + INSERT 会丢失用户对 `builtin_*` 的修改
- 需要额外的版本管理表（如 `builtin_data_meta`）
- JSON 反序列化在 Rust 侧需要严格类型定义

### 适用场景

- 内置数据频繁更新
- 接受启动期短暂延迟

## 方案 C：版本化嵌入资源 + Tauri Command 触发同步（推荐）

**实现位置**：`src-tauri/src/modules/ai_gateway/builtin_sync.rs`（待实现）

### 设计要点

1. **资源嵌入**：构建时通过 `include_str!("./builtin_data.v3.json")` 嵌入
2. **版本表**：新增 `builtin_data_versions` 表，记录每个内置资源类型（providers / models）的当前版本号
3. **同步策略**：
   - 启动期只读取版本号对比，不写入数据
   - 若发现新版本，通过 Tauri Event 通知前端
   - 前端弹窗询问用户「检测到内置数据更新，是否立即同步？」
   - 用户确认后调用 `invoke('builtin_data_sync')` 触发同步
4. **增量更新**：通过 `builtin_id` 主键 + `INSERT OR IGNORE` 仅追加新项
5. **字段级合并**：对已有项，仅更新非用户自定义字段（需要一个 `is_user_modified` 标记列）

### 优点

- **启动零延迟**：仅查询版本号，不写数据
- **用户感知**：更新前征求用户同意
- **保留用户修改**：通过 `is_user_modified` 标记避免覆盖
- **离线可用**：数据嵌入二进制
- **可演进**：后续可扩展为从远程拉取版本号 + JSON

### 缺点

- 实现复杂度最高
- `is_user_modified` 字段需要 UI 层配合维护
- 增量合并逻辑需要为每张表单独实现

### 适用场景

- 中长期推荐方案
- 内置数据更新频率中等（月度 / 季度）
- 用户可能深度自定义内置项

## 方案 D：远程 API 同步

**实现位置**：`src-tauri/src/modules/ai_gateway/remote_sync.rs`（待实现）

### 设计要点

- 应用连接到远程服务（如 i-code 官方 CDN）拉取最新内置数据
- 首次启动必须联网（离线模式禁用「从内置供应商添加」功能）
- 本地缓存 JSON，下次启动若网络不可用则使用缓存

### 优点

- 数据可热更新，无需发布新版本
- 体积小（不嵌入二进制）

### 缺点

- **离线不可用**：违背本地优先原则
- 需要维护远程服务
- 网络请求增加启动延迟与失败率
- 隐私问题：每次启动都向服务端暴露使用情况

### 适用场景

- SaaS 模式
- 数据频繁更新（每日 / 每周）

## 选型对比

| 维度 | 方案 A | 方案 B | 方案 C | 方案 D |
|------|--------|--------|--------|--------|
| 实现成本 | 低 ✓ | 中 | 高 ✗ | 高 |
| 启动性能 | 零开销 ✓ ✓ | 有延迟 ✗ | 零开销 ✓ ✓ | 网络延迟 ✗ |
| 离线可用 | 完整 ✓ ✓ | 完整 ✓ ✓ | 完整 ✓ ✓ | 不支持 ✗ |
| 用户自定义保护 | 无 ✗ | 无 ✗ | 完整 ✓ ✓ | 视实现 |
| 数据更新频率 | 版本绑定 | 版本绑定 | 版本绑定 | 实时 ✓ ✓ |
| 二进制体积 | 大 ✗ | 大 ✗ | 大 ✗ | 小 ✓ |
| 维护成本 | 低 ✓ | 中 | 高 ✗ | 高 |

## 决策

### v0.1 决策

**采用方案 A：构建脚本生成 SQL 迁移文件**

理由：

1. 实现成本最低，与现有迁移机制无缝集成
2. `builtin_*` 表对用户只读（用户修改应在 `providers` / `gateway_models` 层进行）
3. 数据更新可通过版本发布控制
4. 离线优先原则得到保证

### v0.2+ 演进建议

**评估方案 C**：当出现以下情况时迁移到方案 C

- 用户反馈内置项被覆盖问题（多次发生）
- 内置数据更新频率提升至月度
- 需要在不发布新版本的情况下推送紧急内置数据修复

迁移路径：

1. 新增 `builtin_data_versions` 表（V003 迁移）
2. 在 `builtin_*` 表新增 `is_user_modified INTEGER DEFAULT 0` 列（V004 迁移）
3. 实现 `builtin_sync` 模块
4. 前端添加「内置数据同步」设置入口

## 数据版本管理（方案 A 落地参考）

### 版本号约定

- 内置数据版本独立于 schema 版本
- 格式：`{major}.{minor}`，如 `1.0`、`1.1`、`2.0`
- major 变更表示破坏性更新（如字段重命名）
- minor 变更表示新增或修订

### 同步 SQL 示例

```sql
-- V002__seed_builtin_providers.sql
INSERT OR REPLACE INTO builtin_providers (id, display_name, category, provider_type, base_url, ...)
VALUES
  ('openai', 'OpenAI', 'official', 'openai-chat-completion', 'https://api.openai.com/v1', ...),
  ('anthropic', 'Anthropic', 'official', 'anthropic', 'https://api.anthropic.com/v1', ...),
  ...;

INSERT OR REPLACE INTO builtin_provider_auth_types (builtin_provider_id, auth_method, is_default, sort_order, created_at)
VALUES
  ('openai', 'api-key', 1, 0, datetime('now')),
  ('anthropic', 'api-key', 1, 0, datetime('now')),
  ...;
```

### 脚本输出结构

```
src-tauri/src/db/migrations/
├── V001__init.sql                         # schema + app_settings 默认数据
├── V002__seed_builtin_providers.sql       # 内置供应商预设
├── V003__seed_builtin_models.sql          # 内置模型清单
├── V004__seed_builtin_provider_models.sql # 供应商 × 模型关联
└── V005__seed_builtin_model_aliases.sql   # 模型别名映射
```

## 参考实现

- 参考项目的 `well-known/models.ts` 与 `well-known/providers.ts`
- VSCode 的 extensions 资源同步：使用版本化 JSON 与本地缓存
- Homebrew 的 formula 同步：版本化 manifest + 增量更新
