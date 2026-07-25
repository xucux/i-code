//! # 敏感凭据模块 Tauri Command 声明
//!
//! 前端通过 `invoke('secret_*', payload)` 调用这些命令。
//! Commands 层仅做参数校验与 Service 调用，不包含业务逻辑。

use tauri::State;

use crate::error::IcodeResult;

use super::service::SecretServiceHandle;
use super::types::{SaveSecretInput, SecretKind, SecretMask, SecretReferenceScanResult};

/// 保存 Secret（明文经后端加密后存入数据库）
///
/// # 参数
/// - `input`: 包含 kind、plaintext、label 的输入对象
///
/// # 返回
/// SecretMask（不包含明文与密文）
#[tauri::command]
pub async fn secret_save(
    state: State<'_, SecretServiceHandle>,
    input: SaveSecretInput,
) -> IcodeResult<SecretMask> {
    state
        .service()
        .save_secret(input.kind, &input.plaintext, input.label.as_deref())
}

/// 更新已有 Secret 的明文
///
/// # 参数
/// - `id`: Secret UUID
/// - `plaintext`: 新的明文值
/// - `label`: 新的标签（可选）
#[tauri::command]
pub async fn secret_update(
    state: State<'_, SecretServiceHandle>,
    id: String,
    plaintext: String,
    label: Option<String>,
) -> IcodeResult<SecretMask> {
    state
        .service()
        .update_secret(&id, &plaintext, label.as_deref())
}

/// 删除 Secret
///
/// 幂等操作：不存在的 ID 也返回成功。
#[tauri::command]
pub async fn secret_delete(
    state: State<'_, SecretServiceHandle>,
    id: String,
) -> IcodeResult<()> {
    state.service().delete_secret(&id)
}

/// 列出所有 Secret（掩码视图）
///
/// 返回的列表不包含明文与密文，仅用于 UI 展示与选择。
#[tauri::command]
pub async fn secret_list(state: State<'_, SecretServiceHandle>) -> IcodeResult<Vec<SecretMask>> {
    state.service().list_secrets()
}

/// 列出所有 Secret 的 kind 值
///
/// 用于前端表单生成下拉选项。
#[tauri::command]
pub async fn secret_list_kinds() -> IcodeResult<Vec<String>> {
    Ok(vec![
        SecretKind::ApiKey.as_str().to_string(),
        SecretKind::OauthToken.as_str().to_string(),
        SecretKind::ProxyAuth.as_str().to_string(),
        SecretKind::GatewayKey.as_str().to_string(),
        SecretKind::WebDavPassword.as_str().to_string(),
    ])
}

/// 扫描 JSON 配置中的 Secret 引用
///
/// 用于导出供应商配置时标记 `missingSecrets` 列表。
///
/// # 参数
/// - `value`: 待扫描的 JSON 配置对象
#[tauri::command]
pub async fn secret_scan_references(
    state: State<'_, SecretServiceHandle>,
    value: serde_json::Value,
) -> IcodeResult<SecretReferenceScanResult> {
    state.service().scan_references(&value)
}

/// 清理未引用的孤立 Secret
///
/// 扫描所有业务表中的 Secret 引用，删除不再被任何配置引用的记录。
/// 返回被清理的记录数。
///
/// **建议在前端「设置 → 凭据管理」页面提供手动触发入口**，
/// 由用户确认后执行。自动清理可作为后续迭代的定时任务。
#[tauri::command]
pub async fn secret_cleanup_orphaned(state: State<'_, SecretServiceHandle>) -> IcodeResult<usize> {
    state.service().cleanup_orphaned()
}

/// 解密一段可能包含 `$SECRET:{snowflake_id}$` 引用的文本
///
/// 用于前端需要从供应商 authJson 等配置中还原明文的场景。
/// 若文本中不含 Secret 引用则原样返回；若引用不存在或解密失败则返回错误。
///
/// # 参数
/// - `value`: 待解密的文本（允许为明文或 `$SECRET:{snowflake_id}$` 引用）
///
/// # 返回
/// 解密后的明文
#[tauri::command]
pub async fn secret_decrypt_text(
    state: State<'_, SecretServiceHandle>,
    value: String,
) -> IcodeResult<String> {
    state.service().resolve_in_text(&value)
}
