//! # 额度查询 Provider 注册表与分发
//!
//! 按 `BalanceConfig.method` 查找并调用对应 Provider 的 `refresh()` 方法。

pub mod aihubmix;
pub mod antigravity;
pub mod claude_relay_service;
pub mod code_assist_quota;
pub mod codex;
pub mod deepseek;
pub mod gemini_cli;
pub mod kimi_code;
pub mod minimax;
pub mod moonshot_ai;
pub mod newapi;
pub mod openrouter;
pub mod siliconflow;

use crate::error::{IcodeError, IcodeResult};

use super::provider::{BalanceProvider, BalanceRefreshInput};
use super::types::{BalanceConfig, BalanceMethod, BalanceSnapshot};

/// 根据 BalanceConfig 获取对应的 Provider 实例
pub fn get_provider(method: BalanceMethod) -> Option<Box<dyn BalanceProvider>> {
    match method {
        BalanceMethod::Deepseek => Some(Box::new(deepseek::DeepSeekBalanceProvider)),
        BalanceMethod::Openrouter => Some(Box::new(openrouter::OpenRouterBalanceProvider)),
        BalanceMethod::Siliconflow => Some(Box::new(siliconflow::SiliconFlowBalanceProvider)),
        BalanceMethod::MoonshotAi => Some(Box::new(moonshot_ai::MoonshotAiBalanceProvider)),
        BalanceMethod::KimiCode => Some(Box::new(kimi_code::KimiCodeBalanceProvider)),
        BalanceMethod::Newapi => Some(Box::new(newapi::NewApiBalanceProvider)),
        BalanceMethod::Aihubmix => Some(Box::new(aihubmix::AihubmixBalanceProvider)),
        BalanceMethod::ClaudeRelayService => Some(Box::new(
            claude_relay_service::ClaudeRelayServiceBalanceProvider,
        )),
        BalanceMethod::Antigravity => Some(Box::new(antigravity::AntigravityBalanceProvider)),
        BalanceMethod::GeminiCli => Some(Box::new(gemini_cli::GeminiCliBalanceProvider)),
        BalanceMethod::Codex => Some(Box::new(codex::CodexBalanceProvider)),
        BalanceMethod::Minimax => Some(Box::new(minimax::MinimaxBalanceProvider)),
        // none / synthetic / script 不走通用 Provider 注册表
        BalanceMethod::None | BalanceMethod::Synthetic | BalanceMethod::Script => None,
    }
}

/// 分发额度查询
///
/// 1. `none` → 空快照
/// 2. `synthetic` → 合成测试数据
/// 3. `script` → 加载 active 模板并执行 Rhai 脚本
/// 4. 其他 → 查找 Provider 并调用 `refresh()`
/// 5. 未实现的 Provider → 返回 INTERNAL 错误
pub async fn dispatch_refresh(
    config: &BalanceConfig,
    input: &BalanceRefreshInput,
) -> IcodeResult<BalanceSnapshot> {
    let method = config.method();

    // none / synthetic 特殊处理
    if method == BalanceMethod::None {
        return Ok(BalanceSnapshot {
            updated_at: now_ms(),
            items: Vec::new(),
        });
    }

    if method == BalanceMethod::Synthetic {
        return Ok(synthetic_snapshot());
    }

    // 自定义脚本：要求模板存在且 status=active
    if let BalanceConfig::Script(cfg) = config {
        return refresh_with_script(cfg, input).await;
    }

    // 查找 Provider
    if let Some(provider) = get_provider(method) {
        return provider.refresh(input).await;
    }

    // 未实现
    Err(IcodeError::internal(format!(
        "余额查询方法 '{}' 暂未实现，将在后续迭代中支持",
        method.as_str()
    )))
}

/// 执行脚本额度查询
async fn refresh_with_script(
    cfg: &super::types::ScriptBalanceConfig,
    input: &BalanceRefreshInput,
) -> IcodeResult<BalanceSnapshot> {
    use crate::modules::script_template::types::ScriptTemplateStatus;

    let template =
        crate::modules::script_template::repository::find_by_id(&cfg.script_template_id)?;
    let status = ScriptTemplateStatus::from_str(&template.status);
    if status != Some(ScriptTemplateStatus::Active) {
        return Err(IcodeError::validation(format!(
            "脚本模板未启用（当前状态: {}），无法刷新额度",
            template.status
        )));
    }
    if template.script_body.trim().is_empty() {
        return Err(IcodeError::validation("脚本模板正文为空"));
    }

    // 从 BalanceRefreshInput 还原 Provider 视图供脚本变量注入
    let provider = crate::modules::ai_gateway::types::Provider {
        id: input.provider_id.clone().unwrap_or_default(),
        slug: input.provider_slug.clone().unwrap_or_default(),
        display_name: input.provider_name.clone().unwrap_or_default(),
        provider_type: input.provider_type.clone().unwrap_or_default(),
        base_url: input.base_url.clone().unwrap_or_default(),
        use_raw_base_url: false,
        transport: None,
        service_tier: None,
        auth_json: input
            .auth_method
            .as_ref()
            .map(|m| format!(r#"{{"method":"{}"}}"#, m)),
        auth_expires_at: None,
        auth_method: input.auth_method.clone(),
        balance_provider_json: None,
        timeout_json: None,
        retry_json: None,
        proxy_json: None,
        script_variables_json: None,
        auto_fetch_official_models: false,
        context_cache_json: None,
        well_known_template_id: None,
        is_enabled: input.provider_is_enabled.unwrap_or(true),
        sort_order: 0,
        created_at: String::new(),
        updated_at: String::new(),
    };

    let timeout_ms = cfg
        .timeout_ms
        .unwrap_or(template.default_timeout_ms.max(1000) as u64)
        .min(30_000);

    let mut allowed_hosts: Vec<String> = cfg.allowed_hosts.clone().unwrap_or_default();
    if let Some(json) = &template.allowed_hosts_json {
        if let Ok(hosts) = serde_json::from_str::<Vec<String>>(json) {
            for h in hosts {
                if !allowed_hosts.iter().any(|x| x == &h) {
                    allowed_hosts.push(h);
                }
            }
        }
    }

    // 本地新建/手动编辑的脚本不强制 host 白名单，仅公共仓市场脚本强制校验
    let enforce_host_whitelist = template.is_marketplace();

    let result = super::script::execute_balance_script(
        &template.script_body,
        &template,
        &provider,
        input,
        timeout_ms,
        &allowed_hosts,
        enforce_host_whitelist,
    )
    .await?;

    Ok(result.snapshot)
}

/// 获取当前毫秒时间戳
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 合成测试数据快照
fn synthetic_snapshot() -> BalanceSnapshot {
    use super::types::{BalanceDirection, BalanceMetric, BalanceStatusValue};

    BalanceSnapshot {
        updated_at: now_ms(),
        items: vec![
            BalanceMetric::amount(
                "balance",
                BalanceDirection::Remaining,
                serde_json::json!(99.50),
                Some("$"),
            ),
            BalanceMetric::amount(
                "used",
                BalanceDirection::Used,
                serde_json::json!(0.50),
                Some("$"),
            ),
            BalanceMetric::amount(
                "limit",
                BalanceDirection::Limit,
                serde_json::json!(100.00),
                Some("$"),
            ),
            BalanceMetric::status(
                "status",
                BalanceStatusValue::Ok,
                Some("合成测试数据"),
            ),
        ],
    }
}
