//! # HTTP Host Functions
//!
//! `http.get` / `http.post` / `http.request` / `http.get_json`
//! 约束：超时、host 白名单、仅 http(s)、响应体积上限。

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use rhai::{Dynamic, Engine, Map};

/// 响应 body 上限 2MB
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
/// 全局超时上限
const MAX_TIMEOUT_MS: u64 = 30_000;

/// HTTP 运行时状态（共享给 host 闭包）
pub struct HttpHostState {
    timeout: Duration,
    allowed_hosts: HashSet<String>,
    api_key_redact: Option<String>,
}

impl HttpHostState {
    pub fn new(
        timeout_ms: u64,
        provider_base_url: String,
        extra_hosts: Vec<String>,
        api_key: Option<String>,
    ) -> Self {
        let mut allowed_hosts = HashSet::new();
        if let Some(host) = host_from_url(&provider_base_url) {
            allowed_hosts.insert(host);
        }
        for h in extra_hosts {
            let h = h.trim().to_lowercase();
            if !h.is_empty() {
                allowed_hosts.insert(h);
            }
        }
        let timeout_ms = timeout_ms.min(MAX_TIMEOUT_MS).max(1000);
        Self {
            timeout: Duration::from_millis(timeout_ms),
            allowed_hosts,
            api_key_redact: api_key.filter(|s| !s.is_empty()),
        }
    }

    fn ensure_url_allowed(&self, url: &str) -> Result<(), Box<rhai::EvalAltResult>> {
        let parsed = url::Url::parse(url).map_err(|e| format!("无效 URL: {e}"))?;
        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" {
            return Err("仅允许 http/https URL".into());
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| "URL 缺少 host".to_string())?
            .to_lowercase();
        if self.allowed_hosts.is_empty() {
            // 无 base_url 时放行会不安全；至少要求配置了某个 host
            return Err("未配置允许的 host（provider.base_url 为空）".into());
        }
        if !self.allowed_hosts.contains(&host) {
            return Err(format!(
                "host '{host}' 不在白名单内（允许: {}）",
                self.allowed_hosts
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .into());
        }
        Ok(())
    }

    fn do_request(
        &self,
        method: &str,
        url: &str,
        body: Option<&str>,
        headers: Option<Map>,
    ) -> Result<Map, Box<rhai::EvalAltResult>> {
        self.ensure_url_allowed(url)?;

        let builder = reqwest::blocking::Client::builder()
            .timeout(self.timeout);
        // 应用全局代理配置
        let builder = crate::modules::shared::apply_global_proxy_blocking(builder);
        let client = builder
            .build()
            .map_err(|e| format!("HTTP 客户端创建失败: {e}"))?;

        let method = method.to_uppercase();
        let m = method
            .parse::<reqwest::Method>()
            .map_err(|_| format!("不支持的 HTTP 方法: {method}"))?;

        let mut req = client.request(m, url);
        if let Some(hmap) = headers {
            for (k, v) in hmap.iter() {
                let val = v.clone().into_string().unwrap_or_else(|_| v.to_string());
                req = req.header(k.as_str(), val);
            }
        }
        if let Some(b) = body {
            req = req.body(b.to_string());
        }

        // 开发日志：脱敏 URL path + method
        let path = url::Url::parse(url)
            .map(|u| u.path().to_string())
            .unwrap_or_else(|_| url.to_string());
        log::debug!("[balance-script] HTTP {} {}", method, path);

        let response = req
            .send()
            .map_err(|e| format!("HTTP 请求失败: {}", redact(&e.to_string(), &self.api_key_redact)))?;

        let status = response.status().as_u16() as i64;
        let mut resp_headers = Map::new();
        for (k, v) in response.headers().iter() {
            if let Ok(s) = v.to_str() {
                resp_headers.insert(k.as_str().into(), Dynamic::from(s.to_string()));
            }
        }

        let bytes = response
            .bytes()
            .map_err(|e| format!("读取响应失败: {e}"))?;
        if bytes.len() > MAX_BODY_BYTES {
            return Err(format!(
                "响应体过大（{} bytes，上限 {}）",
                bytes.len(),
                MAX_BODY_BYTES
            )
            .into());
        }
        let body_str = String::from_utf8_lossy(&bytes).to_string();

        log::debug!(
            "[balance-script] HTTP {} {} → {} ({} bytes)",
            method,
            path,
            status,
            body_str.len()
        );

        let mut result = Map::new();
        result.insert("status".into(), Dynamic::from(status));
        result.insert("body".into(), Dynamic::from(body_str));
        result.insert("headers".into(), Dynamic::from_map(resp_headers));
        Ok(result)
    }
}

