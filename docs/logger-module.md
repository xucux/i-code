# 日志模块设计文档

> 模块路径：`src-tauri/src/modules/logger/`（后端）+ `src/modules/logger/`（前端类型）+ `src/components/ui/log-viewer.tsx`（UI 组件）
>
> 本模块使用**自研内存 Logger 框架**，与项目中的 `tauri-plugin-log` 完全隔离。详见 §2 与项目级文档 [`log-framework.md`](./log-framework.md)。

---

## 1. 模块定位

日志模块为 i-code 提供运行时日志的**写入、缓冲、查询、导出、实时推送、文件持久化**能力。主要服务对象：

- **网关转发**：每次请求转发完成后，由 `gateway_runtime` 的拦截器写入一条日志
- **上游供应商 API**：转发详细日志开启时，记录请求体/响应体
- **系统事件**：网关启停、配置变更、异常等
- **前后端 Command 交互**：Tauri Command 调用记录到系统日志
- **前端通用日志**：前端通过 `log_message` 写入 DEBUG/INFO/WARN/ERROR 消息

日志同时写入**内存环形缓冲区**和**日志文件**（文件持久化可按需开启）。

### 1.1 与项目日志框架的关系

i-code 同时存在两套日志机制，职责与输出目标不同，但**完全隔离、互不干扰**：

| 维度 | tauri-plugin-log | 自研内存 Logger（本模块） |
|------|------------------|------------------------|
| **定位** | 开发调试、运行时通用追踪 | 业务运行时诊断、运维可见 |
| **后端入口** | `log::info!` / `log::warn!` / `log::error!` / `log::debug!` | `crate::modules::logger::Log::info` 等；`LoggerServiceHandle.service().log_system` |
| **前端入口** | 无（Rust 后端宏） | `src/modules/logger/logger.ts` 的 `logger.info` 等 |
| **输出目标** | 终端、WebView 控制台、应用日志目录文件 | 内存环形缓冲区、应用内「日志」页面、可选按天滚动文件 |
| **存储形态** | 文本行 | 结构化 `LogEntry`，可导出 JSON/CSV |
| **级别控制** | `app_settings.log_level` | `log_settings` 表中的各项开关 |
| **是否本模块** | ❌ 否 | ✅ 是 |

#### 1.1.1 为什么同时存在两套日志

- `tauri-plugin-log` 解决原 `env_logger` 未初始化导致 `log::` 宏无输出的问题，满足开发调试和运行时追踪需求。
- 自研内存 Logger 满足**业务诊断**需求：结构化、可筛选、可导出、可在应用内「日志」页面实时查看。

#### 1.1.2 隔离保证

1. **写入路径隔离**：`log::` 宏走 `tauri-plugin-log` 注册的 logger；`Log::info` / `log_message` 直接写入 `LoggerService` 的内存缓冲区。
2. **级别控制隔离**：修改「设置」中的全局日志级别只影响 `tauri-plugin-log`；修改「日志」页面设置只影响自研 Logger。
3. **输出目标隔离**：`tauri-plugin-log` 的文件目录由插件管理；自研 Logger 的文件目录由 `log_settings.log_dir` 控制。
4. **失败策略隔离**：`tauri-plugin-log` 写入失败由插件处理；自研 Logger 写入失败（包括前端调用失败）静默忽略，不影响业务逻辑。

#### 1.1.3 选择原则

- 需要在「日志」页面展示 → 使用自研内存 Logger。
- 需要按来源/级别/时间筛选、导出给运维 → 使用自研内存 Logger。
- 仅开发调试、查看终端/WebView 输出 → 使用 `tauri-plugin-log`。
- 不确定时，优先使用 `tauri-plugin-log`。

更完整的选择指南见 [`log-framework.md`](./log-framework.md)。

---

## 2. 架构总览

```
gateway_runtime (upstream.rs)    前端 log_message    Command 拦截
  │ 请求完成 → write()           │ → log_message()    │ → system log
  ▼                              ▼                    ▼
LoggerService
  ├─ push() 非阻塞写入 ──────► LogRingBuffer (Mutex<VecDeque>)
  │                              │ 超容量自动淘汰最旧条目
  │                              ├─ query(LogFilter) → 前端查询
  │                              ├─ list_recent(n)   → 前端最近 N 条
  │                              ├─ export()         → 导出 JSON/CSV
  │                              └─ clear()          → 清空
  │
  └─ mpsc::channel ─────────► 后台文件写入线程
                                 │ 按天滚动 i-code-{YYYY-MM-DD}.log
                                 │ pipe 分隔，时间在最前面
                                 │ 启动时清理 >30 天的过期文件

Tauri Event: log:new-entry → 前端实时推送
```

---

## 3. 核心类型

