//! # 备份业务服务层
//!
//! 提供创建/推送/列出/恢复/删除备份功能。
//!
//! ## v0.2 实现范围
//!
//! - **本地备份**：完整实现
//!   - 目录优先级：`BackupSettings.local_directory` > 程序运行目录下的 `backup/`
//!   - 创建：`CHECKPOINT` 合并 WAL → 复制 db 文件 → zip 压缩 → 写入 backup.json
//!   - 列出：扫描备份目录，解析文件名时间戳
//!   - 恢复：先创建安全备份 → 使用 SQLite Online Backup API 恢复到当前数据库 → 返回 `needs_restart: true`
//!   - 删除：直接删除文件
//!   - 保留策略：按 `BackupSettings.local_retention_count` 自动清理旧备份（0=不限制）
//! - **WebDAV 备份**：完整实现
//!   - 列出：PROPFIND 远程目录
//!   - 推送：本地创建 zip → 可选 AES 加密 → PUT 上传 → 按保留策略清理远程旧备份
//!   - 恢复：下载 → 可选解密 → 调用本地恢复逻辑
//!   - 删除：DELETE 请求
//!   - 加密：AES-256-GCM，密钥 = SHA-256(备份密码)
//! - **安全备份**：恢复前自动创建紧急备份（存放于应用配置目录）
//!
//! ## 备份文件命名
//!
//! - 本地 / WebDAV 未加密：`i-code-backup-{yyyyMMdd-HHmmss}.zip`
//! - WebDAV 加密：`i-code-backup-{yyyyMMdd-HHmmss}.zip.enc`

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use sha2::{Digest, Sha256};
use zip::ZipWriter;

use crate::db;
use crate::error::{IcodeError, IcodeResult};
use crate::modules::balance::script::host_storage::{STORAGE_FILE_NAME, init_script_storage};
use crate::modules::logger::Log;
use crate::modules::secret::SecretServiceHandle;
/// 同时向 tauri-plugin-log 和自研内存 logger 输出日志
macro_rules! backup_info {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        tracing::info!("{}", msg);
        Log::info(&msg);
    }};
}
macro_rules! backup_warn {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        tracing::warn!("{}", msg);
        Log::warn(&msg);
    }};
}
macro_rules! backup_error {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        log::error!("{}", msg);
        Log::error(&msg);
    }};
}


use super::crypto;
use super::repository;
use super::types::{
    BackupErrorCode, BackupFormat, BackupListItem, BackupMeta, BackupResult, BackupSettings,
    BackupTarget, CreateBackupInput, CreateWebDavBackupInput, RestoreBackupResult,
    RestoreWebDavBackupInput, SaveWebDavConfigInput, WebDavConfig, WebDavConfigRecord,
};
use super::webdav;

/// Backup Service 在 Tauri State 中的句柄
pub struct BackupServiceHandle {
    inner: Arc<BackupService>,
}

impl BackupServiceHandle {
    /// 创建 Backup Service
    ///
    /// # 参数
    /// - `db_path`：数据库文件路径（i-code.db）
    /// - `backups_dir`：用户本地备份目录（程序运行目录/backup）
    /// - `safety_backups_dir`：安全备份目录（应用配置目录/backups）
    /// - `app_version`：当前应用版本号（写入 BackupMeta）
    /// - `schema_version`：当前数据库 schema 版本（写入 BackupMeta）
    /// - `secret_handle`：Secret 服务句柄（用于解析 WebDAV 密码与恢复后扫描缺失 Secret）
    pub fn new(
        db_path: PathBuf,
        backups_dir: PathBuf,
        safety_backups_dir: PathBuf,
        app_version: String,
        schema_version: u32,
        secret_handle: SecretServiceHandle,
    ) -> Self {
        Self {
            inner: Arc::new(BackupService::new(
                db_path,
                backups_dir,
                safety_backups_dir,
                app_version,
                schema_version,
                secret_handle,
            )),
        }
    }

    pub fn service(&self) -> &BackupService {
        &self.inner
    }
}

impl Clone for BackupServiceHandle {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Backup Service 业务逻辑
pub struct BackupService {
    db_path: PathBuf,
    backups_dir: PathBuf,
    safety_backups_dir: PathBuf,
    app_version: String,
    schema_version: u32,
    secret_handle: SecretServiceHandle,
}

impl BackupService {
    pub fn new(
        db_path: PathBuf,
        backups_dir: PathBuf,
        safety_backups_dir: PathBuf,
        app_version: String,
        schema_version: u32,
        secret_handle: SecretServiceHandle,
    ) -> Self {
        Self {
            db_path,
            backups_dir,
            safety_backups_dir,
            app_version,
            schema_version,
            secret_handle,
        }
    }

    /// 脚本公共存储文件路径（与数据库同目录的 `script-storage.json`）
    fn storage_path(&self) -> PathBuf {
        self.db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(STORAGE_FILE_NAME)
    }

