//! # 请求头去敏序列化
//!
//! 将一次请求的 `HeaderMap` 序列化为 JSON 字符串，敏感头（认证、密钥、Cookie 等）
//! 的值替换为 `"***"`，避免明文凭证进入自研 logger 日志页面 / 导出文件。

use axum::http::HeaderMap;
use std::collections::BTreeMap;

/// 出现在请求头名称中的敏感片段（小写，子串匹配），命中的头值一律脱敏
const SENSITIVE_FRAGMENTS: [&str; 7] = [
    "authorization",
    "api-key",
    "token",
    "secret",
    "credential",
    "cookie",
    "auth",
];

/// 判断请求头名称是否敏感（不区分大小写，子串匹配）
fn is_sensitive_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    SENSITIVE_FRAGMENTS.iter().any(|f| lower.contains(f))
}

/// 将请求头序列化为去敏 JSON 字符串
///
/// 无任何请求头时返回 `None`。
/// 输出示例：`{"accept":"*/*","authorization":"***","content-type":"application/json"}`
pub fn request_headers_to_json(headers: &HeaderMap) -> Option<String> {
    if headers.is_empty() {
        return None;
    }
    let mut map = BTreeMap::new();
    for (name, value) in headers {
        let key = name.as_str().to_string();
        let val = if is_sensitive_header(&key) {
            "***".to_string()
        } else {
            value
                .to_str()
                .map(|s| s.to_owned())
                .unwrap_or_else(|_| "<binary>".to_string())
        };
        map.insert(key, val);
    }
    if map.is_empty() {
        return None;
    }
    Some(serde_json::json!(map).to_string())
}
