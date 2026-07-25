/**
 * 全局常量定义
 *
 * 包含 $SECRET: 引用前缀、应用名、默认网关地址、主题与语言枚举等。
 * 业务模块引用常量时应从此文件导入，避免硬编码字符串。
 */

/**
 * Secret 引用前缀，配置中以 `$SECRET:{snowflake_id}$` 形式存储敏感值引用
 * 实际明文仅在后端 secret 模块中解密，前端永远不接触明文。
 */
export const SECRET_PREFIX = '$SECRET:'
export const SECRET_SUFFIX = '$'

/**
 * 完整 Secret 引用正则，用于扫描配置字符串中的所有引用
 * 全局匹配模式：`$SECRET:{snowflake_id}$`
 */
export const SECRET_REF_REGEX = /\$SECRET:(\d{1,19})\$/g

/** 应用名 */
export const APP_NAME = 'i-code'

/** 数据库文件名（位于应用数据目录下） */
export const DB_FILE_NAME = 'i-code.db'

/** 默认网关监听地址 */
export const DEFAULT_GATEWAY_HOST = '127.0.0.1'
export const DEFAULT_GATEWAY_PORT = 54321

/** 默认网络超时（毫秒） */
export const DEFAULT_NETWORK_TIMEOUT_MS = 120_000

/**
 * 应用支持的六种主题
 * 与 `src/modules/theme/themes/` 下的 CSS 文件一一对应
 */
export const THEMES = [
  'light',
  'dark',
  'claude-light',
  'claude-dark',
  'deepseek-light',
  'deepseek-dark',
] as const

/** 应用支持的语言列表 */
export const LOCALES = ['zh-CN', 'en'] as const

/**
 * 内置数据来源标识，对应 `gateway_models.source` 字段
 * - manual：用户手动添加
 * - builtin：从 builtin_models 选择
 * - official：从供应商 API 拉取
 */
export const MODEL_SOURCES = ['manual', 'builtin', 'official'] as const

/**
 * CLI 模型映射的输入模式
 * - select：从 Gateway 模型列表选择（路由模式）
 * - manual：手动输入真实模型 ID（直连模式）
 */
export const CLI_MAPPING_INPUT_MODES = ['select', 'manual'] as const

/**
 * 工作区 MCP 传输协议枚举
 */
export const MCP_TRANSPORTS = ['stdio', 'sse', 'http'] as const
