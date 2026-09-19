//! # 请求头模板变量解析器
//!
//! 在网关转发时，将供应商附加请求头（`provider_extra_headers`）中的模板变量占位符
//! 替换为运行时实际值。
//!
//! ## 支持的占位符
//!
//! | 占位符 | 说明 | 示例 |
//! |--------|------|------|
//! | `${uuid()}` | 每次请求生成新的 UUID v4 | `msg_550e8400-e29b-...` |
//! | `${uuid_by_day()}` | 基于当天日期生成确定性 UUID（全天不变） | `ses_550e8400-e29b-...` |
//! | `${opencode_session()}` | OpenCode CLI 会话 ID（`ses_` + 时间戳编码 + 14 位随机字符，当天固定） | `ses_f525e4699ffe5hmrr6t1FiPVca` |
//! | `${opencode_request()}` | OpenCode CLI 请求 ID（`msg_` + 时间戳编码 + 14 位随机字符，每次请求刷新） | `msg_0ada1b968001cxm5rgh28JxlDH` |
//! | `${variables["key"]}` | 从供应商扩展变量中取值 | `variables["project_id"]` |
//!
//! ## OpenCode ID 生成算法
//!
//! OpenCode CLI（v1.18+）不再接受 UUID 形态的会话/请求 ID，服务端会校验其内部
//! snowflake 风格 ID 的时间戳新鲜度，乱填直接 403。算法（与 opencode 源码对齐，
//! 形如 `0ada1b968001cxm5rgh28JxlDH`，共 26 字符）：
//!
//! 1. `id = now_ms * 4096 + ctr`（ctr 为同一毫秒内的自增计数器，保证唯一）；
//! 2. 会话 ID 取按位取反（`~v`），请求 ID 不取反；
//! 3. 取低 48 位（6 字节）从高到低每字节两位十六进制编码，得 12 位时间戳段；
//! 4. 追加 14 个 base62 随机字符（源码 14 随机字节 % 62）。
//!
//! 会话 ID 按"当天"确定性缓存：同一天内保持不变，跨天自动更换，避免上游按
//! 会话维度统计用量时出现异常跳变。
//!
//! ## 设计原则
//!
//! - 纯字符串替换，不依赖 Rhai 引擎，保持轻量（仅用于请求头，非脚本执行）。
//! - 与 `variables` 系统（`script_variables_json`）共享相同的变量键空间。
//! - 解析顺序：`uuid_by_day` → `uuid` → `opencode_*` → `variables`，避免嵌套冲突。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use uuid::Uuid;

/// OpenCode 会话 ID 的确定性缓存（key = 当天日期字符串，value = `ses_...`）
///
/// `resolve_header_template_variables` 为纯函数并被多线程并发调用，
/// 使用进程级 Mutex 缓存保证同一毫秒/同一天的多次解析结果一致。
static OPENCODE_SESSION_CACHE: Mutex<Option<(String, String)>> = Mutex::new(None);

/// 解析请求头值中的模板变量占位符
///
/// # 参数
///
/// * `value` - 原始请求头值，可能包含 `${uuid()}`, `${uuid_by_day()}`,
///   `${opencode_session()}`, `${opencode_request()}`, `${variables["key"]}` 等占位符
/// * `script_variables` - 供应商扩展变量映射表（key → value）
///
/// # 返回
///
/// 替换后的字符串，所有占位符被替换为运行时实际值
pub fn resolve_header_template_variables(
    value: &str,
    script_variables: &HashMap<String, String>,
) -> String {
    let mut result = value.to_string();

    // 1. 先处理 uuid_by_day（全天不变，且优先级高）
    if result.contains("${uuid_by_day()}") {
        let day_uuid = generate_uuid_by_day();
        result = result.replace("${uuid_by_day()}", &day_uuid);
    }

    // 2. 处理 uuid（每次请求新生成）
    if result.contains("${uuid()}") {
        let uuid = Uuid::new_v4().to_string();
        result = result.replace("${uuid()}", &uuid);
    }

    // 3. 处理 OpenCode 专有 ID（会话 ID 当天固定，请求 ID 每次刷新）
    if result.contains("${opencode_session()}") {
        let session = generate_opencode_session_id();
        result = result.replace("${opencode_session()}", &session);
    }
    if result.contains("${opencode_request()}") {
        let request_id = generate_opencode_request_id();
        result = result.replace("${opencode_request()}", &request_id);
    }

    // 4. 处理 variables["key"] 模式（支持双引号和单引号）
    //    使用简单字符串扫描替换，避免正则依赖
    result = resolve_variables_refs(&result, script_variables);

    result
}