fn host_from_url(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
}

fn redact(text: &str, api_key: &Option<String>) -> String {
    if let Some(key) = api_key {
        if !key.is_empty() {
            return text.replace(key, "***");
        }
    }
    text.to_string()
}

/// 注册 HTTP host 函数
///
/// - 扁平名：`http_get` / `http_post` / `http_request` / `http_get_json`
/// - 静态模块：`http::get` / `http::post` / `http::request` / `http::get_json`
///
/// 注意：Rhai 静态模块必须用 `::`，写成 `http.get` 会被解析为变量属性访问并报
/// `Variable not found: http`。
pub fn register(engine: &mut Engine, state: Arc<HttpHostState>) {
    // 使用闭包捕获 state
    let s1 = state.clone();
    engine.register_fn(
        "http_get",
        move |url: &str, headers: Dynamic| -> Result<Map, Box<rhai::EvalAltResult>> {
            let headers_map = dynamic_to_map(headers);
            s1.do_request("GET", url, None, headers_map)
        },
    );

    let s2 = state.clone();
    engine.register_fn(
        "http_get",
        move |url: &str| -> Result<Map, Box<rhai::EvalAltResult>> {
            s2.do_request("GET", url, None, None)
        },
    );

    let s3 = state.clone();
    engine.register_fn(
        "http_post",
        move |url: &str, body: &str, headers: Dynamic| -> Result<Map, Box<rhai::EvalAltResult>> {
            let headers_map = dynamic_to_map(headers);
            s3.do_request("POST", url, Some(body), headers_map)
        },
    );

    let s4 = state.clone();
    engine.register_fn(
        "http_post",
        move |url: &str, body: &str| -> Result<Map, Box<rhai::EvalAltResult>> {
            s4.do_request("POST", url, Some(body), None)
        },
    );

    let s5 = state.clone();
    engine.register_fn(
        "http_request",
        move |method: &str, url: &str, body: Dynamic, headers: Dynamic| -> Result<Map, Box<rhai::EvalAltResult>> {
            let body_str = if body.is_unit() || body.is_string() && body.clone().into_string().map(|s| s.is_empty()).unwrap_or(true) {
                None
            } else if body.is_string() {
                Some(body.into_string().unwrap_or_default())
            } else {
                // 尝试 JSON 序列化
                Some(dynamic_to_json_string(&body)?)
            };
            let headers_map = dynamic_to_map(headers);
            s5.do_request(method, url, body_str.as_deref(), headers_map)
        },
    );

    let s6 = state.clone();
    engine.register_fn(
        "http_get_json",
        move |url: &str, headers: Dynamic| -> Result<Dynamic, Box<rhai::EvalAltResult>> {
            let headers_map = dynamic_to_map(headers);
            let resp = s6.do_request("GET", url, None, headers_map)?;
            let status = resp
                .get("status")
                .and_then(|v| v.as_int().ok())
                .unwrap_or(0);
            if !(200..300).contains(&status) {
                let body = resp
                    .get("body")
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                return Err(format!("HTTP {status}: {body}").into());
            }
            let body = resp
                .get("body")
                .cloned()
                .unwrap_or(Dynamic::UNIT)
                .into_string()
                .unwrap_or_default();
            json_parse_dynamic(&body)
        },
    );

    let s7 = state.clone();
    engine.register_fn(
        "http_get_json",
        move |url: &str| -> Result<Dynamic, Box<rhai::EvalAltResult>> {
            let resp = s7.do_request("GET", url, None, None)?;
            let status = resp
                .get("status")
                .and_then(|v| v.as_int().ok())
                .unwrap_or(0);
            if !(200..300).contains(&status) {
                let body = resp
                    .get("body")
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                return Err(format!("HTTP {status}: {body}").into());
            }
            let body = resp
                .get("body")
                .cloned()
                .unwrap_or(Dynamic::UNIT)
                .into_string()
                .unwrap_or_default();
            json_parse_dynamic(&body)
        },
    );

    // 兼容提案中的 http.get 命名：注册模块
    let mut module = rhai::Module::new();
    let s_get = state.clone();
    module.set_native_fn("get", move |url: &str| {
        s_get.do_request("GET", url, None, None).map(Dynamic::from_map)
    });
    let s_get2 = state.clone();
    module.set_native_fn("get", move |url: &str, headers: Map| {
        s_get2
            .do_request("GET", url, None, Some(headers))
            .map(Dynamic::from_map)
    });
    let s_post = state.clone();
    module.set_native_fn("post", move |url: &str, body: &str| {
        s_post
            .do_request("POST", url, Some(body), None)
            .map(Dynamic::from_map)
    });
    let s_post2 = state.clone();
    module.set_native_fn("post", move |url: &str, body: &str, headers: Map| {
        s_post2
            .do_request("POST", url, Some(body), Some(headers))
            .map(Dynamic::from_map)
    });
    let s_req = state.clone();
    module.set_native_fn("request", move |method: &str, url: &str| {
        s_req
            .do_request(method, url, None, None)
            .map(Dynamic::from_map)
    });
    let s_gj = state.clone();
    module.set_native_fn("get_json", move |url: &str| {
        let resp = s_gj.do_request("GET", url, None, None)?;
        let status = resp.get("status").and_then(|v| v.as_int().ok()).unwrap_or(0);
        if !(200..300).contains(&status) {
            let body = resp.get("body").map(|v| v.to_string()).unwrap_or_default();
            return Err(format!("HTTP {status}: {body}").into());
        }
        let body = resp
            .get("body")
            .cloned()
            .unwrap_or(Dynamic::UNIT)
            .into_string()
            .unwrap_or_default();
        json_parse_dynamic(&body)
    });

    engine.register_static_module("http", module.into());
}

