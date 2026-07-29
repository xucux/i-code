# 日志框架迁移：`tauri-plugin-log` → `tracing` + `tracing-subscriber`

> 将 `tauri-plugin-log` 替换为 `tracing` 生态，获得请求级 span 上下文、结构化日志、`tower-http` `TraceLayer` 集成。
> 评估结论见 `docs/plan/log-migration-evaluation.md`（如已生成）。

---

## 1. 目标

1. 移除 `tauri-plugin-log` 依赖，改用 `tracing` + `tracing-subscriber` + `tracing-appender`
2. 保持现有 **180 处** `log::` 宏调用（跨 26 个文件，2026-07-29 实测）**零改动**继续工作（通过 `tracing` 的 `log` feature 桥接）
3. 获得全操作链路 trace_id（网关请求 / Tauri Command / 定时任务 / 托盘菜单 / 模块启动），便于关联同一操作的多条日志
4. trace_id 采用雪花 ID 转 32 进制（13 字符，趋势递增），替代 UUID
5. 集成 `tower-http::TraceLayer` 到网关路由
6. 保留 WebView 控制台日志输出（DevTools 可见）
7. 保留运行时日志级别动态控制

> **说明**：`tracing` 默认不提供跨服务分布式 trace ID。本迁移通过雪花 ID 生成器（§4.3c）+ 自定义 Layer/FormatEvent（§4.3b）+ 操作入口注入（§4.3d）实现「单进程内全操作链路 trace_id」，覆盖网关请求、Tauri Command、定时任务、托盘菜单、模块启动 5 类入口。如后续需要跨服务分布式 trace ID，再评估 `tracing-opentelemetry`。

---

## 2. 依赖变更

### 2.1 移除

```toml
# Cargo.toml — 删除
tauri-plugin-log = "2.8.0"
```

### 2.2 新增

```toml
# Cargo.toml — 新增
tracing = { version = "0.1", features = ["log"] }        # log feature 桥接 log:: 宏
tracing-subscriber = { version = "0.3", features = ["registry", "env-filter"] }
tracing-appender = "0.2"                                   # 文件滚动
```

> **feature 说明**：
> - `registry`：必需，提供 `Layer` 组合能力
> - `env-filter`：启用 `EnvFilter`，开发时可用 `RUST_LOG=debug` 覆盖默认级别
> - 未启用 `json`：示例使用 `fmt::layer().compact()`，未用 JSON 输出；如后续需要机器可读日志再启用

### 2.3 依赖关系

```
log::info! 等（180 处，零改动）
    │
    └─→ tracing（log feature 自动捕获为 tracing::Event）
            │
            ├─→ AtomicLevelFilter          → 全局级别过滤（运行时可调）
            ├─→ tracing-subscriber::fmt    → stdout（开发终端）
            ├─→ tracing-appender           → 文件按天滚动（日志目录，最多 30 个）
            └─→ WebViewLayer（自定义）     → 前端 DevTools 控制台
```

---

## 3. 核心改动清单

### 3.1 Phase 1：核心替换

| # | 文件 | 改动内容 | 工作量 |
|---|------|----------|--------|
| 1 | `src-tauri/Cargo.toml` | 删 `tauri-plugin-log`，加 `tracing` / `tracing-subscriber` / `tracing-appender` | 5 行 |
| 2 | `src-tauri/src/core/atomic_filter.rs` | **新建**：`AtomicLevelFilter`（自定义 `Filter`，运行时可调级别） | ~50 行 |
| 3 | `src-tauri/src/core/trace_id.rs` | **新建**：`TraceIdGenerator` + 32 进制编码 + 全局生成器（雪花 ID，详见 §4.3c） | ~110 行 |
| 4 | `src-tauri/src/core/trace_id_layer.rs` | **新建**：`TraceIdLayer` + `TraceIdFormat` + thread-local + `enter_operation`/`enter_operation_async`（详见 §4.3b、§4.3d） | ~160 行 |
| 5 | `src-tauri/src/core/size_aware_appender.rs` | **新建**：`SizeAwareFileAppender`（按天 + 按大小双维度滚动，详见 §4.3e） | ~120 行 |
| 6 | `src-tauri/src/main.rs` | 替换 `tauri_plugin_log::Builder` 为 `tracing-subscriber` 初始化；注册 `WorkerGuard`、`Arc<AtomicLevelFilter>`、`TraceIdLayer`、`TraceIdFormat`；setup 入口 `enter_operation("startup")` | ~60 行 |
| 7 | `src-tauri/src/modules/settings/types.rs` | 新增 `to_tracing_level()` 方法 | ~10 行 |
| 8 | `src-tauri/src/modules/settings/commands.rs` | `log::set_max_level()` → `AtomicLevelFilter::set_level()`；Command 入口 `enter_operation_async` | ~10 行 |
| 9 | `docs/log-framework.md` | 更新文档 | — |

### 3.2 Phase 2：WebView 转发

| # | 文件 | 改动内容 | 工作量 |
|---|------|----------|--------|
| 7 | `src-tauri/src/modules/tracing_webview.rs` | **新建**：自定义 `tracing` Layer 转发到 WebView | ~90 行 |
| 8 | `src-tauri/src/main.rs` | 注册 `WebViewLayer` 到 subscriber | ~5 行 |
| 9 | `src/main.tsx` 或 `src/core/events.ts` | 前端监听 `console:log` 事件并打印到 DevTools | ~25 行 |

### 3.3 Phase 3：`tower-http` `TraceLayer` 集成

| # | 文件 | 改动内容 | 工作量 |
|---|------|----------|--------|
| 10 | `src-tauri/src/modules/gateway_runtime/router.rs` | 添加 `TraceLayer` 中间件（放在认证中间件外层） | ~10 行 |

> **不再升级 `TauriLogEmitter`**：原 §4.8 提议将 `TauriLogEmitter` 的 `log::info!` 包裹在 `tracing::info_span!` 中，但该记录器只在 `LogPipeline::record` 内同步调用一次，包裹 span 无跨函数传播价值，且 `log::info!` 经 `log` feature 已自动转为 tracing event。初次迁移阶段保持原样，避免无收益的风险引入。

### 3.4 Phase 4（可选）：增量升级

| # | 文件 | 改动内容 | 工作量 |
|---|------|----------|--------|
| 11 | 各模块（按需） | `log::info!` → `tracing::info!` + 结构化字段 | 渐进式 |

---

## 4. 详细实现

### 4.1 Phase 1：`Cargo.toml`

```toml
# 删除
# tauri-plugin-log = "2.8.0"
# log = "0.4"  ← 保留！tracing 的 log feature 依赖它

# 新增
tracing = { version = "0.1", features = ["log"] }
tracing-subscriber = { version = "0.3", features = ["registry", "env-filter"] }
tracing-appender = "0.2"
```

### 4.2 Phase 1：`main.rs` 初始化替换

#### 改动点 1：删除 import

```rust
// 删除
use tauri_plugin_log::{Target, TargetKind};
```

#### 改动点 2：替换 `plugin()` 调用为 `setup()` 内初始化

