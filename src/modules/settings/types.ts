/**
 * 应用设置模块类型定义
 *
 * 与 `docs/database.md` §4.1 中的 `app_settings` 表结构对齐。
 * 后端 Rust 通过 ts-rs 自动生成同名类型，前端禁止手写同步覆盖此文件。
 */

import type { Locale, Theme, Timestamp } from '@/core/types'
import type {
  RetryConfig,
  TimeoutConfig,
} from '@/modules/ai-gateway/types'
import type { BackupSettings } from '@/modules/backup/types'

/**
 * 全局代理配置
 * 对应 app_settings.global_proxy_json 字段
 *
 * 与供应商级代理 `ProxyConfig` 区分：全局代理支持「直连」「自定义」「系统代理」
 * 「VS Code 代理」四种策略，用于应用级网络设置。
 */
export interface GlobalProxyConfig {
  type: 'direct' | 'custom' | 'system' | 'vscode'
  /** 自定义代理 URL（仅 `custom` 类型生效） */
  url?: string
  /** 代理认证凭据或 `$SECRET:{snowflake_id}$` 引用 */
  authorization?: string
  /** 是否严格校验 SSL 证书（默认 true） */
  strictSSL?: boolean
  /** 不走代理的主机列表（NO_PROXY 环境变量等价物） */
  noProxy?: string[]
}

/**
 * 全局日志级别
 *
 * 对应 `app_settings.log_level` 列，控制 tauri-plugin-log 的输出级别。
 */
export type LogLevel = 'trace' | 'debug' | 'info' | 'warn' | 'error'

/** 日志级别可选项 */
export const LOG_LEVEL_OPTIONS: { value: LogLevel; labelKey: string }[] = [
  { value: 'trace', labelKey: 'settings.logLevel.trace' },
  { value: 'debug', labelKey: 'settings.logLevel.debug' },
  { value: 'info', labelKey: 'settings.logLevel.info' },
  { value: 'warn', labelKey: 'settings.logLevel.warn' },
  { value: 'error', labelKey: 'settings.logLevel.error' },
]

/**
 * 标题栏信息展示配置
 *
 * 与后端 `TitleBarInfoConfig` 对齐，控制标题栏中间胶囊展示的信息项。
 * 最多同时展示 3 项，超出时按优先级取舍由调用方决定。
 */
export interface TitleBarInfoConfig {
  /** 展示 Token 消耗总数 */
  showTokens: boolean
  /** 展示每分钟请求数（RPM） */
  showRpm: boolean
  /** 展示平均请求延迟 */
  showLatency: boolean
  /** 展示应用内存占用 */
  showMemory: boolean
  /** 展示网关运行状态 */
  showGatewayStatus: boolean
}

/** 标题栏信息配置默认值 */
export const DEFAULT_TITLEBAR_INFO_CONFIG: TitleBarInfoConfig = {
  showTokens: true,
  showRpm: true,
  showLatency: false,
  showMemory: true,
  showGatewayStatus: true,
}

/**
 * 应用全局设置
 *
 * 对应 `app_settings` 表，固定单例记录（id = 'default'）。
 * 全局代理、超时、重试等 JSON 字段在运行时由后端解析为对应结构。
 * 网关监听地址、端口、默认 API Key 已迁移到 `gateway_settings` 表。
 */
export interface AppSettings {
  /** 固定值 `'default'`，保证单例 */
  id: 'default'
  theme: Theme
  locale: Locale
  /** 全局代理配置 JSON（GlobalProxyConfig 序列化） */
  globalProxyJson?: string
  /** 默认请求超时（毫秒），默认 120000 */
  networkTimeoutMs?: number
  /** 全局重试策略 JSON（RetryConfig 序列化） */
  networkRetryJson?: string
  /** 是否使用系统密钥链存储敏感数据；false=本地 AES-GCM 加密 */
  storeSecretsInKeychain: boolean
  /** 通用密码（1-20 位），经 SHA-256 派生为 AES-256-GCM 密钥，用于加密 Secret 与远端备份文件 */
  configKey?: string
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 应用设置 DTO（前端表单与后端通信使用）
 *
 * 与 `AppSettings` 区别：将 JSON 字符串字段反序列化为强类型对象，
 * 便于前端表单直接绑定与校验。
 */
export interface AppSettingsDto {
  theme: Theme
  locale: Locale
  globalProxyEnabled: boolean
  globalProxy?: GlobalProxyConfig
  networkTimeoutMs?: number
  networkRetry?: RetryConfig
  networkTimeout?: TimeoutConfig
  storeSecretsInKeychain: boolean
  configKey?: string
  /** 标题栏信息展示配置 */
  titlebarInfo: TitleBarInfoConfig
  /** 标题栏信息展示配置 JSON（原始字符串，与数据库字段对应） */
  titlebarInfoJson: string
  /** 备份设置 */
  backupSettings: BackupSettings
  /** 开机自启开关 */
  autoStartEnabled: boolean
  /** 网关上次关闭时的运行状态（用于开机自启时恢复网关） */
    gatewayLastRunning: boolean
    /** 全局日志级别 */
    logLevel: LogLevel
  }

  /**
   * 更新应用设置的输入参数
   * 所有字段均为可选，仅传递需要变更的字段。
   */
  export interface UpdateSettingsInput {
  theme?: Theme
  locale?: Locale
  globalProxyEnabled?: boolean
  globalProxy?: GlobalProxyConfig
  networkTimeoutMs?: number
  networkRetry?: RetryConfig
  networkTimeout?: TimeoutConfig
  storeSecretsInKeychain?: boolean
  /** 通用密码（1-20 位）；传 `null` 表示清空，不传表示不更新 */
  configKey?: string | null
  /** 标题栏信息展示配置 */
  titlebarInfo?: TitleBarInfoConfig
  /** 备份设置 */
  backupSettings?: BackupSettings
  /** 开机自启开关 */
    autoStartEnabled?: boolean
    /** 网关上次关闭时的运行状态 */
    gatewayLastRunning?: boolean
    /** 全局日志级别 */
    logLevel?: LogLevel
  }
