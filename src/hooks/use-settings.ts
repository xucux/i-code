import { invokeCommand } from '@/hooks/use-command'
import type { AppSettingsDto, UpdateSettingsInput } from '@/modules/settings/types'

/**
 * Settings 模块读写操作
 *
 * 直接调用后端 Tauri Commands 完成应用设置读取与更新。
 */

export async function getSettings(): Promise<AppSettingsDto> {
  return invokeCommand<AppSettingsDto>('settings_get', {})
}

export async function updateSettings(input: UpdateSettingsInput): Promise<AppSettingsDto> {
  return invokeCommand<AppSettingsDto>('settings_update', { input })
}

/**
 * 获取 tauri-plugin-log 日志文件目录
 *
 * 返回应用日志目录的绝对路径，用于在设置页展示日志文件实际存储位置。
 */
export async function getLogDir(): Promise<string> {
  return invokeCommand<string>('settings_log_dir', {})
}

/**
 * 获取应用配置目录
 *
 * 与数据库（`i-code.db`）同目录；提示词库（`prompt/`）、备份等均在此目录下。
 */
export async function getConfigDir(): Promise<string> {
  return invokeCommand<string>('settings_config_dir', {})
}

/**
 * 通过系统文件浏览器打开目录
 *
 * 跨平台：Windows 资源管理器 / macOS Finder / Linux 系统文件管理器。
 */
export async function openDirectory(path: string): Promise<void> {
  return invokeCommand<void>('settings_open_directory', { path })
}
