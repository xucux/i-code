//! # 备份模块类型定义
//!
//! 与前端 `src/modules/backup/types.ts` 对齐。

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// WebDAV 服务预设
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WebDavPreset {
    /// 坚果云
    Jianguoyun,
    /// Koofr
    Koofr,
    /// Nextcloud / ownCloud
    Nextcloud,
    /// 自定义
    Custom,
}

impl WebDavPreset {
    /// 预设显示名称
    pub fn label(&self) -> &'static str {
        match self {
            Self::Jianguoyun => "坚果云",
            Self::Koofr => "Koofr",
            Self::Nextcloud => "Nextcloud",
            Self::Custom => "自定义",
        }
    }

    /// 预设默认 URL
    pub fn default_url(&self) -> &'static str {
        match self {
            Self::Jianguoyun => "https://dav.jianguoyun.com/dav/",
            Self::Koofr => "https://app.koofr.net/dav/Koofr/",
            Self::Nextcloud => "https://example.com/remote.php/dav/files/{username}/",
            Self::Custom => "",
        }
    }

    /// 预设默认远程目录
    pub fn default_remote_path(&self) -> &'static str {
        "/i-code-backups/"
    }

    /// 转换为数据库存储字符串（kebab-case 小写）
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Jianguoyun => "jianguoyun",
            Self::Koofr => "koofr",
            Self::Nextcloud => "nextcloud",
            Self::Custom => "custom",
        }
    }

    /// 从数据库存储字符串解析
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "jianguoyun" => Some(Self::Jianguoyun),
            "koofr" => Some(Self::Koofr),
            "nextcloud" => Some(Self::Nextcloud),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

/// 备份目标
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupTarget {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "webdav")]
    Webdav,
}

/// 备份格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum BackupFormat {
    /// 默认，跨平台友好
    #[default]
    Zip,
    /// tar.gz 压缩包
    TarGz,
}

/// 备份元数据
///
/// 包含在备份包内的 `backup.json` 文件，用于恢复前兼容性校验
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupMeta {
    /// 备份格式版本，如 `"1.0"`
    pub version: String,
    /// 生成备份时的应用版本号
    pub app_version: String,
    /// ISO 8601 时间戳
    pub created_at: String,
    /// 数据库迁移版本号，用于恢复前兼容性校验
    pub database_schema_version: u32,
    /// 数据库文件 SHA-256 校验和
    pub checksum: String,
    /// 备份包含的文件清单
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
    /// 备份目标
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<BackupTarget>,
    /// 是否加密（WebDAV 加密备份）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted: Option<bool>,
}

/// WebDAV 连接配置
///
/// 密码字段必须经 `secret` 模块加密存储，配置中仅存 `$SECRET:{snowflake_id}$` 引用。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavConfig {
    /// WebDAV 服务器 URL
    pub url: String,
    /// 用户名
    pub username: String,
    /// 密码的 Secret 引用 ID
    pub password_secret_id: String,
    /// 远程目录路径，默认 `/i-code-backups/`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_path: Option<String>,
    /// 是否校验 TLS 证书，默认 true
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict_ssl: Option<bool>,
}

impl WebDavConfig {
    /// 从 WebDavConfigRecord 构造运行时连接配置
    ///
    /// 将数据库中明文存储的 WebDAV 密码转换为 Secret 引用占位形式，
    /// 供现有 WebDAV 备份方法（resolve_webdav_password）直接解析使用。
    pub fn from_record(record: &WebDavConfigRecord) -> Self {
        Self {
            url: record.url.clone(),
            username: record.username.clone(),
            password_secret_id: format!("$PLAIN:{}$", record.password),
            remote_path: Some(record.remote_path.clone()),
            strict_ssl: Some(record.strict_ssl),
        }
    }
}

/// WebDAV 已保存配置记录
///
/// 对应 `webdav_configs` 表，密码按当前业务需求以明文落库。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavConfigRecord {
    /// 配置唯一标识（UUID）
    pub id: String,
    /// 配置显示名称
    pub name: String,
    /// WebDAV 服务器 URL
    pub url: String,
    /// 用户名
    pub username: String,
    /// 密码（明文）
    pub password: String,
    /// 远程目录路径
    pub remote_path: String,
    /// 是否校验 TLS 证书
    pub strict_ssl: bool,
    /// 服务预设
    pub preset: WebDavPreset,
    /// 排序权重
    pub sort_order: u32,
    /// 是否启用
    pub is_enabled: bool,
    /// 创建时间（RFC3339）
    pub created_at: String,
    /// 更新时间（RFC3339）
    pub updated_at: String,
}

/// 保存 WebDAV 配置的输入参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveWebDavConfigInput {
    /// 配置 ID；传 None 表示新建
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 配置显示名称
    pub name: String,
    /// WebDAV 服务器 URL
    pub url: String,
    /// 用户名
    pub username: String,
    /// 密码（明文）
    pub password: String,
    /// 远程目录路径
    pub remote_path: String,
    /// 是否校验 TLS 证书
    pub strict_ssl: bool,
    /// 服务预设
    pub preset: WebDavPreset,
}

/// 备份操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    /// 是否成功
    pub success: bool,
    /// 备份 ID（基于时间戳生成）
    pub backup_id: String,
    /// 备份目标
    pub target: BackupTarget,
    /// 备份文件大小（字节）
    pub size_bytes: u64,
    /// 备份文件路径（本地路径或 WebDAV 远程路径）
    pub path: String,
    /// 备份创建时间戳
    pub created_at: String,
    /// 是否加密
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted: Option<bool>,
    /// 失败时的错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 备份列表项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupListItem {
    /// 备份 ID（基于文件名解析）
    pub id: String,
    /// 备份目标
    pub target: BackupTarget,
    /// 文件路径
    pub path: String,
    /// 创建时间戳
    pub created_at: String,
    /// 文件大小（字节）
    pub size_bytes: u64,
    /// 生成备份时的应用版本号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
    /// 数据库 schema 版本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_schema_version: Option<u32>,
    /// 是否加密
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted: Option<bool>,
}

