//! # 内置脚本 Snippet

use crate::modules::script_template::types::ScriptSnippet;

/// 返回内置 snippet 列表
pub fn list_snippets() -> Vec<ScriptSnippet> {
    vec![
        ScriptSnippet {
            id: "balance-get-bearer".into(),
            name: "余额 GET + Bearer".into(),
            description: "OpenAI 兼容 /user/balance 风格，Bearer 鉴权".into(),
            body: BALANCE_GET_BEARER.into(),
        },
        ScriptSnippet {
            id: "items-skeleton".into(),
            name: "返回 items 骨架".into(),
            description: "仅返回标准 BalanceSnapshot 结构，便于改写".into(),
            body: ITEMS_SKELETON.into(),
        },
        ScriptSnippet {
            id: "bearer-header".into(),
            name: "Bearer 请求头".into(),
            description: "构造 Authorization Bearer 头 map".into(),
            body: BEARER_HEADER.into(),
        },
    ]
}

const BALANCE_GET_BEARER: &str = r#"// 示例：OpenAI 兼容 /user/balance 风格
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

const ITEMS_SKELETON: &str = r#"#{
    items: [
        #{
            id: "balance",
            type: "amount",
            direction: "remaining",
            value: 0,
            currencySymbol: "$",
            primary: true,
            label: "余额",
            period: "current"
        }
    ]
}
"#;

const BEARER_HEADER: &str = r#"let headers = #{
    "Authorization": "Bearer " + api_key,
    "Accept": "application/json"
};
"#;
