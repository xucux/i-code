pub const BALANCE_GET_BEARER: &str = r#"// 示例：OpenAI 兼容 /user/balance 风格
// 引擎：Rhai（语法接近 JS，map 用 #{ }，数组用 [ ]）
// 模块函数用 :: 调用，如 http::get / json::parse（不是 http.get）
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
// 按实际响应改写路径
let total = data["balance"];

#{
    items: [
        #{
            id: "balance",
            type: "amount",
            direction: "remaining",
            value: total,
            currencySymbol: "$",
            primary: true,
            label: "余额",
            period: "current"
        }
    ]
}
"#;