/**
 * 备份与恢复模块类型定义
 *
 * 对应 `docs/development.md` §5.15。将应用 SQLite 数据库及相关配置压缩打包，
 * 支持备份到本地目录或远程 WebDAV，并支持以覆盖方式恢复/同步备份。
 *
 * **覆盖原则**：备份的「推送」与「恢复」均采用**覆盖式**同步——
 * 目标端只保留最新一份同名备份；恢复时当前数据库被备份文件完全覆盖，不做增量合并。
 *
 * 依赖方向：依赖 `db` 和 `secret` 模块，仅由前端通过 Commands 触发，
 * 不反向依赖业务模块。
 */

import type { Timestamp, SnowflakeId } from '@/core/types'

/**
 * WebDAV 已保存配置记录
 *
 * 对应数据库 `webdav_configs` 表，密码以明文返回给前端表单。
 */
export interface WebDavConfigRecord {
  /** 配置唯一标识（雪花 ID） */
  id: SnowflakeId
  /** 配置显示名称 */
  name: string
  /** WebDAV 服务器 URL */
  url: string
  /** 用户名 */
  username: string
  /** 密码（明文） */
  password: string
  /** 远程目录路径 */
  remotePath: string
  /** 是否校验 TLS 证书 */
  strictSsl: boolean
  /** 服务预设 */
  preset: WebDavPreset
  /** 排序权重 */
  sortOrder: number
  /** 是否启用 */
  isEnabled: boolean
  /** 创建时间 */
  createdAt: Timestamp
  /** 更新时间 */
  updatedAt: Timestamp
}

/**
 * 保存 WebDAV 配置的输入参数
 */
export interface SaveWebDavConfigInput {
  /** 配置 ID；为空表示新建 */
  id?: SnowflakeId
  /** 配置显示名称 */
  name: string
  /** WebDAV 服务器 URL */
  url: string
  /** 用户名 */
  username: string
  /** 密码（明文） */
  password: string
  /** 远程目录路径 */
  remotePath: string
  /** 是否校验 TLS 证书 */
  strictSsl: boolean
  /** 服务预设 */
  preset: WebDavPreset
}

/**
 * WebDAV 服务预设
 */
export type WebDavPreset = 'jianguoyun' | 'koofr' | 'nextcloud' | 'custom'

/**
 * 备份目标
 * - `local`：本地磁盘
 * - `webdav`：远程 WebDAV 服务器
 */
export type BackupTarget = 'local' | 'webdav'

/**
 * 备份格式
 * - `zip`：默认，跨平台友好
 * - `tar-gz`：tar.gz 压缩包
 */
export type BackupFormat = 'zip' | 'tar-gz'

/**
 * 备份元数据
 * 包含在备份包内的 `backup.json` 文件，用于恢复前兼容性校验
 */
export interface BackupMeta {
  /** 备份格式版本，如 `"1.0"` */
  version: string
  /** 生成备份时的应用版本号 */
  appVersion: string
  /** ISO 8601 时间戳 */
  createdAt: Timestamp
  /** 数据库迁移版本号，用于恢复前兼容性校验 */
  databaseSchemaVersion: number
  /** 数据库文件 SHA-256 校验和 */
  checksum: string
  /** 备份包含的文件清单 */
  files?: string[]
  /** 备份目标 */
  target?: BackupTarget
  /** 是否加密 */
  encrypted?: boolean
}

/**
 * WebDAV 连接配置
 *
 * 密码字段必须经 `secret` 模块加密存储，配置中仅存 `$SECRET:{snowflake_id}$` 引用。
 * 禁止明文落库。
 */
export interface WebDavConfig {
  /** WebDAV 服务器 URL，如 `https://dav.example.com` */
  url: string
  /** 用户名 */
  username: string
  /** 密码的 Secret 引用 ID */
  passwordSecretId: SnowflakeId
  /** 远程目录路径，默认 `/i-code-backups/` */
  remotePath?: string
  /** 是否校验 TLS 证书，默认 true */
  strictSsl?: boolean
}

/**
 * 备份操作结果
 */