### 3.1 LogEntry（单条日志记录）

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `String` | UUID |
| `timestamp` | `String` | 时间戳，格式 `yyyy-MM-dd HH:mm:ss.SSS` |
| `level` | `LogLevel` | DEBUG / INFO / WARN / ERROR |
| `source` | `LogSource` | gateway / provider-api / system |
| `method` | `Option<String>` | HTTP 方法（POST 等） |
| `url` | `Option<String>` | 请求 URL（已脱敏） |
| `statusCode` | `Option<u16>` | HTTP 状态码 |
| `durationMs` | `Option<u64>` | 请求耗时（毫秒） |
| `promptTokens` | `Option<u64>` | 提示 Token 数 |
| `completionTokens` | `Option<u64>` | 补全 Token 数 |
| `totalTokens` | `Option<u64>` | 总 Token 数 |
| `cachedTokens` | `Option<u64>` | 缓存命中 Token 数 |
| `errorMessage` | `Option<String>` | 错误信息 / 通用日志消息内容 |
| `requestId` | `Option<String>` | 网关唯一请求追踪 ID |
| `requestBody` | `Option<String>` | 请求体内容（转发详细日志开启时） |
| `responseBody` | `Option<String>` | 响应体内容（转发详细日志开启时） |
| `fileName` | `Option<String>` | 源文件名（如 `upstream.rs`） |
| `lineNumber` | `Option<u32>` | 源文件行号 |

> `errorMessage` 双重用途：对于网关日志记录错误信息，对于通用日志（`log_message`）记录日志消息内容。前端通用日志 `source` 固定为 `system`。

### 3.2 LogLevel（日志级别）

| 枚举值 | 序号 | 用途 |
|--------|------|------|
| `DEBUG` | 0 | 调试信息 |
| `INFO` | 1 | 正常请求完成 |
| `WARN` | 2 | 可恢复异常（重试、降级） |
| `ERROR` | 3 | 不可恢复错误 |

### 3.3 LogSource（日志来源）

| 枚举值 | 说明 |
|--------|------|
| `gateway` | 本地网关请求转发（路由匹配、SSE 透传等） |
| `provider-api` | 上游供应商 API 调用（转发详细日志） |
| `system` | 系统级日志（启动、停止、配置变更、Command 交互、前端通用日志） |

---

## 4. 时间格式

### 4.1 格式规范

全局统一使用 `yyyy-MM-dd HH:mm:ss.SSS` 格式：

```
2026-07-17 08:30:15.123
```

Rust 格式化常量：

```rust
pub const LOG_TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.3f";
```

### 4.2 使用位置

| 位置 | 代码 |
|------|------|
| LogEntry 构造 | `chrono::Local::now().format(LOG_TIME_FORMAT).to_string()` |
| 日志文件写入 | 同上（timestamp 字段直接写入） |
| 前端展示 | `formatTimestamp()` 解析并取时分秒毫秒部分 |

### 4.3 前端时间展示

前端 `formatTimestamp()` 函数：
- 如果 timestamp 已是 `yyyy-MM-dd HH:mm:ss.SSS` 格式，直接取 `HH:mm:ss.SSS` 部分
- 兼容 ISO 8601 格式，使用 `new Date()` 解析后格式化

---

## 5. 内存环形缓冲区

### 5.1 数据结构

```rust
pub struct LogRingBuffer {
    inner: Mutex<VecDeque<LogEntry>>,
    capacity: usize,  // 默认 5000
}
```

### 5.2 设计要点

| 特性 | 实现 |
|------|------|
| **线程安全** | `Mutex` 保护（写入为热路径，锁持有时间极短：仅 push_back） |
| **容量控制** | 超过 `capacity` 时 `pop_front()` 淘汰最旧条目 |
| **默认容量** | 5000 条（可通过 `LogRollingConfig.bufferSize` 配置） |
| **查询** | `list_all()` / `list_recent(n)` 均从内存读取，无文件 IO |
| **Mutex 中毒恢复** | 使用 `expect("日志缓冲区 Mutex 中毒")`，panic 级别 |

### 5.3 容量规划

| 缓冲区大小 | 单条日志估算 | 内存占用估算 |
|------------|-------------|-------------|
| 5000（默认） | ~500 B（无 requestBody） | ~2.5 MB |
| 5000（开启请求体） | ~2 KB（含截断请求体） | ~10 MB |

---

## 6. 文件持久化

### 6.1 概述

日志在写入内存环形缓冲区的同时，可选异步写入本地日志文件。通过 `LogRollingConfig.enableFilePersistence` 控制（默认关闭）。

### 6.2 文件格式

日志文件使用 **pipe（`|`）分隔**，时间在最前面：

```
2026-07-17 08:30:15.123|INFO|gateway|POST|https://api.openai.com/v1/chat/completions|200|150||||||req-abc123||||
```

