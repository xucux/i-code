//! # 运行时可调级别过滤器
//!
//! 自定义 `tracing_subscriber::layer::Filter`，通过原子变量实现运行时级别调整。
//! 替代 `tracing_subscriber::reload::Layer`，避免其泛型类型在模块间难以传递的问题。
//!
//! ## 设计要点
//!
//! - 放弃 `reload::Layer`：其泛型 `reload::Handle<LevelFilter, S>` 中的 `S`
//!   取决于 subscriber 的 layer 组合方式，难以在 `main.rs` 与 `settings/commands.rs`
//!   之间稳定传递。
//! - 改用基于 `AtomicU8` 的自定义 `Filter`，类型简单（`Arc<AtomicLevelFilter>`），
//!   可直接注册为 Tauri State。
//! - 同时注册到 subscriber（`.with(filter.clone())`）与 Tauri State（`app.manage(filter)`）。

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
    /// 当前生效的级别（按 level_to_u8 编码到 u8）
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

/// 将 `tracing::Level` 编码为 `u8`，级别越低数值越小（ERROR=0 … TRACE=4）
const fn level_to_u8(level: tracing::Level) -> u8 {
    match level {
        tracing::Level::ERROR => 0,
        tracing::Level::WARN => 1,
        tracing::Level::INFO => 2,
        tracing::Level::DEBUG => 3,
        tracing::Level::TRACE => 4,
    }
}

/// 将 `u8` 解码为 `tracing::Level`，未知值视为 TRACE
const fn u8_to_level(v: u8) -> tracing::Level {
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
        // meta 级别数值 ≤ 当前阈值时通过（ERROR=0 优先级最高）
        level_to_u8(*meta.level()) <= max
    }
}

/// `Arc<AtomicLevelFilter>` 的 newtype 包装，使其可作为 per-layer filter 使用。
///
/// `tracing-subscriber` 仅对 `Arc<dyn Filter + Send + Sync>` 提供了 blanket impl，
/// 对具体类型的 `Arc<T>` 没有，且直接为 `Arc<AtomicLevelFilter>` 实现 `Filter` 会触发
/// 孤儿规则（`Arc` 与 `Filter` 均为外部类型）。用 newtype 包装绕过此限制。
#[derive(Debug, Clone)]
pub struct SharedLevelFilter(pub std::sync::Arc<AtomicLevelFilter>);

impl<S: Subscriber> Filter<S> for SharedLevelFilter {
    fn enabled(&self, meta: &tracing::Metadata<'_>, cx: &Context<'_, S>) -> bool {
        self.0.enabled(meta, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_encoding_roundtrip() {
        for level in [
            tracing::Level::ERROR,
            tracing::Level::WARN,
            tracing::Level::INFO,
            tracing::Level::DEBUG,
            tracing::Level::TRACE,
        ] {
            assert_eq!(u8_to_level(level_to_u8(level)), level);
        }
    }

    #[test]
    fn test_set_level_takes_effect_immediately() {
        let filter = AtomicLevelFilter::new(tracing::Level::INFO);
        assert_eq!(filter.get_level(), tracing::Level::INFO);
        filter.set_level(tracing::Level::DEBUG);
        assert_eq!(filter.get_level(), tracing::Level::DEBUG);
    }
}
