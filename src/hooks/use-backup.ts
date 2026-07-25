import { invokeCommand } from '@/hooks/use-command'
import type {
  BackupListItem,
  BackupResult,
  BackupTarget,
  CreateBackupInput,
  CreateWebDavBackupInput,
  RestoreBackupResult,
  RestoreWebDavBackupInput,
  SaveWebDavConfigInput,
  WebDavConfig,
  WebDavConfigRecord,
} from '@/modules/backup/types'
import type { SecretMask } from '@/modules/secret/types'

/**
 * 更新已有 Secret 明文
 */
export async function updateSecret(id: string, plaintext: string, label?: string): Promise<SecretMask> {
  return invokeCommand<SecretMask>('secret_update', { id, plaintext, label })
}

/**
 * 备份模块命令调用集合
 *
 * 封装本地备份、WebDAV 备份的创建、列出、恢复、删除命令。
 */

/** 创建本地备份 */
export async function createBackup(input: CreateBackupInput): Promise<BackupResult> {
  return invokeCommand<BackupResult>('backup_create', { input })
}

/** 列出本地备份 */
export async function listLocalBackups(directory?: string): Promise<BackupListItem[]> {
  return invokeCommand<BackupListItem[]>('backup_list_local', { directory })
}

/** 恢复本地备份 */
export async function restoreBackup(path: string): Promise<RestoreBackupResult> {
  return invokeCommand<RestoreBackupResult>('backup_restore', { path })
}

/** 删除本地或 WebDAV 备份 */
export async function deleteBackup(target: BackupTarget, path: string): Promise<void> {
  return invokeCommand<void>('backup_delete', { target, path })
}

/** 推送备份到 WebDAV */
export async function pushWebDavBackup(input: CreateWebDavBackupInput): Promise<BackupResult> {
  return invokeCommand<BackupResult>('backup_push_webdav', { input })
}

/** 列出 WebDAV 备份 */
export async function listWebDavBackups(config: WebDavConfig): Promise<BackupListItem[]> {
  return invokeCommand<BackupListItem[]>('backup_list_webdav', { config })
}

/** 恢复 WebDAV 备份 */
export async function restoreWebDavBackup(
  input: RestoreWebDavBackupInput
): Promise<RestoreBackupResult> {
  return invokeCommand<RestoreBackupResult>('backup_restore_webdav', { input })
}

/** 删除 WebDAV 备份 */
export async function deleteWebDavBackup(
  config: WebDavConfig,
  remotePath: string
): Promise<void> {
  return invokeCommand<void>('backup_delete_webdav', { config, remotePath })
}

/**
 * 保存 WebDAV 密码为 Secret
 *
 * 返回 SecretMask，其中 id 可作为 WebDavConfig.passwordSecretId 使用。
 */
export async function saveWebDavPassword(plaintext: string, label?: string): Promise<SecretMask> {
  return invokeCommand<SecretMask>('secret_save', {
    input: {
      kind: 'webdav-password',
      plaintext,
      label: label ?? 'WebDAV Password',
    },
  })
}

/**
 * 列出已保存的 WebDAV 配置
 */
export async function listWebDavConfigs(): Promise<WebDavConfigRecord[]> {
  return invokeCommand<WebDavConfigRecord[]>('backup_webdav_config_list', {})
}

/**
 * 获取单个 WebDAV 配置
 */
export async function getWebDavConfig(id: string): Promise<WebDavConfigRecord | null> {
  return invokeCommand<WebDavConfigRecord | null>('backup_webdav_config_get', { id })
}

/**
 * 保存 WebDAV 配置（新建或更新）
 */
export async function saveWebDavConfig(input: SaveWebDavConfigInput): Promise<WebDavConfigRecord> {
  return invokeCommand<WebDavConfigRecord>('backup_webdav_config_save', { input })
}

/**
 * 删除 WebDAV 配置
 */
export async function deleteWebDavConfig(id: string): Promise<void> {
  return invokeCommand<void>('backup_webdav_config_delete', { id })
}