export interface BackupResult {
  /** 是否成功 */
  success: boolean
  /** 备份 ID（基于时间戳生成） */
  backupId: string
  /** 备份目标 */
  target: BackupTarget
  /** 备份文件大小（字节） */
  sizeBytes: number
  /** 备份文件路径（本地路径或 WebDAV 远程路径） */
  path: string
  /** 备份创建时间戳 */
  createdAt: Timestamp
  /** 是否加密 */
  encrypted?: boolean
  /** 失败时的错误信息 */
  error?: string
}

/**
 * 备份列表项
 * 用于本地与 WebDAV 备份列表展示
 */
export interface BackupListItem {
  /** 备份 ID（基于文件名解析） */
  id: string
  /** 备份目标 */
  target: BackupTarget
  /** 文件路径 */
  path: string
  /** 创建时间戳 */
  createdAt: Timestamp
  /** 文件大小（字节） */
  sizeBytes: number
  /** 生成备份时的应用版本号（从文件名或 backup.json 解析） */
  appVersion?: string
  /** 数据库 schema 版本（从 backup.json 解析，未解析时为空） */
  databaseSchemaVersion?: number
  /** 是否加密 */
  encrypted?: boolean
}

/**
 * 备份错误细分
 * 对应 development.md §5.15.8
 */
export type BackupErrorCode =
  | 'DatabaseLocked'
  | 'ChecksumMismatch'
  | 'SchemaVersionTooNew'
  | 'WebDavAuthFailed'
  | 'WebDavNetworkError'
  | 'WebDavQuotaExceeded'
  | 'RestoreSafetyBackupFailed'
  | 'Unknown'

/**
 * 恢复备份结果
 */
export interface RestoreBackupResult {
  /** 是否成功 */
  success: boolean
  /** 恢复的备份文件路径 */
  backupPath: string
  /** 自动生成的安全备份路径（恢复前自动创建的紧急备份） */
  safetyBackupPath?: string
  /** 恢复后扫描发现缺失的 Secret 引用 ID 列表（系统密钥链模式下跨设备恢复） */
  missingSecrets?: SnowflakeId[]
  /** 数据库 schema 版本差异信息 */
  schemaVersionInfo?: {
    backupVersion: number
    currentVersion: number
    migrated: boolean
  }
  /** 是否需要重启应用（覆盖式数据库文件替换后必须重启才能重新加载连接） */
  needsRestart?: boolean
  /** 错误码 */
  errorCode?: BackupErrorCode
  /** 错误信息 */
  errorMessage?: string
}

/**
 * 创建备份的输入参数
 */
export interface CreateBackupInput {
  format: BackupFormat
  /** 是否包含 app_settings.json 快照 */
  includeSettings?: boolean
  /** 是否包含 secret_manifest.json 清单 */
  includeSecretManifest?: boolean
}

/**
 * 推送备份到本地的输入参数
 */
export interface PushToLocalInput {
  backupId: string
  /** 目标目录；为空时使用配置的默认目录 */
  directory?: string
}

/**
 * 推送备份到 WebDAV 的输入参数
 */
export interface CreateWebDavBackupInput {
  config: WebDavConfig
  /** 是否使用通用密码加密备份 */
  encrypt: boolean
}

/**
 * 恢复 WebDAV 备份的输入参数
 */
export interface RestoreWebDavBackupInput {
  config: WebDavConfig
  /** 远程文件路径 */
  remotePath: string
  /** 是否加密 */
  encrypted: boolean
}

/**
 * 备份设置
 * 持久化到 app_settings 或独立配置文件
 */
export interface BackupSettings {
  /** 本地备份目录，默认应用数据目录下的 `backups/` */
  localDirectory?: string
  /** 默认备份格式 */
  defaultFormat: BackupFormat
  /** 本地备份保留份数，0=不限制 */
  localRetentionCount?: number
  /** WebDAV 配置（密码为 Secret 引用） */
  webdav?: WebDavConfig
  /** WebDAV 备份保留份数，0=不限制 */
  webdavRetentionCount?: number
  /** 是否在恢复前自动创建安全备份，默认 true */
  enableSafetyBackupBeforeRestore: boolean
  /** WebDAV 服务预设 */
  webdavPreset?: WebDavPreset
}