字段顺序：

```
timestamp|level|source|method|url|statusCode|durationMs|promptTokens|completionTokens|totalTokens|cachedTokens|errorMessage|requestId|fileName|lineNumber|requestBody|responseBody
```

共 17 个字段，空值字段保留空字符串。

### 6.3 按天滚动

- 文件名格式：`i-code-{YYYY-MM-DD}.log`
- 每天自动切换新文件
- 文件存储路径：`{应用数据目录}/i-code/logs/`

### 6.4 异步写入机制

```
LoggerService.write(entry)
  → RingBuffer.push(entry)               // 内存
  → mpsc::Sender.send(format_pipe_line)  // 发送到后台线程

后台线程:
  → mpsc::Receiver.recv(line)
  → 计算当前日期
  → 日期变化时切换文件（OpenOptions::append）
  → writeln!(file, line)
```

使用 `std::sync::mpsc` 通道实现异步写入：
- 写入端（主线程）仅 `send()` 一次，不阻塞
- 后台线程持续 `recv()` 并追加到当天文件
- 文件句柄按需切换，不频繁开关

### 6.5 过期清理

- 启动时清理超过 `max_retention_days`（默认 30 天）的日志文件
- 比较文件名中的日期部分与截止日期
- 匹配模式：`i-code-YYYY-MM-DD.log`

```rust
fn cleanup_old_logs(log_dir: &Path, max_retention_days: u32) {
    let cutoff = Local::now() - Duration::days(max_retention_days as i64);
    // 删除文件名日期 < cutoff 的日志文件
}
```

### 6.6 LogRollingConfig

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `bufferSize` | 5000 | 内存缓冲队列大小 |
| `enableFilePersistence` | `false` | 是否启用文件持久化 |
| `maxFileSizeMb` | 10 | 单个日志文件大小上限（预留） |
| `maxFileCount` | 7 | 保留的日志文件数量（预留） |
| `maxRetentionDays` | 30 | 日志文件保留天数 |
| `fileLogLevel` | `INFO` | 文件写入级别阈值（预留） |

---

## 7. 查询与过滤

### 7.1 LogFilter

| 字段 | 类型 | 过滤逻辑 |
|------|------|----------|
| `levels` | `Vec<LogLevel>` | OR 关系，为空不过滤 |
| `sources` | `Vec<LogSource>` | OR 关系 |
| `statusCodes` | `Vec<u16>` | OR 关系 |
| `keyword` | `Option<String>` | 模糊匹配 URL、errorMessage |
| `timeRange` | `Option<TimeRange>` | 时间区间过滤（from/to，字符串比较） |
| `requestId` | `Option<String>` | 精确匹配 |

### 7.2 查询流程

```
前端 invoke('log_list', { filter })
  → Commands: log_list
  → Service: query(&filter)
    → RingBuffer: list_all()     // 克隆全部条目
    → filter: matches(entry)     // 逐条匹配
  → 返回 Vec<LogEntry>
```

---

## 8. 转发详细日志（LogSettings 统一管理）

### 8.1 设计背景

默认日志仅记录请求元信息（method、url、statusCode、durationMs），不记录请求体/响应体。调试场景下需要查看完整请求/响应内容，因此提供运行时可切换的转发详细日志功能。

### 8.2 ForwardLogConfig（从 LogSettings 派生）

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `enableRequestLog` | `bool` | `false` | 是否记录请求体 |
| `enableResponseLog` | `bool` | `false` | 是否记录响应体 |
| `maxBodyLength` | `usize` | `4096` | 单条最大记录字符数，超出截断追加 `...[truncated]` |

配置持久化到 `log_settings` 数据库表（单行 `id='default'`），启动时从 DB 加载到 `LoggerService` 内存缓存。运行时通过 `log_set_settings` Command 更新，同时写入 DB 和更新内存缓存。`GatewaySharedState` 不再持有独立的 `forward_log_config`，改为从 `LoggerService` 读取。详见 §17。

### 8.3 写入时机与逻辑

```
upstream.rs: forward_chat_completions / forward_anthropic_messages
  │
  ├─ 1. 读取 ForwardLogConfig
  ├─ 2. 请求发送前：enableRequestLog=true → truncate_body → logged_request_body
  ├─ 3. 请求失败 → write_forward_error_log(..., logged_request_body)
  ├─ 4. 流式响应 → 不记录响应体 → write_forward_log(..., logged_request_body, None)
  └─ 5. 非流式响应 → enableResponseLog=true → build_json_response_with_body → write_forward_log(..., logged_request_body, logged_response_body)
```

### 8.4 截断保护

```rust
fn truncate_body(s: &str, max_len: usize) -> String {
    if s.len() <= max_len { s.to_string() }
    else { format!("{}...[truncated]", &s[..max_len]) }
}
```