```rust
// 删除整个 .plugin(tauri_plugin_log::Builder::new()...) block
// 注意：实际代码中还含 .max_file_size(1024 * 1024 * 20) 与
//       .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepAll) 两行，
//       回滚时务必一并恢复（见 §6）

// 在 .setup(|app| { ... }) 回调最前面插入 init_tracing：
fn init_tracing(app: &tauri::App) {
    use std::sync::Arc;
    use tracing_subscriber::prelude::*;
    use crate::core::atomic_filter::AtomicLevelFilter;

    // 1. 日志目录
    let log_dir = app.path().app_log_dir().expect("无法获取日志目录");
    std::fs::create_dir_all(&log_dir).ok();

    // 2. 文件滚动：按天 + 按大小（20MB）双维度滚动
    //    tracing-appender 原生仅支持按时间滚动，自定义 SizeAwareFileAppender 实现：
    //    - 按天创建文件：i-code-2026-07-29.log
    //    - 超过 20MB 分片：i-code-2026-07-29.1.log、i-code-2026-07-29.2.log
    //    - 保留最近 30 天文件，超期自动清理
    //    详见 §4.3e
    let file_appender = crate::core::size_aware_appender::SizeAwareFileAppender::new(
        &log_dir,
        "i-code",
        "log",
        20 * 1024 * 1024,  // 20MB
        30,                // 保留 30 天
    ).expect("构建日志文件 appender 失败");

    // 3. non_blocking 包装：返回的 guard 必须 Send + 'static 保活，
    //    否则后台写入线程在 setup 返回时立即终止，文件日志全部丢失。
    //    将 guard 注册为 Tauri State 以保活到应用退出。
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    app.manage(WorkerGuardState(guard));

    // 4. 全局级别过滤：运行时可通过 AtomicLevelFilter::set_level 调整
    //    用 Arc 包装以便同时注册到 subscriber 与 Tauri State
    let atomic_filter = Arc::new(AtomicLevelFilter::new(tracing::Level::INFO));

    // 5. EnvFilter 兜底：开发时可用 RUST_LOG=debug 临时覆盖
    //    未设置 RUST_LOG 时回退到 info 级别
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    // 6. fmt layer：使用自定义 FormatEvent（TraceIdFormat），
    //    在每行日志前注入当前请求的 trace_id（若处于请求 span 内）。
    //    TraceIdLayer 负责 span 进入/退出时维护 thread-local trace_id。
    //    详见 §4.3b。
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .compact()
        .with_event_format(crate::core::trace_id::TraceIdFormat);

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)              // 先经 EnvFilter（开发覆盖）
        .with(atomic_filter.clone())   // 再经运行时可调级别
        .with(crate::core::trace_id::TraceIdLayer) // 维护 thread-local trace_id
        .with(fmt_layer.with_writer(std::io::stdout))
        .with(
            fmt_layer
                .with_writer(file_writer)
                .with_ansi(false)      // 文件不输出 ANSI 颜色
        );

    tracing::subscriber::set_global_default(subscriber)
        .expect("设置全局 tracing subscriber 失败");

    // 7. 注册到 Tauri State，供 settings 模块运行时调整级别
    app.manage(atomic_filter);
}

/// 保活 tracing_appender::non_blocking 的 WorkerGuard
/// 必须注册为 Tauri State，存活到应用退出
pub struct WorkerGuardState(pub tracing_appender::non_blocking::WorkerGuard);
```

> **执行顺序要点**：
> - `tauri::Builder::plugin()` 在 `.setup()` 之前执行，但 `tracing-subscriber` 初始化必须在 `setup()` 中完成（需要 `app.path()` 获取日志目录）。
> - 当前 [main.rs](../../src-tauri/src/main.rs) 的 `setup` 闭包中，DB 初始化后立即 `log::info!("数据库初始化完成")`。**必须把 `init_tracing(app)` 放在 DB 初始化之前**，否则该日志会被丢弃。
> - 启动期（`main()` 入口 → `.setup()` 之间）的日志会被丢弃，但 i-code 启动期无关键业务日志，可接受。

#### 改动点 3：`log::set_max_level` → `AtomicLevelFilter::set_level`

```rust
// main.rs 中 setup 回调内的设置初始化部分：

// 原来
log::set_max_level(level_filter);
log::info!("Settings 模块初始化完成，日志级别：{}", settings.log_level.as_str());

// 替换为
let atomic_filter = app.state::<Arc<AtomicLevelFilter>>();
atomic_filter.set_level(settings.log_level.to_tracing_level());
log::info!("Settings 模块初始化完成，日志级别：{}", settings.log_level.as_str());
```

**调整后的初始化顺序**：

```rust
.setup(|app| {
    // 0. 先初始化 tracing-subscriber（仅需 app.path()，不依赖 DB）
    init_tracing(app);

    // 1. 初始化数据库
    let app_config_dir = app.path().app_config_dir().expect("无法获取应用配置目录");
    let db_path = app_config_dir.join("i-code.db");
    db::init_db_pool(&db_path).expect("数据库连接池初始化失败");
    db::run_migrations().expect("数据库迁移执行失败");
    log::info!("数据库初始化完成：{}", db_path.display());

    // 2. 初始化 Settings，并应用用户日志级别（覆盖 INFO 默认值）
    let settings_handle = modules::settings::SettingsServiceHandle::new();
    match settings_handle.service().get_settings() {
        Ok(settings) => {
            app.state::<Arc<AtomicLevelFilter>>()
                .set_level(settings.log_level.to_tracing_level());
            log::info!("Settings 模块初始化完成，日志级别：{}", settings.log_level.as_str());
        }
        Err(e) => {
            log::warn!("读取设置失败，使用默认日志级别 Info：{}", e.message);
        }
    }
    app.manage(settings_handle.clone());

    // 3. ... 其余模块初始化（log::info! 此时已被 tracing 捕获）
    Ok(())
})
```

### 4.3c Phase 1：`trace_id` 生成器（雪花 ID 转 32 进制）

**新建文件**：`src-tauri/src/core/trace_id.rs`

> **设计目标**：生成趋势递增、紧凑的 trace_id，替代 UUID v4。
>
> **方案**：简化版雪花算法（单机，无需分布式 worker ID 协调）：
> - 41 位毫秒时间戳（约 69 年）
> - 10 位机器 ID（桌面应用固定为 1，或用 PID 取模）
> - 12 位序列号（同毫秒内递增，每毫秒 4096 个）
> - 64 位整数转 32 进制（字符集 `0-9a-v`），最多 13 字符
>
> **优势对比 UUID**：
> | 维度 | UUID v4 | 雪花 ID 转 32 进制 |
> |------|---------|-------------------|
> | 长度 | 36 字符 | 13 字符 |
> | 排序 | 随机 | 趋势递增 |
> | 依赖 | uuid crate | 纯标准库 |

```rust
//! # trace_id 生成器（雪花 ID 转 32 进制）
//!
//! 简化版雪花算法，生成趋势递增的 64 位整数，转 32 进制字符串。
//! 用于网关请求、Tauri Command、定时任务、托盘菜单、模块启动等所有操作入口的链路追踪。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// 32 进制字符集：0-9a-v
const BASE32_CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuv";

/// 简化版雪花 ID 生成器（单机，无需 worker ID 协调）
///
/// 结构：[41 位毫秒时间戳] [10 位机器 ID] [12 位序列号]
/// 转成 32 进制后约 13 字符，趋势递增，便于日志按时间近似排序。
pub struct TraceIdGenerator {
    /// 机器 ID 左移 12 位（10 位机器 ID << 12）
    machine_id_shifted: u64,
    /// 上次生成的时间戳（毫秒）
    last_timestamp: AtomicU64,
    /// 同毫秒内序列号
    sequence: AtomicU64,
}

impl TraceIdGenerator {
    /// 创建生成器，machine_id 取低 10 位
    pub fn new(machine_id: u16) -> Self {
        Self {
            machine_id_shifted: ((machine_id & 0x3FF) as u64) << 12,
            last_timestamp: AtomicU64::new(0),
            sequence: AtomicU64::new(0),
        }
    }

    /// 生成下一个 64 位整数 ID
    pub fn next_id(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // CAS 循环确保同毫秒内序列号递增
        loop {
            let last = self.last_timestamp.load(Ordering::Relaxed);
            if now > last {
                // 新毫秒，重置序列号
                if self
                    .last_timestamp
                    .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
                {
                    self.sequence.store(0, Ordering::Relaxed);
                    break;
                }
            } else if now == last {
                // 同毫秒，序列号递增
                let seq = self.sequence.fetch_add(1, Ordering::Relaxed);
                if seq < 0xFFF {
                    break;
                }
                // 序列号耗尽，自旋等待下一毫秒（桌面应用几乎不会触发）
                continue;
            } else {
                // 时钟回拨，用 last_timestamp 保证递增
                break;
            }
        }

        let ts = self.last_timestamp.load(Ordering::Relaxed);
        let seq = self.sequence.load(Ordering::Relaxed) & 0xFFF;
        (ts << 22) | self.machine_id_shifted | seq
    }

    /// 生成 32 进制字符串（小写 0-9a-v），最多 13 字符
    pub fn next_trace_id(&self) -> String {
        encode_base32(self.next_id())
    }
}

/// 64 位整数转 32 进制字符串
fn encode_base32(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut buf = [0u8; 13];
    let mut i = 12;
    while n > 0 {
        buf[i] = BASE32_CHARS[(n & 0x1F) as usize];
        n >>= 5;
        i -= 1;
    }
    String::from_utf8(&buf[i + 1..]).unwrap()
}

/// 全局生成器实例（machine_id 固定为 1，桌面应用无需区分多实例）
static GLOBAL_GENERATOR: TraceIdGenerator = TraceIdGenerator::new(1);

/// 生成新的 trace_id（32 进制字符串，约 13 字符）
pub fn next_trace_id() -> String {
    GLOBAL_GENERATOR.next_trace_id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_base32() {
        assert_eq!(encode_base32(0), "0");
        assert_eq!(encode_base32(1), "1");
        assert_eq!(encode_base32(31), "v");
        assert_eq!(encode_base32(32), "10");
        // i64::MAX
        let s = encode_base32(u64::MAX);
        assert_eq!(s.len(), 13);
    }

    #[test]
    fn test_next_trace_id_unique() {
        let gen = TraceIdGenerator::new(1);
        let id1 = gen.next_trace_id();
        let id2 = gen.next_trace_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_next_trace_id_monotonic() {
        let gen = TraceIdGenerator::new(1);
        let id1 = gen.next_id();
        let id2 = gen.next_id();
        assert!(id2 >= id1, "trace_id 应趋势递增");
    }
}
```

