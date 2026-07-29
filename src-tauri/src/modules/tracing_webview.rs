//! # WebView 日志输出 Layer
//!
//! 自定义 `tracing-subscriber` Layer，将日志事件通过 Tauri Event 转发到 WebView 控制台。
//! 替代 `tauri-plugin-log` 的 Webview 目标。
//!
//! ## 关键约束
//!
//! - `on_event` 会被 `log::info!` 桥接后调用，而 `log::info!` 大量出现在
//!   `router.rs`、`tauri_emitter.rs` 的 **async 函数**中（网关请求热路径）。
//! - **禁止使用 `tokio::sync::Mutex::blocking_lock()`**：在 Tokio 运行时线程上调用
//!   会阻塞 reactor，轻则高延迟，重则 panic。
//! - 改用 `std::sync::Mutex`（`AppHandle` 是 `Clone + Send + Sync`，无需 async 锁）；
//!   `on_event` 内仅做轻量提取，IPC 发射交给 `tauri::async_runtime::spawn`，
//!   避免阻塞日志热路径。
//!
//! ## 级别过滤
//!
//! 内部持有 `Arc<AtomicLevelFilter>`，在 `on_event` 中检查级别。
//! 不使用 `Layer::with_filter` 是因为 `Arc<WebViewLayer>` 与 `with_filter` 的类型
//! 组合在 tracing-subscriber 0.3 中存在限制。

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter};
use tracing::Subscriber;
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::core::atomic_filter::AtomicLevelFilter;

/// 将 `tracing` 事件转发到 WebView 控制台的 Layer
///
/// 通过 `app.emit("console:log", payload)` 将日志事件发送到前端，
/// 前端通过 `listen("console:log", ...)` 接收并调用 `console.log` 输出。
///
/// 级别过滤由内部持有的 `AtomicLevelFilter` 在 `on_event` 中完成，
/// 与 fmt 输出层共享同一个 `Arc<AtomicLevelFilter>` 实例，确保级别一致。
pub struct WebViewLayer {
    // 用 std::sync::Mutex 而非 tokio::sync::Mutex：
    // on_event 可能在 async 上下文（Tokio 线程）中被调用，
    // tokio Mutex 的 blocking_lock 会阻塞 reactor 导致 panic。
    // AppHandle 是 Clone + Send + Sync，无需 async 锁。
    //
    // 用 Arc 包裹使 WebViewLayer 可 Clone：
    // 一份注册到 subscriber，另一份注册到 Tauri State 供 setup 后注入 AppHandle。
    // tracing-subscriber 0.3.23 未为 Arc<L> 提供 Layer blanket impl，
    // 故不用外层 Arc，而是内部共享。
    app_handle: Arc<Mutex<Option<AppHandle>>>,
    /// 共享级别过滤器，与 fmt 层使用同一实例
    level_filter: Arc<AtomicLevelFilter>,
}

impl Clone for WebViewLayer {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
            level_filter: self.level_filter.clone(),
        }
    }
}

impl WebViewLayer {
    /// 创建 Layer，传入共享的 `Arc<AtomicLevelFilter>` 用于级别过滤
    pub fn new(level_filter: Arc<AtomicLevelFilter>) -> Self {
        Self {
            app_handle: Arc::new(Mutex::new(None)),
            level_filter,
        }
    }

    /// 在 Tauri setup 完成后注入 AppHandle
    ///
    /// `init_tracing` 阶段尚未拿到 `AppHandle`，先创建 Layer 注册到 subscriber；
    /// setup 回调中拿到 `app.handle()` 后调用此方法注入。
    /// 内部用 Arc 共享，clone 后的任意副本调用此方法均对全部副本生效。
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

        // 级别过滤：与 fmt 层共享同一 AtomicLevelFilter 实例
        let meta = event.metadata();
        let current_level = self.level_filter.get_level();
        if *meta.level() > current_level {
            return;
        }

        // 提取事件元数据
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
            "traceId": crate::core::trace_id_layer::current_trace_id(),
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
