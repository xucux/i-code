//! # trace ID（trace_id）注入 Layer
//!
//! 通过 thread-local + 自定义 Layer/FormatEvent，让所有操作入口的日志行
//! 带 `[tid=...]` 前缀，便于关联同一操作链路的多条日志。
//!
//! ## 工作流
//!
//! 1. 各操作入口（`enter_operation` / `enter_operation_async`）创建 span，
//!    注入 `trace_id` 字段（值来自 [`crate::core::trace_id`] 生成器）
//! 2. [`TraceIdLayer::on_new_span`] 在 span 创建时提取 `trace_id` 并存入 span extensions
//! 3. [`TraceIdLayer::on_enter`] 在 span 进入时从 extensions 读取 trace_id 写入 thread-local
//! 4. [`TraceIdFormat::format_event`] 格式化日志行时读 thread-local，写前缀
//! 5. [`TraceIdLayer::on_exit`] 在 span 退出时清除 thread-local
//!
//! ## async 与 thread-local
//!
//! `enter_operation_async` 通过 `tracing::Instrument` 让 span 跟随 task。
//! `TraceIdLayer::on_enter/on_exit` 在 poll 期间设置/清除 thread-local，
//! await 挂起时清除、resume 时重设。因此 async 路径的 `log::info!` 在 poll
//! 期间能正确读到 thread-local。
//!
//! **注意**：若 async 函数内部 `tokio::spawn` 了子任务，子任务不会继承父 span，
//! 需显式 `.instrument(span.clone())` 或在子任务内重新 `enter_operation_async`。

use std::cell::RefCell;
use std::fmt;

use tracing::{field, Event, Instrument, Subscriber};
use tracing_subscriber::fmt::{FormatEvent, FormatFields, FmtContext};
use tracing_subscriber::fmt::format::Writer as FmtWriter;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;
use tracing_subscriber::registry::LookupSpan;

/// SSE chunk 日志的独立 target
///
/// 此类日志只进入专属文件（`i-code-sse.*.log`，按小时滚动），
/// 并在该文件中省略 target / file:line 前缀；常规文件与控制台不输出。
pub const SSE_LOG_TARGET: &str = "i_code::sse";

thread_local! {
    /// 当前线程活跃的 trace_id（在操作 span 内有值）
    static CURRENT_TRACE_ID: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// 读取当前线程的 trace_id（供 WebViewLayer 等外部消费者使用）
pub fn current_trace_id() -> Option<String> {
    CURRENT_TRACE_ID.with(|r| r.borrow().clone())
}

/// 存储在 span extensions 中的 trace_id 值
///
/// 在 `on_new_span` 时从 span 属性提取并插入 extensions，
/// 在 `on_enter` 时从 extensions 读取并写入 thread-local。
struct TraceIdValue(String);

/// 在 span 进入/退出时维护 thread-local trace_id
///
/// 实现 `Layer` trait：
/// - `on_new_span`：从 span 属性提取 `trace_id` 字段，存入 span extensions
/// - `on_enter`：从 extensions 读取 trace_id，写入 thread-local
/// - `on_exit`：清除 thread-local
pub struct TraceIdLayer;

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for TraceIdLayer {
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: Context<'_, S>,
    ) {
        // 从 span 属性中提取 trace_id 字段值
        let mut visitor = TraceIdVisitor { value: None };
        attrs.record(&mut visitor);
        if let Some(rid) = visitor.value {
            if let Some(span) = ctx.span(id) {
                let mut extensions = span.extensions_mut();
                extensions.insert(TraceIdValue(rid));
            }
        }
    }

    fn on_enter(&self, id: &tracing::Id, ctx: Context<'_, S>) {
        // 从 span extensions 中读取 trace_id
        if let Some(span) = ctx.span(id) {
            let extensions = span.extensions();
            if let Some(rid) = extensions.get::<TraceIdValue>() {
                CURRENT_TRACE_ID.with(|r| *r.borrow_mut() = Some(rid.0.clone()));
            }
        }
    }

    fn on_exit(&self, _id: &tracing::Id, _ctx: Context<'_, S>) {
        CURRENT_TRACE_ID.with(|r| *r.borrow_mut() = None);
    }
}

/// 从 span 属性中提取 trace_id 字段
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
///
/// 输出格式（`with_location`）：
/// ```text
/// 2026-07-29 10:23:45.123  INFO i_code::modules::xxx file.rs:108 [tid=1v2u0j3w4x5y] 消息内容
/// ```
///
/// 未在操作 span 内的日志（极少数）不带 `[tid=...]` 前缀。
///
/// `without_location`（SSE 专属文件使用）不输出 target 与 file:line：
/// ```text
/// 2026-07-29 10:23:45.123 DEBUG [tid=1v2u0j3w4x5y] SSE chunk | ...
/// ```
#[derive(Clone, Copy)]
pub struct TraceIdFormat {
    /// 是否输出 target 与 file:line（SSE 专属文件层关闭该信息）
    with_location: bool,
}

impl TraceIdFormat {
    /// 完整格式：stdout 与常规日志文件使用（含 target 与 file:line）
    pub const fn with_location() -> Self {
        Self { with_location: true }
    }

    /// 精简格式：SSE 专属文件使用（不输出 target 与 file:line）
    pub const fn without_location() -> Self {
        Self { with_location: false }
    }
}

impl<S, N> FormatEvent<S, N> for TraceIdFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: FmtWriter<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let meta = event.metadata();

        // 1. 时间戳（本地时区）
        write!(
            writer,
            "{} ",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f")
        )?;

        // 2. 级别（左对齐占 5 字符）
        write!(writer, "{:5} ", meta.level())?;

        // 3. trace_id 前缀（若处于请求 span 内）
        if let Some(rid) = current_trace_id() {
            write!(writer, "[tid={}] ", rid)?;
        }

        // 4. target 与 file:line（SSE 专属文件层关闭，保持输出精简）
        if self.with_location {
            write!(writer, "{} ", meta.target())?;

            // 4.1 file:line（仅保留文件名，避免完整路径过长）
            //     同时处理 Unix '/' 和 Windows '\' 两种路径分隔符
            if let (Some(file), Some(line)) = (meta.file(), meta.line()) {
                let file = file.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(file);
                write!(writer, "{}:{} ", file, line)?;
            }
        }

        // 5. 消息字段（委托给默认 FormatFields）
        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

// ===== 操作入口辅助函数 =====

/// 操作入口 guard：生成 trace_id 并进入操作 span
///
/// 用于同步入口（托盘菜单、模块启动）。drop 时自动退出 span 并清除 thread-local。
///
/// # 用法
///
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
    OperationGuard { _span: span.entered() }
}

/// async 操作入口包装器：生成 trace_id 并在 span 内执行 future
///
/// 用于 Tauri Command、定时任务等 async 入口。通过 `tracing::Instrument` 让 span
/// 跟随 task，跨 await 点保持。
///
/// # 用法
///
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_trace_id_outside_span() {
        // 未进入 span 时，current_trace_id 应返回 None
        CURRENT_TRACE_ID.with(|r| r.borrow_mut().take());
        assert!(current_trace_id().is_none());
    }
}