**trace_id 示例**：
```
1v2u0j3w4x5y   (13 字符，趋势递增)
2w3v4u5x6y7z   (后续生成的更大)
```

> **机器 ID 取值**：桌面应用固定为 1。若未来支持多实例（如多用户模式），可改用 `std::process::id() & 0x3FF`。
>
> **时钟回拨处理**：若检测到时钟回拨（`now < last`），沿用 `last_timestamp` 保证 ID 递增。桌面应用极少触发。
>
> **性能**：纯原子操作，无锁，单次生成 < 100ns。

### 4.3b Phase 1：`TraceIdFormat` 与 `TraceIdLayer`（trace ID 注入）

**新建文件**：`src-tauri/src/core/trace_id_layer.rs`

> **设计目标**：让所有操作入口（网关请求、Tauri Command、定时任务、托盘菜单、模块启动）的日志行带 `[tid=xxxxxxxxxxxxx]` 前缀，便于关联同一操作链路的多条日志。
>
> **方案**：
> - `TraceIdLayer`（自定义 `Layer`）：在 span 进入时把 `trace_id` 字段值写入 thread-local，退出时清除。
> - `TraceIdFormat`（自定义 `FormatEvent`）：格式化日志行时读取 thread-local，写入前缀。
> - `MakeSpan`（§4.7）在 `TraceLayer` 创建 span 时注入 `trace_id` 字段。
>
> **覆盖范围**：所有操作入口（网关请求、Tauri Command、定时任务、托盘菜单、模块启动）均通过 §4.3d 的 `enter_operation()` 或 `tracing::instrument` 注入 trace_id 到 span，本 Layer 从 span 读取并写入日志前缀。未在操作 span 内的日志（极少数）不带前缀。

```rust
//! # trace ID（trace_id）注入 Layer
//!
//! 通过 thread-local + 自定义 Layer/FormatEvent，让所有操作入口的日志行
//! 带 `[tid=...]` 前缀，便于关联同一操作链路的多条日志。
//!
//! 工作流：
//! 1. 各操作入口（§4.3d）创建 span，注入 `trace_id` 字段（值来自 §4.3c 生成器）
//! 2. `TraceIdLayer::on_enter` 在 span 进入时把 `trace_id` 写入 thread-local
//! 3. `TraceIdFormat::format_event` 格式化日志行时读 thread-local，写前缀
//! 4. `TraceIdLayer::on_exit` 在 span 退出时清除 thread-local

use std::cell::RefCell;
use std::fmt;

use tracing::{field, Event, Subscriber};
use tracing_subscriber::fmt::{FormatEvent, FormatFields, FmtContext};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;
use tracing_subscriber::registry::LookupSpan;

thread_local! {
    /// 当前线程活跃的 trace_id（在操作 span 内有值）
    static CURRENT_TRACE_ID: RefCell<Option<String>> = RefCell::new(None);
}

/// 读取当前线程的 trace_id（供 WebViewLayer 等外部消费者使用）
pub fn current_trace_id() -> Option<String> {
    CURRENT_TRACE_ID.with(|r| r.borrow().clone())
}

/// 在 span 进入/退出时维护 thread-local trace_id
pub struct TraceIdLayer;

impl<S: Subscriber> Layer<S> for TraceIdLayer {
    fn on_enter(&self, id: &tracing::Id, ctx: Context<'_, S>) {
        // 从 span 中读取 trace_id 字段值
        if let Some(span) = ctx.span(id) {
            let mut visitor = TraceIdVisitor { value: None };
            span.records(&mut visitor);
            if let Some(rid) = visitor.value {
                CURRENT_TRACE_ID.with(|r| *r.borrow_mut() = Some(rid));
            }
        }
    }

    fn on_exit(&self, _id: &tracing::Id, _ctx: Context<'_, S>) {
        CURRENT_TRACE_ID.with(|r| *r.borrow_mut() = None);
    }
}

/// 从 span records 中提取 trace_id 字段
struct TraceIdVisitor {
    value: Option<String>,
}

impl field::Visit for TraceIdVisitor {
    fn record_str(&mut self, field: &field::Field, value: &str) {
        if field.name() == "trace_id" {
            self.value = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &field::Field, value: &dyn fmt::Debug) {
        if field.name() == "trace_id" {
            self.value = Some(format!("{:?}", value));
        }
    }
}

/// 自定义日志格式化器：在默认 compact 格式前注入 `[tid=...]` 前缀
pub struct TraceIdFormat;

impl<S, N> FormatEvent<S, N> for TraceIdFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: tracing_subscriber::fmt::Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let meta = event.metadata();

        // 1. 时间戳（本地时区）
        write!(
            writer,
            "{} ",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f")
        )?;

        // 2. 级别
        write!(writer, "{:5} ", meta.level())?;

        // 3. trace_id 前缀（若处于请求 span 内）
        if let Some(rid) = current_trace_id() {
            write!(writer, "[tid={}] ", rid)?;
        }

        // 4. target
        write!(writer, "{} ", meta.target())?;

        // 5. file:line
        if let (Some(file), Some(line)) = (meta.file(), meta.line()) {
            // 仅保留文件名，避免完整路径过长
            let file = file.rsplit('/').next().unwrap_or(file);
            write!(writer, "{}:{} ", file, line)?;
        }

        // 6. 消息字段（委托给默认 FormatFields）
        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}
```

**日志行示例**：

```
# 网关请求（带 tid，13 字符雪花 ID 转 32 进制）
2026-07-29 10:23:45.123  INFO i_code::modules::gateway_runtime::router router.rs:108 [tid=1v2u0j3w4x5y] Gateway | POST /v1/chat/completions | status=200 | duration=1234ms

# Tauri Command 调用（带 tid）
2026-07-29 10:23:46.456  INFO i_code::modules::balance::commands commands.rs:42 [tid=2w3v4u5x6y7z] Command: balance_refresh_manual | provider=openai

# 模块启动（带 tid）
2026-07-29 10:23:47.789  INFO i_code::db main.rs:525 [tid=3x4y5z6a7b8c] 数据库初始化完成：/path/to/i-code.db

# 极少数未在操作 span 内的日志（无 tid）
2026-07-29 10:23:47.790  DEBUG i_code::core::atomic_filter atomic_filter.rs:30 filter level set to INFO
```

