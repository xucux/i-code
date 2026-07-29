//! # 带代理的 HTTP Host Functions
//!
//! `proxied_http.get` / `proxied_http.post` / `proxied_http.request` / `proxied_http.get_json`
//!
//! 与 `http` 模块功能一致，但**自动应用供应商代理或全局代理设置**。
//! 适用于脚本中需要通过应用代理配置发送请求的场景。
//!
//! ## 代理解析优先级
//!
//! 1. 若供应商配置了 `socks`/`http` 代理 → 使用供应商代理
//! 2. 若供应商代理策略为 `global`（或未配置）且全局代理已启用 → 使用全局代理
//! 3. 若供应商代理策略为 `direct` → 强制直连
//! 4. 以上均不满足 → 强制直连（不读取系统环境变量代理）

use std::sync::Arc;

use rhai::{Dynamic, Engine, Map};

use super::host_http::HttpHostState;

/// 注册带代理的 HTTP host 函数
///
/// - 模块名：`proxied_http`
/// - 提供与 `http` 相同的函数签名，但请求自动走供应商/全局代理
/// - 不提供 `set_proxy`（代理配置由系统自动解析）
pub fn register(engine: &mut Engine, state: Arc<HttpHostState>, provider_proxy_json: Option<String>) {
    // 预解析代理 URL，供所有请求共用
    let resolved_proxy = resolve_proxy_url(provider_proxy_json.as_deref());

    let s1 = state.clone();
    let proxy1 = resolved_proxy.clone();
    engine.register_fn(
        "proxied_http_get",
        move |url: &str, headers: Dynamic| -> Result<Map, Box<rhai::EvalAltResult>> {
            let headers_map = if headers.is_unit() { None } else { headers.try_cast::<Map>() };
            s1.do_request_with_proxy("GET", url, None, headers_map, proxy1.as_deref())
        },
    );

    let s2 = state.clone();
    let proxy2 = resolved_proxy.clone();
    engine.register_fn(
        "proxied_http_get",
        move |url: &str| -> Result<Map, Box<rhai::EvalAltResult>> {
            s2.do_request_with_proxy("GET", url, None, None, proxy2.as_deref())
        },
    );

    let s3 = state.clone();
    let proxy3 = resolved_proxy.clone();
    engine.register_fn(
        "proxied_http_post",
        move |url: &str, body: &str, headers: Dynamic| -> Result<Map, Box<rhai::EvalAltResult>> {
            let headers_map = if headers.is_unit() { None } else { headers.try_cast::<Map>() };
            s3.do_request_with_proxy("POST", url, Some(body), headers_map, proxy3.as_deref())
        },
    );

    let s4 = state.clone();
    let proxy4 = resolved_proxy.clone();
    engine.register_fn(
        "proxied_http_post",
        move |url: &str, body: &str| -> Result<Map, Box<rhai::EvalAltResult>> {
            s4.do_request_with_proxy("POST", url, Some(body), None, proxy4.as_deref())
        },
    );

    let s5 = state.clone();
    let proxy5 = resolved_proxy.clone();
    engine.register_fn(
        "proxied_http_request",
        move |method: &str, url: &str, body: Dynamic, headers: Dynamic| -> Result<Map, Box<rhai::EvalAltResult>> {
            let body_str = if body.is_unit() || body.is_string() && body.clone().into_string().map(|s| s.is_empty()).unwrap_or(true) {
                None
            } else if body.is_string() {
                Some(body.into_string().unwrap_or_default())
            } else {
                Some(crate::modules::balance::script::host_http::dynamic_to_json_string(&body)?)
            };
            let headers_map = if headers.is_unit() { None } else { headers.try_cast::<Map>() };
            s5.do_request_with_proxy(method, url, body_str.as_deref(), headers_map, proxy5.as_deref())
        },
    );

    let s6 = state.clone();
    let proxy6 = resolved_proxy.clone();
    engine.register_fn(
        "proxied_http_get_json",
        move |url: &str, headers: Dynamic| -> Result<Dynamic, Box<rhai::EvalAltResult>> {
            let headers_map = if headers.is_unit() { None } else { headers.try_cast::<Map>() };
            let resp = s6.do_request_with_proxy("GET", url, None, headers_map, proxy6.as_deref())?;
            let status = resp.get("status").and_then(|v| v.as_int().ok()).unwrap_or(0);
            if !(200..300).contains(&status) {
                let body = resp.get("body").map(|v| v.to_string()).unwrap_or_default();
                return Err(format!("HTTP {status}: {body}").into());
            }
            let body = resp.get("body").cloned().unwrap_or(Dynamic::UNIT).into_string().unwrap_or_default();
            crate::modules::balance::script::host_http::json_parse_dynamic(&body)
        },
    );

    let s7 = state.clone();
    let proxy7 = resolved_proxy.clone();
    engine.register_fn(
        "proxied_http_get_json",
        move |url: &str| -> Result<Dynamic, Box<rhai::EvalAltResult>> {
            let resp = s7.do_request_with_proxy("GET", url, None, None, proxy7.as_deref())?;
            let status = resp.get("status").and_then(|v| v.as_int().ok()).unwrap_or(0);
            if !(200..300).contains(&status) {
                let body = resp.get("body").map(|v| v.to_string()).unwrap_or_default();
                return Err(format!("HTTP {status}: {body}").into());
            }
            let body = resp.get("body").cloned().unwrap_or(Dynamic::UNIT).into_string().unwrap_or_default();
            crate::modules::balance::script::host_http::json_parse_dynamic(&body)
        },
    );

    // 注册静态模块 proxied_http::get / post / request / get_json
    let mut module = rhai::Module::new();

    let s_get = state.clone();
    let proxy_get = resolved_proxy.clone();
    module.set_native_fn("get", move |url: &str| {
        s_get.do_request_with_proxy("GET", url, None, None, proxy_get.as_deref()).map(Dynamic::from_map)
    });

    let s_get2 = state.clone();
    let proxy_get2 = resolved_proxy.clone();
    module.set_native_fn("get", move |url: &str, headers: Map| {
        s_get2.do_request_with_proxy("GET", url, None, Some(headers), proxy_get2.as_deref()).map(Dynamic::from_map)
    });

    let s_post = state.clone();
    let proxy_post = resolved_proxy.clone();
    module.set_native_fn("post", move |url: &str, body: &str| {
        s_post.do_request_with_proxy("POST", url, Some(body), None, proxy_post.as_deref()).map(Dynamic::from_map)
    });

    let s_post2 = state.clone();
    let proxy_post2 = resolved_proxy.clone();
    module.set_native_fn("post", move |url: &str, body: &str, headers: Map| {
        s_post2.do_request_with_proxy("POST", url, Some(body), Some(headers), proxy_post2.as_deref()).map(Dynamic::from_map)
    });

    let s_req = state.clone();
    let proxy_req = resolved_proxy.clone();
    module.set_native_fn("request", move |method: &str, url: &str| {
        s_req.do_request_with_proxy(method, url, None, None, proxy_req.as_deref()).map(Dynamic::from_map)
    });

    let s_gj = state.clone();
    let proxy_gj = resolved_proxy;
    module.set_native_fn("get_json", move |url: &str| {
        let resp = s_gj.do_request_with_proxy("GET", url, None, None, proxy_gj.as_deref())?;
        let status = resp.get("status").and_then(|v| v.as_int().ok()).unwrap_or(0);
        if !(200..300).contains(&status) {
            let body = resp.get("body").map(|v| v.to_string()).unwrap_or_default();
            return Err(format!("HTTP {status}: {body}").into());
        }
        let body = resp.get("body").cloned().unwrap_or(Dynamic::UNIT).into_string().unwrap_or_default();
        crate::modules::balance::script::host_http::json_parse_dynamic(&body)
    });

    engine.register_static_module("proxied_http", module.into());
}

