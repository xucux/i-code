# Grok Build（xAI 订阅）内置额度监控方案

> 状态：提案（待实现）  
> 关联：
> - `docs/proposals/balance-script-templates.md`（脚本模板体系，**本方案不修改**）
> - `docs/database.md` §4.3 / §5.10、`provider_balance_snapshots`
> - `src-tauri/src/modules/balance/`（`providers/` 硬编码 Provider 注册表）
> - `src/modules/balance/`、`src/modules/ai-gateway/ui/balance-config-form`（`BalanceConfigForm`）
> - 参考实现：`router-for-me/CLIProxyAPI`（xAI executor）+ `router-for-me/Cli-Proxy-API-Management-Center`（management 面板 xAI 额度页）

---

## 1. 背景与目标

### 1.1 现状

项目对 Grok（`xai-grok-build` 供应商）目前**没有官方额度监控**：

- 现有 `grok-usage-thirdparty` 脚本 snippet（`balance/script/snippets/grok_usage.rs`）面向**第三方中转站**（newapi 兼容格式，默认回退域名 `sub.yxxb.eu.cc`），返回 `quota.remaining/used/limit` + `usage.today/total`；
- xAI 官方开放文档（docs.x.ai）**没有公开的套餐额度 REST 端点**（REST API Reference 仅 Chat / Images / Videos / Voice / Models / Files / Batches / Other / Legacy）；
- Grok Build 订阅（SuperGrok / X Premium+ / 免费档）额度是**每周滚动周期**（`USAGE_PERIOD_TYPE_WEEKLY`），官方 CLI 通过内部 chat-proxy 获取。

### 1.2 目标

| 编号 | 目标 | 说明 |
|------|------|------|
| G1 | 内置官方额度查询 | 新增硬编码 Provider（`BalanceMethod::GrokBuild`），直连 Grok CLI chat-proxy `v1/billing` 端点 |
| G2 | 覆盖免费档与付费档 | OAuth 账号走 `cli-chat-proxy.grok.com`；付费 API 账号回退 `api.x.ai` 健康探测 |
| G3 | 完整指标映射 | 周用量百分比、按产品用量、月度额度、按量付费余额、计费周期 |
| G4 | 不动脚本模板 | 不改 `grok-usage` snippet、不改脚本模板 CRUD / 市场，两者并存 |

### 1.3 非目标（本期不做）

- 修改 `docs/proposals/balance-script-templates.md` 及脚本模板相关代码（snippet 源码、模板市场）
- 在 i-code 内嵌 OAuth 登录（沿用现有 `xai-grok-oauth` 认证，复用已解密 access token）
- 订阅管理 / 充值 / 变更套餐

---

## 2. 端点规格（逆向结论，2026-08 验证）

Grok CLI 内部 chat-proxy 暴露了额度端点，CLIProxyAPI 生态已验证可用：

### 2.1 周额度（免费档主接口）

```
GET https://cli-chat-proxy.grok.com/v1/billing?format=credits
```

响应示例（`body` 为 JSON 字符串）：

```json
{
  "config": {
    "currentPeriod": {
      "type": "USAGE_PERIOD_TYPE_WEEKLY",
      "start": "2026-07-31T01:47:42.522375+00:00",
      "end": "2026-08-07T01:47:42.522375+00:00"
    },
    "creditUsagePercent": 4.0,
    "onDemandCap": { "val": 0 },
    "onDemandUsed": { "val": 0 },
    "productUsage": [
      { "product": "GrokBuild", "usagePercent": 4.0 },
      { "product": "GrokChat" }
    ],
    "isUnifiedBillingUser": true,
    "prepaidBalance": { "val": 0 },
    "topUpMethod": "TOP_UP_METHOD_SAVED_PAYMENT_METHOD",
    "billingPeriodStart": "2026-07-31T01:47:42.522375+00:00",
    "billingPeriodEnd": "2026-08-07T01:47:42.522375+00:00"
  }
}
```

### 2.2 月额度

```
GET https://cli-chat-proxy.grok.com/v1/billing
```

响应 `config` 追加：`monthlyLimit: { val }`、`used: { val }`、`billingPeriodStart/End`（月度周期）。  
`{ val }` 字段单位为 **cent**（1 美元 = 100）。

