/**
 * 日志控制台模块类型定义
 *
 * 对应 `docs/development.md` §5.11。logger 模块聚焦于**运行时诊断**，
 * 记录请求的 URL、状态码、耗时、Token 用量、错误信息等。
 *
 * 数据存储在后端内存环形缓冲区（Ring Buffer），可按需持久化到文件。
 * 与审计日志不同，不记录用户身份与敏感请求体内容。
 */

/**
 * 日志级别
 */
export type LogLevel = 'DEBUG' | 'INFO' | 'WARN' | 'ERROR'

/**
 * 日志来源
 * - `gateway`：本地网关请求转发
 * - `provider-api`：上游供应商 API 调用
 * - `system`：系统级日志（如启动、停止、错误）
 */
export type LogSource = 'gateway' | 'provider-api' | 'system'

/**
 * 单条日志记录
 *
 * 由 `gateway-runtime` 的响应拦截器在请求完成后异步写入。
 * Token 用量从响应头或响应体中提取（见 development.md §5.13）。
 */
export interface LogEntry {
  /** 日志记录唯一标识 */
  id: string
  /** 日志时间戳（格式：yyyy-MM-dd HH:mm:ss.SSS） */
  timestamp: string
  level: LogLevel
  source: LogSource
  /** HTTP 方法，如 `POST` */
  method?: string
  /** 请求 URL（已脱敏，去除 query 中的敏感参数） */
  url?: string
  /** HTTP 状态码 */
  statusCode?: number
  /** 请求耗时（毫秒） */
  durationMs?: number
  /** 提示 Token 数 */
  promptTokens?: number
  /** 补全 Token 数 */
  completionTokens?: number
  /** 总 Token 数 */
  totalTokens?: number
  /** 缓存命中 Token 数 */
  cachedTokens?: number
  /** 错误信息（如有） */
  errorMessage?: string
  /** 网关生成的唯一请求追踪 ID */
  requestId?: string
  /** 请求使用的模型 ID（网关暴露的 model 字段，含 provider_slug 前缀） */
  modelId?: string
  /** 请求头 JSON（已由后端去敏，敏感值以 *** 代替） */
  requestHeaders?: string
  /** 请求体内容（转发详细日志开启时记录） */
  requestBody?: string
  /** 响应体内容（转发详细日志开启时记录） */
  responseBody?: string
  /** 协议/场景标签，如 `sse`、`websocket` */
  tags?: string[]
  /** 源文件名（如 upstream.rs） */
  fileName?: string
  /** 源文件行号 */
  lineNumber?: number
}

/**
 * 日志过滤参数
 * 用于 `log_list` 命令查询与前端过滤面板
 */
export interface LogFilter {
  /** 按级别过滤（OR 关系） */
  levels?: LogLevel[]
  /** 按来源过滤（OR 关系） */
  sources?: LogSource[]
  /** 按状态码过滤（OR 关系） */
  statusCodes?: number[]
  /** 关键词模糊匹配（URL、errorMessage） */
  keyword?: string
  /** 时间范围 */
  timeRange?: { from?: string; to?: string }
  /** 请求 ID 精确匹配 */
  requestId?: string
}

/**
 * 日志导出格式
 */
export type LogExportFormat = 'json' | 'csv'

/**
 * 日志导出结果
 */
export interface LogExportResult {
  /** 导出文件路径（保存到应用临时目录） */
  filePath: string
  /** 导出的日志条数 */
  count: number
  /** 导出格式 */
  format: LogExportFormat
}

/**
 * 日志滚动记录配置
 * 对应 `LogRollingConfig` 组件
 */
export interface LogRollingConfig {
  /** 内存缓冲队列大小（条数），默认 5000 */
  bufferSize: number
  /** 是否启用本地日志文件持久化 */
  enableFilePersistence: boolean
  /** 单个日志文件大小上限（MB），默认 10 */
  maxFileSizeMb: number
  /** 保留的日志文件数量，默认 7 */
  maxFileCount: number
  /** 日志文件保留天数，默认 30 */
  maxRetentionDays: number
  /** 日志级别阈值（低于此级别不写入文件） */
  fileLogLevel?: LogLevel
}

/**
 * 转发详细日志配置
 *
 * 控制网关转发时是否记录请求/响应体到日志缓冲区。
 */
export interface ForwardLogConfig {
  /** 是否记录转发请求体（包含 model、messages 等） */
  enableRequestLog: boolean
  /** 是否记录转发响应体（包含 choices、usage 等） */
  enableResponseLog: boolean
  /** 单条请求/响应体最大记录长度（字符数），超出截断 */
  maxBodyLength: number
}

/**
 * 前后端 Command 交互日志配置
 *
 * 控制 Tauri Command 调用时是否记录请求/响应到系统日志。
 */
export interface CommandLogConfig {
  /** 是否记录 Command 调用到系统日志 */
  enableCommandLog: boolean
  /** 是否记录 Command 请求参数 */
  enableCommandRequestLog: boolean
  /** 是否记录 Command 响应数据 */
  enableCommandResponseLog: boolean
  /** 单条请求/响应最大记录长度（字符数），超出截断 */
  maxBodyLength: number
}

/**
 * 统一日志配置
 *
 * 合并 ForwardLogConfig / CommandLogConfig / LogRollingConfig 为单一配置对象。
 * 持久化到 log_settings 数据库表。
 */
export interface LogSettings {
  // === 基础设置 ===
  /** 内存缓冲队列大小（条数），默认 5000 */
  bufferSize: number
  /** 日志文件目录（空字符串使用默认目录） */
  logDir: string
  /** 日志文件保留天数，默认 30 */
  maxRetentionDays: number
  /** 是否启用文件持久化 */
  enableFilePersistence: boolean
  /** 单个日志文件大小上限（MB），默认 10 */
  maxFileSizeMb: number
  /** 保留的日志文件数量，默认 7 */
  maxFileCount: number
  /** 文件写入级别阈值 */
  fileLogLevel?: LogLevel

  // === 转发详细日志 ===
  /** 是否记录转发请求体 */
  enableRequestLog: boolean
  /** 是否记录转发响应体 */
  enableResponseLog: boolean
  /** 转发日志最大记录长度 */
  forwardMaxBodyLength: number

  // === 直连网关请求日志 ===
  /** 是否记录直连网关请求体 */
  enableGatewayRequestLog: boolean
  /** 是否记录直连网关响应体 */
  enableGatewayResponseLog: boolean
  /** 直连网关日志最大记录长度 */
  gatewayMaxBodyLength: number

  // === Command 交互日志 ===
  /** 是否记录 Command 调用 */
  enableCommandLog: boolean
  /** 是否记录 Command 请求参数 */
  enableCommandRequestLog: boolean
  /** 是否记录 Command 响应数据 */
  enableCommandResponseLog: boolean
  /** Command 日志最大记录长度 */
  commandMaxBodyLength: number
}
