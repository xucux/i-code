# 日志框架说明

> 项目级日志框架文档，说明 i-code 中两套日志机制的职责、使用方式与隔离边界。
> 模块级实现细节见 [`logger-module.md`](./logger-module.md)。

---

## 1. 概述

i-code 同时维护两套日志机制，分别面向**开发调试/运行时追踪**和**业务运维诊断**。两套机制在代码路径、输出目标、级别控制上完全隔离，调整其中一方不会影响另一方。

| 维度 | tauri-plugin-log 日志 | 自研内存 logger |
|------|----------------------|----------------|
| **定位** | 开发调试、运行时通用追踪 | 业务运行时诊断、运维可见 |
| **后端入口** | `log::info!` / `log::warn!` / `log::error!` / `log::debug!` | `crate::modules::logger::Log::info` 等 |
| **前端入口** | 不涉及（Rust 后端宏） | `src/modules/logger/logger.ts` 的 `logger.info` 等 |
| **输出目标** | 终端、WebView 控制台、应用日志目录文件 | 内存环形缓冲区、应用内「日志」页面、可选按天滚动文件 |
| **存储形态** | 文本行，按日期/大小滚动 | 结构化 `LogEntry`，内存为主、可导出 JSON/CSV |
| **级别控制** | `app_settings.log_level`（全局设置） | `log_settings` 表（转发/Command/网关/文件阈值） |
| **可见性** | 开发者查看终端 / DevTools / 日志文件 | 用户在应用内「日志」页面查看 |

---

## 2. tauri-plugin-log 日志

### 2.1 引入原因

原项目使用 `log` crate 作为日志门面，并声明了 `env_logger` 依赖，但**未初始化**，导致所有 `log::` 宏调用没有实际输出目标。为解决这个问题，引入官方插件 `tauri-plugin-log`，统一接管 `log` crate 的 logger 实现。

### 2.2 初始化配置

在 [`src-tauri/src/main.rs`](../src-tauri/src/main.rs) 中初始化：

```rust
.plugin(
    tauri_plugin_log::Builder::new()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir { file_name: None }),
            Target::new(TargetKind::Webview),
        ])
        .level(log::LevelFilter::Trace)
        .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
        .build(),
)
```

- `Stdout`：输出到后端控制台。
- `LogDir`：输出到应用日志目录文件。
- `Webview`：输出到前端 WebView 控制台（DevTools）。
- `level(Trace)`：允许运行时通过 `log::set_max_level` 下调到任意级别。

### 2.3 使用方式

后端任意位置：

```rust
log::info!("模块启动完成");
log::warn!("请求重试：{}", retry_count);
log::error!("上游调用失败：{}", e);
```

### 2.4 级别控制

全局日志级别保存在 `app_settings.log_level` 列，启动和更新时调用 `log::set_max_level` 实时生效。默认 `Info`，可在「设置」页面调整。

### 2.5 网关与供应商 API 日志约定

`gateway_runtime` 与 `ai_gateway` 模块在维持自研内存 logger 的同时，通过 `tauri-plugin-log` 输出完整、未截断的调试信息，方便开发/运维在终端、日志文件或 WebView 控制台中追踪请求链路。

#### 2.5.1 网关被调用（`router.rs`）

本地 axum 网关收到请求后统一输出：

- **`info`**：`Gateway | {method} {path} | status={status} | duration={ms}ms | tokens={prompt}/{completion}/{total} | request_id={id}`
- **`debug`**：请求体、响应体（完整 JSON，不截断）
- **`warn`**：当响应状态码非 2xx 时，额外输出一次完整请求体与响应体

实现位置：

- [`src-tauri/src/modules/gateway_runtime/router.rs`](../src-tauri/src/modules/gateway_runtime/router.rs) 的 `emit_gateway_tauri_logs`。

#### 2.5.2 供应商 API 调用（`upstream.rs`）

请求转发到真实供应商后统一输出：

- **`info`**：`Provider API | {method} {url} | status={status} | duration={ms}ms | tokens={prompt}/{completion}/{total} | request_id={id}`
- **`debug`**：请求体、响应体（完整 JSON，不截断）
- **`warn`**：状态码 ≥ 400 或网络/解析失败时，额外输出一次请求体、响应体与错误信息

实现位置：

- [`src-tauri/src/modules/gateway_runtime/upstream.rs`](../src-tauri/src/modules/gateway_runtime/upstream.rs) 的 `emit_provider_api_tauri_logs`。

#### 2.5.3 SSE / WebSocket 流式输出

- **SSE**：每次从上游接收到 chunk 立即 `debug` 打印，内容为原始事件文本与字节数。
  - 实现位置：[`src-tauri/src/modules/gateway_runtime/client/mod.rs`](../src-tauri/src/modules/gateway_runtime/client/mod.rs) 的 `build_sse_response`。
- **WebSocket**：请求发出时 `debug` 打印请求体；每次接收到流式帧时 `debug` 打印（当前未完整实现流式转发，已用 `warn` 标记）。
  - 实现位置：[`src-tauri/src/modules/gateway_runtime/client/websocket_client.rs`](../src-tauri/src/modules/gateway_runtime/client/websocket_client.rs)。

#### 2.5.4 其他 API 请求（模型列表拉取等）

`ai_gateway/service.rs` 中所有官方模型拉取函数统一输出：

- **`info`**：`Provider API other | GET {url} | status={status} | duration={ms}ms`
- **`debug`**：响应体（完整 JSON）
- **`warn`**：网络失败、非 2xx、JSON 解析失败时输出 URL、状态码、耗时与响应/错误信息

覆盖范围：OpenAI 兼容、Anthropic 原生、Ollama、Gemini、Vertex AI、GitHub Copilot。