    /// 解析本地备份目录
    ///
    /// 优先级：
    /// 1. `override_dir` 显式传入的目录（前端直接指定）
    /// 2. `BackupSettings.local_directory` 配置项
    /// 3. 初始化时传入的默认目录（程序运行目录/backup）
    fn resolve_backups_dir(&self, override_dir: Option<&Path>) -> IcodeResult<PathBuf> {
        if let Some(dir) = override_dir {
            return Ok(dir.to_path_buf());
        }
        if let Some(cfg_dir) = self.load_backup_settings()?.local_directory {
            if !cfg_dir.is_empty() {
                return Ok(PathBuf::from(cfg_dir));
            }
        }
        Ok(self.backups_dir.clone())
    }

    /// 创建本地备份
    ///
    /// 1. 解析本地备份目录：优先使用 `BackupSettings.local_directory`，
    ///    未配置时回退到初始化时传入的默认目录（程序运行目录/backup）。
    /// 2. 执行 `PRAGMA wal_checkpoint(TRUNCATE)` 强制合并 WAL
    /// 3. 计算数据库文件 SHA-256 校验和
    /// 4. 生成 BackupMeta
    /// 5. 压缩 db 文件 + backup.json 到 zip 包
    /// 6. 按本地保留策略清理旧备份
    /// 7. 返回 BackupResult
    pub fn create_backup(&self, input: &CreateBackupInput) -> IcodeResult<BackupResult> {
        let backups_dir = self.resolve_backups_dir(None)?;
        backup_info!("开始创建本地备份: dir={}", backups_dir.to_string_lossy());
        fs::create_dir_all(&backups_dir)?;

        self.checkpoint_wal()?;
        let checksum = self.compute_db_checksum()?;
        backup_info!("当前数据库校验和: {checksum}");

        let now = Utc::now();
        let timestamp = now.format("%Y%m%d-%H%M%S").to_string();
        let backup_id = timestamp.clone();
        let created_at = now.to_rfc3339();

        let meta = BackupMeta {
            version: "1.0".to_string(),
            app_version: self.app_version.clone(),
            created_at: created_at.clone(),
            database_schema_version: self.schema_version,
            checksum,
            files: Some({
                let mut files = vec!["i-code.db".to_string(), "backup.json".to_string()];
                // 脚本公共存储存在时一并备份
                if self.storage_path().exists() {
                    files.push(STORAGE_FILE_NAME.to_string());
                }
                files
            }),
            target: Some(BackupTarget::Local),
            encrypted: Some(false),
        };

        let ext = match input.format {
            BackupFormat::Zip => "zip",
            BackupFormat::TarGz => "tar.gz",
        };
        let file_name = format!("i-code-backup-{timestamp}.{ext}");
        let backup_path = backups_dir.join(&file_name);

        match input.format {
            BackupFormat::Zip => self.create_zip(&backup_path, &meta)?,
            BackupFormat::TarGz => {
                return Err(IcodeError::internal("tar.gz 格式暂未实现，请使用 zip 格式"));
            }
        }

        let size = fs::metadata(&backup_path)?.len();

        // 按配置保留份数清理旧本地备份
        if let Err(e) = self.cleanup_old_local_backups(&backups_dir) {
            backup_warn!("创建本地备份后清理旧备份失败: {e}");
        }

        backup_info!(
            "本地备份创建完成: path={}, size={} bytes",
            backup_path.to_string_lossy(),
            size
        );

        Ok(BackupResult {
            success: true,
            backup_id,
            target: BackupTarget::Local,
            size_bytes: size,
            path: backup_path.to_string_lossy().to_string(),
            created_at,
            encrypted: Some(false),
            error: None,
        })
    }

