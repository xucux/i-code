pub const HTTP_SET_PROXY_MANUAL: &str = r#"// 示例：手动为 http 客户端设置固定代理
// 适用场景：脚本需要固定代理，且不想依赖应用的代理配置
// 支持的代理 URL 格式：http://host:port / http://user:pass@host:port / socks5://host:port

http::set_proxy("http://127.0.0.1:7890");

let base = provider.base_url;
if base.ends_with("/") {
    base = base.sub_string(0, base.len() - 1);
}
let url = base + "/v1/user/balance";

let headers = #{
    "Authorization": "Bearer " + api_key,
    "Accept": "application/json"
};

let resp = http::get(url, headers);
if resp.status < 200 || resp.status >= 300 {
    error(`HTTP ${resp.status}: ${resp.body}`);
}

let data = json::parse(resp.body);

#{
    items: [
        #{
            id: "balance",
            type: "amount",
            direction: "remaining",
            value: data["balance"],
            currencySymbol: "$",
            primary: true,
            label: "余额",
            period: "current"
        }
    ]
}
"#;

pub const HTTP_SET_PROXY_FROM_VARS: &str = r#"// 示例：根据应用代理配置动态设置 http 代理
// 优先使用供应商级代理；若供应商代理策略为 global 或未配置，则回退到全局代理
// proxy 变量由运行时自动注入，脚本只读

let proxy_url = "";

if proxy.provider_type == "http" || proxy.provider_type == "socks" {
    proxy_url = proxy.provider_url;
} else if proxy.global_enabled && (proxy.global_type == "http" || proxy.global_type == "socks") {
    proxy_url = proxy.global_url;
}

if proxy_url != "" {
    http::set_proxy(proxy_url);
}

let base = provider.base_url;
if base.ends_with("/") {
    base = base.sub_string(0, base.len() - 1);
}
let url = base + "/v1/user/balance";

let headers = #{
    "Authorization": "Bearer " + api_key,
    "Accept": "application/json"
};

let resp = http::get(url, headers);
if resp.status < 200 || resp.status >= 300 {
    error(`HTTP ${resp.status}: ${resp.body}`);
}

let data = json::parse(resp.body);

#{
    items: [
        #{
            id: "balance",
            type: "amount",
            direction: "remaining",
            value: data["balance"],
            currencySymbol: "$",
            primary: true,
            label: "余额",
            period: "current"
        }
    ]
}
"#;

pub const PROXIED_HTTP_AUTO: &str = r#"// 示例：使用 proxied_http 自动走应用代理配置
// 代理解析优先级：
//   1. 供应商级 socks/http 代理
//   2. 供应商代理策略为 global（或未配置）且全局代理已启用
//   3. 供应商策略为 direct 或全局代理关闭 → 直连
//
// 与 http 模块 API 完全一致，只是多了自动代理

let base = provider.base_url;
if base.ends_with("/") {
    base = base.sub_string(0, base.len() - 1);
}
let url = base + "/v1/user/balance";

let headers = #{
    "Authorization": "Bearer " + api_key,
    "Accept": "application/json"
};

let resp = proxied_http::get(url, headers);
if resp.status < 200 || resp.status >= 300 {
    error(`HTTP ${resp.status}: ${resp.body}`);
}

let data = json::parse(resp.body);

#{
    items: [
        #{
            id: "balance",
            type: "amount",
            direction: "remaining",
            value: data["balance"],
            currencySymbol: "$",
            primary: true,
            label: "余额",
            period: "current"
        }
    ]
}
"#;
