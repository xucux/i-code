//! # 额度监控 Rhai 脚本运行时
//!
//! 提供自定义脚本额度查询能力：变量注入、HTTP/JSON host functions、
//! Dynamic → BalanceSnapshot 映射。

pub mod context;
pub mod host_http;
pub mod host_json;
pub mod host_log;
pub mod host_str_math;
pub mod snapshot_map;
pub mod snippets;

use std::sync::{Arc, Mutex};

use rhai::{Dynamic, Engine, Scope};

use crate::error::{IcodeError, IcodeResult};
use crate::modules::ai_gateway::types::Provider;
use crate::modules::script_template::types::ScriptTemplate;

use super::provider::BalanceRefreshInput;
use super::types::BalanceSnapshot;

use self::context::ScriptContext;
use self::host_http::HttpHostState;
use self::host_log::ScriptLogBuffer;

/// 脚本执行结果
pub struct ScriptRunResult {
    pub snapshot: BalanceSnapshot,
    pub logs: Vec<String>,
}

/// 执行额度监控脚本
///
/// - 每次新建 Engine + Scope，避免状态泄漏
/// - HTTP 受 host 白名单与超时约束（仅对市场脚本强制执行）
/// - 返回 map 校验为 BalanceSnapshot
pub async fn execute_balance_script(
    script_body: &str,
    template: &ScriptTemplate,
    provider: &Provider,
    input: &BalanceRefreshInput,
    timeout_ms: u64,
    extra_allowed_hosts: &[String],
    enforce_host_whitelist: bool,
) -> IcodeResult<ScriptRunResult> {
    if script_body.len() > 64 * 1024 {
        return Err(IcodeError::validation("脚本正文不能超过 64 KiB"));
    }
    if script_body.trim().is_empty() {
        return Err(IcodeError::validation("脚本正文为空"));
    }

    let ctx = ScriptContext::from_parts(template, provider, input);
    let logs = Arc::new(Mutex::new(ScriptLogBuffer::new(input.api_key.clone())));
    let http_state = Arc::new(HttpHostState::new(
        timeout_ms,
        provider.base_url.clone(),
        extra_allowed_hosts.to_vec(),
        input.api_key.clone(),
        enforce_host_whitelist,
    ));

    // Rhai Engine 本身非 async；HTTP host 在内部用 reqwest blocking 风格
    // 为不阻塞 tokio，放到 spawn_blocking
    let script_body = script_body.to_string();
    let logs_clone = logs.clone();
    let http_clone = http_state.clone();
    let ctx_owned = ctx;

    let dynamic = tokio::task::spawn_blocking(move || {
        run_rhai(&script_body, &ctx_owned, http_clone, logs_clone)
    })
    .await
    .map_err(|e| IcodeError::internal(format!("脚本执行任务失败: {e}")))??;

    let mut snapshot = snapshot_map::map_to_snapshot(dynamic)?;
    if snapshot.updated_at <= 0 {
        snapshot.updated_at = now_ms();
    }

    let log_lines = logs
        .lock()
        .map(|b| b.lines.clone())
        .unwrap_or_default();

    Ok(ScriptRunResult {
        snapshot,
        logs: log_lines,
    })
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn run_rhai(
    script_body: &str,
    ctx: &ScriptContext,
    http_state: Arc<HttpHostState>,
    logs: Arc<Mutex<ScriptLogBuffer>>,
) -> IcodeResult<Dynamic> {
    let mut engine = Engine::new();
    // 限制最大操作数，防止死循环
    engine.set_max_operations(100_000);
    engine.set_max_expr_depths(64, 64);
    // 禁用潜在危险默认能力：Rhai 默认不带 FS

    // 注册 host functions
    host_json::register(&mut engine);
    host_log::register(&mut engine, logs.clone());
    host_http::register(&mut engine, http_state);
    host_str_math::register(&mut engine);

    // error(msg) → 抛业务错误
    engine.register_fn("error", |msg: &str| -> Result<(), Box<rhai::EvalAltResult>> {
        Err(format!("脚本错误: {msg}").into())
    });

    // url.join(base, path)
    engine.register_fn("url_join", |base: &str, path: &str| -> String {
        join_url(base, path)
    });

    let mut scope = Scope::new();
    ctx.inject_into_scope(&mut scope);

    // 执行；限制整体 wall-clock 通过 HTTP 超时间接控制
    let result = engine
        .eval_with_scope::<Dynamic>(&mut scope, script_body)
        .map_err(|e| {
            let msg = format_rhai_error(&e);
            IcodeError::validation(msg)
        })?;

    Ok(result)
}

fn format_rhai_error(err: &rhai::EvalAltResult) -> String {
    // 不回传可能含密钥的完整脚本上下文
    let s = err.to_string();
    // 截断
    s.chars().take(800).collect()
}

fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{path}")
    }
}

/// 供测试：同步执行简单脚本（无 HTTP）
#[cfg(test)]
pub fn execute_simple_for_test(script_body: &str) -> IcodeResult<BalanceSnapshot> {
    let mut engine = Engine::new();
    engine.set_max_operations(10_000);
    host_json::register(&mut engine);
    engine.register_fn("error", |msg: &str| -> Result<(), Box<rhai::EvalAltResult>> {
        Err(format!("脚本错误: {msg}").into())
    });

    let mut scope = Scope::new();
    scope.push_constant("api_key", "");
    scope.push_constant("now_ms", now_ms());
    let mut provider_map = rhai::Map::new();
    provider_map.insert("id".into(), Dynamic::from("p1"));
    provider_map.insert("slug".into(), Dynamic::from("test"));
    provider_map.insert("name".into(), Dynamic::from("Test"));
    provider_map.insert("base_url".into(), Dynamic::from("https://example.com"));
    provider_map.insert("provider_type".into(), Dynamic::from("custom"));
    provider_map.insert("is_enabled".into(), Dynamic::from(true));
    scope.push_constant("provider", Dynamic::from_map(provider_map));
    scope.push_constant("auth", Dynamic::from_map(rhai::Map::new()));
    let mut template_map = rhai::Map::new();
    template_map.insert("id".into(), Dynamic::from("t1"));
    template_map.insert("name".into(), Dynamic::from("t"));
    template_map.insert("kind".into(), Dynamic::from("balance"));
    scope.push_constant("template", Dynamic::from_map(template_map));

    let dynamic = engine
        .eval_with_scope::<Dynamic>(&mut scope, script_body)
        .map_err(|e| IcodeError::validation(e.to_string()))?;
    let mut snapshot = snapshot_map::map_to_snapshot(dynamic)?;
    if snapshot.updated_at <= 0 {
        snapshot.updated_at = now_ms();
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_from_script() {
        let script = r#"
#{
  items: [
    #{
      id: "balance",
      type: "amount",
      direction: "remaining",
      value: 12.34,
      currencySymbol: "¥",
      primary: true,
      label: "余额",
      period: "current"
    }
  ]
}
"#;
        let snap = execute_simple_for_test(script).unwrap();
        assert_eq!(snap.items.len(), 1);
        assert_eq!(snap.items[0].id, "balance");
    }

    #[test]
    fn test_invalid_snapshot() {
        let script = r#"#{ items: "not-array" }"#;
        let err = execute_simple_for_test(script).unwrap_err();
        assert_eq!(err.code, "VALIDATION");
    }
}