fn dynamic_to_map(d: Dynamic) -> Option<Map> {
    if d.is_unit() {
        return None;
    }
    d.try_cast::<Map>()
}

fn dynamic_to_json_string(d: &Dynamic) -> Result<String, Box<rhai::EvalAltResult>> {
    let v = dynamic_to_serde(d)?;
    serde_json::to_string(&v).map_err(|e| format!("JSON 序列化失败: {e}").into())
}

fn json_parse_dynamic(text: &str) -> Result<Dynamic, Box<rhai::EvalAltResult>> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("JSON 解析失败: {e}"))?;
    Ok(serde_to_dynamic(&v))
}

fn serde_to_dynamic(v: &serde_json::Value) -> Dynamic {
    match v {
        serde_json::Value::Null => Dynamic::UNIT,
        serde_json::Value::Bool(b) => Dynamic::from(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Dynamic::from(i)
            } else if let Some(f) = n.as_f64() {
                Dynamic::from(f)
            } else {
                Dynamic::from(n.to_string())
            }
        }
        serde_json::Value::String(s) => Dynamic::from(s.clone()),
        serde_json::Value::Array(arr) => {
            let list: rhai::Array = arr.iter().map(serde_to_dynamic).collect();
            Dynamic::from(list)
        }
        serde_json::Value::Object(obj) => {
            let mut map = Map::new();
            for (k, val) in obj {
                map.insert(k.clone().into(), serde_to_dynamic(val));
            }
            Dynamic::from_map(map)
        }
    }
}

fn dynamic_to_serde(d: &Dynamic) -> Result<serde_json::Value, Box<rhai::EvalAltResult>> {
    if d.is_unit() {
        return Ok(serde_json::Value::Null);
    }
    if let Some(b) = d.clone().try_cast::<bool>() {
        return Ok(serde_json::Value::Bool(b));
    }
    if let Ok(i) = d.as_int() {
        return Ok(serde_json::json!(i));
    }
    if let Ok(f) = d.as_float() {
        return Ok(serde_json::json!(f));
    }
    if let Ok(s) = d.clone().into_string() {
        return Ok(serde_json::Value::String(s));
    }
    if let Some(arr) = d.clone().try_cast::<rhai::Array>() {
        let mut out = Vec::new();
        for item in arr {
            out.push(dynamic_to_serde(&item)?);
        }
        return Ok(serde_json::Value::Array(out));
    }
    if let Some(map) = d.clone().try_cast::<Map>() {
        let mut obj = serde_json::Map::new();
        for (k, v) in map {
            obj.insert(k.to_string(), dynamic_to_serde(&v)?);
        }
        return Ok(serde_json::Value::Object(obj));
    }
    Ok(serde_json::Value::String(d.to_string()))
}