    /// 列出本地备份
    ///
    /// `directory` 显式传入时优先使用；否则按 `BackupSettings.local_directory` 解析；
    /// 均未设置时回退到默认备份目录。
    pub fn list_local_backups(&self, directory: Option<&Path>) -> IcodeResult<Vec<BackupListItem>> {
        let dir = self.resolve_backups_dir(directory)?;
        backup_info!("列出本地备份: dir={}", dir.to_string_lossy());
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut items = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            if !name.starts_with("i-code-backup-") {
                continue;
            }
            let metadata = entry.metadata()?;
            let (backup_id, encrypted) = parse_backup_file_name(name);
            if backup_id.is_empty() {
                continue;
            }

            items.push(BackupListItem {
                id: backup_id,
                target: BackupTarget::Local,
                path: path.to_string_lossy().to_string(),
                created_at: metadata.modified().map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .map(|d| {
                            chrono::DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0)
                                .map(|dt| dt.to_rfc3339())
                                .unwrap_or_default()
                        })
                        .unwrap_or_default()
                }).unwrap_or_default(),
                size_bytes: metadata.len(),
                app_version: None,
                database_schema_version: None,
                encrypted: Some(encrypted),
            });
        }
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        backup_info!("列出本地备份完成: 共 {} 条", items.len());
        Ok(items)
    }

    /// 恢复备份（覆盖式，使用 SQLite Online Backup API）
    ///
    /// 1. 前置校验：读取 backup.json，比较 schema 版本
    /// 2. 安全备份：对当前数据库创建紧急备份
    /// 3. 解压备份包中的 db 文件到临时文件
    /// 4. 校验解压后 db 文件的 SHA-256
    /// 5. 使用 SQLite Online Backup API 将备份数据库恢复到当前数据库
    /// 6. 如备份 schema 版本低于当前，运行迁移
    /// 7. 扫描缺失的 Secret 引用
    pub fn restore_backup(&self, backup_path: &Path) -> IcodeResult<RestoreBackupResult> {
        backup_info!(
            "开始恢复备份: path={}",
            backup_path.to_string_lossy()
        );
        if !backup_path.exists() {
            backup_error!("备份文件不存在: {}", backup_path.to_string_lossy());
            return Err(IcodeError::not_found("备份文件", backup_path.to_str()));
        }

        let meta = self.read_backup_meta_from_zip(backup_path)?;
        let backup_version = meta.database_schema_version;
        let current_version = self.schema_version;
        let mut schema_version_info = super::types::SchemaVersionInfo {
            backup_version,
            current_version,
            migrated: false,
        };

        if backup_version > current_version {
            return Ok(RestoreBackupResult {
                success: false,
                backup_path: backup_path.to_string_lossy().to_string(),
                safety_backup_path: None,
                missing_secrets: Vec::new(),
                schema_version_info: Some(schema_version_info),
                needs_restart: false,
                error_code: Some(BackupErrorCode::SchemaVersionTooNew),
                error_message: Some(format!(
                    "备份版本 {} 高于当前应用版本 {}，请先升级应用",
                    backup_version, current_version
                )),
            });
        }

        let safety_path = self.create_safety_backup()?;
        backup_info!(
            "安全备份已创建: path={}",
            safety_path.to_string_lossy()
        );

        let restore_db_path = self.db_path.with_extension("db.restored");
        self.extract_db_from_zip(backup_path, &restore_db_path)?;
        backup_info!("已从备份包解压数据库到临时文件");

        // 恢复脚本公共存储（script-storage.json；旧备份无此文件时跳过）
        self.restore_script_storage(backup_path)?;

        let restored_checksum = compute_file_checksum(&restore_db_path)?;
        if restored_checksum != meta.checksum {
            backup_error!(
                "数据库校验和失败: expected={}, got={}",
                meta.checksum, restored_checksum
            );
            let _ = fs::remove_file(&restore_db_path);
            return Ok(RestoreBackupResult {
                success: false,
                backup_path: backup_path.to_string_lossy().to_string(),
                safety_backup_path: Some(safety_path.to_string_lossy().to_string()),
                missing_secrets: Vec::new(),
                schema_version_info: Some(schema_version_info),
                needs_restart: false,
                error_code: Some(BackupErrorCode::ChecksumMismatch),
                error_message: Some("备份文件校验失败，文件可能已损坏".to_string()),
            });
        }

        // 使用 SQLite Online Backup API 恢复，避免替换文件导致的 Windows 句柄占用问题
        backup_info!("使用 SQLite Online Backup API 恢复数据库");
        let pool = db::get_db_pool()?;
        let mut dst_conn = pool.get()?;
        let src_conn = rusqlite::Connection::open(&restore_db_path)
            .map_err(|e| IcodeError::database(format!("打开备份数据库失败: {e}")))?;
        {
            let backup = rusqlite::backup::Backup::new(&src_conn, &mut dst_conn)
                .map_err(|e| IcodeError::database(format!("初始化 SQLite Backup 失败: {e}")))?;
            backup.run_to_completion(5, Duration::from_millis(250), None)
                .map_err(|e| IcodeError::database(format!("SQLite Backup 恢复失败: {e}")))?;
        }
        // 恢复后运行迁移（仅当备份版本低于当前时实际执行）
        if backup_version < current_version {
            backup_info!(
                "备份 schema 版本低于当前，执行迁移: {} -> {}",
                backup_version, current_version
            );
            db::run_migrations_with_conn(&mut dst_conn)
                .map_err(|e| IcodeError::database(format!("恢复后数据库迁移失败: {e}")))?;
        }
        drop(dst_conn);
        let _ = fs::remove_file(&restore_db_path);

        schema_version_info.migrated = backup_version < current_version;
        let missing_secrets = self.scan_missing_secrets().unwrap_or_default();

        backup_info!(
            "备份恢复完成: path={}, missing_secrets={}, migrated={}",
            backup_path.to_string_lossy(),
            missing_secrets.len(),
            schema_version_info.migrated
        );

        Ok(RestoreBackupResult {
            success: true,
            backup_path: backup_path.to_string_lossy().to_string(),
            safety_backup_path: Some(safety_path.to_string_lossy().to_string()),
            missing_secrets,
            schema_version_info: Some(schema_version_info),
            needs_restart: true,
            error_code: None,
            error_message: None,
        })
    }

    /// 删除本地备份文件
    pub fn delete_local_backup(&self, backup_path: &Path) -> IcodeResult<()> {
        if !backup_path.exists() {
            return Ok(());
        }
        backup_info!("删除本地备份文件: {}", backup_path.to_string_lossy());
        fs::remove_file(backup_path)?;
        Ok(())
    }

    /// 推送备份到 WebDAV
    ///
    /// 1. 解析本地临时目录（同本地备份目录策略）
    /// 2. 本地创建 zip 备份
    /// 3. 可选 AES 加密
    /// 4. 确保远程目录存在
    /// 5. PUT 上传
    /// 6. 按 WebDAV 保留策略清理远程旧备份
    pub async fn push_to_webdav(
        &self,
        input: &CreateWebDavBackupInput,
    ) -> IcodeResult<BackupResult> {
        let password_plaintext = self.resolve_webdav_password(&input.config)?;

        backup_info!(
            "开始推送 WebDAV 备份: url={}, remote_dir={:?}, encrypt={}",
            input.config.url, input.config.remote_path, input.encrypt
        );

        // 1. 在临时目录创建 zip
        let backups_dir = self.resolve_backups_dir(None)?;
        fs::create_dir_all(&backups_dir)?;
        self.checkpoint_wal()?;
        let checksum = self.compute_db_checksum()?;

        let now = Utc::now();
        let timestamp = now.format("%Y%m%d-%H%M%S").to_string();
        let backup_id = timestamp.clone();
        let created_at = now.to_rfc3339();

        let encrypt = input.encrypt;
        let universal_password = if encrypt {
            Some(self.load_config_key()?.ok_or_else(|| {
                IcodeError::validation("加密备份必须先在「设置」或「备份设置」中配置 1-20 位通用密码")
            })?)
        } else {
            None
        };

        let file_name = if encrypt {
            format!("i-code-backup-{timestamp}.zip.enc")
        } else {
            format!("i-code-backup-{timestamp}.zip")
        };
        let local_temp_path = backups_dir.join(&file_name);

        let meta = BackupMeta {
            version: "1.0".to_string(),
            app_version: self.app_version.clone(),
            created_at: created_at.clone(),
            database_schema_version: self.schema_version,
            checksum: checksum.clone(),
            files: Some(vec!["i-code.db".to_string(), "backup.json".to_string()]),
            target: Some(BackupTarget::Webdav),
            encrypted: Some(encrypt),
        };

        self.create_zip(&local_temp_path, &meta)?;

        // 2. 可选加密
        let upload_bytes = if let Some(password) = &universal_password {
            let plain_bytes = fs::read(&local_temp_path)?;
            crypto::encrypt_backup(password, &plain_bytes)?
        } else {
            fs::read(&local_temp_path)?
        };

        // 3. 上传
        let client = webdav::build_client()?;
        let remote_dir = input.config.remote_path.as_deref().unwrap_or("/i-code-backups/");
        let remote_path = webdav::normalize_remote_path(remote_dir, &file_name);

        webdav::ensure_directory(
            &client,
            &input.config.url,
            &input.config.username,
            &password_plaintext,
            remote_dir,
        )
        .await?;

        webdav::upload_file(
            &client,
            &input.config.url,
            &input.config.username,
            &password_plaintext,
            &remote_path,
            upload_bytes.clone(),
        )
        .await?;

        // 按 WebDAV 保留策略清理远程旧备份
        if let Err(e) = self
            .cleanup_old_webdav_backups(&input.config, &password_plaintext, remote_dir)
            .await
        {
            backup_warn!("推送 WebDAV 备份后清理远程旧备份失败: {e}");
        }

        // 清理本地临时文件
        let _ = fs::remove_file(&local_temp_path);

        backup_info!(
            "推送 WebDAV 备份成功: backup_id={}, remote_path={}, size={}bytes",
            backup_id, remote_path, upload_bytes.len()
        );

        Ok(BackupResult {
            success: true,
            backup_id,
            target: BackupTarget::Webdav,
            size_bytes: upload_bytes.len() as u64,
            path: remote_path,
            created_at,
            encrypted: Some(encrypt),
            error: None,
        })
    }

    /// 列出 WebDAV 备份
    pub async fn list_webdav_backups(&self, config: &WebDavConfig) -> IcodeResult<Vec<BackupListItem>> {
        let password_plaintext = self.resolve_webdav_password(config)?;

        backup_info!(
            "开始列出 WebDAV 备份: url={}, remote_dir={:?}",
            config.url, config.remote_path
        );

        let client = webdav::build_client()?;
        let remote_dir = config.remote_path.as_deref().unwrap_or("/i-code-backups/");

        let items = webdav::list_directory(
            &client,
            &config.url,
            &config.username,
            &password_plaintext,
            remote_dir,
        )
        .await?;

        backup_info!(
            "WebDAV PROPFIND 原始条目数: {}, remote_dir={}",
            items.len(),
            remote_dir
        );

        let mut backups = Vec::new();
        for item in items {
            let name = &item.display_name;
            if !name.starts_with("i-code-backup-") {
                continue;
            }
            let (backup_id, encrypted) = parse_backup_file_name(name);
            if backup_id.is_empty() {
                continue;
            }
            backups.push(BackupListItem {
                id: backup_id,
                target: BackupTarget::Webdav,
                path: item.path,
                created_at: item.last_modified,
                size_bytes: item.content_length,
                app_version: None,
                database_schema_version: None,
                encrypted: Some(encrypted),
            });
        }
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        backup_info!(
            "列出 WebDAV 备份完成: 共 {} 条, remote_dir={}",
            backups.len(),
            remote_dir
        );

        Ok(backups)
    }

    /// 恢复 WebDAV 备份
    pub async fn restore_webdav_backup(
        &self,
        input: &RestoreWebDavBackupInput,
    ) -> IcodeResult<RestoreBackupResult> {
        backup_info!(
            "开始恢复 WebDAV 备份: url={}, remote_path={}",
            input.config.url, input.remote_path
        );
        let password_plaintext = self.resolve_webdav_password(&input.config)?;
        let client = webdav::build_client()?;

        let encrypted_bytes = webdav::download_file(
            &client,
            &input.config.url,
            &input.config.username,
            &password_plaintext,
            &input.remote_path,
        )
        .await?;

        backup_info!("WebDAV 备份下载完成: size={} bytes", encrypted_bytes.len());

        let zip_bytes = if input.encrypted {
            backup_info!("开始解密 WebDAV 备份");
            let password = self.load_config_key()?.ok_or_else(|| {
                IcodeError::validation("恢复加密备份必须先在「设置」或「备份设置」中配置 1-20 位通用密码")
            })?;
            crypto::decrypt_backup(&password, &encrypted_bytes)?
        } else {
            encrypted_bytes
        };

        // 写入临时文件后调用本地恢复逻辑
        let backups_dir = self.resolve_backups_dir(None)?;
        let temp_zip = backups_dir.join("webdav-restore-temp.zip");
        fs::create_dir_all(&backups_dir)?;
        fs::write(&temp_zip, zip_bytes)?;
        backup_info!("已下载 WebDAV 备份到临时文件，准备调用本地恢复逻辑");

        let result = self.restore_backup(&temp_zip);
        let _ = fs::remove_file(&temp_zip);
        result
    }

    /// 删除 WebDAV 备份
    pub async fn delete_webdav_backup(&self, config: &WebDavConfig, remote_path: &str) -> IcodeResult<()> {
        backup_info!("开始删除 WebDAV 备份: remote_path={}", remote_path);
        let password_plaintext = self.resolve_webdav_password(config)?;
        let client = webdav::build_client()?;
        webdav::delete_file(
            &client,
            &config.url,
            &config.username,
            &password_plaintext,
            remote_path,
        )
        .await?;
        backup_info!("WebDAV 备份删除完成: remote_path={}", remote_path);
        Ok(())
    }

    /// 读取备份设置（从 app_settings.backup_settings_json）
    ///
    /// 该辅助方法通过数据库直接读取，避免循环依赖 SettingsService。
    pub fn load_backup_settings(&self) -> IcodeResult<BackupSettings> {
        let pool = db::get_db_pool()?;
        let conn = pool.get()?;
        let mut stmt = conn.prepare("SELECT backup_settings_json FROM app_settings WHERE id = 'default'")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let json: Option<String> = row.get(0)?;
            match json {
                Some(v) if !v.is_empty() => {
                    Ok(serde_json::from_str(&v).unwrap_or_default())
                }
                _ => Ok(BackupSettings::default()),
            }
        } else {
            Ok(BackupSettings::default())
        }
    }

    /// 读取通用密码（从 app_settings.config_key）
    ///
    /// 该密码同时用于 Secret 模块加密 API Key 与 Backup 模块加密远端备份文件。
    /// 返回 `Ok(None)` 表示用户尚未配置密码。
    fn load_config_key(&self) -> IcodeResult<Option<String>> {
        let pool = db::get_db_pool()?;
        let conn = pool.get()?;
        let mut stmt = conn.prepare("SELECT config_key FROM app_settings WHERE id = 'default'")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let value: Option<String> = row.get(0)?;
            Ok(value.filter(|s| !s.is_empty()))
        } else {
            Ok(None)
        }
    }

    // ===== WebDAV 配置管理 =====

    /// 列出已保存的 WebDAV 配置
    pub fn list_webdav_configs(&self) -> IcodeResult<Vec<WebDavConfigRecord>> {
        repository::list_webdav_configs()
    }

    /// 获取单个 WebDAV 配置
    pub fn get_webdav_config(&self, id: &str) -> IcodeResult<Option<WebDavConfigRecord>> {
        repository::get_webdav_config(id)
    }

    /// 保存 WebDAV 配置
    pub fn save_webdav_config(&self, input: &SaveWebDavConfigInput) -> IcodeResult<WebDavConfigRecord> {
        repository::save_webdav_config(input)
    }

    /// 删除 WebDAV 配置
    pub fn delete_webdav_config(&self, id: &str) -> IcodeResult<()> {
        repository::delete_webdav_config(id)
    }

    // ===== 内部辅助方法 =====

    /// 解析 WebDAV 密码（password_secret_id -> 明文）
    ///
    /// 支持两种格式：
    /// - `$PLAIN:{password}$`：来自 webdav_configs 表的明文密码占位
    /// - `$SECRET:{snowflake_id}$` 或裸雪花 ID：来自 Secret 模块的加密引用
    fn resolve_webdav_password(&self, config: &WebDavConfig) -> IcodeResult<String> {
        if config.password_secret_id.is_empty() {
            return Err(IcodeError::validation("WebDAV 密码未配置"));
        }

        // 优先处理明文占位符（新增 webdav_configs 表使用）
        if let Some(plain) = config.password_secret_id.strip_prefix("$PLAIN:") {
            if let Some(plain) = plain.strip_suffix("$") {
                if plain.is_empty() {
                    return Err(IcodeError::validation("WebDAV 密码不能为空"));
                }
                return Ok(plain.to_string());
            }
        }

        self.secret_handle.service().read_secret(&config.password_secret_id)
    }

    /// 执行 WAL checkpoint
    fn checkpoint_wal(&self) -> IcodeResult<()> {
        let pool = db::get_db_pool()?;
        let conn = pool.get()?;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    /// 计算数据库文件 SHA-256
    fn compute_db_checksum(&self) -> IcodeResult<String> {
        compute_file_checksum(&self.db_path)
    }

    /// 创建 zip 压缩包
    fn create_zip(&self, zip_path: &Path, meta: &BackupMeta) -> IcodeResult<()> {
        let file = fs::File::create(zip_path)?;
        let mut zip = ZipWriter::new(file);

        let db_options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let db_name = self
            .db_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("i-code.db");
        zip.start_file(db_name, db_options)?;
        let mut db_file = fs::File::open(&self.db_path)?;
        let mut buf = Vec::new();
        db_file.read_to_end(&mut buf)?;
        zip.write_all(&buf)?;

        let meta_options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("backup.json", meta_options)?;
        let meta_json = serde_json::to_string_pretty(meta)?;
        zip.write_all(meta_json.as_bytes())?;

        // 脚本公共存储（存在时打包）
        let storage_path = self.storage_path();
        if storage_path.exists() {
            let storage_options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file(STORAGE_FILE_NAME, storage_options)?;
            let mut storage_file = fs::File::open(&storage_path)?;
            let mut buf = Vec::new();
            storage_file.read_to_end(&mut buf)?;
            zip.write_all(&buf)?;
        }

        zip.finish()?;
        Ok(())
    }

    /// 从 zip 中读取 backup.json
    fn read_backup_meta_from_zip(&self, zip_path: &Path) -> IcodeResult<BackupMeta> {
        let file = fs::File::open(zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if entry.name() == "backup.json" {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                let meta: BackupMeta = serde_json::from_str(&content)?;
                return Ok(meta);
            }
        }
        Err(IcodeError::internal("备份包中未找到 backup.json"))
    }

    /// 从 zip 中提取数据库文件到指定路径
    fn extract_db_from_zip(&self, zip_path: &Path, dest: &Path) -> IcodeResult<()> {
        let file = fs::File::open(zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.name().to_string();
            if name == "i-code.db" || name.ends_with("/i-code.db") {
                let mut out = fs::File::create(dest)?;
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                out.write_all(&buf)?;
                return Ok(());
            }
        }
        Err(IcodeError::internal("备份包中未找到 i-code.db"))
    }

    /// 从 zip 中提取任意文件到指定路径；包内不存在时返回 `Ok(false)`（不报错）
    fn extract_file_from_zip(
        &self,
        zip_path: &Path,
        file_name: &str,
        dest: &Path,
    ) -> IcodeResult<bool> {
        let file = fs::File::open(zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.name().to_string();
            if name == file_name || name.ends_with(&format!("/{file_name}")) {
                let mut out = fs::File::create(dest)?;
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                out.write_all(&buf)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// 恢复脚本公共存储：从备份包提取 `script-storage.json` 并重新加载全局单例
    ///
    /// 备份包中无该文件（旧版备份）时保持当前存储不变。
    fn restore_script_storage(&self, backup_path: &Path) -> IcodeResult<()> {
        let storage_path = self.storage_path();
        if let Some(parent) = storage_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if self.extract_file_from_zip(backup_path, STORAGE_FILE_NAME, &storage_path)? {
            backup_info!(
                "已从备份包恢复脚本公共存储: {}",
                storage_path.to_string_lossy()
            );
            // 重新加载全局存储单例（幂等；失败不阻塞主流程）
            if let Some(parent) = storage_path.parent() {
                if let Err(e) = init_script_storage(parent) {
                    backup_warn!("重新加载脚本公共存储失败（重启后自动恢复）: {e}");
                }
            }
        } else {
            backup_info!("备份包中无脚本公共存储，跳过");
        }
        Ok(())
    }

    /// 创建紧急安全备份（恢复前调用）
    fn create_safety_backup(&self) -> IcodeResult<PathBuf> {
        fs::create_dir_all(&self.safety_backups_dir)?;

        let now = Utc::now();
        let timestamp = now.format("%Y%m%d-%H%M%S").to_string();
        let file_name = format!("auto-restore-safety-{timestamp}.zip");
        let safety_path = self.safety_backups_dir.join(&file_name);

        backup_info!(
            "开始创建紧急安全备份: path={}",
            safety_path.to_string_lossy()
        );
        self.checkpoint_wal()?;
        let checksum = self.compute_db_checksum()?;
        let meta = BackupMeta {
            version: "1.0".to_string(),
            app_version: self.app_version.clone(),
            created_at: now.to_rfc3339(),
            database_schema_version: self.schema_version,
            checksum,
            files: Some(vec!["i-code.db".to_string(), "backup.json".to_string()]),
            target: Some(BackupTarget::Local),
            encrypted: Some(false),
        };
        self.create_zip(&safety_path, &meta)?;
        self.cleanup_old_safety_backups(3)?;
        backup_info!(
            "紧急安全备份创建完成: path={}",
            safety_path.to_string_lossy()
        );

        Ok(safety_path)
    }

    /// 清理旧的安全备份，保留最近 N 份
    fn cleanup_old_safety_backups(&self, keep: usize) -> IcodeResult<()> {
        if !self.safety_backups_dir.exists() {
            return Ok(());
        }
        let mut safety_files: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
        for entry in fs::read_dir(&self.safety_backups_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("auto-restore-safety-") && name.ends_with(".zip") {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(modified) = meta.modified() {
                            safety_files.push((path, modified));
                        }
                    }
                }
            }
        }
        safety_files.sort_by(|a, b| b.1.cmp(&a.1));
        for (path, _) in safety_files.iter().skip(keep) {
            let _ = fs::remove_file(path);
        }
        Ok(())
    }

    /// 按本地保留策略清理旧备份
    ///
    /// 读取 `BackupSettings.local_retention_count`：
    /// - `0` 或 `None` 表示不限制，直接返回
    /// - 大于 0 时按文件修改时间保留最新的 N 份，删除其余文件
    fn cleanup_old_local_backups(&self, dir: &Path) -> IcodeResult<()> {
        let settings = self.load_backup_settings()?;
        let keep = match settings.local_retention_count {
            Some(count) if count > 0 => count as usize,
            _ => return Ok(()),
        };

        if !dir.exists() {
            return Ok(());
        }
        backup_info!("开始清理旧本地备份: dir={}, keep={}", dir.to_string_lossy(), keep);

        let mut backup_files: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("i-code-backup-") && (name.ends_with(".zip") || name.ends_with(".zip.enc")) {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(modified) = meta.modified() {
                            backup_files.push((path, modified));
                        }
                    }
                }
            }
        }
        backup_files.sort_by(|a, b| b.1.cmp(&a.1));
        let to_delete: Vec<_> = backup_files.iter().skip(keep).collect();
        let delete_count = to_delete.len();
        for (path, _) in to_delete {
            if let Err(e) = fs::remove_file(path) {
                backup_warn!("删除旧本地备份失败: path={}, err={}", path.to_string_lossy(), e);
            }
        }
        backup_info!("旧本地备份清理完成: 删除 {} 条", delete_count);
        Ok(())
    }

    /// 按 WebDAV 保留策略清理远程旧备份
    ///
    /// 读取 `BackupSettings.webdav_retention_count`：
    /// - `0` 或 `None` 表示不限制，直接返回
    /// - 大于 0 时按 `last_modified` 保留最新的 N 份，删除其余远程文件
    async fn cleanup_old_webdav_backups(
        &self,
        config: &WebDavConfig,
        password_plaintext: &str,
        remote_dir: &str,
    ) -> IcodeResult<()> {
        let settings = self.load_backup_settings()?;
        let keep = match settings.webdav_retention_count {
            Some(count) if count > 0 => count as usize,
            _ => return Ok(()),
        };
        backup_info!("开始清理远程 WebDAV 旧备份: remote_dir={}, keep={}", remote_dir, keep);

        let client = webdav::build_client()?;
        let items = webdav::list_directory(
            &client,
            &config.url,
            &config.username,
            password_plaintext,
            remote_dir,
        )
        .await?;

        let mut backup_items: Vec<(String, String)> = Vec::new();
        for item in items {
            let name = &item.display_name;
            if name.starts_with("i-code-backup-") && (name.ends_with(".zip") || name.ends_with(".zip.enc")) {
                backup_items.push((item.path, item.last_modified));
            }
        }
        backup_items.sort_by(|a, b| b.1.cmp(&a.1));

        let to_delete: Vec<_> = backup_items.iter().skip(keep).collect();
        let delete_count = to_delete.len();
        for (remote_path, _) in to_delete {
            if let Err(e) = webdav::delete_file(
                &client,
                &config.url,
                &config.username,
                password_plaintext,
                remote_path,
            )
            .await
            {
                backup_warn!("清理远程旧 WebDAV 备份失败: path={}, error={}", remote_path, e);
            }
        }
        backup_info!("远程 WebDAV 旧备份清理完成: 删除 {} 条", delete_count);

        Ok(())
    }

    /// 扫描数据库中缺失的 Secret 引用
    fn scan_missing_secrets(&self) -> IcodeResult<Vec<String>> {
        let pool = db::get_db_pool()?;
        let conn = pool.get()?;

        let mut all_ids: Vec<String> = Vec::new();

        let queries = [
            "SELECT auth_json FROM providers WHERE auth_json IS NOT NULL",
            "SELECT balance_provider_json FROM providers WHERE balance_provider_json IS NOT NULL",
            "SELECT proxy_json FROM providers WHERE proxy_json IS NOT NULL",
            "SELECT auth_json FROM cli_providers WHERE auth_json IS NOT NULL",
            "SELECT balance_json FROM cli_providers WHERE balance_json IS NOT NULL",
            "SELECT global_proxy_json FROM app_settings WHERE global_proxy_json IS NOT NULL",
            "SELECT network_retry_json FROM app_settings WHERE network_retry_json IS NOT NULL",
        ];
        for sql in queries {
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map([], |row| row.get::<_, Option<String>>(0))?;
            for row in rows {
                if let Some(json_str) = row? {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json_str) {
                        collect_secret_ids(&value, &mut all_ids);
                    }
                }
            }
        }

        let text_query = "SELECT value FROM provider_extra_headers
                          UNION ALL SELECT value FROM model_config_extra_headers";
        let mut stmt = conn.prepare(text_query)?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            let value = row?;
            if let Some(id) = crate::modules::secret::types::parse_secret_ref(&value) {
                all_ids.push(id.to_string());
            }
        }

        all_ids.sort();
        all_ids.dedup();

        let mut missing = Vec::new();
        let secret_svc = self.secret_handle.service();
        for id in &all_ids {
            if secret_svc.read_secret(id).is_err() {
                missing.push(id.clone());
            }
        }

        Ok(missing)
    }
}