实现位置：

- [`src-tauri/src/modules/ai_gateway/service.rs`](../src-tauri/src/modules/ai_gateway/service.rs) 的各 `fetch_*_models` 函数。

#### 2.5.5 安全与性能约定

1. **请求/响应体不截断**：`tauri-plugin-log` 的日志行输出完整 body，便于复现问题；应用内「日志」页面仍受 `log_settings.max_body_length` 控制。
2. **URL 脱敏**：打印 URL 前通过 `redact_url_key_param` 将查询参数中的 `key`（如 Gemini `?key=...`）替换为 `<redacted>`，避免 API Key 泄漏。
3. **Secret 明文禁止写入日志**：认证头、请求体中的 `api_key` 等字段仍可能以明文形式存在于上游请求中，但日志输出时不得额外打印这些字段的完整值。
4. **不阻塞请求**：`tauri-plugin-log` 的打印不等待磁盘/网络，失败不影响网关转发。

---

## 3. 自研内存 logger

### 3.1 设计目标

- 聚焦**运行时诊断**：网关请求、供应商 API 调用、系统事件、Command 调用。
- 用户可在应用内「日志」页面实时查看、筛选、导出。
- 非阻塞写入，默认保留最近 5000 条（可在设置中调整）。

### 3.2 后端使用方式

#### 持有 `LoggerServiceHandle` 时

```rust
use crate::modules::logger::LoggerServiceHandle;

logger_handle.service().log_system(
    crate::modules::logger::types::LogLevel::Info,
    "开机自启已启用",
    Some(file!()),
);
```

#### 无法获取 `AppHandle` / `State` 时

使用全局工具类 [`Log`](../src-tauri/src/modules/logger/logging.rs)：

```rust
use crate::modules::logger::Log;

Log::info("模块启动完成");
Log::warn_with_loc("配置加载失败", file!(), line!());
Log::error("发生未知错误");
```

全局句柄在 [`main.rs`](../src-tauri/src/main.rs) 初始化 `LoggerServiceHandle` 后注册：

```rust
modules::logger::set_global_logger_handle(logger_handle.clone());
```

### 3.3 前端使用方式

使用 [`src/modules/logger/logger.ts`](../src/modules/logger/logger.ts)：

```typescript
import { logger } from '@/modules/logger/logger'

await logger.info('用户执行了导出操作')
await logger.error('导出失败', 'export-panel.tsx', 42)
```

### 3.4 级别与开关控制

配置持久化在 `log_settings` 表，包括：

- 内存缓冲区大小
- 是否启用文件持久化、保留天数、文件级别阈值
- 转发详细日志（请求/响应体）
- 直连网关请求日志
- Command 交互日志

通过「日志」页面或 `log_get_settings` / `log_set_settings` Command 调整。

---

## 4. 隔离边界

两套日志机制在以下层面完全隔离：

1. **写入路径不同**
   - `log::` 宏 → `tauri-plugin-log` 注册的 logger → 终端/WebView/文件。
   - `Log::info` / `logger.info` → `LoggerService.write` → 内存缓冲区 → 「日志」页面/可选文件。

2. **级别控制独立**
   - 修改「设置」中的全局日志级别只影响 `tauri-plugin-log`。
   - 修改「日志」页面设置只影响自研 logger。

3. **输出目标独立**
   - `tauri-plugin-log` 的文件目录由插件管理。
   - 自研 logger 的文件目录由 `log_settings.log_dir` 控制。

4. **失败策略独立**
   - `tauri-plugin-log` 写入失败由插件处理。
   - 自研 logger 写入失败（包括前端调用失败）静默忽略，不影响业务逻辑。

---

## 5. 选择指南

开发/需求阶段必须根据场景选择日志框架：

| 场景 | 推荐框架 |
|------|----------|
| 调试 Rust 后端代码、查看循环变量、追踪调用链 | tauri-plugin-log |
| 模块启动/停止、初始化结果、临时错误排查 | tauri-plugin-log |
| 需要输出到终端 / WebView 控制台 / 日志文件 | tauri-plugin-log |
| 网关请求诊断（URL、状态码、耗时、Token） | 自研内存 logger + tauri-plugin-log |
| 供应商 API 调用记录、错误聚合 | 自研内存 logger + tauri-plugin-log |
| 系统事件需要在「日志」页面展示 | 自研内存 logger |
| 需要按来源/级别/时间筛选、导出给运维 | 自研内存 logger |
| 前端业务事件需要后端持久化并展示 | 自研内存 logger |

**通用原则**：

- 若不确定，优先使用 `tauri-plugin-log`，因为自研 logger 有 UI 可见性要求。
- 一旦需求明确要求"在日志页面展示"、"可筛选导出"、"运维可见"，必须使用自研内存 logger。
- 任何日志中禁止写入 Secret 明文。

---

## 6. 相关文件

- 后端实现：[src-tauri/src/modules/logger/](../src-tauri/src/modules/logger/)
- 后端全局工具：[src-tauri/src/modules/logger/logging.rs](../src-tauri/src/modules/logger/logging.rs)
- 后端 Commands：[src-tauri/src/modules/logger/commands.rs](../src-tauri/src/modules/logger/commands.rs)
- 前端工具：[src/modules/logger/logger.ts](../src/modules/logger/logger.ts)
- 前端类型：[src/modules/logger/types.ts](../src/modules/logger/types.ts)
- 设置页日志级别：[src/routes/settings.tsx](../src/routes/settings.tsx)
- 日志页面：[src/routes/logs/index.tsx](../src/routes/logs/index.tsx)
- Agent 指引：[AGENTS.md §11](../AGENTS.md)
