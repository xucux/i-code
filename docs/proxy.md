# 网络代理（Proxy）开发指南

> 本文档说明 i-code 中 HTTP 代理的两层配置体系、核心函数、网络路径与日志约定。
> 修改任何网络/代理相关逻辑前，请先通读本文。
>
> 相关代码：
> - 类型与应用层：[`src-tauri/src/modules/shared/mod.rs`](../src-tauri/src/modules/shared/mod.rs)
> - 网关转发：[`src-tauri/src/modules/gateway_runtime/client/mod.rs`](../src-tauri/src/modules/gateway_runtime/client/mod.rs)
> - 模型拉取 / OAuth：[`src-tauri/src/modules/ai_gateway/service.rs`](../src-tauri/src/modules/ai_gateway/service.rs)、[`src-tauri/src/modules/ai_gateway/auth/oauth2.rs`](../src-tauri/src/modules/ai_gateway/auth/oauth2.rs)
> - 前端表单：[`src/modules/ai-gateway/ui/provider-form.tsx`](../src/modules/ai-gateway/ui/provider-form.tsx)

---

## 1. 设计原则

### 1.1 全局代理 = 应用级网络策略总开关

`app_settings.global_proxy_enabled` 是应用级网络总开关。**未启用时强制直连**（`reqwest::ClientBuilder::no_proxy()`），不再回落到 reqwest 默认行为（读取系统 `HTTP_PROXY` / `HTTPS_PROXY` 环境变量）。

> 这条语义是 2026-07-27 代理重构的核心修正。此前未启用时返回原始 builder，reqwest 会读取系统环境变量代理，导致「系统设了代理环境变量但代理不可用」时，直连可达的供应商也拉取/转发失败。

### 1.2 供应商代理策略

供应商 `proxy_json` 优先于全局代理。策略为 `global` 时**回退到全局代理**（全局未启用则直连）；其余策略独立生效。

### 1.3 两条网络路径必须策略一致

- **拉取模型 / OAuth 授权**（`ai_gateway` 模块）
- **网关转发**（`gateway_runtime` 模块）

两者必须复用同一份代理逻辑（`shared::apply_provider_proxy`），禁止各自实现，避免策略漂移。

---

## 2. 配置类型

### 2.1 全局代理 `ProxyConfig`

存储于 `app_settings.global_proxy_json`，受 `global_proxy_enabled` 开关控制。

```rust
// src-tauri/src/modules/shared/mod.rs
pub struct ProxyConfig {
    #[serde(rename = "type")]
    pub proxy_type: ProxyType,      // direct | system | http | socks
    pub url: Option<String>,        // http / socks 生效，可含 user:pass
    pub no_proxy: Vec<String>,      // NO_PROXY 等价物
}
```

| `type` | 行为 |
|--------|------|
| `direct` | 显式 `no_proxy()`，不走任何代理 |
| `system` | 沿用 reqwest 默认（读取 `HTTP_PROXY` / `HTTPS_PROXY` 环境变量） |
| `http` | `reqwest::Proxy::all(url)`，HTTP 代理 |
| `socks` | `reqwest::Proxy::all(url)`，SOCKS5 代理（reqwest 按 scheme 自动选择） |

> **注意**：`direct` 与「全局代理未启用」都走 `no_proxy()`，但语义不同——前者是用户显式选择直连，后者是总开关关闭。两者最终网络行为一致。

### 2.2 供应商代理 `ProviderProxyConfig`

存储于 `providers.proxy_json`。

```rust
pub struct ProviderProxyConfig {
    #[serde(rename = "type")]
    pub proxy_type: ProviderProxyType,  // global | direct | socks | http
    pub url: Option<String>,            // socks / http 生效
}
```

| `type` | 行为 |
|--------|------|
| `global` | 应用全局代理（全局未启用则直连） |
| `direct` | 显式 `no_proxy()` |
| `socks` | SOCKS5 代理 |
| `http` | HTTP 代理 |

---

## 3. 核心函数（`shared` 层）

所有代理逻辑集中在 `src-tauri/src/modules/shared/mod.rs`，供各模块复用。

### 3.1 `apply_global_proxy` / `apply_global_proxy_blocking`