/// 解析代理 URL：供应商代理 > 全局代理
///
/// 返回 `Some(proxy_url)` 表示需要走代理；`None` 表示直连。
fn resolve_proxy_url(provider_proxy_json: Option<&str>) -> Option<String> {
    // 1. 尝试解析供应商代理配置
    if let Some(json) = provider_proxy_json {
        if let Ok(cfg) = serde_json::from_str::<crate::modules::shared::ProviderProxyConfig>(json) {
            match cfg.proxy_type {
                crate::modules::shared::ProviderProxyType::Direct => {
                    log::trace!("[proxied-http] provider strategy=direct → no proxy");
                    return None;
                }
                crate::modules::shared::ProviderProxyType::Socks
                | crate::modules::shared::ProviderProxyType::Http => {
                    if let Some(url) = cfg.url.as_deref().filter(|s| !s.is_empty()) {
                        log::trace!("[proxied-http] provider strategy={:?} → proxy={}", cfg.proxy_type, crate::modules::shared::redact_proxy_url(url));
                        return Some(url.to_string());
                    }
                }
                crate::modules::shared::ProviderProxyType::Global => {
                    // 回退到全局代理
                }
            }
        }
    }

    // 2. 回退到全局代理
    let settings = match crate::modules::settings::repository::find() {
        Ok(s) => s,
        Err(_) => return None,
    };
    if !settings.global_proxy_enabled {
        log::trace!("[proxied-http] global enabled=false → no proxy");
        return None;
    }
    let json = match settings.global_proxy_json.as_deref() {
        Some(j) => j,
        None => return None,
    };
    let cfg: crate::modules::shared::ProxyConfig = match serde_json::from_str(json) {
        Ok(c) => c,
        Err(_) => return None,
    };
    match cfg.proxy_type {
        crate::modules::shared::ProxyType::Direct => None,
        crate::modules::shared::ProxyType::System => {
            // 系统代理模式：不指定具体 proxy_url，让 reqwest 自行读取环境变量
            // 通过特殊标记 "system" 告知 do_request_with_proxy
            Some("system".to_string())
        }
        crate::modules::shared::ProxyType::Http | crate::modules::shared::ProxyType::Socks => {
            cfg.url.as_deref().filter(|s| !s.is_empty()).map(|u| {
                log::trace!("[proxied-http] global strategy={:?} → proxy={}", cfg.proxy_type, crate::modules::shared::redact_proxy_url(u));
                u.to_string()
            })
        }
    }
}