---

## 9. Command 交互日志（LogSettings 统一管理）

### 9.1 设计背景

前后端通过 Tauri Command 交互时，需要记录调用日志到系统日志，便于排查前后端通信问题。Command 请求参数和响应数据可选记录。

### 9.2 CommandLogConfig（从 LogSettings 派生）

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `enableCommandLog` | `bool` | `true` | 是否记录 Command 调用到系统日志 |
| `enableCommandRequestLog` | `bool` | `false` | 是否记录请求参数 |
| `enableCommandResponseLog` | `bool` | `false` | 是否记录响应数据 |
| `maxBodyLength` | `usize` | `4096` | 单条最大记录字符数，超出截断 |

配置持久化到 `log_settings` 数据库表（单行 `id='default'`），启动时从 DB 加载到 `LoggerService` 内存缓存。运行时通过 `log_set_settings` Command 更新，同时写入 DB 和更新内存缓存。`GatewaySharedState` 不再持有独立的 `command_log_config`，改为从 `LoggerService` 读取。详见 §17。

### 9.3 前端通用日志（log_message）

前端可通过 [`src/modules/logger/logger.ts`](../src/modules/logger/logger.ts) 便捷写入自研内存 Logger：

```typescript
import { logger } from '@/modules/logger/logger'

await logger.info('用户执行了导出操作')
await logger.error('导出失败', 'export-panel.tsx', 42)
```

`logger.ts` 内部调用 `log_message` Command，失败静默忽略，不影响业务逻辑。等价于直接 invoke：

```
前端 invoke('log_message', { level: 'INFO', message: '用户点击了xxx', fileName: 'model-list.tsx', lineNumber: 42 })
  → 后端构造 LogEntry { source: System, errorMessage: message, fileName, lineNumber }
  → LoggerService.write()
  → 前端实时收到 log:new-entry 事件
```

---

## 10. 实时推送

```
后端 Service.write(entry)
  → RingBuffer.push(entry)
  → [开启文件持久化] mpsc::Sender.send(format_pipe_line)
  → Commands 层: app.emit("log:new-entry", &entry)
  → 前端 listen("log:new-entry", callback)
```

---

## 11. 导出

### 11.1 支持格式

| 格式 | 扩展名 | 说明 |
|------|--------|------|
| JSON | `.json` | `serde_json::to_string_pretty`，完整结构 |
| CSV | `.csv` | 18 列（含 fileName、lineNumber） |

### 11.2 CSV 字段

```
id,timestamp,level,source,method,url,statusCode,durationMs,promptTokens,completionTokens,totalTokens,cachedTokens,errorMessage,requestId,requestBody,responseBody,fileName,lineNumber
```

### 11.3 导出路径

文件保存到 `app.path().app_cache_dir()`，文件名格式 `i-code-logs-{YYYYMMDD-HHmmss}.{ext}`。

---

## 12. Tauri Command 清单

| Command | 说明 | 参数 | 返回 |
|---------|------|------|------|
| `log_list` | 查询日志 | `filter?: LogFilter` | `Vec<LogEntry>` |
| `log_recent` | 获取最近 N 条 | `limit?: number` | `Vec<LogEntry>` |
| `log_write` | 写入一条日志 | `entry: LogEntry` | `void` |
| `log_message` | 前端通用日志写入（推荐通过 `src/modules/logger/logger.ts` 调用） | `level, message, fileName?, lineNumber?` | `void` |
| `log_clear` | 清空日志 | — | `void` |
| `log_count` | 当前条数 | — | `number` |
| `log_export` | 导出日志 | `filter?, format` | `LogExportResult` |
| `log_get_settings` | 读取统一日志设置 | — | `LogSettings` |
| `log_set_settings` | 更新统一日志设置 | `settings: LogSettings` | `LogSettings` |

> `gateway_get_forward_log_config` / `gateway_set_forward_log_config` / `log_get_command_config` / `log_set_command_config` 已废弃，由 `log_get_settings` / `log_set_settings` 统一替代。

---

## 13. 前端组件

### 13.1 LogViewer（通用日志浏览组件）

路径：`src/components/ui/log-viewer.tsx`

| 功能 | 实现 |
|------|------|
| 表格展示 | 级别（彩色 Badge）+ 时间 + 来源 + 消息 |
| 点击展开 | **所有行**均可点击展开，以表格形式显示全部非空字段 |
| 展开标记 | 所有行显示 `▶` 箭头，展开后旋转为 `▼` |
| 字段标签 | 中文映射（FIELD_LABELS）：方法、URL、状态码、耗时、请求体、响应体、文件名、行号等 |
| 长文本 | 超过 100 字符的字段用 `<pre>` 包裹，`max-h-40` 可滚动 |
| 时间格式 | `yyyy-MM-dd HH:mm:ss.SSS` → 展示 `HH:mm:ss.SSS` |

