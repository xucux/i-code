pub const MIMO_BALANCE: &str = r#"// 小米 MiMo 按量计费额度查询
// 引擎：Rhai（语法接近 JS，map 用 #{ }，数组用 [ ]）
// Cookie 请填入供应商的 API Key 字段，参考格式：
//   serviceToken="..."; xiaomichatbot_ph="..."; userId=...; ...
// 有效期约 24 小时，过期需重新获取。

let base = "https://platform.xiaomimimo.com";
let url = url_join(base, "/api/v1/balance");

let headers = #{
    "Cookie": api_key,
    "Accept": "application/json",
    "User-Agent": "i-code/1.0"
};

let resp = http::get(url, headers);
if resp.status < 200 || resp.status >= 300 {
    error(`HTTP ${resp.status}: ${resp.body}`);
}

let json = json::parse(resp.body);
if json["code"] != 0 {
    error(`API error: ${json["message"]}`);
}

let data = json["data"];
let currency = data["currency"];
if currency == () { currency = "CNY"; }

let items = [];

items.push(#{
    id: "balance",
    type: "amount",
    direction: "remaining",
    value: data["balance"],
    currencySymbol: currency,
    primary: true,
    label: "总余额",
    period: "current"
});

items.push(#{
    id: "cashBalance",
    type: "amount",
    direction: "remaining",
    value: data["cashBalance"],
    currencySymbol: currency,
    label: "现金余额",
    period: "current"
});

items.push(#{
    id: "giftBalance",
    type: "amount",
    direction: "remaining",
    value: data["giftBalance"],
    currencySymbol: currency,
    label: "赠送余额",
    period: "current"
});

items.push(#{
    id: "frozenBalance",
    type: "amount",
    direction: "remaining",
    value: data["frozenBalance"],
    currencySymbol: currency,
    label: "冻结金额",
    period: "current"
});

items.push(#{
    id: "overdraftLimit",
    type: "amount",
    direction: "limit",
    value: data["overdraftLimit"],
    currencySymbol: currency,
    label: "透支额度",
    period: "current"
});

items.push(#{
    id: "remainingOverdraftLimit",
    type: "amount",
    direction: "remaining",
    value: data["remainingOverdraftLimit"],
    currencySymbol: currency,
    label: "剩余透支额度",
    period: "current"
});

#{
    items: items
}
"#;