### 2.3 请求头（OAuth / 免费档）

| Header | 值 |
|--------|-----|
| `Authorization` | `Bearer {oauth_access_token}` |
| `x-xai-token-auth` | `xai-grok-cli` |
| `x-grok-client-version` | `0.2.91`（随 Grok CLI 版本浮动，对齐 `auth_resolver.rs` 常量） |
| `User-Agent` | `grok-pager/0.2.91 grok-shell/0.2.91 (macos; aarch64)` |
| `x-userid` | 可选；auth 文件含 user_id 时附加 |

### 2.4 付费档回退（API key / `using_api` 账号）

```
GET  https://api.x.ai/v1/me
POST https://api.x.ai/v1/chat/completions   # { model: "grok-4.5", max_tokens: 1, stream: false }
```

纯 `Authorization: Bearer {api_key}`。`/me` 返回 profile（user_id / team_id 等），chat 健康探测通过即视为「账号有效、套餐可用」，无法给出精确剩余额度。

### 2.5 限流与错误

- 免费档额度用尽时 chat-proxy 返回 `429`，错误体含 `code: "subscription:free-usage-exhausted"`，文案 `"Usage resets over a rolling 24-hour window"`（部分模型）或周窗口耗尽；
- `billing` 查询本身失败时按 HTTP 状态码透传，Provider 转 `IcodeError`。

---

## 3. 方案设计（内置硬编码 Provider）

沿用现有 `providers/*.rs` 模式，**新增** `providers/grok_build.rs`，注册到 `providers/mod.rs`：

```
BalanceMethod::GrokBuild → grok_build::GrokBuildBalanceProvider
```

### 3.1 类型改动（Rust 侧）

`balance/types.rs`：

```rust
pub enum BalanceMethod {
    // ...
    /// Grok Build（xAI 订阅，官方 chat-proxy billing 端点）
    GrokBuild,
}
```

- `from_str` / `as_str` 增加 `"grok-build"`；
- `BalanceConfig::GrokBuild` 无额外配置（沿用 provider 表 base_url / api_key / auth_method）。

前端 `src/modules/ai-gateway/types.ts` 同步增加 `'grok-build'`（手工维护，前后端同步）。

### 3.2 请求构建

```rust
// 1) 解析 base_url（provider.base_url）：
//    - 为空 / 官方默认 → https://cli-chat-proxy.grok.com/v1
//    - 否则原样使用（第三方兼容）
// 2) 端点：GET {base}/billing?format=credits（周）+ GET {base}/billing（月）
// 3) 头：Authorization: Bearer {api_key（已解密 OAuth token / API key）}
//        x-xai-token-auth: xai-grok-cli
//        x-grok-client-version: 0.2.91
//        User-Agent: grok-pager/0.2.91 grok-shell/0.2.91 (macos; aarch64)
// 4) auth_method == xai-grok-oauth 时附加 X-XAI-Token-Auth 等身份头；
//    非 OAuth（API key）直接走 api.x.ai 路径（见 3.4）
```

### 3.3 响应 → `BalanceSnapshot` 映射

| 上游字段 | 指标 | id / type / direction / period |
|----------|------|-------------------------------|
| `config.creditUsagePercent` | 周信用额度用量 % | `id: credit_percent`，`type: percent`，`period: week` |
| `config.currentPeriod` | 当前周窗口（start/end） | `id: period_week`，`type: time`，`period: week`，`value: end`（重置时刻） |
| `config.productUsage[]` | 按产品用量 %（GrokBuild / GrokChat） | 每条 `id: product_{product}`，`type: percent`，`scope: product` |
| `config.monthlyLimit` / `used` | 月度额度（cent→美元） | `id: monthly_limit` / `id: monthly_used`，`type: amount`，`direction: limit/used`，`period: month` |
| `config.onDemandCap` / `onDemandUsed` | 按量付费余额（cent→美元） | `id: on_demand_remaining`，`type: amount`，`direction: remaining`，`period: current` |
| `config.prepaidBalance` | 预付余额（cent→美元） | `id: prepaid_balance`，`type: amount`，`direction: remaining` |
| `config.isUnifiedBillingUser` | 统一计费标识 | `id: unified_billing`，`type: status` |
| 429 `free-usage-exhausted` | 套餐耗尽 | `id: status`，`type: status`，`value: exhausted` |