### 13.2 日志页面（/logs）

路径：`src/routes/logs/index.tsx`

| 功能 | 控件 |
|------|------|
| Tab 切换 | 网关日志 / 系统日志 / 日志设置 |
| 时间范围 | datetime-local 输入 |
| 关键词搜索 | Input 模糊匹配 |
| 自动刷新 | Switch 开关 |
| 清空 / 导出 | Button |

### 13.3 日志设置 Tab

路径：`src/routes/logs/index.tsx` 内"日志设置" Tab

包含三个 Card：

#### Card 1：基础设置

| 配置项 | 控件 | 对应字段 | 默认值 |
|--------|------|----------|--------|
| 缓冲区大小 | InputNumber | `bufferSize` | 5000 |
| 日志目录 | Input（只读展示） | `logDirectory` | 应用数据目录 |
| 日志保留天数 | InputNumber | `maxRetentionDays` | 30 |
| 文件持久化 | Switch | `enableFilePersistence` | false |

#### Card 2：转发详细日志

| 配置项 | 控件 | 对应字段 | 默认值 |
|--------|------|----------|--------|
| 记录请求体 | Switch | `enableRequestLog` | false |
| 记录响应体 | Switch | `enableResponseLog` | false |
| 单条最大长度 | InputNumber | `forwardMaxBodyLength` | 4096 |

#### Card 3：Command 交互日志

| 配置项 | 控件 | 对应字段 | 默认值 |
|--------|------|----------|--------|
| 记录 Command 调用 | Switch | `enableCommandLog` | true |
| 记录请求参数 | Switch | `enableCommandRequestLog` | false（依赖 Command 日志开关） |
| 记录响应数据 | Switch | `enableCommandResponseLog` | false（依赖 Command 日志开关） |
| 单条最大长度 | InputNumber | `commandMaxBodyLength` | 4096 |

> 顶部工具栏不再显示配置开关（记录请求体/响应体/Command 日志等），这些配置统一移至"日志设置" Tab。设置变更通过 `log_set_settings` Command 提交。

---

## 14. 数据流全链路

### 14.1 普通网关日志

```
客户端 → 本地 Gateway
  → upstream.rs: forward_chat_completions
  → 上游响应
  → finish_call_log()                        → call_records 表
  → log_gateway_request()                    → LoggerService.write() → RingBuffer + [文件]
  → app.emit("log:new-entry")                → 前端实时更新
```

### 14.2 转发详细日志

```
upstream.rs: forward_chat_completions
  → 从 LoggerService.settings() 获取 LogSettings
  → .to_forward_config() → ForwardLogConfig
  → [开启] truncate_body(requestBody) → logged_request_body
  → 上游响应
  → [非流式 + 开启] truncate_body(responseBody) → logged_response_body
  → write_forward_log(..., logged_request_body, logged_response_body)
    → LoggerService.write(LogEntry { source: ProviderApi, requestBody, responseBody })
    → app.emit("log:new-entry")
  → 前端 LogViewer: 点击展开 → 显示全部字段详情
```

### 14.3 前端通用日志

```
前端调用 log_message({ level, message, fileName, lineNumber })
  → 后端构造 LogEntry { source: System, errorMessage: message, fileName, lineNumber }
  → LoggerService.write() → RingBuffer + [文件]
  → app.emit("log:new-entry")
  → 前端实时显示
```

### 14.4 日志设置更新

```
前端日志设置 Tab 修改配置
  → invoke('log_set_settings', { settings })
  → Commands: log_set_settings(settings)
  → Service: update_settings(&settings)
    → Repository: upsert(settings)              // 写 log_settings 表
    → self.settings.write().replace(settings)    // 更新 LoggerService 内存缓存
  → 返回更新后的 LogSettings
  → 下次读取时立即生效（转发日志 / Command 日志 / 文件持久化等）
```

---

## 15. 性能考量

| 场景 | 影响 | 缓解措施 |
|------|------|----------|
| 默认关闭转发日志 | 零开销 | 仅多一次 `RwLock::read()`，纳秒级 |
| 开启请求体记录 | 每次请求多一次 `body.to_string()` + 截断 | `maxBodyLength` 截断保护 |
| 开启响应体记录（非流式） | 需先 `bytes()` 消费响应再构造 axum Response | `build_json_response_with_body` |
| 响应体记录（流式） | **不记录** | 数据量大且持续流入 |
| 环形缓冲区 | 写入 O(1)，查询 O(n) | 默认 5000 条上限 |
| 文件写入 | `mpsc::send()` 非阻塞 | 后台线程异步追加 |
| Mutex 锁 | 写入路径持有时间极短 | 仅 `push_back` 一个操作 |

---

## 16. 安全约束

