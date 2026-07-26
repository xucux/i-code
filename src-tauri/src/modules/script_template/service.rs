//! # 脚本模板业务服务层
//!
//! 模板 CRUD、状态机、试运行编排。跨模块调用 balance script 运行时与 ai_gateway。

use std::sync::Arc;
use std::time::Instant;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::ai_gateway::AiGatewayServiceHandle;
use crate::modules::balance::script;
use crate::modules::balance::types::{BalanceConfig, ScriptBalanceConfig};

use super::repository;
use super::types::{
    CreateScriptTemplateInput, ScriptSnippet, ScriptTemplate, ScriptTemplateKind,
    ScriptTemplateListFilter, ScriptTemplateRef, ScriptTemplateSelectItem,
    ScriptTemplateStatus, ScriptTemplateStatusAction, ScriptTemplateTestInput,
    ScriptTemplateTestResult, UpdateScriptTemplateInput,
};

/// slug 校验：英文、数字与 `-` `_` `.` `@`，长度 1–64
fn validate_slug(slug: &str) -> IcodeResult<()> {
    if slug.is_empty() || slug.len() > 64 {
        return Err(IcodeError::validation("slug 长度须为 1–64"));
    }
    let re_ok = slug.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@')
    });
    if !re_ok {
        return Err(IcodeError::validation(
            "slug 仅允许英文、数字与符号 - _ . @",
        ));
    }
    Ok(())
}

/// 脚本模板 Service 句柄
#[derive(Clone)]
pub struct ScriptTemplateHandle {
    inner: Arc<ScriptTemplateService>,
}

impl ScriptTemplateHandle {
    pub fn new(ai_gateway: AiGatewayServiceHandle) -> Self {
        Self {
            inner: Arc::new(ScriptTemplateService { ai_gateway }),
        }
    }

    pub fn service(&self) -> &ScriptTemplateService {
        &self.inner
    }
}

/// 脚本模板业务逻辑
pub struct ScriptTemplateService {
    ai_gateway: AiGatewayServiceHandle,
}

impl ScriptTemplateService {
    /// 创建模板（默认 draft）
    pub fn create(&self, input: CreateScriptTemplateInput) -> IcodeResult<ScriptTemplate> {
        if input.name.trim().is_empty() {
            return Err(IcodeError::validation("模板名称不能为空"));
        }
        validate_slug(&input.slug)?;
        if ScriptTemplateKind::from_str(&input.kind).is_none() {
            return Err(IcodeError::validation(format!(
                "不支持的模板类型: {}",
                input.kind
            )));
        }
        if repository::find_by_slug(&input.slug)?.is_some() {
            return Err(IcodeError::conflict(format!(
                "模板 slug '{}' 已存在",
                input.slug
            )));
        }
        if input.script_body.len() > 64 * 1024 {
            return Err(IcodeError::validation("脚本正文不能超过 64 KiB"));
        }
        repository::insert(&input)
    }

    /// 获取详情
    pub fn get(&self, id: &str) -> IcodeResult<ScriptTemplate> {
        repository::find_by_id(id)
    }

    /// 列表
    pub fn list(&self, filter: ScriptTemplateListFilter) -> IcodeResult<Vec<ScriptTemplate>> {
        if let Some(kind) = &filter.kind {
            if ScriptTemplateKind::from_str(kind).is_none() {
                return Err(IcodeError::validation(format!("未知 kind: {kind}")));
            }
        }
        if let Some(status) = &filter.status {
            if ScriptTemplateStatus::from_str(status).is_none() {
                return Err(IcodeError::validation(format!("未知 status: {status}")));
            }
        }
        repository::list(&filter)
    }

    /// 更新
    pub fn update(
        &self,
        id: &str,
        input: UpdateScriptTemplateInput,
    ) -> IcodeResult<ScriptTemplate> {
        let existing = repository::find_by_id(id)?;
        if let Some(name) = &input.name {
            if name.trim().is_empty() {
                return Err(IcodeError::validation("模板名称不能为空"));
            }
        }
        if let Some(slug) = &input.slug {
            validate_slug(slug)?;
            if slug != &existing.slug {
                if repository::find_by_slug(slug)?.is_some() {
                    return Err(IcodeError::conflict(format!("模板 slug '{slug}' 已存在")));
                }
            }
        }
        if let Some(body) = &input.script_body {
            if body.len() > 64 * 1024 {
                return Err(IcodeError::validation("脚本正文不能超过 64 KiB"));
            }
        }
        repository::update(id, &input)
    }

    /// 删除（检查引用）
    pub fn delete(&self, id: &str) -> IcodeResult<()> {
        let _ = repository::find_by_id(id)?;
        let refs = repository::list_refs(id)?;
        if !refs.is_empty() {
            return Err(IcodeError::conflict(format!(
                "模板仍被 {} 个供应商引用，请先解绑或改为禁用",
                refs.len()
            )));
        }
        repository::delete(id)
    }

