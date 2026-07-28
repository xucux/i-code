pub const MIMO_TOKEN_PLAN: &str = r#"// 小米 MiMo 套餐积分查询
// 引擎：Rhai（语法接近 JS，map 用 #{ }，数组用 [ ]）
// Cookie 请在供应商的「扩展模板变量」中添加 key 为 `cookie` 的变量，参考格式：
//   serviceToken="..."; xiaomichatbot_ph="..."; userId=...; ...
// 有效期约 24 小时，过期需重新获取。
// 积分值单位：百M（1000000 原始积分 = 1 百M）

let base = "https://platform.xiaomimimo.com";
let url = url_join(base, "/api/v1/tokenPlan/usage");

let headers = #{
    "Cookie": variables["cookie"],
    "Accept": "application/json",
    "User-Agent": "i-code/1.0"
};

let resp = http::get(url, headers);
if resp.status < 200 || resp.status >= 300 {
    error(`HTTP ${resp.status}: ${resp.body}`);
}

let json = json::parse(resp.body);
let items_raw = json["data"]["usage"]["items"];

let SCALE = 1000000;
let items = [];

// 遍历 API 返回的套餐项
for item in items_raw {
    let name = item["name"];

    // 匹配已知套餐名称
    let plan_name = "";
    if name == "plan_total_token" {
        plan_name = "套餐积分";
    } else if name == "compensation_total_token" {
        plan_name = "补偿积分";
    } else {
        // 未知项目跳过
        continue;
    }

    let used = item["used"];
    let limit = item["limit"];
    let remaining = math::max(limit - used, 0);

    // 转换为百M单位
    let used_scaled = used / SCALE;
    let limit_scaled = limit / SCALE;
    let remaining_scaled = remaining / SCALE;

    // 计算百分比
    let percent = 0.0;
    if limit > 0 {
        percent = (used / limit) * 100.0;
    }

    // 套餐用量指标
    items.push(#{
        id: name,
        type: "token",
        used: used_scaled,
        limit: limit_scaled,
        remaining: remaining_scaled,
        label: plan_name,
        period: "current",
        primary: true
    });

    // 消耗进度百分比指标
    items.push(#{
        id: name + "_percent",
        type: "percent",
        value: percent,
        basis: "used",
        label: plan_name + "消耗",
        period: "current"
    });
}

if items.len() == 0 {
    error("未找到任何套餐数据（预期字段: plan_total_token / compensation_total_token）");
}

#{
    items: items
}
"#;