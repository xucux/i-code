//! # 备份模块 Tauri Command 声明
//!
//! 前端通过 `invoke('backup_*', payload)` 调用这些命令。

use std::path::PathBuf;

use log;
use tauri::{AppHandle, Manager, State};

use crate::error::IcodeResult;

use super::service::BackupServiceHandle;
use super::types::{
    BackupListItem, BackupResult, BackupTarget, CreateBackupInput, CreateWebDavBackupInput,
    RestoreBackupResult, RestoreWebDavBackupInput, SaveWebDavConfigInput, WebDavConfig,
    WebDavConfigRecord,
};

/// 创建备份
///
/// 将当前数据库压缩为 zip/tar.gz，保存到本地备份目录。
#[tauri::command]
pub async fn backup_create(
    state: State<'_, BackupServiceHandle>,
    input: CreateBackupInput,
) -> IcodeResult<BackupResult> {
    state.service().create_backup(&input)
}

/// 列出本地备份
///
/// `directory` 为空时使用默认备份目录。
#[tauri::command]
pub async fn backup_list_local(
    state: State<'_, BackupServiceHandle>,
    directory: Option<String>,
) -> IcodeResult<Vec<BackupListItem>> {
    let dir = directory.map(PathBuf::from);
    state.service().list_local_backups(dir.as_deref())
}

/// 恢复本地备份
///
/// **覆盖式恢复**：当前数据库将被备份文件完全覆盖。
/// 恢复前自动创建安全备份；恢复成功后自动重启应用以重新加载数据库。
#[tauri::command]
pub async fn backup_restore(
    app: AppHandle,
    state: State<'_, BackupServiceHandle>,
    path: String,
) -> IcodeResult<RestoreBackupResult> {
    let result = state.service().restore_backup(&PathBuf::from(path))?;
    if result.success && result.needs_restart {
        log::info!("本地备份恢复成功，准备重启应用");
        tauri::process::restart(&app.env());
    }
    Ok(result)
}

/// 删除本地备份
#[tauri::command]
pub async fn backup_delete(
    state: State<'_, BackupServiceHandle>,
    target: BackupTarget,
    path: String,
) -> IcodeResult<()> {
    match target {
        BackupTarget::Local => state.service().delete_local_backup(&PathBuf::from(path)),
        BackupTarget::Webdav => Err(crate::error::IcodeError::validation(
            "WebDAV 备份删除请使用 backup_delete_webdav 命令",
        )),
    }
}

/// 推送备份到 WebDAV
///
/// 本地创建 zip 后备份，可选 AES 加密后上传到 WebDAV 远程目录。
#[tauri::command]
pub async fn backup_push_webdav(
    state: State<'_, BackupServiceHandle>,
    input: CreateWebDavBackupInput,
) -> IcodeResult<BackupResult> {
    log::info!("Command backup_push_webdav 被调用");
    state
        .service()
        .push_to_webdav(&input)
        .await
        .map_err(|e| {
            log::error!("Command backup_push_webdav 失败: {e}");
            e
        })
}

/// 列出 WebDAV 备份
#[tauri::command]
pub async fn backup_list_webdav(
    state: State<'_, BackupServiceHandle>,
    config: WebDavConfig,
) -> IcodeResult<Vec<BackupListItem>> {
    log::info!("Command backup_list_webdav 被调用");
    state
        .service()
        .list_webdav_backups(&config)
        .await
        .map_err(|e| {
            log::error!("Command backup_list_webdav 失败: {e}");
            e
        })
}

/// 恢复 WebDAV 备份
///
/// 下载远程备份文件，解密（如加密）后覆盖式恢复到本地数据库；
/// 恢复成功后自动重启应用以重新加载数据库。
#[tauri::command]
pub async fn backup_restore_webdav(
    app: AppHandle,
    state: State<'_, BackupServiceHandle>,
    input: RestoreWebDavBackupInput,
) -> IcodeResult<RestoreBackupResult> {
    log::info!("Command backup_restore_webdav 被调用");
    let result = state
        .service()
        .restore_webdav_backup(&input)
        .await
        .map_err(|e| {
            log::error!("Command backup_restore_webdav 失败: {e}");
            e
        })?;
    if result.success && result.needs_restart {
        log::info!("WebDAV 备份恢复成功，准备重启应用");
        tauri::process::restart(&app.env());
    }
    Ok(result)
}

/// 删除 WebDAV 备份
#[tauri::command]
pub async fn backup_delete_webdav(
    state: State<'_, BackupServiceHandle>,
    config: WebDavConfig,
    remote_path: String,
) -> IcodeResult<()> {
    log::info!("Command backup_delete_webdav 被调用");
    state
        .service()
        .delete_webdav_backup(&config, &remote_path)
        .await
        .map_err(|e| {
            log::error!("Command backup_delete_webdav 失败: {e}");
            e
        })
}

/// 列出已保存的 WebDAV 配置
#[tauri::command]
pub async fn backup_webdav_config_list(
    state: State<'_, BackupServiceHandle>,
) -> IcodeResult<Vec<WebDavConfigRecord>> {
    log::info!("Command backup_webdav_config_list 被调用");
    state.service().list_webdav_configs().map_err(|e| {
        log::error!("Command backup_webdav_config_list 失败: {e}");
        e
    })
}

/// 获取单个 WebDAV 配置
#[tauri::command]
pub async fn backup_webdav_config_get(
    state: State<'_, BackupServiceHandle>,
    id: String,
) -> IcodeResult<Option<WebDavConfigRecord>> {
    log::info!("Command backup_webdav_config_get 被调用");
    state.service().get_webdav_config(&id).map_err(|e| {
        log::error!("Command backup_webdav_config_get 失败: {e}");
        e
    })
}

/// 保存 WebDAV 配置
///
/// 新建或更新 `webdav_configs` 表中的记录，密码以明文存储。
#[tauri::command]
pub async fn backup_webdav_config_save(
    state: State<'_, BackupServiceHandle>,
    input: SaveWebDavConfigInput,
) -> IcodeResult<WebDavConfigRecord> {
    log::info!("Command backup_webdav_config_save 被调用");
    state.service().save_webdav_config(&input).map_err(|e| {
        log::error!("Command backup_webdav_config_save 失败: {e}");
        e
    })
}

/// 删除 WebDAV 配置
#[tauri::command]
pub async fn backup_webdav_config_delete(
    state: State<'_, BackupServiceHandle>,
    id: String,
) -> IcodeResult<()> {
    log::info!("Command backup_webdav_config_delete 被调用");
    state.service().delete_webdav_config(&id).map_err(|e| {
        log::error!("Command backup_webdav_config_delete 失败: {e}");
        e
    })
}
