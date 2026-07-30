# 事件总线文档

> 本文档汇总 i-code 前端内部事件（mitt）与前后端 Tauri 事件（`app_handle.emit` → `listen`）。
> 新增事件时，应同步更新本文档、[`src/core/events.ts`](file:///d:/ProjectApp/i-code/src/core/events.ts) 以及 Rust 侧 emit 位置。

## 1. 事件分层

```
后端状态变化 → Tauri Event emit ────────────────────────┐
                                                         │
前端内部状态变化 → mitt eventBus.emit ───┐                │
                                         ↓                ↓
                              组件/Hook 监听 → Zustand / useState 更新 → UI 重渲染
```

- **前端内部事件**：使用 [`mitt`](https://github.com/developit/mitt)，仅在 `src/hooks/` 与业务组件内 emit/listen，用于**同一窗口内**的 UI 状态同步。
- **后端 Tauri 事件**：Rust 通过 `tauri::Emitter::emit()` 推送，前端通过 `@tauri-apps/api/event` 的 `listen()` 接收，用于**后端 → 前端**的跨进程状态同步。

## 2. 命名规范

| 范围 | 命名风格 | 示例 |
|------|----------|------|
| 前端内部事件 | `模块:动作` | `provider:changed`、`workspace:switched` |
| 后端事件 | `模块:动作` 或 `模块-动作` | `gateway:status-changed`、`memory-usage`、`log:new-entry` |

> 历史原因：`provider-changed`（托盘菜单）使用连字符，与前端常量 `provider:changed` 不同名，接入时需注意。

## 3. 后端 Tauri 事件清单

事件名常量定义在 [`src/core/events.ts`](file:///d:/ProjectApp/i-code/src/core/events.ts) 的 `BACKEND_EVENTS`。

| 常量 | 事件名 | 触发位置 | payload 类型 | 前端消费者 | 说明 |
|------|--------|----------|--------------|------------|------|
| `GATEWAY_STATUS_CHANGED` | `gateway:status-changed` | [`gateway_runtime/service.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/modules/gateway_runtime/service.rs#L147-L151) `start()` / `stop()` | `GatewayRuntimeState` | [`hooks/use-gateway-status.ts`](file:///d:/ProjectApp/i-code/src/hooks/use-gateway-status.ts#L62-L66) | 网关启停后广播当前运行状态 |
| `LOG_NEW_ENTRY` | `log:new-entry` | [`logger/commands.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/modules/logger/commands.rs) `log_write` / `log_message` | `LoggerLogEntry` | [`hooks/use-logs.ts`](file:///d:/ProjectApp/i-code/src/hooks/use-logs.ts#L101-L111) | 新日志写入后实时推送到日志面板 |
| `LOG_CLEARED` | `log:cleared` | [`logger/commands.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/modules/logger/commands.rs#L66-L79) `log_clear` | `()` | [`hooks/use-logs.ts`](file:///d:/ProjectApp/i-code/src/hooks/use-logs.ts#L114-L118) | 日志缓冲区被清空 |
| `WORKSPACE_APPLIED` | `workspace:applied` | [`workspace/commands.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/modules/workspace/commands.rs#L242-L250) `workspace_apply` | `ApplyWorkspaceResult` | 暂无 | 工作区配置已写入 CLI 配置文件 |
| `MEMORY_USAGE` | `memory-usage` | [`main.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/main.rs#L461-L473) 托盘定时线程 | `number`（KB） | [`modules/system/use-memory-usage.ts`](file:///d:/ProjectApp/i-code/src/modules/system/use-memory-usage.ts#L43-L48) | 每 5 秒广播一次进程物理内存 |
| `CALL_RECORD_UPDATED` | `call-record:updated` | [`gateway_runtime/upstream.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/modules/gateway_runtime/upstream.rs#L685-L686) `finish_call_log_full` | `ModelCallLog` | 暂无 | 网关请求完成、调用记录落库后广播 |
| `PROVIDER_CHANGED` | `provider:changed` | [`ai_gateway/commands.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/modules/ai_gateway/commands.rs#L72-L122) `gateway_provider_create` / `update` / `delete` | `{ action: 'create'\|'update'\|'delete', providerId: string }` | 暂无（托盘已监听） | 供应商增删改后广播，托盘额度子菜单据此动态增删菜单项 |
| `BALANCE_SNAPSHOT_UPDATED` | `balance:snapshot-updated` | [`ai_gateway/commands.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/modules/ai_gateway/commands.rs#L648) `balance_refresh_provider` / [`balance/commands.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/modules/balance/commands.rs#L68) `balance_refresh` | `BalanceRefreshResult` | 暂无（托盘已监听） | 余额查询成功并持久化快照后广播；托盘额度子菜单据此动态新增菜单项 |
| `SETTINGS_CHANGED` | `settings:changed` | [`settings/commands.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/modules/settings/commands.rs#L42-L43) `settings_update` | `TitleBarInfoConfig` | [`components/ui/title-bar-info-container.tsx`](file:///d:/ProjectApp/i-code/src/components/ui/title-bar-info-container.tsx#L49-L55) | 设置更新后广播最新标题栏信息配置 |
| `CHAT_STREAM_CHUNK` | `chat:stream-chunk` | [`chat/service.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/modules/chat/service.rs) SSE/HTTP 增量 | `ChatStreamChunkEvent` | [`hooks/use-chat.ts`](file:///d:/ProjectApp/i-code/src/hooks/use-chat.ts) `useChatSession` | 聊天助手正文/思考过程增量；详见 [`chat-module.md`](./chat-module.md) |
| `CHAT_STREAM_DONE` | `chat:stream-done` | [`chat/service.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/modules/chat/service.rs) 请求成功结束 | `ChatStreamDoneEvent` | [`hooks/use-chat.ts`](file:///d:/ProjectApp/i-code/src/hooks/use-chat.ts) | 聊天完成：content、thinking、usage |
| `CHAT_STREAM_ERROR` | `chat:stream-error` | [`chat/service.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/modules/chat/service.rs) 请求失败 | `ChatStreamErrorEvent`（含 errorCode/errorBody） | [`hooks/use-chat.ts`](file:///d:/ProjectApp/i-code/src/hooks/use-chat.ts) | 聊天失败：气泡展示错误码与 body |

### 3.1 托盘占位事件

| 事件名 | 触发位置 | payload | 说明 |
|--------|----------|---------|------|
| `provider-changed` | [`main.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/main.rs) 托盘菜单"选择供应商"子菜单点击 | `String`（菜单 ID，如 `provider:openai`） | 当前为占位逻辑，前端未监听 |

> 注意：`provider-changed`（连字符）是托盘菜单点击占位事件，与 §3 中 `provider:changed`（冒号，供应商数据变更）是**两个不同事件**，勿混淆。

## 4. 前端内部事件清单

事件名常量定义在 [`src/core/events.ts`](file:///d:/ProjectApp/i-code/src/core/events.ts) 的 `EVENT_NAMES`，类型映射在 `FrontendEvents`。

| 常量 | 事件名 | payload | 状态 |
|------|--------|---------|------|
| `PROVIDER_CHANGED` | `provider:changed` | `{ providerId?, action?: 'create' \| 'update' \| 'delete' }` | 保留类型，当前无 emit |
| `WORKSPACE_SWITCHED` | `workspace:switched` | `{ workspaceId: string }` | 保留类型，当前无 emit |
| `WORKSPACE_APPLIED` | `workspace:applied` | `{ workspaceId: string; appliedAt: string }` | 保留类型，当前无 emit |
| `GATEWAY_STATUS_CHANGED` | `gateway:status-changed` | `{ isRunning: boolean; error?: string }` | 保留类型，当前无 emit |
| `SETTINGS_CHANGED` | `settings:changed` | `{ key: string; value: unknown }` | 保留类型，当前无 emit |
| `LOCALE_CHANGED` | `locale:changed` | `{ locale: string }` | 保留类型，当前无 emit |
| `THEME_CHANGED` | `theme:changed` | `{ theme: string }` | 保留类型，当前无 emit |
| `BALANCE_REFRESHED` | `balance:refreshed` | `{ cliProviderId: string }` | 保留类型，当前无 emit |
| `VIRTUAL_PROVIDER_HEALTH_CHANGED` | `virtual-provider:health-changed` | `{ routeId: string; isHealthy: boolean }` | 保留类型，当前无 emit |
| `SECRET_CHANGED` | `secret:changed` | `{ secretId: string; action?: 'create' \| 'delete' }` | 保留类型，当前无 emit |

> 前端内部事件当前均处于**类型保留**状态，暂无 `eventBus.emit` 调用点。需要跨组件同步时可直接使用。

## 5. 新增事件流程

1. **命名**：按 §2 规范命名，避免与已有事件冲突。
2. **常量**：在 [`src/core/events.ts`](file:///d:/ProjectApp/i-code/src/core/events.ts) 中：
   - 后端事件加入 `BACKEND_EVENTS`。
   - 前端内部事件加入 `EVENT_NAMES` 与 `FrontendEvents` 类型映射。
3. **后端 emit**：
   - Command 中增加 `app_handle: tauri::AppHandle` 参数。
   - `use tauri::Emitter;`。
   - 业务动作成功后 `app_handle.emit("事件名", &payload)`。
4. **前端 listen**：
   - 使用 `@tauri-apps/api/event` 的 `listen<T>()`。
   - 在 `useEffect` 中订阅，return 中调用 `unlisten()`。
5. **文档**：更新本文档 §3 / §4，说明触发位置、payload、消费者。

## 6. 推荐改造点

以下位置目前仍依赖轮询，可改为监听后端事件：

- 仪表盘统计 / 模型调用记录：可监听 `BACKEND_EVENTS.CALL_RECORD_UPDATED`，在收到事件后刷新统计。
- 工作区 / CLI 管理页面：可监听 `BACKEND_EVENTS.WORKSPACE_APPLIED`，刷新 `pending_apply` 状态。
- 余额展示：可监听 `BACKEND_EVENTS.BALANCE_SNAPSHOT_UPDATED`，直接更新 provider 余额快照。