```rust
pub fn apply_global_proxy(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder
pub fn apply_global_proxy_blocking(builder: reqwest::blocking::ClientBuilder) -> reqwest::blocking::ClientBuilder
```

- 读取 `app_settings`，全局代理未启用 → `builder.no_proxy()`（**强制直连**）
- 已启用 → 按 `ProxyConfig` 应用
- 读 DB 或解析失败 → `no_proxy()`（fail-safe 直连）

**使用场景**：应用自身发起的、与供应商代理无关的网络请求，如版本更新检查（`update_version.rs`）、Rhai 脚本 HTTP（`balance/script/host_http.rs`）。

### 3.2 `apply_provider_proxy`

```rust
pub fn apply_provider_proxy(
    builder: reqwest::ClientBuilder,
    provider_proxy_json: Option<&str>,
) -> Result<reqwest::ClientBuilder, String>
```

- `None` 或 `global` → 调用 `apply_global_proxy`（含全局未启用回退直连）
- `direct` → `no_proxy()`
- `socks` / `http` → `reqwest::Proxy::all(url)`，缺 URL 或构造失败返回 `Err`

**使用场景**：所有面向供应商的网络请求——模型拉取、OAuth、网关转发。**新增网络路径必须复用此函数，禁止 `reqwest::Client::new()`。**

---

## 4. 网络路径清单

| 路径 | 模块 | 函数 | 代理应用方式 |
|------|------|------|-------------|
| 网关转发（流式/非流式） | `gateway_runtime` | `client/mod.rs::http_client_for` → `apply_proxy` | `apply_provider_proxy` |
| 模型列表拉取 | `ai_gateway` | `service.rs::fetch_official_models` / `fetch_models_by_protocol` → `build_provider_http_client` | `apply_provider_proxy` |
| OAuth 授权 / 换 token | `ai_gateway` | `auth/oauth2.rs::OAuth2Client::new_for_provider` | `apply_provider_proxy` |
| 版本更新检查 | `update_version` | `check_update` / `download_and_install` | `apply_global_proxy` |
| Rhai 脚本 HTTP | `balance` | `script/host_http.rs` | `apply_global_proxy_blocking` |

### 4.1 `build_provider_http_client`（模型拉取专用）

`ai_gateway/service.rs` 末尾的辅助函数，封装「供应商代理 + 超时」：

```rust
fn build_provider_http_client(provider: &Provider) -> IcodeResult<reqwest::Client> {
    // 默认 UA / 连接超时 10s / 响应总超时 30s
    // 供应商 timeout_json 覆盖连接超时
    // apply_provider_proxy 应用供应商代理
}
```

> **历史缺陷**：此前 `fetch_official_models` / `fetch_models_by_protocol` 使用裸 `reqwest::Client::new()`，既忽略供应商代理，又会读系统环境变量代理，导致直连可达的供应商也失败。已统一改为 `build_provider_http_client`。

---

## 5. 前端表单约定

### 5.1 `proxyJson` 必须始终序列化

`src/modules/ai-gateway/ui/provider-form.tsx` 的 `handleSubmit`：

```ts
// ❌ 错误：global 时返回 undefined
const proxyJson = proxyMode === 'global' ? undefined : JSON.stringify({...})

// ✅ 正确：始终序列化
const proxyJson = JSON.stringify({
  type: proxyMode,
  url: proxyMode === 'socks' || proxyMode === 'http' ? proxyUrl : undefined,
})
```

**原因**：Tauri invoke 经 `JSON.stringify` 会**省略** `undefined` 字段，后端 `Option<T>` 反序列化为 `None`，而 `repository.rs::update_provider` 用 `if let Some(ref v) = input.proxy_json` 判断更新——`None` = 跳过更新，DB 保留旧值，表现为「一旦设了 socks/http 就无法切回 global」。

### 5.2 通用规则：update 类字段想「置空」要么始终传值，要么用 `Option<Option<T>>`

- **方案 A（推荐用于 JSON 字段）**：始终序列化字符串值（如本例 `proxyJson`）。
- **方案 B（用于需要区分「不动」与「置空」）**：双层 `Option<Option<T>>`，参考 `auth_json` 的 `Some(None)` 置空模式。

---

## 6. 日志约定（tauri-plugin-log）