> **与 `chrono` 依赖**：项目已依赖 `chrono = "0.4"`（[Cargo.toml#L69](../../src-tauri/Cargo.toml#L69)），无需新增。
>
> **关于 `compact()` 的取舍**：`with_event_format(TraceIdFormat)` 会替换 `fmt::layer().compact()` 的默认格式化器。`TraceIdFormat` 已自行实现紧凑格式（时间戳 + 级别 + rid + target + file:line + 消息），不依赖 `compact()`。若希望保留 `compact()` 的某些行为（如 ANSI 颜色），可在 `TraceIdFormat` 中按需补充。

### 4.3d Phase 1：操作入口 trace_id 注入（全链路覆盖）

> **设计目标**：让所有操作入口的日志带 `[tid=...]` 前缀，覆盖网关请求、Tauri Command、定时任务、托盘菜单、模块启动 5 类入口，实现操作链路追踪。
>
> **核心机制**：每个操作入口创建一个 `tracing::info_span!`，注入 `trace_id` 字段（值来自 §4.3c `next_trace_id()`）。`TraceIdLayer`（§4.3b）从 span 读取并写入 thread-local，`TraceIdFormat` 写入日志前缀。

#### 辅助函数：`enter_operation`（同步入口用）

**位置**：`src-tauri/src/core/trace_id_layer.rs`（与 §4.3b 同文件）

```rust
/// 操作入口 guard：生成 trace_id 并进入操作 span
///
/// 用于同步入口（托盘菜单、模块启动）。drop 时自动退出 span 并清除 thread-local。
///
/// 用法：
/// ```ignore
/// fn on_tray_menu_click() {
///     let _guard = enter_operation("tray:balance_refresh");
///     // 此处所有 log::info! 自动带 [tid=xxx] 前缀
/// }
/// ```
pub struct OperationGuard {
    _span: tracing::span::EnteredSpan,
}

pub fn enter_operation(name: &'static str) -> OperationGuard {
    let trace_id = crate::core::trace_id::next_trace_id();
    let span = tracing::info_span!(
        "operation",
        trace_id = %trace_id,
        op = %name,
    );
    OperationGuard { _span: span.enter() }
}
```

#### async 入口：`tracing::instrument` + 手动 trace_id

Tauri Command 与定时任务是 async，thread-local 跨 await 点可能丢失。用 `tracing::instrument` + `Instrument` 保证 span 跟随 task。

**方案 A：`#[tracing::instrument]` 宏 + 手动 trace_id 字段**

```rust
use tracing::Instrument;
use crate::core::trace_id::next_trace_id;

#[tauri::command]
pub async fn balance_refresh_manual(
    state: State<'_, BalanceServiceHandle>,
    app_handle: AppHandle,
) -> IcodeResult<()> {
    let trace_id = next_trace_id();
    let span = tracing::info_span!(
        "operation",
        trace_id = %trace_id,
        op = "balance_refresh_manual",
    );
    async move {
        log::info!("开始手动刷新额度");
        state.service().refresh_all(&app_handle)?;
        log::info!("额度刷新完成");
        Ok(())
    }
    .instrument(span)
    .await
}
```

**方案 B：封装 async helper（减少样板代码）**

```rust
/// async 操作入口包装器：生成 trace_id 并在 span 内执行 future
///
/// 用于 Tauri Command、定时任务等 async 入口。
///
/// 用法：
/// ```ignore
/// #[tauri::command]
/// pub async fn settings_update(...) -> IcodeResult<...> {
///     enter_operation_async("settings_update", async {
///         // 业务逻辑
///         Ok(dto)
///     }).await
/// }
/// ```
pub async fn enter_operation_async<F, T>(name: &'static str, fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    let trace_id = crate::core::trace_id::next_trace_id();
    let span = tracing::info_span!(
        "operation",
        trace_id = %trace_id,
        op = %name,
    );
    fut.instrument(span).await
}
```

#### 各入口点使用示例

| 入口类型 | 使用方式 | 示例 |
|----------|----------|------|
| 网关请求 | `MakeSpan`（§4.7） | `.layer(TraceLayer::new_for_http().make_span_with(TraceIdSpan))` |
| Tauri Command | `enter_operation_async` | `enter_operation_async("settings_update", async { ... }).await` |
| 定时任务 | `enter_operation_async` | `enter_operation_async("scheduled:balance_refresh", async { ... }).await` |
| 托盘菜单 | `enter_operation`（同步） | `let _guard = enter_operation("tray:open_mini_panel");` |
| 模块启动 | `enter_operation`（同步） | `let _guard = enter_operation("startup");` |

**模块启动示例**（[main.rs](../../src-tauri/src/main.rs) `.setup`）：

```rust
.setup(|app| {
    // 0. 先初始化 tracing-subscriber
    init_tracing(app);

    // 0.1 进入 startup 操作 span，所有启动日志带同一 trace_id
    let _startup_guard = crate::core::trace_id_layer::enter_operation("startup");

    // 1. 初始化数据库
    let db_path = ...;
    db::init_db_pool(&db_path).expect("数据库连接池初始化失败");
    log::info!("数据库初始化完成：{}", db_path.display());

    // 2. 初始化 Settings
    // ...

    // 3. 其余模块初始化（所有 log::info! 带 [tid=xxx]）
    Ok(())
})
```

**定时任务示例**：

```rust
// 定时刷新额度
fn spawn_scheduled_balance_refresh(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        crate::core::trace_id_layer::enter_operation_async(
            "scheduled:balance_refresh",
            async move {
                log::info!("定时额度刷新开始");
                // ... 刷新逻辑
                log::info!("定时额度刷新完成");
            },
        )
        .await;
    });
}
```

**托盘菜单示例**：

```rust
// 托盘菜单点击「刷新额度」
let refresh_item = MenuItem::with_id(app, "refresh_balance", "刷新额度", true, None)?;
app.on_menu_event(move |_app, event| {
    if event.id() == "refresh_balance" {
        let _guard = crate::core::trace_id_layer::enter_operation("tray:refresh_balance");
        log::info!("用户点击托盘「刷新额度」");
        // ... 触发刷新
    }
});
```

> **async 与 thread-local 的关键说明**：
> - `enter_operation_async` 通过 `tracing::Instrument` 让 span 跟随 task，跨 await 点保持。
> - `TraceIdLayer::on_enter/on_exit` 在 span 进入/退出时维护 thread-local。async task 在 poll 时进入 span，此时 thread-local 被设置；await 挂起时 span 退出，thread-local 被清除；resume 时再次进入，thread-local 重新设置。
> - 因此 async 路径的 `log::info!` 在 poll 期间能正确读到 thread-local，无需额外处理。
> - **注意**：若 async 函数内部 `tokio::spawn` 了子任务，子任务不会继承父 span，需显式 `.instrument(span.clone())` 或在子任务内重新 `enter_operation_async`。

### 4.3e Phase 1：`SizeAwareFileAppender`（按天 + 按大小双维度滚动）

**新建文件**：`src-tauri/src/core/size_aware_appender.rs`

> **设计目标**：`tracing-appender` 原生仅支持按时间滚动（`Hourly`/`Daily`/`Never`），不支持按大小。本 appender 实现「按天 + 超过 20MB 分文件」双维度滚动，避免单日日志过大。
>
> **文件命名规则**：
> ```
> i-code-2026-07-29.log        # 当天首个文件
> i-code-2026-07-29.1.log      # 超过 20MB 后
> i-code-2026-07-29.2.log      # 再次超过
> i-code-2026-07-30.log        # 跨天切换，序号重置
> ```
>
> **旧文件清理**：启动时扫描日志目录，删除超过 `max_days` 天的文件。

```rust
//! # 按天 + 按大小双维度滚动的文件 appender
//!
//! `tracing-appender` 原生仅支持按时间滚动，本 appender 额外支持按大小分片：
//! - 按天创建文件：i-code-YYYY-MM-DD.log
//! - 超过 max_size 分片：i-code-YYYY-MM-DD.1.log、.2.log
//! - 启动时清理超过 max_days 天的旧文件

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use chrono::{Datelike, Local};

/// 按天 + 按大小滚动的文件 appender
pub struct SizeAwareFileAppender {
    inner: Mutex<Inner>,
}

struct Inner {
    log_dir: PathBuf,
    prefix: String,
    suffix: String,
    max_size: u64,
    max_days: u32,
    current_file: Option<File>,
    current_size: u64,
    current_date: String,
    current_seq: u32,
}

impl SizeAwareFileAppender {
    /// 创建 appender
    ///
    /// - `log_dir`：日志目录
    /// - `prefix`：文件名前缀（如 "i-code"）
    /// - `suffix`：文件名后缀（如 "log"）
    /// - `max_size`：单文件最大字节数（如 20 * 1024 * 1024 = 20MB）
    /// - `max_days`：保留天数（如 30）
    pub fn new(
        log_dir: impl AsRef<Path>,
        prefix: &str,
        suffix: &str,
        max_size: u64,
        max_days: u32,
    ) -> io::Result<Self> {
        let log_dir = log_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&log_dir)?;

        // 启动时清理超过 max_days 的旧文件
        Self::cleanup_old_files(&log_dir, prefix, suffix, max_days)?;

        let today = today_string();
        let path = Self::build_path(&log_dir, prefix, &today, 0, suffix);
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let size = file.metadata()?.len();

