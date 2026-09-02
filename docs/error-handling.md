# i-code 错误处理规范

> 本文档定义前后端错误类型、错误提示、系统日志记录、全局异常捕获等规范。
> 与 `src-tauri/src/error.rs`、`src/core/errors.ts`、`src/hooks/use-command.ts`、
> `src-tauri/src/modules/logger/` 对应，修改前请同步更新。

---

## 1. 统一错误类型

### 1.1 后端：`IcodeError`

所有后端模块的 Command / Service / Repository 层统一返回 `IcodeResult<T>`，
错误由 `src-tauri/src/error.rs` 中的 [`IcodeError`](file:///d:/ProjectApp/i-code/src-tauri/src/error.rs#L80-L88) 封装。

```rust
pub struct IcodeError {
    pub code: String,           // SCREAMING_SNAKE_CASE 错误码
    pub message: String,        // 用户可见描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}
```

错误码与语义：

| code | 语义 | 典型场景 |
|------|------|----------|
| `UNKNOWN` | 未分类错误 | 兜底 |
| `VALIDATION` | 参数/表单校验失败、JSON 反序列化失败 | 必填字段缺失、JSON 解析错误 |
| `NOT_FOUND` | 资源不存在 | 查询的供应商/模型不存在 |
| `UNAUTHORIZED` | 未认证或会话过期 | API Key 缺失/失效 |
| `FORBIDDEN` | 权限不足 | 访问被禁止的资源 |
| `CONFLICT` | 唯一约束冲突 | slug 重复、主键冲突 |
| `GATEWAY` | 网关请求转发异常 | 上游超时、连接失败、HTTP 错误 |
| `DATABASE` | 数据库操作失败 | SQLite 错误、迁移失败 |
| `INTERNAL` | 后端内部错误 | IO 错误、不应暴露给用户的异常 |

**关键约束：**
- `message` 必须是用户可读、**不含堆栈/SQL 原文/内部路径/敏感信息**。
- Repository 层应将底层异常转换为 `IcodeError`，禁止把 `rusqlite::Error` 等原始错误直接抛给前端。
- `serde_json::Error` 自动映射为 `VALIDATION`（见 `From<serde_json::Error>`）。

### 1.2 前端：`IcodeError` 与 `toIcodeError`

前端 `src/core/errors.ts` 定义与后端对齐的错误基类：

```ts
export class IcodeError extends Error {
  readonly code: ErrorCode
  readonly details?: Record<string, unknown>
}
```

[`toIcodeError(e: unknown)`](file:///d:/ProjectApp/i-code/src/core/errors.ts#L134-L154) 用于兜底转换，
优先按 `{ code, message, details }` 解析后端返回的 JSON，避免 UI 展示 `[object Object]`。

---

## 2. 错误提醒规范（Toast）

### 2.1 何时必须提示

以下场景必须通过 `toast` 通知用户：

1. **用户主动发起的写操作失败**：创建、更新、删除、保存、应用配置等。
2. **用户主动触发的查询/加载失败且结果为空不可恢复**：例如点击按钮拉取官方模型列表失败。
3. **全局状态变更失败**：例如网关启动/停止失败。

以下场景**可以**不弹 Toast，但应写入系统日志：

1. 后台自动刷新失败（如轮询状态、自动同步）。
2. 非关键预加载数据失败（如预填充列表）。
3. 组件卸载后的异步回调错误。

### 2.2 Toast 内容规范

- 必须显示**错误码 + 可读消息**，禁止直接 `String(err)`（可能输出 `[object Object]`）。
- 推荐写法：

```ts
import { toIcodeError } from '@/core/errors'
import { toast } from 'sonner'

try {
  await invokeCommand('some_command', payload)
} catch (err) {
  const error = toIcodeError(err)
  toast.error(`[${error.code}] ${error.message}`)
}
```

- 或复用 `provider-list.tsx` 中的 [`getErrorMessage`](file:///d:/ProjectApp/i-code/src/modules/ai-gateway/ui/provider-list.tsx#L60-L67) 模式：

```ts
function getErrorMessage(error: unknown): string {
  if (error && typeof error === 'object') {
    const icodeError = error as IcodeError
    if (icodeError.code && icodeError.message) {
      return `[${icodeError.code}] ${icodeError.message}`
    }
    if ('message' in error) return String((error as Error).message)
  }
  return String(error)
}
```

### 2.3 禁止行为

- ❌ `toast.error(String(err))` —— 可能展示 `[object Object]`。
- ❌ `catch { /* 完全静默 */ }` —— 用户和日志都感知不到错误。
- ❌ 把后端 `INTERNAL`/`DATABASE` 的堆栈详情透传给用户。

---

## 3. 系统日志记录规范

### 3.1 日志来源

`LogSource` 枚举：

- `gateway`：本地网关请求转发。
- `provider-api`：上游供应商 API 调用。
- `system`：系统级日志（启动、停止、错误、配置变更）。

### 3.2 何时写入系统日志

后端关键异常/状态变化应调用 `LoggerService::log_system`：

```rust
self.shared.logger_handle.service().log_system(
    LogLevel::Error,
    &format!("内置模型预设加载失败: {}", err),
    Some(file!()),
);
```

推荐场景：

1. **启动/停止/运行时错误**：网关启动失败、端口绑定失败、panic 类异常。
2. **持久化数据异常**：数据库连接失败、迁移失败、内置 JSON 数据解析失败。
3. **上游调用异常**：供应商 API 返回非 2xx、超时、连接失败。
4. **不应直接 Toast 的后台错误**：自动同步失败、轮询失败。

### 3.3 日志级别

| 级别 | 使用场景 |
|------|----------|
| `DEBUG` | 调试信息、请求详情、开发期跟踪 |
| `INFO` | 正常生命周期事件、成功启动/停止、配置变更 |
| `WARN` | 可恢复异常、降级行为、潜在风险 |
| `ERROR` | 功能受损、数据异常、用户可见操作失败 |

### 3.4 前端写入系统日志

前端可通过 `log_message` Command 写入系统日志：

```ts
await invokeCommand('log_message', {
  level: 'ERROR',
  message: `加载内置模型预设失败: ${error.message}`,
  fileName: 'provider-form.tsx',
})
```

适用场景：
- 前端捕获到不应静默的后台错误时。
- 后台自动刷新/轮询失败时（不弹 Toast，但必须记日志）。
- 需要跨页面持久化诊断信息的错误。

---

## 4. 全局错误捕获

### 4.1 当前状态

i-code **目前没有全局错误捕获机制**：

- 前端无 React Error Boundary（`components/ui/error-boundary.tsx` 在文档中存在但实际未创建）。
- 前端无 `window.onerror` / `unhandledrejection` 监听。
- 后端无 `std::panic::catch_unwind` 或 panic hook。

### 4.2 建议补齐

#### 前端

在 `src/main.tsx` 增加全局异常监听，将未捕获异常写入系统日志并 Toast：

```ts
window.addEventListener('error', (event) => {
  const message = `未捕获异常: ${event.message} at ${event.filename}:${event.lineno}`
  console.error(message, event.error)
  // 可选：写入系统日志
  invokeCommand('log_message', { level: 'ERROR', message }).catch(() => {})
})

window.addEventListener('unhandledrejection', (event) => {
  const message = `未捕获 Promise 拒绝: ${String(event.reason)}`
  console.error(message, event.reason)
  // 可选：写入系统日志
  invokeCommand('log_message', { level: 'ERROR', message }).catch(() => {})
})
```

#### 后端

在 `src-tauri/src/main.rs` 初始化阶段设置 panic hook：

```rust
std::panic::set_hook(Box::new(|info| {
    eprintln!("Panic: {}", info);
    // 可选：写入 LoggerService / 弹窗提示
}));
```

### 4.3 Error Boundary

建议新增 `components/ui/error-boundary.tsx`，包裹 `<RouterProvider />`，
在渲染层崩溃时展示降级 UI 并记录日志。

---

## 5. Command 调用层错误处理

`src/hooks/use-command.ts` 提供两种调用方式：

### 5.1 `useCommand` Hook

自动管理 `data / error / loading` 状态：

```ts
const { data, error, loading, execute } = useCommand<SomeType>('command_name', {
  onSuccess: (data) => toast.success('成功'),
  onError: (err) => toast.error(`[${err.code}] ${err.message}`),
})
```

**注意：** `onError` 需要调用方显式传入才会触发 Toast，否则错误只停留在 `error` 状态中。

### 5.2 `invokeCommand` 函数

直接抛出 `IcodeError`，调用方必须 `try/catch`：

```ts
try {
  await invokeCommand('command_name', payload)
} catch (err) {
  const error = toIcodeError(err)
  // 必须至少做以下之一：
  // 1. toast.error
  // 2. 写入系统日志
  // 3. 设置组件错误状态展示给用户
}
```

**严禁空 catch：**

```ts
// ❌ 错误示范
try {
  await invokeCommand('gateway_builtin_models_list')
} catch {
  setAllBuiltinModels([])
}
```

---

## 6. 典型案例：`gateway_builtin_models_list` 静默错误

### 6.1 现象

调用 `gateway_builtin_models_list` 时返回：

```json
{ "code": "VALIDATION", "message": "JSON 解析失败：missing field `type` at line 613 column 20" }
```

但系统日志未记录，也未弹出 Toast。

### 6.2 根因

1. **后端**：`seed::load_builtin_models()` 使用 `serde_json::from_str` 解析编译时嵌入的
   `data/builtin-models.json`，反序列化失败时由 `From<serde_json::Error>` 转换为
   `IcodeError::validation(...)`。**未调用 `log_system` 写入系统日志**。
2. **前端**：[`provider-form.tsx`](file:///d:/ProjectApp/i-code/src/modules/ai-gateway/ui/provider-form.tsx#L752-L759)
   的 `loadAllBuiltinModels` 使用空 catch：

```ts
const loadAllBuiltinModels = async () => {
  try {
    const result = await invokeCommand<BuiltinModel[]>('gateway_builtin_models_list')
    setAllBuiltinModels(result)
  } catch {
    setAllBuiltinModels([])
  }
}
```

错误被吞掉，既无 Toast 也无日志。

### 6.3 修复方案

#### 方案 A：前端修复（最小改动）

在 catch 中 Toast 并写入系统日志：

```ts
import { toIcodeError } from '@/core/errors'
import { toast } from 'sonner'

const loadAllBuiltinModels = async () => {
  try {
    const result = await invokeCommand<BuiltinModel[]>('gateway_builtin_models_list')
    setAllBuiltinModels(result)
  } catch (err) {
    setAllBuiltinModels([])
    const error = toIcodeError(err)
    toast.error(`[${error.code}] 加载内置模型预设失败: ${error.message}`)
    void invokeCommand('log_message', {
      level: 'ERROR',
      message: `加载内置模型预设失败: ${error.message}`,
      fileName: 'provider-form.tsx',
    }).catch(() => {})
  }
}
```

#### 方案 B：后端修复（推荐）

在 `seed::load_builtin_models()` 失败时写入系统日志，确保无论前端如何处理，错误都被记录：

```rust
pub fn load_builtin_models() -> IcodeResult<Vec<BuiltinModel>> {
    const JSON: &str = include_str!("../../../data/builtin-models.json");
    serde_json::from_str::<BuiltinModelsManifest>(JSON).map(|m| m.models).map_err(|e| {
        let err = IcodeError::validation(format!("JSON 解析失败：{}", e));
        // 通过全局 LoggerServiceHandle 写入系统日志
        // （需将 LoggerServiceHandle 注入到 AiGatewayService）
        err
    })
}
```

#### 方案 C：全局兜底（长期）

- 后端 Command 统一在返回 `Err` 前写入系统日志（或在 `commands.rs` 层包装）。
- 前端 `invokeCommand` 在 catch 时自动 Toast（但可能造成过多弹窗，需按错误码过滤）。

### 6.4 推荐决策

- **立即**：采用方案 A，修复 `provider-form.tsx` 中的空 catch。
- **短期**：采用方案 B，在数据加载类 Service/seed 函数中将持久化/编译期数据异常写入系统日志。
- **长期**：实现第 4 节的全局错误捕获和第 7 节的 Command 错误兜底策略。

---

## 7. 待补齐项

| 项 | 位置 | 优先级 | 说明 |
|----|------|--------|------|
| React Error Boundary | `components/ui/error-boundary.tsx` | 中 | 文档已列出但实际不存在 |
| 全局 JS 异常监听 | `src/main.tsx` | 中 | `error` / `unhandledrejection` |
| Rust panic hook | `src-tauri/src/main.rs` | 低 | 记录 panic 到日志 |
| Command 错误自动日志 | `commands.rs` 层或中间件 | 低 | 避免每个调用点重复处理 |
| 统一 `getErrorMessage` | `src/core/errors.ts` | 高 | 避免各组件重复实现 |

---

## 8. 相关文件索引

- 后端错误类型：[`src-tauri/src/error.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/error.rs)
- 前端错误类型：[`src/core/errors.ts`](file:///d:/ProjectApp/i-code/src/core/errors.ts)
- Command 封装：[`src/hooks/use-command.ts`](file:///d:/ProjectApp/i-code/src/hooks/use-command.ts)
- Toast 组件：[`src/components/ui/sonner.tsx`](file:///d:/ProjectApp/i-code/src/components/ui/sonner.tsx)
- 日志服务：[`src-tauri/src/modules/logger/service.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/modules/logger/service.rs)
- 日志 Command：[`src-tauri/src/modules/logger/commands.rs`](file:///d:/ProjectApp/i-code/src-tauri/src/modules/logger/commands.rs)
- 事件文档：[`docs/events.md`](file:///d:/ProjectApp/i-code/docs/events.md)