| 约束 | 实现 |
|------|------|
| URL 脱敏 | 去除 query 中的敏感参数（API Key 等） |
| 请求体可能含敏感内容 | `maxBodyLength` 截断；关闭状态下不记录 |
| 响应体可能含完整输出 | 同上截断保护 |
| CSV/文件导出 | requestBody/responseBody 列同样包含，需注意文件保管 |
| Secret 引用 | 请求体中的 `$SECRET:{uuid}$` 引用不会被解析为明文 |
| 日志文件权限 | 默认存储在应用数据目录，遵循操作系统权限控制 |

---

## 17. 统一日志配置（LogSettings）

### 17.1 设计背景

日志模块原有 `ForwardLogConfig`、`CommandLogConfig`、`LogRollingConfig` 三个独立配置类型，分别存储在 `GatewaySharedState` 的独立字段中且不持久化。统一为 `LogSettings` 后：

- **单表持久化**：所有配置项合并到 `log_settings` 数据库表（单行 `id='default'`），重启后配置不丢失
- **统一管理**：前端通过一个"日志设置" Tab 管理全部配置，而非散落在不同位置
- **内存同步**：启动时从 DB 加载到 `LoggerService` 内存缓存；运行时通过 `log_set_settings` 同时写 DB 和更新内存

### 17.2 LogSettings 字段

| # | 字段 | 类型 | 说明 | 原属配置 |
|---|------|------|------|----------|
| 1 | `bufferSize` | `u32` | 内存缓冲队列大小 | LogRollingConfig |
| 2 | `enableFilePersistence` | `bool` | 是否启用文件持久化 | LogRollingConfig |
| 3 | `maxFileSizeMb` | `u32` | 单个日志文件大小上限 MB（预留） | LogRollingConfig |
| 4 | `maxFileCount` | `u32` | 保留的日志文件数量（预留） | LogRollingConfig |
| 5 | `maxRetentionDays` | `u32` | 日志文件保留天数 | LogRollingConfig |
| 6 | `fileLogLevel` | `String` | 文件写入级别阈值（预留） | LogRollingConfig |
| 7 | `logDirectory` | `String` | 日志文件存储目录（只读展示） | LogRollingConfig |
| 8 | `enableRequestLog` | `bool` | 是否记录转发请求体 | ForwardLogConfig |
| 9 | `enableResponseLog` | `bool` | 是否记录转发响应体 | ForwardLogConfig |
| 10 | `forwardMaxBodyLength` | `u32` | 转发日志单条最大字符数 | ForwardLogConfig |
| 11 | `enableCommandLog` | `bool` | 是否记录 Command 调用 | CommandLogConfig |
| 12 | `enableCommandRequestLog` | `bool` | 是否记录 Command 请求参数 | CommandLogConfig |
| 13 | `enableCommandResponseLog` | `bool` | 是否记录 Command 响应数据 | CommandLogConfig |
| 14 | `commandMaxBodyLength` | `u32` | Command 日志单条最大字符数 | CommandLogConfig |

### 17.3 默认值

| 字段 | 默认值 |
|------|--------|
| `bufferSize` | 5000 |
| `enableFilePersistence` | `false` |
| `maxFileSizeMb` | 10 |
| `maxFileCount` | 7 |
| `maxRetentionDays` | 30 |
| `fileLogLevel` | `"INFO"` |
| `logDirectory` | `{应用数据目录}/i-code/logs/` |
| `enableRequestLog` | `false` |
| `enableResponseLog` | `false` |
| `forwardMaxBodyLength` | 4096 |
| `enableCommandLog` | `true` |
| `enableCommandRequestLog` | `false` |
| `enableCommandResponseLog` | `false` |
| `commandMaxBodyLength` | 4096 |

### 17.4 转换方法

`LogSettings` 提供以下方法，供各消费方获取所需配置结构：

| 方法 | 返回类型 | 说明 |
|------|----------|------|
| `to_forward_config()` | `ForwardLogConfig` | 提取 `enableRequestLog` + `enableResponseLog` + `forwardMaxBodyLength` |
| `to_command_config()` | `CommandLogConfig` | 提取 `enableCommandLog` + `enableCommandRequestLog` + `enableCommandResponseLog` + `commandMaxBodyLength` |
| `to_rolling_config()` | `LogRollingConfig` | 提取 `bufferSize` + `enableFilePersistence` + `maxFileSizeMb` + `maxFileCount` + `maxRetentionDays` + `fileLogLevel` |

> 原有 `ForwardLogConfig`、`CommandLogConfig`、`LogRollingConfig` 类型保留，作为 `LogSettings` 的派生视图，避免修改下游消费方代码。

### 17.5 存储位置

- **数据库**：`log_settings` 表（详见 §19），单行 `id='default'`
- **内存缓存**：`LoggerService` 持有 `Arc<RwLock<LogSettings>>`，作为运行时配置的快速读取源

