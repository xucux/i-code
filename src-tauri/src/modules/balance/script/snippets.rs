//! # 内置脚本 Snippet
//!
//! 每个常量定义在独立文件中，便于维护。

use crate::modules::script_template::types::ScriptSnippet;

mod balance_get_bearer;
mod bearer_header;
mod grok_usage;
mod items_skeleton;
mod joyagent_balance;
mod mimo_balance;
mod mimo_token_plan;

use balance_get_bearer::BALANCE_GET_BEARER;
use bearer_header::BEARER_HEADER;
use grok_usage::GROK_USAGE;
use items_skeleton::ITEMS_SKELETON;
use joyagent_balance::JOYAGENT_BALANCE;
use mimo_balance::MIMO_BALANCE;
use mimo_token_plan::MIMO_TOKEN_PLAN;

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
        ScriptSnippet {
            id: "mimo-balance".into(),
            name: "小米 MiMo 按量计费".into(),
            description: "查询小米 MiMo 平台余额、现金余额、赠送余额、冻结金额、透支额度等".into(),
            body: MIMO_BALANCE.into(),
        },
        ScriptSnippet {
            id: "mimo-token-plan".into(),
            name: "小米 MiMo TokenPlan".into(),
            description: "查询小米 MiMo 平台套餐积分与补偿积分的已用/总量/剩余/百分比".into(),
            body: MIMO_TOKEN_PLAN.into(),
        },
        ScriptSnippet {
            id: "grok-usage".into(),
            name: "公益Grok监控".into(),
            description: "公益 Grok 额度监控：quota 金额 + 今日/累计 token + 账户状态".into(),
            body: GROK_USAGE.into(),
        },
        ScriptSnippet {
            id: "joyagent-balance".into(),
            name: "京东 JoyAgent 积分".into(),
            description: "Cookie 鉴权查询 JoyAgent 积分：剩余积分 / 上限 / 已用 / 百分比 / 钱包金额 / 状态".into(),
            body: JOYAGENT_BALANCE.into(),
        },
    ]
}