/// 解析 `${variables["key"]}` 或 `${variables['key']}` 占位符
///
/// 使用正则匹配，支持双引号和单引号包裹的 key 名。
/// 变量不存在时保留原占位符不变。
fn resolve_variables_refs(value: &str, script_variables: &HashMap<String, String>) -> String {
    // 正则：匹配 ${variables["key"]} 或 ${variables['key']}
    let re = regex::Regex::new(r#"\$\{variables\["([^"]+)"\]\}|\$\{variables\['([^']+)'\]\}"#)
        .expect("无效的正则表达式");

    re.replace_all(value, |caps: &regex::Captures| {
        let key = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| m.as_str())
            .unwrap_or("");
        script_variables
            .get(key)
            .cloned()
            // 找不到 key 时，用原始捕获文本保留原样
            .unwrap_or_else(|| {
                caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default()
            })
    })
    .to_string()
}

/// 基于当天日期生成确定性 UUID v5
///
/// 使用 `UUID v5` + `NAMESPACE_DNS` + `YYYY-MM-DD` 格式日期字符串，
/// 确保同一天内生成的 UUID 一致，跨天自动变化。
fn generate_uuid_by_day() -> String {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let namespace = Uuid::NAMESPACE_DNS;
    Uuid::new_v5(&namespace, today.as_bytes()).to_string()
}

/// OpenCode base62 字符表（`0-9` → `A-Z` → `a-z`）
const OPENCODE_BASE62_CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// 生成 OpenCode 风格 ID（snowflake 时间戳段 + base62 随机段）
///
/// 与 opencode 源码 `gen()` 算法对齐（形如 `0ada1b968001cxm5rgh28JxlDH`，26 字符）：
/// 1. `v = now_ms * 4096 + ctr`（`ctr` 进程级自增，保证同一毫秒不重复）；
/// 2. `negate = true` 时对 `v` 按位取反（会话 ID 用，请求 ID 不取反）；
/// 3. 取 **低 48 位**（6 字节），从高到低每字节两位十六进制 → 12 字符时间戳段；
/// 4. 追加 14 个 base62 随机字符（源码 14 随机字节 % 62）。
///
/// # 参数
///
/// * `negate` - 会话 ID 传 `true`（按位取反），请求 ID 传 `false`
fn generate_opencode_id(negate: bool) -> String {
    /// 同一毫秒内的自增计数器（opencode 源码 `ctr`，从 1 开始）
    static CTR: AtomicU64 = AtomicU64::new(0);
    /// 上一次使用的毫秒时间戳（用于检测毫秒边界）
    static LAST_MS: AtomicU64 = AtomicU64::new(0);

    let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    // 同一毫秒内 ctr 递增；跨毫秒重置为 1（与源码 `ctr = ts != last ? 1 : ctr + 1` 一致）
    let ctr = if LAST_MS.load(Ordering::Relaxed) != now_ms {
        LAST_MS.store(now_ms, Ordering::Relaxed);
        CTR.store(0, Ordering::Relaxed);
        1
    } else {
        CTR.fetch_add(1, Ordering::Relaxed) + 1
    };

    let mut v: u128 = (now_ms as u128) * 0x1000 + (ctr as u128);
    if negate {
        // 按位取反（源码为 BigInt 的 ~，低 48 位语义一致）
        v = !v;
    }

    // 低 48 位（6 字节）从高到低每字节两位十六进制 → 12 字符时间戳段
    const LOW_48: u128 = 0xFFFF_FFFF_FFFF;
    let mut id = format!("{:012x}", v & LOW_48);
    // 14 个 base62 随机字符（源码 14 随机字节 % 62）
    let mut random_bytes = [0u8; 14];
    rand::Rng::fill(&mut rand::thread_rng(), &mut random_bytes[..]);
    for byte in random_bytes {
        id.push(OPENCODE_BASE62_CHARS[(byte % 62) as usize] as char);
    }
    id
}

/// 生成 OpenCode 会话 ID（`ses_` 前缀，当天固定）
///
/// 进程级按天缓存：同一天内多次调用返回同一 ID，跨天自动更换，
/// 避免上游按会话维度统计用量时出现异常跳变。
fn generate_opencode_session_id() -> String {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    if let Ok(mut cache) = OPENCODE_SESSION_CACHE.lock() {
        if let Some((cached_day, cached_id)) = cache.as_ref() {
            if cached_day == &today {
                return cached_id.clone();
            }
        }
        let id = format!("ses_{}", generate_opencode_id(true));
        *cache = Some((today, id.clone()));
        return id;
    }
    // 缓存锁异常时退化为直接生成（理论上不会发生）
    format!("ses_{}", generate_opencode_id(true))
}