### 17.6 生命周期

```
应用启动
  → LoggerService::new()
  → 从 log_settings 表读取（若空则插入默认行）
  → 写入内存缓存 Arc<RwLock<LogSettings>>

运行时更新
  → 前端调用 log_set_settings
  → Commands: log_set_settings(settings)
  → Service: update_settings(&settings)
    → Repository: upsert(settings)           // 写 DB
    → self.settings.write().replace(settings) // 更新内存
```

---

## 18. 系统日志写入规范（log_system）

### 18.1 设计目标

后端模块在发生异常或重要状态变更时，除了通过 `log::error!` / `log::warn!` / `log::info!` 输出到标准输出外，还应将同一条消息写入 Logger 模块的**内存环形缓冲区**，使前端日志页面（`/logs` → 系统日志 Tab）能够实时查看。

### 18.2 log_system 方法签名

```rust
pub fn log_system(&self, level: LogLevel, message: &str, file_name: Option<&str>)
```

- `level`：日志级别（`DEBUG` / `INFO` / `WARN` / `ERROR`）
- `message`：日志消息正文
- `file_name`：源文件名，传 `Some(file!())` 即可

### 18.3 使用规范

后端模块在发生异常或重要状态变更时，应**同时**输出到 `tauri-plugin-log`（标准输出/WebView/文件）和自研内存 Logger（环形缓冲区/日志页面）：

| 场景 | tauri-plugin-log | 自研内存 Logger |
|------|------------------|----------------|
| 不可恢复错误 | `log::error!` | `log_system(LogLevel::Error, ...)` 或 `Log::error(...)` |
| 可恢复异常 | `log::warn!` | `log_system(LogLevel::Warn, ...)` 或 `Log::warn(...)` |
| 重要状态变更 | `log::info!` | `log_system(LogLevel::Info, ...)` 或 `Log::info(...)` |

- 能获取 `LoggerServiceHandle` 时，优先使用 `logger_handle.service().log_system(...)`。
- 无法获取 `LoggerServiceHandle` 时，使用全局工具类 `Log::info(...)` / `Log::error(...)` 等（见 §18.6）。
- `Log::*` 与 `log_system` 最终都写入同一份内存缓冲区，选择哪个取决于是否能便捷获取句柄。

### 18.4 使用示例

```rust
// 在 GatewayRuntimeService 中（已有 self.shared.logger_handle）
self.shared.logger_handle.service().log_system(
    crate::modules::logger::types::LogLevel::Error,
    &format!("网关转发失败: {}", err),
    Some(file!()),
);

// 在 axum Handler 中（通过 State<GatewaySharedState> 提取）
state.logger_handle.service().log_system(
    crate::modules::logger::types::LogLevel::Warn,
    &format!("读取响应体失败: {}", e),
    Some(file!()),
);

// 在 Tray Menu 事件回调中（通过 app.state::<LoggerServiceHandle>() 获取）
app.state::<modules::logger::LoggerServiceHandle>().service().log_system(
    crate::modules::logger::types::LogLevel::Info,
    "开机自启已启用",
    Some(file!()),
);

// 在无法获取 LoggerServiceHandle 的工具函数中
use crate::modules::logger::Log;
Log::info("模块启动完成");
Log::error_with_loc("解析失败", file!(), line!());
```

### 18.5 注意事项

| 规则 | 说明 |
|------|------|
| **tokio::spawn 限制** | `tokio::spawn` 的 `async move` 闭包不能捕获非 `Send` 的引用，因此无法在闭包内访问 `self.shared.logger_handle`。此类场景仅使用 `log::error!` 输出到标准输出，并在调用方（`start()` 方法）通过 `emit_status_changed()` 广播状态兜底。 |
| **SSE 流闭包** | `build_sse_response` 中的 `move` 闭包需先 `clone()` 再传入：`let logger = logger_handle.clone();` |
| **不要吞掉 log_system** | `log_system` 返回 `()`，无需 `let _ =` 或 `unwrap`。 |
| **不要重复记录** | 如果外层调用方已经调用了 `log_system`，内层函数（如 `degrade_virtual_route`）无需重复记录。 |
| **消息格式** | 与 `log::*!` 保持一致，避免加额外的前缀或后缀。 |
| **全局工具类** | 无法获取 `LoggerServiceHandle` 时，优先使用 `crate::modules::logger::Log::info` 等全局工具类（见 §18.6），而不是放弃写入系统日志。 |

> `GatewaySharedState` 不再持有 `forward_log_config` / `command_log_config` 独立字段，转发和 Command 日志配置统一从 `LoggerService.settings()` 获取。

### 18.6 全局日志工具类 `Log`

