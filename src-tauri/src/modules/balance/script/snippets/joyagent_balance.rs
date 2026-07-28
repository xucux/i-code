pub const JOYAGENT_BALANCE: &str = r#"// 京东 JoyAgent 积分额度查询
// 引擎：Rhai（语法接近 JS，map 用 #{ }，数组用 [ ]）
// 鉴权：Cookie（请在供应商的「扩展模板变量」中添加 key 为 `cookie` 的变量，
//   将完整 Cookie 字符串填入 value，脚本通过 variables["cookie"] 读取后作为 Cookie 头发送）。
// 有效期较短（含 joyya/thor 等 session），过期需重新抓取后更新变量值。
//
// 端点：GET https://agentrs.jd.com/api/saas/coupon/v1/getUserAmount
// host 与 provider.base_url（agentrs.jd.com）一致，host 白名单可过。
// 返回示例：
//   { "code": 0, "data": { "availablePoints": "80138.77", ... }, "msg": "成功" }
// 总积分 totalPoints 为常量 500000（接口不返回，由脚本内置）。

// 1) 构造 URL（固定端点，与 base_url 的路径无关但同 host）
let url = "https://agentrs.jd.com/api/saas/coupon/v1/getUserAmount";

// 2) 请求头：Cookie 用扩展变量注入；Referer / UA 必填，否则会被风控
let headers = #{
    "Cookie": variables["cookie"],
    "Accept": "application/json, text/plain, */*",
    "Referer": "https://joyagent.jd.com/",
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36 Edg/150.0.0.0"
};

let resp = http::get(url, headers);
if resp.status < 200 || resp.status >= 300 {
    error(`HTTP ${resp.status}: ${resp.body}`);
}

let json = json::parse(resp.body);
if json["code"] != 0 {
    error(`API error: code=${json["code"]}, msg=${json["msg"]}`);
}

let data = json["data"];
if data == () {
    error("返回缺少 data 字段");
}

// 3) 提取积分（接口返回字符串，需转为浮点）
let available_str = data["availablePoints"];
if available_str == () {
    error("返回缺少 data.availablePoints 字段");
}
let available = str::to_float(available_str);

// 4) 总量常量、已用、剩余百分比
let limit = 500000.0;
let used = limit - available;
let pct = 0.0;
if limit > 0 {
    pct = available * 100.0 / limit;
}

log::info(`JoyAgent 剩余积分: ${available} / ${limit}`);

#{
    items: [
        // 主指标：剩余积分
        #{
            id: "available_points",
            type: "amount",
            direction: "remaining",
            value: available,
            primary: true,
            label: "剩余积分",
            period: "current"
        },
        // 积分上限（常量）
        #{
            id: "limit_points",
            type: "amount",
            direction: "limit",
            value: limit,
            label: "积分上限",
            period: "current"
        },
        // 已用积分
        #{
            id: "used_points",
            type: "amount",
            direction: "used",
            value: used,
            label: "已用积分",
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
        // 其他金额字段（接口返回字符串，转浮点后展示）
        #{
            id: "coupon_amount",
            type: "amount",
            direction: "remaining",
            value: str::to_float(data["couponAmount"]),
            label: "优惠券金额",
            period: "current"
        },
        #{
            id: "wallet_amount",
            type: "amount",
            direction: "remaining",
            value: str::to_float(data["walletAmount"]),
            label: "钱包金额",
            period: "current"
        },
        #{
            id: "arrear_amount",
            type: "amount",
            direction: "used",
            value: str::to_float(data["arrearAmount"]),
            label: "欠款金额",
            period: "current"
        },
        // 账户状态
        #{
            id: "status",
            type: "status",
            value: "ok",
            message: `msg=${json["msg"]}`
        }
    ]
}
"#;