/// 生成 OpenCode 请求 ID（`msg_` 前缀，每次请求刷新）
fn generate_opencode_request_id() -> String {
    format!("msg_{}", generate_opencode_id(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_uuid() {
        let vars = HashMap::new();
        let result = resolve_header_template_variables("msg_${uuid()}", &vars);
        assert!(result.starts_with("msg_"));
        assert_eq!(result.len(), 40); // "msg_" + 36 chars UUID
    }

    #[test]
    fn test_resolve_uuid_by_day() {
        let vars = HashMap::new();
        let result = resolve_header_template_variables("ses_${uuid_by_day()}", &vars);
        assert!(result.starts_with("ses_"));
        assert_eq!(result.len(), 40); // "ses_" + 36 chars UUID

        // 同一天内应一致
        let result2 = resolve_header_template_variables("ses_${uuid_by_day()}", &vars);
        assert_eq!(result, result2);
    }

    #[test]
    fn test_resolve_variables() {
        let mut vars = HashMap::new();
        vars.insert("project_id".to_string(), "my-project".to_string());
        vars.insert("session_token".to_string(), "tok_abc123".to_string());

        let result = resolve_header_template_variables(
            r#"${variables["project_id"]}"#,
            &vars,
        );
        assert_eq!(result, "my-project");

        let result = resolve_header_template_variables(
            "${variables['session_token']}",
            &vars,
        );
        assert_eq!(result, "tok_abc123");
    }

    #[test]
    fn test_resolve_mixed() {
        let mut vars = HashMap::new();
        vars.insert("project".to_string(), "icode".to_string());

        let result = resolve_header_template_variables(
            r#"${variables["project"]}-${uuid_by_day()}"#,
            &vars,
        );
        assert!(result.starts_with("icode-"));
        assert_eq!(result.len(), 42); // "icode-" + 36 chars UUID
    }

    #[test]
    fn test_resolve_unknown_variable_keeps_original() {
        let vars = HashMap::new();
        let result = resolve_header_template_variables(
            r#"${variables["unknown_key"]}"#,
            &vars,
        );
        // 找不到时保留原样
        assert_eq!(result, r#"${variables["unknown_key"]}"#);
    }

    #[test]
    fn test_resolve_no_placeholder() {
        let vars = HashMap::new();
        let result = resolve_header_template_variables("x-opencode-project: global", &vars);
        assert_eq!(result, "x-opencode-project: global");
    }

    #[test]
    fn test_resolve_opencode_session() {
        let vars = HashMap::new();
        let result = resolve_header_template_variables("${opencode_session()}", &vars);
        // ses_ + 26 字符（12 位十六进制时间戳段 + 14 位 base62 随机段）
        assert!(result.starts_with("ses_"));
        assert_eq!(result.len(), 30);
        let ts_part = &result[4..16];
        assert!(ts_part.chars().all(|c| c.is_ascii_hexdigit()));

        // 同一天内应一致（进程级按天缓存）
        let result2 = resolve_header_template_variables("${opencode_session()}", &vars);
        assert_eq!(result, result2);
    }

    #[test]
    fn test_resolve_opencode_request() {
        let vars = HashMap::new();
        let result = resolve_header_template_variables("${opencode_request()}", &vars);
        assert!(result.starts_with("msg_"));
        assert_eq!(result.len(), 30);
        let ts_part = &result[4..16];
        assert!(ts_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_opencode_ids_freshness() {
        // 时间戳段解码：低 48 位 = (now_ms mod 2^36) << 12 | ctr
        // - msg（非取反）解码毫秒 = now_ms mod 2^36
        // - ses（取反）解码毫秒 = 2^36 - 1 - (now_ms mod 2^36)
        const MS_MOD: u64 = 1u64 << 36;
        const SEG_MASK: u64 = 0x0000_000F_FFFF_FFFF; // 2^36 - 1
        let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
        let expected = now_ms % MS_MOD;

        let request = resolve_header_template_variables("${opencode_request()}", &HashMap::new());
        let msg_ts = u64::from_str_radix(&request[4..16], 16).expect("hex");
        let msg_decoded = msg_ts >> 12;
        // 容许与生成时刻相差 1ms（测试执行跨毫秒边界）
        let drift = (msg_decoded as i64 - expected as i64).abs();
        assert!(
            drift <= 1,
            "请求 ID 时间戳段过期: decoded={} expected={}",
            msg_decoded,
            expected
        );

        // 会话 ID 为取反形态：与同毫秒请求 ID 的毫秒段之和应为 2^36 - 1（容许 ±2 跨毫秒）
        let session = resolve_header_template_variables("${opencode_session()}", &HashMap::new());
        let ses_ts = u64::from_str_radix(&session[4..16], 16).expect("hex");
        let ses_sum = (msg_ts >> 12) + (ses_ts >> 12);
        assert!(
            ses_sum >= SEG_MASK - 2 && ses_sum <= SEG_MASK + 2,
            "会话/请求 ID 取反关系不成立: msg={:x} ses={:x} sum={:x} mask={:x}",
            msg_ts >> 12,
            ses_ts >> 12,
            ses_sum,
            SEG_MASK
        );
    }

    #[test]
    fn test_opencode_session_is_base62_suffix() {
        // 随机段（12 位时间戳段之后的 14 位）必须全部落在 base62 字符表内
        let result = resolve_header_template_variables("${opencode_request()}", &HashMap::new());
        let valid = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
        for c in result[16..].chars() {
            assert!(valid.contains(c), "非 base62 字符: {}", c);
        }
    }
}