对于无法便捷获取 `LoggerServiceHandle` 的后端代码（如独立函数、工具模块、事件回调），使用 [`src-tauri/src/modules/logger/logging.rs`](../src-tauri/src/modules/logger/logging.rs) 提供的全局工具类 `Log`。

#### 初始化

在 [`main.rs`](../src-tauri/src/main.rs) 初始化 `LoggerServiceHandle` 后注册全局句柄：

```rust
let logger_handle = modules::logger::LoggerServiceHandle::with_default();
modules::logger::set_global_logger_handle(logger_handle.clone());
```

#### 使用方式

```rust
use crate::modules::logger::Log;

Log::info("模块启动完成");
Log::warn("配置加载失败");
Log::error_with_loc("解析失败", file!(), line!());
```

#### 方法清单

| 方法 | 说明 |
|------|------|
| `Log::debug(message)` | DEBUG 级别 |
| `Log::debug_with_loc(message, file_name, line_number)` | DEBUG 级别 + 源码位置 |
| `Log::info(message)` | INFO 级别 |
| `Log::info_with_loc(message, file_name, line_number)` | INFO 级别 + 源码位置 |
| `Log::warn(message)` | WARN 级别 |
| `Log::warn_with_loc(message, file_name, line_number)` | WARN 级别 + 源码位置 |
| `Log::error(message)` | ERROR 级别 |
| `Log::error_with_loc(message, file_name, line_number)` | ERROR 级别 + 源码位置 |

#### 设计约束

- `Log` 内部通过 `OnceLock<LoggerServiceHandle>` 访问服务，未初始化时静默丢弃，不会 panic。
- `Log` **不调用 `log::` 宏**，因此与 `tauri-plugin-log` 完全隔离。
- 所有方法均为非阻塞写入；写入失败不影响业务逻辑。

---

## 19. log_settings 表结构

### 19.1 DDL

```sql
CREATE TABLE IF NOT EXISTS log_settings (
    id                  TEXT PRIMARY KEY DEFAULT 'default',
    buffer_size         INTEGER NOT NULL DEFAULT 5000,
    enable_file_persistence INTEGER NOT NULL DEFAULT 0,
    max_file_size_mb    INTEGER NOT NULL DEFAULT 10,
    max_file_count      INTEGER NOT NULL DEFAULT 7,
    max_retention_days  INTEGER NOT NULL DEFAULT 30,
    file_log_level      TEXT    NOT NULL DEFAULT 'INFO',
    log_directory       TEXT    NOT NULL DEFAULT '',
    enable_request_log  INTEGER NOT NULL DEFAULT 0,
    enable_response_log INTEGER NOT NULL DEFAULT 0,
    forward_max_body_length INTEGER NOT NULL DEFAULT 4096,
    enable_command_log  INTEGER NOT NULL DEFAULT 1,
    enable_command_request_log  INTEGER NOT NULL DEFAULT 0,
    enable_command_response_log INTEGER NOT NULL DEFAULT 0,
    command_max_body_length INTEGER NOT NULL DEFAULT 4096
);
```

### 19.2 列说明

| 列名 | SQLite 类型 | 默认值 | 说明 |
|------|-------------|--------|------|
| `id` | TEXT | `'default'` | 固定单行，主键 |
| `buffer_size` | INTEGER | 5000 | 内存缓冲队列大小 |
| `enable_file_persistence` | INTEGER | 0 | 文件持久化开关（0=false, 1=true） |
| `max_file_size_mb` | INTEGER | 10 | 单文件大小上限 MB（预留） |
| `max_file_count` | INTEGER | 7 | 保留文件数（预留） |
| `max_retention_days` | INTEGER | 30 | 保留天数 |
| `file_log_level` | TEXT | `'INFO'` | 文件写入级别（预留） |
| `log_directory` | TEXT | `''` | 日志目录（空串表示使用默认路径） |
| `enable_request_log` | INTEGER | 0 | 转发请求体记录开关 |
| `enable_response_log` | INTEGER | 0 | 转发响应体记录开关 |
| `forward_max_body_length` | INTEGER | 4096 | 转发日志单条最大字符数 |
| `enable_command_log` | INTEGER | 1 | Command 日志总开关 |
| `enable_command_request_log` | INTEGER | 0 | Command 请求参数记录开关 |
| `enable_command_response_log` | INTEGER | 0 | Command 响应数据记录开关 |
| `command_max_body_length` | INTEGER | 4096 | Command 日志单条最大字符数 |

### 19.3 约束

- 表中始终只有一行（`id='default'`），Repository 层 upsert 保证不会产生多行
- `log_directory` 为空串时，Service 层使用默认路径 `{应用数据目录}/i-code/logs/`
- 布尔字段使用 SQLite INTEGER（0/1），Repository 层负责 bool ↔ INTEGER 转换