        Ok(Self {
            inner: Mutex::new(Inner {
                log_dir,
                prefix: prefix.to_string(),
                suffix: suffix.to_string(),
                max_size,
                max_days,
                current_file: Some(file),
                current_size: size,
                current_date: today,
                current_seq: 0,
            }),
        })
    }

    /// 构建文件路径
    /// seq=0 时：prefix-YYYY-MM-DD.suffix
    /// seq>0 时：prefix-YYYY-MM-DD.N.suffix
    fn build_path(dir: &Path, prefix: &str, date: &str, seq: u32, suffix: &str) -> PathBuf {
        if seq == 0 {
            dir.join(format!("{}-{}.{}", prefix, date, suffix))
        } else {
            dir.join(format!("{}-{}.{}.{}", prefix, date, seq, suffix))
        }
    }

    /// 清理超过 max_days 天的旧文件
    fn cleanup_old_files(dir: &Path, prefix: &str, suffix: &str, max_days: u32) -> io::Result<()> {
        let cutoff = SystemTime::now() - Duration::from_secs(max_days as u64 * 86400);
        if !dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            // 仅清理匹配 prefix-*.suffix 的文件
            if !name.starts_with(&format!("{}-", prefix)) || !name.ends_with(&format!(".{}", suffix)) {
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                if let Ok(mtime) = meta.modified() {
                    if mtime < cutoff {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
        Ok(())
    }

    /// 检查并执行滚动（跨天或超大小）
    fn rotate_if_needed(&self, inner: &mut Inner, write_len: usize) -> io::Result<()> {
        let today = today_string();

        // 跨天：重置序号，创建新日期文件
        if today != inner.current_date {
            inner.current_date = today.clone();
            inner.current_seq = 0;
            let path = Self::build_path(&inner.log_dir, &inner.prefix, &today, 0, &inner.suffix);
            inner.current_file = Some(OpenOptions::new().create(true).append(true).open(&path)?);
            inner.current_size = inner.current_file.as_ref().unwrap().metadata()?.len();
        }

        // 超大小：序号 +1，创建分片文件
        if inner.current_size + write_len as u64 > inner.max_size {
            inner.current_seq += 1;
            let path = Self::build_path(
                &inner.log_dir,
                &inner.prefix,
                &inner.current_date,
                inner.current_seq,
                &inner.suffix,
            );
            inner.current_file = Some(OpenOptions::new().create(true).append(true).open(&path)?);
            inner.current_size = 0;
        }

        Ok(())
    }
}

impl Write for SizeAwareFileAppender {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().map_err(|_| io::Error::new(io::ErrorKind::Other, "mutex poisoned"))?;
        self.rotate_if_needed(&mut inner, buf.len())?;
        if let Some(ref mut file) = inner.current_file {
            let written = file.write(buf)?;
            inner.current_size += written as u64;
            Ok(written)
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "no file"))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut inner = self.inner.lock().map_err(|_| io::Error::new(io::ErrorKind::Other, "mutex poisoned"))?;
        if let Some(ref mut file) = inner.current_file {
            file.flush()
        } else {
            Ok(())
        }
    }
}

/// 获取当天日期字符串（YYYY-MM-DD，本地时区）
fn today_string() -> String {
    let now = Local::now();
    format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_build_path_seq0() {
        let dir = Path::new("/tmp");
        let path = SizeAwareFileAppender::build_path(dir, "i-code", "2026-07-29", 0, "log");
        assert_eq!(path.file_name().unwrap(), "i-code-2026-07-29.log");
    }

    #[test]
    fn test_build_path_seq1() {
        let dir = Path::new("/tmp");
        let path = SizeAwareFileAppender::build_path(dir, "i-code", "2026-07-29", 1, "log");
        assert_eq!(path.file_name().unwrap(), "i-code-2026-07-29.1.log");
    }

    #[test]
    fn test_size_rollover() {
        let tmp = TempDir::new().unwrap();
        // max_size=100 字节，触发大小滚动
        let mut appender = SizeAwareFileAppender::new(tmp.path(), "test", "log", 100, 30).unwrap();
        // 写入 150 字节，应触发分片
        let data = vec![b'x'; 150];
        appender.write_all(&data).unwrap();
        appender.flush().unwrap();
        // 验证存在分片文件
        let files: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().count();
        assert!(files >= 2, "应存在至少 2 个文件（原文件 + 分片）");
    }
}
```

> **依赖**：
> - `chrono = "0.4"`（已存在，用于日期）
> - `tempfile = "3"`（dev-dependencies，仅测试用）
>
> **与 `tracing_appender::non_blocking` 的兼容性**：
> `SizeAwareFileAppender` 实现 `std::io::Write + Send + 'static`，可直接传入 `tracing_appender::non_blocking()`。`non_blocking` 会启动后台线程，所有写入异步化，不阻塞日志热路径。
>
> **性能**：
> - 每次 `write` 加锁 `Mutex`，持锁时间极短（仅文件写入）
> - `non_blocking` 后台线程单线程消费，无并发竞争
> - 文件大小检查为 O(1)（内存计数器），无 IO 开销
>
> **边界情况**：
> - 进程重启后，`new()` 会打开当天已有文件继续追加（`append(true)`），`current_size` 从 metadata 读取
> - 跨天但进程未重启：下次 `write` 时检测到日期变化，自动切换
> - 旧文件清理仅在 `new()` 时执行一次，运行时不重复扫描

### 4.3 Phase 1：`AtomicLevelFilter` 定义

**新建文件**：`src-tauri/src/core/atomic_filter.rs`

> **方案选型**：放弃 `tracing_subscriber::reload::Layer`，因为其泛型类型 `reload::Handle<LevelFilter, S>` 中的 `S` 取决于 subscriber 的 layer 组合方式，难以在 `main.rs` 与 `settings/commands.rs` 之间稳定传递。改用基于 `AtomicU8` 的自定义 `Filter`，类型简单（`Arc<AtomicLevelFilter>`），可直接注册为 Tauri State。

```rust
//! # 运行时可调级别过滤器
//!
//! 自定义 `tracing_subscriber::layer::Filter`，通过原子变量实现运行时级别调整。
//! 替代 `tracing_subscriber::reload::Layer`，避免其泛型类型在模块间难以传递的问题。

use std::sync::atomic::{AtomicU8, Ordering};

use tracing::Subscriber;
use tracing_subscriber::layer::{Context, Filter};

/// 运行时级别过滤（通过原子变量实现，避免 reload 泛型传播）
///
/// 使用方式：
/// ```ignore
/// let filter = Arc::new(AtomicLevelFilter::new(tracing::Level::INFO));
/// let subscriber = registry().with(filter.clone());
/// app.manage(filter); // 注册为 Tauri State
/// ```
#[derive(Debug)]
pub struct AtomicLevelFilter {
    level: AtomicU8,
}

impl AtomicLevelFilter {
    /// 创建指定级别的过滤器
    pub const fn new(level: tracing::Level) -> Self {
        Self {
            level: AtomicU8::new(level_to_u8(level)),
        }
    }

    /// 运行时调整级别（线程安全，立即对所有后续事件生效）
    pub fn set_level(&self, level: tracing::Level) {
        self.level.store(level_to_u8(level), Ordering::Relaxed);
    }

    /// 读取当前级别
    pub fn get_level(&self) -> tracing::Level {
        u8_to_level(self.level.load(Ordering::Relaxed))
    }
}

fn level_to_u8(level: tracing::Level) -> u8 {
    match level {
        tracing::Level::ERROR => 0,
        tracing::Level::WARN => 1,
        tracing::Level::INFO => 2,
        tracing::Level::DEBUG => 3,
        tracing::Level::TRACE => 4,
    }
}

fn u8_to_level(v: u8) -> tracing::Level {
    match v {
        0 => tracing::Level::ERROR,
        1 => tracing::Level::WARN,
        2 => tracing::Level::INFO,
        3 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    }
}

impl<S: Subscriber> Filter<S> for AtomicLevelFilter {
    fn enabled(&self, meta: &tracing::Metadata<'_>, _cx: &Context<'_, S>) -> bool {
        let max = self.level.load(Ordering::Relaxed);
        level_to_u8(*meta.level()) <= max
    }
}
```

> **类型注册说明**：`AtomicLevelFilter` 需以 `Arc<AtomicLevelFilter>` 形式同时注册到 subscriber（`.with(filter.clone())`）和 Tauri State（`app.manage(filter)`），settings 模块通过 `app_handle.state::<Arc<AtomicLevelFilter>>()` 获取。

### 4.4 Phase 1：`settings/types.rs` 新增方法

```rust
impl LogLevel {
    // ... 保留现有 to_level_filter()，供其他场景使用

    /// 转换为 tracing::Level，供 AtomicLevelFilter::set_level 使用
    pub fn to_tracing_level(&self) -> tracing::Level {
        match self {
            Self::Trace => tracing::Level::TRACE,
            Self::Debug => tracing::Level::DEBUG,
            Self::Info => tracing::Level::INFO,
            Self::Warn => tracing::Level::WARN,
            Self::Error => tracing::Level::ERROR,
        }
    }
}
```

### 4.5 Phase 1：`settings/commands.rs` 改动

```rust
use std::sync::Arc;
use crate::core::atomic_filter::AtomicLevelFilter;