/// 计算文件 SHA-256 校验和
fn compute_file_checksum(path: &Path) -> IcodeResult<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// 递归收集 JSON 值中的所有 Secret 引用 ID
fn collect_secret_ids(value: &serde_json::Value, ids: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            if let Some(id) = crate::modules::secret::types::parse_secret_ref(s) {
                ids.push(id.to_string());
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_secret_ids(item, ids);
            }
        }
        serde_json::Value::Object(obj) => {
            for (_, v) in obj {
                collect_secret_ids(v, ids);
            }
        }
        _ => {}
    }
}

/// 从备份文件名解析 ID 与是否加密
///
/// 支持：
/// - `i-code-backup-{yyyyMMdd-HHmmss}.zip`
/// - `i-code-backup-{yyyyMMdd-HHmmss}.zip.enc`
fn parse_backup_file_name(name: &str) -> (String, bool) {
    let stripped = name.strip_prefix("i-code-backup-");
    if stripped.is_none() {
        return (String::new(), false);
    }
    let after_prefix = stripped.unwrap();
    if let Some(core) = after_prefix.strip_suffix(".zip.enc") {
        return (core.to_string(), true);
    }
    if let Some(core) = after_prefix.strip_suffix(".zip") {
        return (core.to_string(), false);
    }
    if let Some(core) = after_prefix.strip_suffix(".tar.gz") {
        return (core.to_string(), false);
    }
    (String::new(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_checksum() {
        let tmp = std::env::temp_dir().join("i-code-test-checksum.txt");
        fs::write(&tmp, b"hello world").unwrap();
        let checksum = compute_file_checksum(&tmp).unwrap();
        assert_eq!(checksum, "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_parse_backup_file_name() {
        assert_eq!(
            parse_backup_file_name("i-code-backup-20260101-120000.zip"),
            ("20260101-120000".to_string(), false)
        );
        assert_eq!(
            parse_backup_file_name("i-code-backup-20260101-120000.zip.enc"),
            ("20260101-120000".to_string(), true)
        );
        assert_eq!(
            parse_backup_file_name("i-code-backup-20260101-120000.tar.gz"),
            ("20260101-120000".to_string(), false)
        );
        assert_eq!(parse_backup_file_name("other.zip"), (String::new(), false));
    }
}
