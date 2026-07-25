/**
 * 敏感凭据模块类型定义
 *
 * 与 `docs/database.md` §4.2 中的 `secrets` 表结构对齐。
 * 后端 Rust 通过 ts-rs 自动生成同名类型，前端禁止手写同步覆盖此文件。
 *
 * **架构约束**：所有加密/解密、密钥链读写、`$SECRET:{snowflake_id}$` 引用解析
 * 仅在后端 `secret` 模块进行。前端只有一个 `secret-input.tsx` 密码输入框组件，
 * 将用户输入的明文传给后端 Command 后立即丢弃，从不保留明文。
 */

import type { Timestamp, SnowflakeId } from '@/core/types'

/**
 * 敏感凭据类型
 * 对应 `secrets.kind` 列
 */
export type SecretKind = 'api-key' | 'oauth-token' | 'proxy-auth' | 'gateway-key' | 'webdav-password'

/**
 * 敏感数据存储模式
 * 对应 `app_settings.store_secrets_in_keychain` 字段
 * - `keychain`：使用系统密钥链（macOS Keychain / Windows Credential Manager / libsecret）
 * - `encrypted`：本地 AES-GCM 加密，加密密钥由系统密钥链保护
 */
export type SecretStorageMode = 'keychain' | 'encrypted'

/**
 * 敏感凭据记录
 * 对应 `secrets` 表
 *
 * `encryptedValue` 在数据库中存储：
 * - `keychain` 模式下：密钥链句柄索引（前端不可见明文）
 * - `encrypted` 模式下：AES-GCM 密文（前端不可见明文）
 */
export interface Secret {
  id: SnowflakeId
  kind: SecretKind
  /** AES-GCM 密文或密钥链句柄索引（前端永远不接收此字段） */
  encryptedValue: Uint8Array
  /** 展示用标签 */
  label?: string
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 敏感凭据掩码视图（前端展示用）
 * 后端返回 Secret 列表时使用此类型，永远不暴露明文或密文
 */
export interface SecretMask {
  id: SnowflakeId
  kind: SecretKind
  label?: string
  /** 创建时间，用于展示与排序 */
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * Secret 引用元数据
 * 用于在配置对象中标注哪些字段引用了 Secret
 */
export interface SecretRef {
  id: SnowflakeId
  kind: SecretKind
  label?: string
}

/**
 * 保存 Secret 的输入参数
 * 前端通过 `secret-input.tsx` 收集明文后传给后端 Command
 */
export interface SaveSecretInput {
  kind: SecretKind
  /** 明文值（仅在后端短暂存在，加密后立即丢弃） */
  plaintext: string
  label?: string
}

/**
 * Secret 引用解析结果
 * 后端扫描配置对象中所有 `$SECRET:{snowflake_id}$` 字符串后返回
 */
export interface SecretReferenceScanResult {
  /** 找到的所有 Secret 引用 ID */
  secretIds: SnowflakeId[]
  /** 引用位置详情：字段路径 → Secret ID */
  references: Array<{
    path: string
    secretId: SnowflakeId
  }>
  /** 不存在或无法读取的 Secret ID */
  missing: SnowflakeId[]
}

/**
 * 构造 Secret 引用字符串
 * 在配置中作为占位符存储，运行时由后端 `secret.service.resolve()` 替换为明文
 */
export function buildSecretRef(id: SnowflakeId): string {
  return `$SECRET:${id}$`
}

/**
 * 从字符串中解析 Secret 引用
 * 仅匹配完整的 `$SECRET:{snowflake_id}$` 格式，部分匹配返回 null
 */
export function parseSecretRef(ref: string): SnowflakeId | null {
  const match = ref.match(/^\$SECRET:([^$]+)\$$/)
  return match?.[1] ?? null
}