#[tauri::command]
pub async fn settings_update(
    state: State<'_, SettingsServiceHandle>,
    app_handle: AppHandle,
    input: UpdateSettingsInput,
) -> IcodeResult<AppSettingsDto> {
    let dto = state.service().update_settings(input)?;
    // 实时应用日志级别变更：通过 Arc<AtomicLevelFilter> 调整全局过滤级别
    if let Some(atomic_filter) = app_handle.try_state::<Arc<AtomicLevelFilter>>() {
        atomic_filter.set_level(dto.log_level.to_tracing_level());
    }
    // 广播设置变更事件
    let _ = app_handle.emit("settings:changed", &dto.titlebar_info);
    Ok(dto)
}
```

### 4.6 Phase 2：WebView 转发 Layer

**新建文件**：`src-tauri/src/modules/tracing_webview.rs`

> **关键约束**：
> - `on_event` 会被 `log::info!` 桥接后调用，而 `log::info!` 大量出现在 `router.rs`、`tauri_emitter.rs` 的 **async 函数**中（网关请求热路径）。
> - **禁止使用 `tokio::sync::Mutex::blocking_lock()`**：在 Tokio 运行时线程上调用会阻塞 reactor，轻则高延迟，重则 panic。
> - 改用 `std::sync::Mutex`（`AppHandle` 是 `Clone + Send + Sync`，无需 async 锁）；`on_event` 内仅做轻量提取，IPC 发射交给 `tauri::async_runtime::spawn`，避免阻塞日志热路径。

```rust
//! # WebView 日志输出 Layer
//!
//! 自定义 `tracing-subscriber` Layer，将日志事件通过 Tauri Event 转发到 WebView 控制台。
//! 替代 `tauri-plugin-log` 的 Webview 目标。

use std::sync::Mutex;

use tauri::{AppHandle, Emitter};
use tracing::Subscriber;
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

/// 将 `tracing` 事件转发到 WebView 控制台的 Layer
///
/// 通过 `app.emit("console:log", payload)` 将日志事件发送到前端，
/// 前端通过 `listen("console:log", ...)` 接收并调用 `console.log` 输出。
///
/// 级别过滤由全局 `AtomicLevelFilter` 在到达本 Layer 前完成，
/// 前端无需再做级别过滤，收到事件即按级别打印。
pub struct WebViewLayer {
    // 用 std::sync::Mutex 而非 tokio::sync::Mutex：
    // on_event 可能在 async 上下文（Tokio 线程）中被调用，
    // tokio Mutex 的 blocking_lock 会阻塞 reactor 导致 panic。
    // AppHandle 是 Clone + Send + Sync，无需 async 锁。
    app_handle: Mutex<Option<AppHandle>>,
}

impl WebViewLayer {
    pub fn new() -> Self {
        Self {
            app_handle: Mutex::new(None),
        }
    }

    /// 在 Tauri setup 完成后注入 AppHandle
    pub fn set_app_handle(&self, app: AppHandle) {
        let mut guard = self.app_handle.lock().expect("app_handle mutex poisoned");
        *guard = Some(app);
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for WebViewLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        // 持锁时间极短：仅克隆 AppHandle，不在此处发 IPC
        let app = {
            let guard = self.app_handle.lock().expect("app_handle mutex poisoned");
            guard.clone()
        };
        let Some(app) = app else { return };

        // 提取事件元数据
        let meta = event.metadata();
        let level = meta.level().to_string();
        let target = meta.target();
        let file = meta.file().unwrap_or("");
        let line = meta.line().unwrap_or(0);

        // 格式化消息：log::info!("hello") 桥接后，message 字段以 Debug 形式记录
        //（值是 format_args!("hello") 的 Debug，输出形如 "hello" 带引号）。
        // MessageVisitor 会去除 Debug 输出的外层引号，保持与原 log:: 输出一致。
        let mut message = String::new();
        let mut visitor = MessageVisitor(&mut message);
        event.record(&mut visitor);

        let payload = serde_json::json!({
            "level": level,
            "target": target,
            "file": file,
            "line": line,
            "message": message,
            "traceId": crate::core::trace_id::current_trace_id(),
        });

        // IPC 发射交给后台线程，避免阻塞日志热路径（网关请求每条都会触发）
        tauri::async_runtime::spawn(async move {
            let _ = app.emit("console:log", payload);
        });
    }
}

/// 从 tracing Event 中提取 message 字段值
///
/// `log::info!("hello")` 经 `log` feature 桥接后，message 字段通过 `record_debug` 记录，
/// 值为 `format_args!("hello")` 的 Debug 输出，形如 `"hello"`（带双引号）。
/// 直接 `{:?}` 输出会带引号，需去除首尾双引号以保持与原 `log::` 输出一致。
struct MessageVisitor<'a>(&'a mut String);

impl<'a> tracing::field::Visit for MessageVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let s = format!("{:?}", value);
            // format_args! 的 Debug 输出形如 "hello"，去除首尾引号
            if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
                self.0.push_str(&s[1..s.len() - 1]);
            } else {
                self.0.push_str(&s);
            }
        }
    }
}
```

**前端接收**（在 `src/core/events.ts` 中统一注册，便于管理 `unlisten` 清理）：

```typescript
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

interface ConsoleLogPayload {
  level: string
  target: string
  file: string
  line: number
  message: string
  traceId: string | null
}

/** 注册 Rust 后端日志转发到 DevTools 的监听器，返回卸载函数 */
export async function registerConsoleLogForwarder(): Promise<UnlistenFn> {
  return listen<ConsoleLogPayload>('console:log', (event) => {
    const { level, target, file, line, message, traceId } = event.payload
    // 网关请求路径日志带 traceId，非请求路径为 null
    const ridPrefix = traceId ? `[tid=${traceId}] ` : ''
    const prefix = `${ridPrefix}[${target}] ${file}:${line}`
    switch (level) {
      case 'ERROR':
        console.error(prefix, message)
        break
      case 'WARN':
        console.warn(prefix, message)
        break
      case 'DEBUG':
        console.debug(prefix, message)
        break
      case 'TRACE':
        console.trace(prefix, message)
        break
      default:
        console.log(prefix, message)
    }
  })
}
```

在 `src/main.tsx` 应用启动时调用并保存 `unlisten`，应用卸载时调用：

```typescript
useEffect(() => {
  let unlisten: UnlistenFn | undefined
  registerConsoleLogForwarder().then((fn) => { unlisten = fn })
  return () => { unlisten?.() }
}, [])
```

> **级别过滤说明**：后端 `AtomicLevelFilter` 已在事件到达 `WebViewLayer` 前完成过滤，前端无需重复过滤。修改设置中的日志级别会同时影响终端、文件、WebView 三路输出。

### 4.7 Phase 3：`tower-http` `TraceLayer` 集成

```rust
// src-tauri/src/modules/gateway_runtime/router.rs

use tower_http::trace::TraceLayer;

pub fn build_router(shared: GatewaySharedState) -> Router {
    let auth_state = AuthState::new(
        shared.ai_gateway_handle.clone(),
        shared.secret_handle.clone(),
        shared.inner_cli_api_key.clone(),
    );

    Router::new()
        .route("/health", get(health))
        .route("/readyz", get(readyz))
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/messages", post(anthropic_messages))
        .with_state(shared)
        // layer 顺序：后加的在外层。
        // 先加 auth_middleware（内层）：认证逻辑在 span 内执行
        // 后加 TraceLayer（外层）：即使认证失败也会创建 span，不丢失观测
        .layer(axum::middleware::from_fn_with_state(
            auth_state,
            super::auth::auth_middleware,
        ))
        .layer(TraceLayer::new_for_http())
}
```

`TraceLayer::new_for_http()` 会自动为每个 HTTP 请求创建 `tracing` span，默认包含字段：
- `http.method`、`http.uri`、`http.status_code`
- `http.latency`（响应耗时）

> **关于 trace ID 的说明**：
> - `TraceLayer` 默认**不**生成跨服务分布式 trace ID。它创建的是单进程内的 span，可通过 `tracing::Span::current().id()` 获取 span ID（同一请求的多条日志共享该 span）。
> - 项目已有 `uuid::Uuid::new_v4()` 生成的 `trace_id`（见 [router.rs](../../src-tauri/src/modules/gateway_runtime/router.rs) 各 handler），通过自定义 `MakeSpan` 注入 span 字段，配合 §4.3b 的 `TraceIdLayer` + `TraceIdFormat`，即可让每条日志行带 `[tid=...]` 前缀。
> - 如需真正的跨服务分布式 trace ID，再评估 `tracing-opentelemetry` 集成。

**自定义 `MakeSpan` 注入 `trace_id`（推荐，与 §4.3b 配套）**：

```rust
use tower_http::trace::MakeSpan;
use tracing::Span;
use crate::core::trace_id::next_trace_id;