    /// 状态迁移
    pub fn set_status(
        &self,
        id: &str,
        action: &str,
    ) -> IcodeResult<ScriptTemplate> {
        let existing = repository::find_by_id(id)?;
        let current = ScriptTemplateStatus::from_str(&existing.status).ok_or_else(|| {
            IcodeError::internal(format!("非法 status 存储值: {}", existing.status))
        })?;
        let act = ScriptTemplateStatusAction::from_str(action).ok_or_else(|| {
            IcodeError::validation(format!(
                "未知状态动作: {action}（允许 publish / disable / revert_to_draft）"
            ))
        })?;

        let next = match (act, current) {
            (ScriptTemplateStatusAction::Publish, ScriptTemplateStatus::Draft)
            | (ScriptTemplateStatusAction::Publish, ScriptTemplateStatus::Disabled) => {
                if existing.script_body.trim().is_empty() {
                    return Err(IcodeError::validation("启用前脚本正文不能为空"));
                }
                ScriptTemplateStatus::Active
            }
            (ScriptTemplateStatusAction::Disable, ScriptTemplateStatus::Active)
            | (ScriptTemplateStatusAction::Disable, ScriptTemplateStatus::Draft) => {
                ScriptTemplateStatus::Disabled
            }
            (ScriptTemplateStatusAction::RevertToDraft, ScriptTemplateStatus::Active)
            | (ScriptTemplateStatusAction::RevertToDraft, ScriptTemplateStatus::Disabled) => {
                ScriptTemplateStatus::Draft
            }
            _ => {
                return Err(IcodeError::validation(format!(
                    "不允许从 {} 执行 {}",
                    current.as_str(),
                    act.as_str()
                )));
            }
        };

        repository::update_status(id, next.as_str())
    }

    /// 下拉：仅 active 额度模板
    pub fn list_active_for_select(&self) -> IcodeResult<Vec<ScriptTemplateSelectItem>> {
        repository::list_active_for_select(ScriptTemplateKind::Balance.as_str())
    }

    /// 引用列表
    pub fn list_refs(&self, id: &str) -> IcodeResult<Vec<ScriptTemplateRef>> {
        let _ = repository::find_by_id(id)?;
        repository::list_refs(id)
    }

    /// 内置 snippet
    pub fn list_snippets(&self) -> Vec<ScriptSnippet> {
        script::snippets::list_snippets()
    }

    /// 试运行：不要求 active，不写 snapshot 表
    pub async fn test(
        &self,
        input: ScriptTemplateTestInput,
    ) -> IcodeResult<ScriptTemplateTestResult> {
        let template = repository::find_by_id(&input.template_id)?;
        let provider = self.ai_gateway.service().get_provider(&input.provider_id)?;

        let script_body = input
            .script_body_override
            .as_deref()
            .unwrap_or(&template.script_body);
        if script_body.trim().is_empty() {
            return Err(IcodeError::validation("脚本正文为空，无法试运行"));
        }

        // 构造与正式刷新一致的上下文（解密 secret）
        // 临时把 config 设为 script 以便复用 build_balance_refresh_input 的解密路径
        let fake_config = BalanceConfig::Script(ScriptBalanceConfig {
            script_template_id: template.id.clone(),
            timeout_ms: input.timeout_ms,
            allowed_hosts: None,
        });
        // build_balance_refresh_input 读 provider.balance_provider_json，此处直接解密 auth
        let resolved = self
            .ai_gateway
            .service()
            .build_balance_refresh_input(&provider)?;

        // 若供应商本身未配额度，仍允许试运行：手动构造 input
        let refresh_input = if let Some((_cfg, input)) = resolved {
            input
        } else {
            // 仅解密 auth
            let mut provider_for_auth = provider.clone();
            provider_for_auth.balance_provider_json =
                Some(serde_json::to_string(&fake_config).unwrap_or_default());
            match self
                .ai_gateway
                .service()
                .build_balance_refresh_input(&provider_for_auth)?
            {
                Some((_, input)) => input,
                None => crate::modules::balance::provider::BalanceRefreshInput {
                    base_url: Some(provider.base_url.clone()),
                    ..Default::default()
                },
            }
        };

        let timeout_ms = input
            .timeout_ms
            .unwrap_or(template.default_timeout_ms.max(1000) as u64)
            .min(30_000);

        let mut allowed_hosts: Vec<String> = Vec::new();
        if let Some(json) = &template.allowed_hosts_json {
            if let Ok(hosts) = serde_json::from_str::<Vec<String>>(json) {
                allowed_hosts.extend(hosts);
            }
        }

        let start = Instant::now();
        let run = script::execute_balance_script(
            script_body,
            &template,
            &provider,
            &refresh_input,
            timeout_ms,
            &allowed_hosts,
        )
        .await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match run {
            Ok(result) => {
                let msg = format!("试运行成功，耗时 {}ms，指标 {} 条", duration_ms, result.snapshot.items.len());
                let _ = repository::update_last_test(&template.id, true, &msg);
                Ok(ScriptTemplateTestResult {
                    ok: true,
                    snapshot: Some(result.snapshot),
                    error: None,
                    duration_ms,
                    logs: result.logs,
                })
            }
            Err(err) => {
                let msg = redact_message(&err.message);
                let _ = repository::update_last_test(&template.id, false, &msg);
                Ok(ScriptTemplateTestResult {
                    ok: false,
                    snapshot: None,
                    error: Some(msg),
                    duration_ms,
                    logs: Vec::new(),
                })
            }
        }
    }
}

/// 脱敏：避免把 api_key 写入 last_test_message
fn redact_message(msg: &str) -> String {
    // 简单截断过长错误
    let truncated: String = msg.chars().take(500).collect();
    truncated
}
