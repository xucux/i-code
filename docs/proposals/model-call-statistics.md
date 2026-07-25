# 对外模型统计指标设计提案

## 1. 背景与目标

当前「对外模型」页面仅展示网关暴露模型列表，无法直观反映各模型的真实使用情况。业务侧希望在该页面以**只读表格**形式呈现模型运行指标，帮助用户：

- 快速识别高频/高成本模型
- 评估供应商稳定性（成功率、耗时）
- 观察缓存命中效果与费用分布
- 区分请求入口（CLI、网关、内部路由）

## 2. 指标定义与计算方式

表格字段及口径如下：

| 字段 | 定义 | 计算方式 | 数据来源 |
|---|---|---|---|
| 供应商 | 真实供应商显示名称 | `providers.display_name` | `providers` |
| 模型 ID | 网关暴露的真实模型 ID | `gateway_models.model_id` | `gateway_models` |
| 入口 | 请求来源类型 | 枚举：CLI / 网关 / 内部路由 | `model_call_logs.source`（新增） |
| 请求数 | 该模型在统计周期内的总调用次数 | `COUNT(*)` | `model_call_logs` |
| 成功率 | 成功请求占比 | `SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END) / COUNT(*)` | `model_call_logs.status_code` |
| 总 Token | 消耗 Token 总量 | `SUM(total_tokens)` | `model_call_logs.total_tokens` |
| 缓存 / 命中率 | 命中缓存的 Token 数与命中率 | `SUM(cached_tokens)` + `SUM(CASE WHEN cache_hit THEN 1 ELSE 0 END) / COUNT(*)` | `model_call_logs.cached_tokens`、`cache_hit` |
| 花费金额 | 估算费用（USD） | `SUM(total_tokens / 1_000_000 * unit_price)`，单价按 `provider_type + model_id` 匹配 | `model_call_logs.total_tokens` + 新增 `model_pricing` 表 |
| 费用占比 | 该模型花费占全部模型花费比例 | `该模型花费 / SUM(全部模型花费)` | 同上 |
| $/1M Token | 每百万 Token 平均成本 | `花费金额 / (总 Token / 1_000_000)` | 同上 |
| 平均耗时 | 请求平均耗时（ms） | `AVG(duration_ms)` | `model_call_logs.duration_ms` |
| 平均首字 | 流式响应首字延迟（ms） | `AVG(time_to_first_token_ms)` | `model_call_logs.time_to_first_token_ms`（新增） |
| 平均速率 | 流式输出平均 token/s | `AVG(completion_tokens / (duration_ms - time_to_first_token_ms) * 1000)` | `model_call_logs` |

## 3. 数据存储方案

### 3.1 现有表 `model_call_logs` 字段回顾

已具备：
- `provider_id`、`gateway_model_id`、`model_id`
- `requested_at`、`completed_at`、`duration_ms`、`status_code`
- `prompt_tokens`、`completion_tokens`、`total_tokens`、`cached_tokens`、`cache_hit`
- `route_mode`（Direct / VirtualFallback）

### 3.2 需要新增字段

```sql
-- 请求来源：cli / gateway / internal
ALTER TABLE model_call_logs ADD COLUMN source TEXT NOT NULL DEFAULT 'gateway';

-- 首字延迟（毫秒），流式响应时记录
ALTER TABLE model_call_logs ADD COLUMN time_to_first_token_ms INTEGER;

-- 实际单价快照（USD / 1M tokens），记录时写入避免后续改价影响历史统计
ALTER TABLE model_call_logs ADD COLUMN price_per_1m_tokens REAL;
```

### 3.3 新增 `model_pricing` 表（可选，用于自动填充单价）

```sql
CREATE TABLE IF NOT EXISTS model_pricing (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    model_id TEXT NOT NULL,
    input_price_per_1m REAL NOT NULL DEFAULT 0,
    output_price_per_1m REAL NOT NULL DEFAULT 0,
    cached_price_per_1m REAL NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    effective_from TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(provider_id, model_id, effective_from)
);
```

> 说明：若项目初期不需要精确计费，可先使用 `model_call_logs.price_per_1m_tokens` 字段，由调用处传入一个默认单价；`model_pricing` 表作为后续演进扩展。

### 3.4 入口（source）判定规则

| source 值 | 判定条件 |
|---|---|
| `cli` | 请求来自 CLI 配置文件（通过内部路由转发，Header 含 `X-Icode-Source: cli` 或 URL 路径含 `/cli/`） |
| `gateway` | 请求来自本地 HTTP Gateway（外部客户端或内部 CLI 走网关地址） |
| `internal` | 应用内部直接调用（如 Skills、测试请求、内置功能） |

当前最简实现：在 `gateway_runtime` 转发请求时根据调用上下文写入 `source`；CLI 侧请求统一写 `cli`；Gateway 外部请求写 `gateway`；应用内部直接调用写 `internal`。

## 4. 方案对比

| 方案 | 实时性 | 复杂度 | 存储成本 | 适用阶段 |
|---|---|---|---|---|
| A. 纯 SQL 聚合（直接查 `model_call_logs`） | 中（依赖数据量） | 低 | 低 | v0.1 推荐 |
| B. 预聚合表（按小时/天汇总） | 高 | 中 | 中 | 数据量大后采用 |
| C. 内存聚合 + 异步落库 | 高 | 高 | 低 | 高并发场景 |

### 对比说明