#[derive(Clone)]
struct TraceIdSpan;

impl<B> MakeSpan<B> for TraceIdSpan {
    fn make_span(&mut self, request: &axum::http::Request<B>) -> Span {
        let trace_id = next_trace_id();   // 雪花 ID 转 32 进制，13 字符
        tracing::info_span!(
            "http.request",
            trace_id = %trace_id,   // 关键：字段名必须为 trace_id
            method = %request.method(),
            uri = %request.uri(),
        )
    }
}

// router.rs 中使用
.layer(
    TraceLayer::new_for_http()
        .make_span_with(TraceIdSpan)
)
```

> **字段名约束**：`MakeSpan` 中 span 字段名必须为 `trace_id`，与 §4.3b `TraceIdVisitor` 中匹配的字段名一致。否则 `TraceIdLayer` 无法提取，日志行不会带前缀。
>
> **与现有 `trace_id` 的关系**：[router.rs](../../src-tauri/src/modules/gateway_runtime/router.rs) 各 handler 内已用 `uuid::Uuid::new_v4()` 生成 `trace_id` 并传递给 `tauri_emitter`。引入 `TraceIdSpan` 后，**统一由 `MakeSpan` 生成**（雪花 ID 转 32 进制），handler 内通过 `tracing::Span::current().field("trace_id")` 读取，避免重复生成。此项重构可在 Phase 3 一并完成，或留待 Phase 4 增量优化。

---

## 5. 初始化顺序

```mermaid
sequenceDiagram
    participant Tauri as tauri::Builder
    participant Setup as .setup() 回调
    participant Tracing as tracing-subscriber
    participant DB as 数据库
    participant Settings as Settings 模块
    participant Others as 其他模块

    Tauri->>Setup: 进入 setup
    Setup->>Tracing: 0. 初始化 tracing-subscriber
    Note over Tracing: 注册 EnvFilter + AtomicLevelFilter + stdout + 文件 layer<br/>app.manage(WorkerGuardState) 保活文件写入线程<br/>app.manage(Arc&lt;AtomicLevelFilter&gt;) 供 settings 调整
    Setup->>DB: 1. 初始化数据库
    Note over DB: log::info!("数据库初始化完成") 已被 tracing 捕获
    Setup->>Settings: 2. 读取设置
    Settings->>Settings: 获取 log_level
    Settings->>Tracing: 3. 应用用户日志级别（覆盖 INFO 默认）
    Setup->>Tracing: 4. 注册 WebViewLayer（Phase 2）
    Note over Tracing: 注入 AppHandle 后开始转发到 DevTools
    Setup->>Others: 5. 初始化其他模块
    Note over Others: log::info! 经 tracing 桥接，三路输出同步生效