/// 备份错误细分
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupErrorCode {
    DatabaseLocked,
    ChecksumMismatch,
    SchemaVersionTooNew,
    WebDavAuthFailed,
    WebDavNetworkError,
    WebDavQuotaExceeded,
    RestoreSafetyBackupFailed,
    Unknown,
}

impl BackupErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DatabaseLocked => "DatabaseLocked",
            Self::ChecksumMismatch => "ChecksumMismatch",
            Self::SchemaVersionTooNew => "SchemaVersionTooNew",
            Self::WebDavAuthFailed => "WebDavAuthFailed",
            Self::WebDavNetworkError => "WebDavNetworkError",
            Self::WebDavQuotaExceeded => "WebDavQuotaExceeded",
            Self::RestoreSafetyBackupFailed => "RestoreSafetyBackupFailed",
            Self::Unknown => "Unknown",
        }
    }
}

/// 恢复备份结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreBackupResult {
    /// 是否成功
    pub success: bool,
    /// 恢复的备份文件路径
    pub backup_path: String,
    /// 自动生成的安全备份路径
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_backup_path: Option<String>,
    /// 恢复后扫描发现缺失的 Secret 引用 ID 列表
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_secrets: Vec<String>,
    /// 数据库 schema 版本差异信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version_info: Option<SchemaVersionInfo>,
    /// 是否需要重启应用
    ///
    /// 当前实现为覆盖式数据库文件替换，恢复成功后必须重启应用才能重新加载数据库连接。
    #[serde(default)]
    pub needs_restart: bool,
    /// 错误码
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<BackupErrorCode>,
    /// 错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Schema 版本差异信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaVersionInfo {
    pub backup_version: u32,
    pub current_version: u32,
    pub migrated: bool,
}

/// 创建备份的输入参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBackupInput {
    pub format: BackupFormat,
    /// 是否包含 app_settings.json 快照
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_settings: Option<bool>,
    /// 是否包含 secret_manifest.json 清单
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_secret_manifest: Option<bool>,
}

/// 推送备份到本地的输入参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushToLocalInput {
    pub backup_id: String,
    /// 目标目录；为空时使用配置的默认目录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
}

/// 推送备份到 WebDAV 的输入参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushToWebDavInput {
    pub backup_id: String,
    pub config: WebDavConfig,
}

/// 备份设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettings {
    /// 本地备份目录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_directory: Option<String>,
    /// 默认备份格式
    #[serde(default)]
    pub default_format: BackupFormat,
    /// 本地备份保留份数，0=不限制
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_retention_count: Option<u32>,
    /// WebDAV 配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webdav: Option<WebDavConfig>,
    /// WebDAV 备份保留份数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webdav_retention_count: Option<u32>,
    /// 是否在恢复前自动创建安全备份，默认 true
    #[serde(default = "default_true")]
    pub enable_safety_backup_before_restore: bool,
    /// WebDAV 服务预设
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webdav_preset: Option<WebDavPreset>,
}

fn default_true() -> bool {
    true
}

impl Default for BackupSettings {
    fn default() -> Self {
        Self {
            local_directory: None,
            default_format: BackupFormat::Zip,
            local_retention_count: Some(10),
            webdav: None,
            webdav_retention_count: Some(10),
            enable_safety_backup_before_restore: true,
            webdav_preset: Some(WebDavPreset::Custom),
        }
    }
}

/// 创建 WebDAV 备份的输入参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWebDavBackupInput {
    pub config: WebDavConfig,
    /// 是否使用通用密码加密备份
    #[serde(default)]
    pub encrypt: bool,
}

/// 恢复 WebDAV 备份的输入参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreWebDavBackupInput {
    pub config: WebDavConfig,
    /// 远程文件路径
    pub remote_path: String,
    /// 是否加密
    #[serde(default)]
    pub encrypted: bool,
}

/// 更新备份设置的输入参数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBackupSettingsInput {
    /// 本地备份目录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_directory: Option<Option<String>>,
    /// 默认备份格式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_format: Option<BackupFormat>,
    /// 本地备份保留份数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_retention_count: Option<Option<u32>>,
    /// WebDAV 配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webdav: Option<Option<WebDavConfig>>,
    /// WebDAV 备份保留份数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webdav_retention_count: Option<Option<u32>>,
    /// 恢复前是否自动创建安全备份
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_safety_backup_before_restore: Option<bool>,
    /// WebDAV 服务预设
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webdav_preset: Option<Option<WebDavPreset>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_format_serde() {
        assert_eq!(serde_json::to_string(&BackupFormat::Zip).unwrap(), "\"zip\"");
        assert_eq!(
            serde_json::to_string(&BackupFormat::TarGz).unwrap(),
            "\"tar-gz\""
        );
        let f: BackupFormat = serde_json::from_str("\"zip\"").unwrap();
        assert_eq!(f, BackupFormat::Zip);
    }

    #[test]
    fn test_backup_target_serde() {
        assert_eq!(
            serde_json::to_string(&BackupTarget::Local).unwrap(),
            "\"local\""
        );
        assert_eq!(
            serde_json::to_string(&BackupTarget::Webdav).unwrap(),
            "\"webdav\""
        );
    }

    #[test]
    fn test_backup_error_code_as_str() {
        assert_eq!(BackupErrorCode::DatabaseLocked.as_str(), "DatabaseLocked");
        assert_eq!(BackupErrorCode::Unknown.as_str(), "Unknown");
    }
}