- **方案 A**：在 `model_call_logs` 上按 `provider_id + model_id + source` 分组聚合，实现最快，无需新增汇总表。配合索引可满足万级记录秒级查询。
- **方案 B**：新增 `model_call_stats` 汇总表，由定时任务或触发器更新。适合百万级记录，但实现成本高。
- **方案 C**：在 Gateway Runtime 内存中维护计数器，异步批量写入。适合极高并发，但数据易丢失，实现复杂。

## 5. 决策依据

选择 **方案 A（纯 SQL 聚合）** 作为 v0.1 实现，理由：

1. 当前 `model_call_logs` 数据量可控，索引已覆盖 `provider_id`、`model_id`、`requested_at`
2. 统计逻辑透明，便于后续切换为方案 B
3. 开发周期最短，可快速验证指标口径
4. 新增字段（`source`、`time_to_first_token_ms`、`price_per_1m_tokens`）均为向后兼容的扩展

## 6. 后端 API 设计

### 6.1 命令

```rust
/// 查询模型调用统计
#[tauri::command]
pub async fn gateway_model_call_stats(
    input: ModelCallStatsInput,
) -> Result<Vec<ModelCallStatsRow>, String> { ... }
```

### 6.2 输入参数

```rust
pub struct ModelCallStatsInput {
    /// 统计起始时间（ISO 8601），可选，默认 24 小时前
    pub start_at: Option<String>,
    /// 统计结束时间，可选，默认当前
    pub end_at: Option<String>,
    /// 按入口过滤，可选
    pub source: Option<String>,
    /// 按供应商过滤，可选
    pub provider_id: Option<String>,
}
```

### 6.3 输出结构

```rust
pub struct ModelCallStatsRow {
    pub provider_id: String,
    pub provider_name: String,
    pub model_id: String,
    pub source: String,
    pub request_count: i64,
    pub success_count: i64,
    pub success_rate: f64,
    pub total_tokens: i64,
    pub cached_tokens: i64,
    pub cache_hit_rate: f64,
    /// 总花费金额（CNY，元）
    pub cost_cny: f64,
    pub cost_ratio: f64,
    /// 每百万 Token 成本（CNY / 1M tokens）
    pub cost_per_1m_tokens: f64,
    pub avg_duration_ms: f64,
    pub avg_time_to_first_token_ms: f64,
    pub avg_tokens_per_second: f64,
}
```

### 6.4 SQL 示例

```sql
SELECT
    m.provider_id,
    p.display_name AS provider_name,
    m.model_id,
    m.source,
    COUNT(*) AS request_count,
    SUM(CASE WHEN m.status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END) AS success_count,
    ROUND(
        CAST(SUM(CASE WHEN m.status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END) AS REAL) * 100 / COUNT(*),
        2
    ) AS success_rate,
    SUM(m.total_tokens) AS total_tokens,
    SUM(m.cached_tokens) AS cached_tokens,
    ROUND(
        CAST(SUM(CASE WHEN m.cache_hit THEN 1 ELSE 0 END) AS REAL) * 100 / COUNT(*),
        2
    ) AS cache_hit_rate,
    ROUND(SUM(m.total_tokens * COALESCE(m.price_per_1m_tokens, 0) / 1000000.0), 6) AS cost_cny
FROM model_call_logs m
JOIN providers p ON p.id = m.provider_id
WHERE m.requested_at >= ?1 AND m.requested_at <= ?2
GROUP BY m.provider_id, m.model_id, m.source
ORDER BY request_count DESC;
```

费用占比、¥/1M Token、平均耗时/首字/速率可在后端或前端二次计算。推荐后端计算后返回，前端只负责展示。

## 7. 前端展示方案

### 7.1 页面结构

- 顶部：刷新按钮 + 自动刷新间隔选择器（5s / 10s / 30s / 1m / 5m / 关闭）
- 主体：`Table` 组件展示统计行
- 按「请求数」或「花费金额」默认降序

### 7.2 表格列

| 列 | 展示 |
|---|---|
| 供应商 | 文本 |
| 模型 ID | 等宽字体小字 |
| 入口 | Badge：`CLI` / `网关` / `内部` |
| 请求数 | 数字，右对齐 |
| 成功率 | 百分比 + 进度条 |
| 总 Token | 千分位数字 |
| 缓存 / 命中率 | `cachedTokens` + `cacheHitRate%` |
| 花费金额 | `$0.0000` |
| 费用占比 | 百分比 |
| $/1M Token | `$0.00` |
| 平均耗时 | `ms` |
| 平均首字 | `ms` |
| 平均速率 | `token/s` |

### 7.3 数据来源

前端通过 `useCommand` 调用 `gateway_model_call_stats`，按选定时间范围与入口过滤，配合 `setInterval` 实现自动刷新。

## 8. 演进路径

1. **v0.1**：新增 `source`、`time_to_first_token_ms`、`price_per_1m_tokens` 字段；后端实现 `gateway_model_call_stats` 聚合命令；前端改为统计表格。
2. **v0.2**：引入 `model_pricing` 表，支持按 input/output/cached token 分别计价；统计中拆分 prompt/completion/cached 成本。
3. **v0.3**：当 `model_call_logs` 数据量增大后，迁移至方案 B（预聚合表），保留小时级/天级汇总，提升查询性能。

## 9. 待确认事项

- 是否需要精确到「模型配置」维度（`model_config_id`）统计？当前按 `gateway_models.model_id` 维度更符合业务语义。
- 单价默认值：若未配置 `model_pricing`，统一按 `0` 计算还是使用内置常见模型价格？
- 时间范围默认是「最近 24 小时」还是「全部历史」？建议默认 24 小时，并支持自定义。
