pub const GROK_USAGE: &str = r#"// 额度监控：GET {base}/v1/usage
// 鉴权：x-api-key（使用系统变量 api_key）
// 返回：quota 金额 + 今日/累计 token + 状态

// 1) 拼 URL：优先用 provider.base_url；也可固定域名
let base = provider.base_url;
if base == "" {
    base = "https://sub.yxxb.eu.cc";
}
if base.ends_with("/") {
    base = base.sub_string(0, base.len() - 1);
}
// 若 base 已带 /v1，则只拼 /usage
let url = "";
if base.ends_with("/v1") {
    url = base + "/usage";
} else {
    url = base + "/v1/usage";
}

// 2) 请求头（Key 来自供应商认证解密后的 api_key）
let headers = #{
    "x-api-key": api_key,
    "Accept": "application/json",
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36 Edg/150.0.0.0"
};

let resp = http::get(url, headers);
if resp.status < 200 || resp.status >= 300 {
    error(`HTTP ${resp.status}: ${resp.body}`);
}

let data = json::parse(resp.body);

// 3) 账户/接口有效性
if data["isValid"] == false {
    error("额度接口返回 isValid=false");
}

// 4) 金额：quota.remaining / used / limit
let q = data["quota"];
let remaining = q["remaining"];
let used = q["used"];
let limit = q["limit"];
let unit = data["unit"];
if unit == () {
    unit = q["unit"];
}
let currency = "$";
if unit == "USD" {
    currency = "$";
}

// 5) Token：usage.today / usage.total
let usage = data["usage"];
let today = usage["today"];
let total = usage["total"];

// 6) 状态
let st = data["status"];
let status_value = "ok";
if st == "active" {
    status_value = "ok";
} else if st == "exhausted" {
    status_value = "exhausted";
} else if st == () {
    status_value = "ok";
} else {
    status_value = "unavailable";
}

// 7) 剩余金额占比（0–100），便于列表摘要
let pct = 0.0;
if limit != () && limit != 0 {
    pct = remaining * 100.0 / limit;
}

#{
    items: [
        // 主指标：剩余额度（金额）
        #{
            id: "balance",
            type: "amount",
            direction: "remaining",
            value: remaining,
            currencySymbol: currency,
            primary: true,
            label: "剩余额度",
            period: "current"
        },
        // 已用金额
        #{
            id: "used",
            type: "amount",
            direction: "used",
            value: used,
            currencySymbol: currency,
            label: "已用金额",
            period: "current"
        },
        // 额度上限
        #{
            id: "limit",
            type: "amount",
            direction: "limit",
            value: limit,
            currencySymbol: currency,
            label: "额度上限",
            period: "current"
        },
        // 剩余百分比
        #{
            id: "remaining_pct",
            type: "percent",
            value: pct,
            basis: "remaining",
            label: "剩余%",
            period: "current"
        },
        // 今日 Token
        #{
            id: "tokens_today",
            type: "token",
            used: today["total_tokens"],
            label: "今日 Token",
            period: "day"
        },
        // 累计 Token
        #{
            id: "tokens_total",
            type: "token",
            used: total["total_tokens"],
            label: "累计 Token",
            period: "total"
        },
        // 今日请求次数
        #{
            id: "requests_today",
            type: "integer",
            direction: "used",
            value: today["requests"],
            label: "今日请求",
            period: "day"
        },
        // 账户状态
        #{
            id: "status",
            type: "status",
            value: status_value,
            message: `mode=${data["mode"]}, status=${st}`
        }
    ]
}
"#;