代理相关日志使用 **tauri-plugin-log**（`log::trace!` / `log::error!`），不写入自研内存 logger。原因：代理是网络栈底层，需在终端/日志文件/DevTools 全量可见，且不应污染业务「日志」页面。

> 框架选择依据见 [`docs/log-framework.md`](./log-framework.md) §1。两套日志级别互不影响。

### 6.1 级别约定

| 级别 | 触发时机 | 内容 |
|------|---------|------|
| `trace` | 代理决策每次命中 | 决策来源（全局/供应商）、最终策略、URL（脱敏认证）、是否 no_proxy |
| `error` | 代理构造失败 / 解析失败 / 网络请求失败 | 完整错误链（含 `{:?}` 整个错误对象）、供应商 slug、目标 URL |

> `trace` 默认不输出，需在「设置 → 日志级别」调到 `Trace` 才可见，适合排障时临时开启。生产环境保持 `Info`。

### 6.2 `trace` 日志格式

决策点统一打点，便于还原「为什么走了这条网络路径」：

```
[proxy] global | enabled=false → forced direct (no_proxy)
[proxy] provider {slug} | strategy=global → delegate to global
[proxy] provider {slug} | strategy=socks | url=socks5://127.0.0.1:1080
[proxy] provider {slug} | strategy=direct → no_proxy
```

### 6.3 `error` 日志格式

包含完整网络栈与上下文，便于定位：

```
[proxy] provider {slug} | apply failed | strategy=socks | err={:?}
[proxy] fetch_models | {slug} | GET {url} | err={:?}
```

### 6.4 脱敏：`redact_proxy_url`

代理 URL 常含明文凭据（`http://user:pass@host:port`），写入日志前必须脱敏。`shared/mod.rs` 内部 `redact_proxy_url` 将 userinfo 替换为 `<redacted>`：

```
http://user:pass@127.0.0.1:7890  →  http://<redacted>@127.0.0.1:7890
```

> 所有 `trace` / `error` 日志中出现的代理 URL 必须经 `redact_proxy_url` 处理，禁止直接打印原始 URL。Secret 明文同样禁止入日志（AGENTS §9）。

---

## 7. 修复历史（2026-07-27）

| 缺陷 | 现象 | 根因 | 修复 |
|------|------|------|------|
| 拉取模型失败 | 系统设了代理环境变量但代理不可用时，直连可达的供应商也拉取失败 | `fetch_official_models` / `fetch_models_by_protocol` 用裸 `reqwest::Client::new()`，忽略供应商代理且读环境变量 | 改用 `build_provider_http_client`（含 `apply_provider_proxy`） |
| 聊天 internal_error | 供应商设 global、系统全局代理未开启时网关转发失败 | `apply_global_proxy` 未启用时返回原始 builder，reqwest 读环境变量代理 | 未启用时强制 `no_proxy()` |
| 无法切回全局代理 | 设了 socks/http 后无法在表单切回 global | 前端 global 时 `proxyJson = undefined` 被 invoke 省略，后端跳过更新 | 前端始终序列化 `proxyJson` |
| OAuth 代理不一致 | OAuth 授权走的代理与网关转发不同 | `oauth2.rs::new_for_provider` 的 `Global` 分支什么都不做 | 改用 `apply_provider_proxy` |

---

## 8. 排障示例

### 8.1 拉取模型失败

1. 「设置 → 日志级别」调到 `Trace`
2. 复现拉取操作，查看终端/日志文件
3. 关键日志链：
   ```
   [proxy] fetch_models | provider={slug} | proxy_json=... | timeout_json=...
   [proxy] provider | strategy=global → delegate to global
   [proxy] global | enabled=false → forced direct (no_proxy)
   Provider API other | GET {url} | provider={slug} | send failed | err={:?}
   ```
4. `err={:?}` 会展开完整 reqwest 错误链（含 hyper / connect / tls 来源），据此判断是 DNS、连接超时、TLS 还是代理问题。

### 8.2 网关转发失败（聊天 internal_error）

网关转发路径在 `gateway_runtime/client/mod.rs::http_client_for` → `apply_proxy` → `apply_provider_proxy`，同样会输出上述 `[proxy]` trace 链。结合 `gateway_runtime` 自身的 `Provider API` 日志（见 `docs/log-framework.md` §2.5.2）可定位是代理问题还是上游问题。