主指标（`primary: true`）：`credit_percent`（周用量，免费档核心）。

金额换算：`{val}` 单位为 cent，`value` 输出美元字符串（`val / 100.0`，保留 2 位），货币符号 `$`。**金额统一字符串传输**，避免浮点精度问题（对齐 AGENTS.md §6.2）。

### 3.4 付费档分支

`auth_method == xai-grok-oauth` → 走 chat-proxy `v1/billing`（§2.1/§2.2）；  
其余（API key）→ 走 `api.x.ai` 健康探测（§2.4）：

- `GET {api.x.ai}/v1/me`：成功 → 快照主指标 `status: ok` + `user_id`；
- `POST /v1/chat/completions`（max_tokens: 1）：成功 → `status: ok`；429 → `exhausted`；
- 二者不返回剩余额度，仅提供可用性状态。

### 3.5 与脚本模板的关系（明确边界）

| 能力 | 内置 Provider（本方案） | 脚本 snippet `grok-usage-thirdparty`（不动） |
|------|------------------------|----------------------------------|
| 目标 | Grok Build 官方订阅额度 | 第三方中转站（newapi 兼容） |
| 端点 | `cli-chat-proxy.grok.com/v1/billing` | `{base}/v1/usage`（`x-api-key`） |
| 认证 | OAuth token + chat-proxy 身份头 | API Key |
| 指标 | 周/月百分比、按产品、按量付费 | quota 金额 + 今日/累计 token |

两者在 `BalanceMethod` 下拉中并列：`Grok Build`（内置）与「自定义脚本」分组下的 `grok-usage-thirdparty` 模板。

---

## 4. 实现清单

- [ ] `src-tauri/src/modules/balance/types.rs`：`BalanceMethod::GrokBuild`（enum + from_str/as_str）
- [ ] `src-tauri/src/modules/balance/providers/grok_build.rs`：`GrokBuildBalanceProvider`（周/月 billing + 付费回退）
- [ ] `src-tauri/src/modules/balance/providers/mod.rs`：注册表登记
- [ ] `src/modules/ai-gateway/types.ts`：`BalanceMethod` 同步 `'grok-build'`
- [ ] `src/modules/i18n/locales/zh-CN.json` / `en.json`（及 zh-TW）：`BalanceConfigForm` 下拉文案「Grok Build」
- [ ] `cargo check` + `pnpm type-check` 通过
- [ ] （可选）`docs/development.md` / `docs/database.md` 回写：`BalanceMethod` 枚举与 `provider_balance_snapshots` 说明

## 5. 验收标准

1. `xai-grok-build` + `xai-grok-oauth` 供应商配置额度方法为 `grok-build` 后，刷新额度可展示周用量百分比（`creditUsagePercent`）与本周重置时间；
2. 月度与按量付费账号（统一计费用户）额外展示月度额度 / 按量余额；
3. 免费额度耗尽时状态显示 `exhausted`，不暴露内部错误码；
4. 第三方中转场景（脚本 `grok-usage`）行为不变；
5. 全程不落明文 token 到日志 / DB（沿用 Secret 边界约定）。

## 6. 参考实现

| 来源 | 位置 | 说明 |
|------|------|------|
| CLIProxyAPI | `internal/runtime/executor/xai_executor_request.go` | `applyXAIChatHeaders`、`xaiChatBaseURL`、版本常量 |
| Cli-Proxy-API-Management-Center | `src/utils/quota/constants.ts` | `XAI_BILLING_WEEKLY_URL` / `XAI_BILLING_MONTHLY_URL` / `XAI_REQUEST_HEADERS` |
| Cli-Proxy-API-Management-Center | `src/features/quota/providers/xai/data.ts` | `fetchXaiQuota`：免费档双请求 + 付费档健康探测回退 |
| Cli-Proxy-API-Management-Center | `src/utils/quota/builders.ts` | `buildXaiBillingSummary`：字段归一化、plan 识别（15000=SuperGrok、150000=SuperGrok Heavy） |
