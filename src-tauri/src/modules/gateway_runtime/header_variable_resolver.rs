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
//! | `${variables["key"]}` | 从供应商扩展变量中取值 | `variables["project_id"]` |
//!
//! ## 设计原则
//!
//! - 纯字符串替换，不依赖 Rhai 引擎，保持轻量（仅用于请求头，非脚本执行）。
//! - 与 `variables` 系统（`script_variables_json`）共享相同的变量键空间。
//! - 解析顺序：`uuid_by_day` → `uuid` → `variables`，避免嵌套冲突。

use std::collections::HashMap;

use uuid::Uuid;

/// 解析请求头值中的模板变量占位符
///
/// # 参数
///
/// * `value` - 原始请求头值，可能包含 `${uuid()}`, `${uuid_by_day()}`, `${variables["key"]}` 等占位符
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

    // 3. 处理 variables["key"] 模式（支持双引号和单引号）
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
}