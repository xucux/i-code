//! # 脚本日志 Host Functions
//!
//! `log.info` / `log.warn` / `log.error`：写入自研 logger 并收集到试运行 logs[]。
//! 自动脱敏 api_key。

use std::sync::{Arc, Mutex};

use rhai::Engine;

use crate::modules::logger::Log;

/// 脚本执行期间的日志缓冲
pub struct ScriptLogBuffer {
    pub lines: Vec<String>,
    api_key: Option<String>,
}

impl ScriptLogBuffer {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            lines: Vec::new(),
            api_key: api_key.filter(|s| !s.is_empty()),
        }
    }

    fn redact(&self, msg: &str) -> String {
        if let Some(key) = &self.api_key {
            msg.replace(key, "***")
        } else {
            msg.to_string()
        }
    }

    pub fn push(&mut self, level: &str, msg: &str) {
        let safe = self.redact(msg);
        let line = format!("[{level}] {safe}");
        self.lines.push(line.clone());
        // 写入自研 logger（业务可见）
        match level {
            "warn" => Log::warn(&format!("[balance-script] {safe}")),
            "error" => Log::error(&format!("[balance-script] {safe}")),
            _ => Log::info(&format!("[balance-script] {safe}")),
        }
        // 开发日志
        match level {
            "warn" => log::warn!("[balance-script] {safe}"),
            "error" => log::error!("[balance-script] {safe}"),
            _ => log::info!("[balance-script] {safe}"),
        }
    }
}

/// 注册日志 host 函数
///
/// - 扁平名：`log_info` / `log_warn` / `log_error`
/// - 静态模块：`log::info` / `log::warn` / `log::error`
///
/// 注意：模块调用请用 `log::info(...)`，不要写 `log.info(...)`。
pub fn register(engine: &mut Engine, buffer: Arc<Mutex<ScriptLogBuffer>>) {
    let b1 = buffer.clone();
    engine.register_fn("log_info", move |msg: &str| {
        if let Ok(mut g) = b1.lock() {
            g.push("info", msg);
        }
    });
    let b2 = buffer.clone();
    engine.register_fn("log_warn", move |msg: &str| {
        if let Ok(mut g) = b2.lock() {
            g.push("warn", msg);
        }
    });
    let b3 = buffer.clone();
    engine.register_fn("log_error", move |msg: &str| {
        if let Ok(mut g) = b3.lock() {
            g.push("error", msg);
        }
    });

    let mut module = rhai::Module::new();
    let bi = buffer.clone();
    module.set_native_fn("info", move |msg: &str| {
        if let Ok(mut g) = bi.lock() {
            g.push("info", msg);
        }
        Ok::<(), Box<rhai::EvalAltResult>>(())
    });
    let bw = buffer.clone();
    module.set_native_fn("warn", move |msg: &str| {
        if let Ok(mut g) = bw.lock() {
            g.push("warn", msg);
        }
        Ok::<(), Box<rhai::EvalAltResult>>(())
    });
    let be = buffer;
    module.set_native_fn("error", move |msg: &str| {
        if let Ok(mut g) = be.lock() {
            g.push("error", msg);
        }
        Ok::<(), Box<rhai::EvalAltResult>>(())
    });
    engine.register_static_module("log", module.into());
}