```

> **关键顺序约束**：`init_tracing(app)` 必须在 DB 初始化之前，否则 [main.rs](../../src-tauri/src/main.rs) 中 `log::info!("数据库初始化完成")` 会被丢弃。

---

## 6. 回滚方案

如果 Phase 1/2 实施后出现问题，可以快速回退：

1. 恢复 `Cargo.toml` 中的 `tauri-plugin-log = "2.8.0"` 依赖，删除 `tracing` / `tracing-subscriber` / `tracing-appender`
2. 恢复 `main.rs` 中的 `.plugin(tauri_plugin_log::Builder::new()...)` 调用，**注意完整恢复以下 7 行**（含 `max_file_size` 与 `rotation_strategy`）：
   ```rust
   .plugin(
       tauri_plugin_log::Builder::new()
           .targets([
               Target::new(TargetKind::Stdout),
               Target::new(TargetKind::LogDir { file_name: None }),
               Target::new(TargetKind::Webview),
           ])
           .level(log::LevelFilter::Trace)
           .max_file_size(1024 * 1024 * 20)
           .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
           .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepAll)
           .build(),
   )
   ```
3. 删除 `init_tracing` 函数、`WorkerGuardState`、`use tauri_plugin_log::{Target, TargetKind}` 改回原状
4. 恢复 `settings/types.rs` 的 `to_tracing_level()` 删除（保留原 `to_level_filter()`）
5. 恢复 `settings/commands.rs` 中 `log::set_max_level(dto.log_level.to_level_filter())`
6. 删除新建文件：`src-tauri/src/core/atomic_filter.rs`、`src-tauri/src/modules/tracing_webview.rs`
7. 删除前端 `console:log` 监听器（`src/core/events.ts` 中的 `registerConsoleLogForwarder` 及 `main.tsx` 中的调用）
8. 删除 `router.rs` 中的 `TraceLayer`（Phase 3）

回滚仅影响 ~7 个文件，**不会影响 180 处 `log::` 宏调用**。

---

## 7. 验证清单

| # | 验证项 | 验证方式 |
|---|--------|----------|
| 1 | 终端输出日志 | `pnpm tauri:dev` 观察终端，应见 `INFO` 级别模块启动日志 |
| 2 | 文件日志写入 | 检查 `app_log_dir` 下是否有 `i-code-YYYY-MM-DD.log` |
| 3 | 文件滚动上限 | 连续运行跨天，确认历史文件不超过 30 个 |
| 4 | WorkerGuard 保活 | 连续运行 1 小时后检查日志文件仍在写入（验证 guard 未被提前 drop） |
| 5 | WebView 控制台日志 | 打开 DevTools → Console，观察 Rust 日志（含文件名:行号） |
| 6 | 消息无多余引号 | DevTools 中 `log::info!("hello")` 应显示 `hello` 而非 `"hello"` |
| 7 | 日志级别动态切换 | 设置中切换日志级别，观察终端/文件/WebView 三路是否同步 |
| 8 | 日志级别持久化 | 重启应用，验证日志级别为上次设置的值 |
| 9 | RUST_LOG 覆盖 | 启动前 `set RUST_LOG=debug`，验证覆盖默认 INFO |
| 10 | 网关请求日志 | 发送请求到本地网关，观察终端日志格式与 `trace_id` |
| 11 | `tower-http` TraceLayer | 网关请求的 tracing span 包含 `http.method` / `http.uri` / `http.latency` |
| 12 | 认证失败仍记 span | 发送无凭证请求，确认 TraceLayer span 仍创建（外层观测） |
| 13 | async 上下文无 panic | 网关连续处理 100 个请求，确认 WebViewLayer 无 panic（验证 std::sync::Mutex） |
| 14 | 自研内存 logger 不受影响 | 应用内「日志」页面正常展示业务日志 |
| 15 | `log::` 宏全部正常工作 | 随机检查若干模块的 `log::info!` 输出 |
| 16 | 网关日志带 tid 前缀 | 发送网关请求，终端/文件日志行含 `[tid=xxxxxxxxxxxxx]`（13 字符） |
| 17 | 同请求 tid 一致 | 同一请求的 router/emitter/upstream 多条日志 tid 相同 |
| 18 | Command 链路 tid | 触发 Tauri Command（如刷新额度），Command→Service→Repository 多条日志 tid 相同 |
| 19 | 定时任务 tid | 定时刷新额度触发时，日志含 `[tid=...]` 且同一任务内一致 |
| 20 | 托盘菜单 tid | 点击托盘菜单项，日志含 `[tid=...]` |
| 21 | 模块启动 tid | 应用启动日志（DB/Settings/各模块初始化）含同一 `[tid=...]` |
| 22 | tid 趋势递增 | 连续生成的多个 tid 按字典序递增（雪花 ID 特性） |
| 23 | tid 格式合法 | tid 为 32 进制字符串（`0-9a-v`），长度 ≤ 13 字符 |
| 24 | DevTools 显示 tid | 打开 DevTools → Console，日志前缀含 `[tid=...]` |
| 25 | 极少数日志无 tid | 未在操作 span 内的日志（如 filter 级别变更）不带前缀，属正常 |
| 26 | 文件按天滚动 | 跨天后产生新文件 `i-code-YYYY-MM-DD.log`，旧文件保留 |
| 27 | 文件按大小分片 | 单文件超过 20MB 后产生 `i-code-YYYY-MM-DD.1.log`、`.2.log` |
| 28 | 旧文件自动清理 | 启动时清理超过 30 天的日志文件，保留近期文件 |

---

## 8. 时间估算

| Phase | 内容 | 预估工时 |
|-------|------|----------|
| Phase 1 | 核心替换（Cargo.toml + atomic_filter.rs + trace_id.rs + trace_id_layer.rs + size_aware_appender.rs + main.rs + types.rs + commands.rs） | 8-9 小时 |
| Phase 2 | WebView 转发 Layer（含前端监听器、tid 字段） | 2 小时 |
| Phase 3 | `TraceLayer` 集成 + `MakeSpan` 注入 trace_id | 1.5 小时 |
| Phase 4 | 全操作入口接入（Command/定时/托盘/启动 加 `enter_operation`/`enter_operation_async`） | 3-4 小时 |
| Phase 5 | 增量升级（可选） | 渐进式 |
| **验证** | 完整验证清单（含跨天滚动、大小分片、async 压测、tid 一致性、全链路覆盖） | 3 小时 |
| **合计** | | **17.5-19.5 小时** |

> 较原估算（5-7 小时）上调原因：
> - WorkerGuard 保活、AtomicLevelFilter 双注册、WebViewLayer async 安全、跨天滚动验证
> - **trace_id 生成器（雪花 ID 转 32 进制）**
> - **全操作链路接入**：Command/定时/托盘/启动 5 类入口需逐一加 `enter_operation`/`enter_operation_async`，并验证 async 跨 await 点的 thread-local 一致性
> - **自定义 SizeAwareFileAppender**：按天 + 按大小双维度滚动，需实现 + 测试 + 边界情况验证
> - tid 一致性验证（28 项验证清单）

---

## 9. 注意事项

1. **`tracing` `log` feature 的启动时机**：`log::` 宏只有在 `tracing-subscriber` 初始化后才会被捕获。启动期极早期的日志（如 `main()` 入口到 `init_tracing` 之间）会被丢弃。i-code 启动期无关键业务日志，可接受。**务必把 `init_tracing(app)` 放在 `setup` 闭包第一步、DB 初始化之前**，否则 `log::info!("数据库初始化完成")` 会丢失。

2. **`tracing-appender` 的 `non_blocking` guard 生命周期**（关键修订）：
   - `non_blocking` 返回的 `WorkerGuard` 一旦 drop，后台写入线程立即终止，后续文件日志全部丢失。
   - **`setup` 闭包返回 `Ok(())` 后，其局部变量全部 drop**。原方案"闭包生命周期=应用生命周期"的说法是错误的。
   - **必须用 `app.manage(WorkerGuardState(guard))` 注册为 Tauri State**，存活到应用退出。
   - 验证方式：连续运行 1 小时后检查日志文件仍在写入（见验证清单 #4）。

3. **文件日志格式与滚动（按天 + 按大小双维度）**：
   - `tracing-appender` 使用 `tracing-subscriber::fmt` 的格式化，输出格式与 `tauri-plugin-log` 不同（默认 compact，含 target/file/line + `[tid=...]` 前缀）。
   - **滚动策略**：`tracing-appender` 原生仅支持按时间滚动（`Hourly`/`Daily`/`Never`），**不支持按大小**。本方案自定义 `SizeAwareFileAppender`（§4.3e）实现「按天 + 超过 20MB 分文件」：
     - 当天首个文件：`i-code-YYYY-MM-DD.log`
     - 超过 20MB 分片：`i-code-YYYY-MM-DD.1.log`、`.2.log`（序号递增）
     - 跨天重置序号：`i-code-YYYY-MM-DD+1.log`
   - **旧文件清理**：启动时扫描日志目录，删除超过 30 天的文件（按 mtime 判断，仅清理匹配 `i-code-*.log` 前后缀的文件）。
   - **与原 `tauri-plugin-log` 的差异**：原方案按 20MB 大小滚动（`max_file_size`），不按天；新方案按天为主、大小为辅，文件名含日期便于按天查找。
   - 如需机器可读格式，启用 `tracing-subscriber` 的 `json` feature 并改用 `fmt::layer().json()`。

4. **`WebViewLayer` 的 async 安全**（关键修订）：
   - `on_event` 会被 `log::info!` 桥接后调用，而 `log::info!` 大量出现在 `router.rs`、`tauri_emitter.rs` 的 async 函数中（网关请求热路径）。
   - **禁止使用 `tokio::sync::Mutex::blocking_lock()`**：在 Tokio 运行时线程上调用会阻塞 reactor，轻则高延迟，重则 panic。
   - 改用 `std::sync::Mutex`（`AppHandle` 是 `Clone + Send + Sync`，无需 async 锁），持锁仅做 `clone()`，IPC 发射交给 `tauri::async_runtime::spawn`。

5. **`AtomicLevelFilter` 的双注册**：
   - 必须以 `Arc<AtomicLevelFilter>` 形式同时注册到 subscriber（`.with(filter.clone())`）和 Tauri State（`app.manage(filter)`）。
   - settings 模块通过 `app_handle.state::<Arc<AtomicLevelFilter>>()` 获取；类型必须是 `Arc<AtomicLevelFilter>`，不能是裸 `AtomicLevelFilter`（Tauri State 要求 `'static`）。
   - 放弃 `tracing_subscriber::reload::Layer` 方案：其泛型类型 `reload::Handle<LevelFilter, S>` 中的 `S` 取决于 subscriber 的 layer 组合方式，难以在模块间稳定传递。

6. **trace ID（trace_id）注入的实现边界**：
   - `tower-http` `TraceLayer::new_for_http()` 默认**不**生成分布式 trace ID，仅创建单进程内 span。
   - 本方案通过 §4.3b `TraceIdLayer` + `TraceIdFormat` + §4.3c 雪花 ID 生成器 + §4.3d 操作入口注入 + §4.7 `MakeSpan` 五者配合，实现**全操作链路** `trace_id` 注入到日志行（`[tid=...]` 前缀）。
   - **覆盖范围**：网关请求、Tauri Command、定时任务、托盘菜单、模块启动 5 类入口均通过 `enter_operation`/`enter_operation_async`/`MakeSpan` 注入 trace_id。极少数未在操作 span 内的日志（如 filter 级别变更）不带前缀，属正常。
   - **字段名约束**：所有 span 的 `trace_id` 字段名必须一致，与 `TraceIdVisitor` 匹配；否则 `TraceIdLayer` 无法提取。
   - **async 与 thread-local**：`enter_operation_async` 通过 `tracing::Instrument` 让 span 跟随 task。`TraceIdLayer::on_enter/on_exit` 在 poll 期间设置/清除 thread-local，await 挂起时清除、resume 时重设。因此 async 路径的 `log::info!` 在 poll 期间能正确读到 thread-local。
   - **子任务不继承 span**：async 函数内 `tokio::spawn` 的子任务不会继承父 span，需显式 `.instrument(span.clone())` 或在子任务内重新 `enter_operation_async`。
   - **trace_id 生成方式**：雪花 ID（41 位毫秒 + 10 位机器 + 12 位序列号）转 32 进制，13 字符，趋势递增。纯标准库实现，无需新增依赖。
   - 如需真正的跨服务分布式 trace ID，再评估 `tracing-opentelemetry`。

7. **`TraceLayer` 与认证中间件的顺序**：
   - axum 中后加的 layer 在外层。建议 `.layer(auth_middleware).layer(TraceLayer)`，让 TraceLayer 在最外层，即使认证失败也创建 span，不丢失观测。
   - 若不关心认证失败的观测，可将 TraceLayer 放内层，避免无谓 span 创建。

8. **`tracing` 的 `log` feature 与 `test` 环境**：测试中不需要初始化 `tracing-subscriber`，`log::` 宏会静默丢弃。如果需要测试中查看日志，可在测试 setup 中单独初始化 `tracing-subscriber`（用 `tracing_test` crate 或手动 `set_global_default`）。

9. **MessageVisitor 的引号处理**：`log::info!("hello")` 经 `log` feature 桥接后，message 字段以 `Debug` 形式记录（值是 `format_args!("hello")` 的 Debug 输出，形如 `"hello"` 带双引号）。`MessageVisitor::record_debug` 必须去除首尾双引号，否则 DevTools 会显示 `"hello"` 而非 `hello`。