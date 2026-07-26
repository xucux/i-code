//! # 脚本模板模块 Tauri Command 声明
//!
//! 前端通过 `invoke('script_template_*', payload)` 调用这些命令。

use tauri::State;

use crate::error::IcodeResult;

use super::service::ScriptTemplateHandle;
use super::types::{
    CreateScriptTemplateInput, ScriptSnippet, ScriptTemplate, ScriptTemplateListFilter,
    ScriptTemplateRef, ScriptTemplateSelectItem, ScriptTemplateTestInput,
    ScriptTemplateTestResult, UpdateScriptTemplateInput,
};

/// 列出脚本模板
#[tauri::command]
pub async fn script_template_list(
    state: State<'_, ScriptTemplateHandle>,
    kind: Option<String>,
    status: Option<String>,
    keyword: Option<String>,
) -> IcodeResult<Vec<ScriptTemplate>> {
    state.service().list(ScriptTemplateListFilter {
        kind,
        status,
        keyword,
    })
}

/// 获取脚本模板详情
#[tauri::command]
pub async fn script_template_get(
    state: State<'_, ScriptTemplateHandle>,
    id: String,
) -> IcodeResult<ScriptTemplate> {
    state.service().get(&id)
}

/// 创建脚本模板（默认 draft）
#[tauri::command]
pub async fn script_template_create(
    state: State<'_, ScriptTemplateHandle>,
    input: CreateScriptTemplateInput,
) -> IcodeResult<ScriptTemplate> {
    state.service().create(input)
}

/// 更新脚本模板
#[tauri::command]
pub async fn script_template_update(
    state: State<'_, ScriptTemplateHandle>,
    id: String,
    input: UpdateScriptTemplateInput,
) -> IcodeResult<ScriptTemplate> {
    state.service().update(&id, input)
}

/// 删除脚本模板（有引用时拒绝）
#[tauri::command]
pub async fn script_template_delete(
    state: State<'_, ScriptTemplateHandle>,
    id: String,
) -> IcodeResult<()> {
    state.service().delete(&id)
}

/// 状态迁移：publish / disable / revert_to_draft
#[tauri::command]
pub async fn script_template_set_status(
    state: State<'_, ScriptTemplateHandle>,
    id: String,
    action: String,
) -> IcodeResult<ScriptTemplate> {
    state.service().set_status(&id, &action)
}

/// 试运行（不写 snapshot 表）
#[tauri::command]
pub async fn script_template_test(
    state: State<'_, ScriptTemplateHandle>,
    input: ScriptTemplateTestInput,
) -> IcodeResult<ScriptTemplateTestResult> {
    state.service().test(input).await
}

/// 供应商表单下拉：仅 kind=balance & status=active
#[tauri::command]
pub async fn script_template_list_active_for_select(
    state: State<'_, ScriptTemplateHandle>,
) -> IcodeResult<Vec<ScriptTemplateSelectItem>> {
    state.service().list_active_for_select()
}

/// 内置 snippet 列表
#[tauri::command]
pub async fn script_template_list_snippets(
    state: State<'_, ScriptTemplateHandle>,
) -> IcodeResult<Vec<ScriptSnippet>> {
    Ok(state.service().list_snippets())
}

/// 查询引用该模板的供应商
#[tauri::command]
pub async fn script_template_list_refs(
    state: State<'_, ScriptTemplateHandle>,
    id: String,
) -> IcodeResult<Vec<ScriptTemplateRef>> {
    state.service().list_refs(&id)
}
