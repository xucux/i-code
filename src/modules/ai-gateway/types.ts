/**
 * AI Gateway 模块类型定义
 *
 * 与 `docs/database.md` §4.3-§4.13 中的表结构对齐。
 * 后端 Rust 通过 ts-rs 自动生成同名类型，前端禁止手写同步覆盖此文件。
 */

import type { SnowflakeId, Timestamp } from '@/core/types'

/**
 * 供应商协议类型
 * 对应 database.md §5.1，与参考项目 ProviderType 对齐。
 * 新增类型必须通过 scripts/sync-builtin-data.ts 脚本同步生成。
 */
export type ProviderType =
  | 'anthropic'
  | 'claude-code'
  | 'google-ai-studio'
  | 'google-vertex-ai'
  | 'google-antigravity'
  | 'google-gemini-cli'
  | 'github-copilot'
  | 'openai-chat-completion'
  | 'openai-codex'
  | 'openai-responses'
  | 'xai-grok-build'
  | 'ollama'
  | 'custom'

/** 传输方式 */
export type TransportType = 'auto' | 'sse' | 'websocket'

/** 模型来源标识
 * - manual：用户手动添加
 * - builtin：从 builtin_models 选择
 * - official：从供应商 API 拉取
 */
export type ModelSource = 'manual' | 'builtin' | 'official'

/** 编辑器工具提示 */
export type ModelEditTool = 'find-replace' | 'multi-find-replace' | 'apply-patch' | 'code-rewrite'

/** 模型能力配置 */
export interface ModelCapabilities {
  /** 是否支持工具/函数调用；布尔或最大工具数 */
  toolCalling?: boolean | number
  /** 是否支持图片/视觉输入 */
  imageInput?: boolean
  /** 编辑器工具提示 */
  editTools?: ModelEditTool
}

/** 模型思考配置 */
export interface ModelThinkingConfig {
  type: 'enabled' | 'disabled' | 'auto'
  effort?: string
  budgetTokens?: number
}

/**
 * AI Gateway 供应商
 * 对应 `providers` 表
 */
export interface Provider {
  id: SnowflakeId
  slug: string
  displayName: string
  providerType: ProviderType
  baseUrl: string
  useRawBaseUrl: boolean
  transport?: TransportType
  serviceTier?: string
  /** 认证配置 JSON（多态联合类型），密钥以 `$SECRET:{snowflake_id}$` 引用
   * 后端序列化为 JSON 字符串，前端使用时需通过 parseAuthConfig() 解析 */
  authJson?: string
  /** 额度监控配置 JSON */
  balanceProviderJson?: string
  timeoutJson?: string
  retryJson?: string
  proxyJson?: string
  /** 供应商扩展模板变量 JSON（ProviderScriptVariables 序列化） */
  scriptVariablesJson?: string
  autoFetchOfficialModels: boolean
  contextCacheJson?: string
  wellKnownTemplateId?: string
  isEnabled: boolean
  sortOrder: number
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 网关暴露的模型
 * 对应 `gateway_models` 表
 * 对外路由 ID：`{provider.slug}/{gateway_model.model_id}`
 */
export interface GatewayModel {
  id: SnowflakeId
  providerId: SnowflakeId
  modelConfigId: SnowflakeId
  /** 真实模型 ID，如 `gpt-4.1` */
  modelId: string
  /** 暴露层展示名，为空时回退 `model_configs.name` */
  displayName?: string
  /** 暴露层模型族，为空时回退 `model_configs.family` */
  family?: string
  source: ModelSource
  isExposed: boolean
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 模型完整配置
 * 对应 `model_configs` 表
 * 标量字段拆为列，复杂嵌套对象（capabilities、thinking 等）以 JSON 列存储
 */
export interface ModelConfig {
  id: SnowflakeId
  name: string
  family?: string
  maxInputTokens?: number
  maxOutputTokens?: number
  tokenizer?: string
  tokenCountMultiplier: number
  /** 每百万 token 单价（元 / 1M tokens） */
  pricePer1mTokens?: number
  stream?: boolean
  temperature?: number
  topK?: number
  topP?: number
  frequencyPenalty?: number
  presencePenalty?: number
  parallelToolCalling?: boolean
  serviceTier?: string
  verbosity?: 'low' | 'medium' | 'high'
  capabilitiesJson?: string
  thinkingJson?: string
  multiAgentJson?: string
  webSearchJson?: string
  memoryTool?: boolean
  presetTemplatesJson?: string
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 供应商附加请求头
 * 对应 `provider_extra_headers` 表
 */
export interface ProviderExtraHeader {
  providerId: SnowflakeId
  key: string
  /** 值支持 `$SECRET:{snowflake_id}$` 引用 */
  value: string
  sortOrder: number
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 供应商附加请求体参数
 * 对应 `provider_extra_body` 表
 */
export interface ProviderExtraBody {
  providerId: SnowflakeId
  key: string
  /** JSON 值文本 */
  valueJson: string
  sortOrder: number
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 模型级附加请求头
 * 对应 `model_config_extra_headers` 表
 * 合并优先级高于供应商级别
 */
export interface ModelConfigExtraHeader {
  modelConfigId: SnowflakeId
  key: string
  value: string
  sortOrder: number
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 模型级附加请求体参数
 * 对应 `model_config_extra_body` 表
 */
export interface ModelConfigExtraBody {
  modelConfigId: SnowflakeId
  key: string
  valueJson: string
  sortOrder: number
  createdAt: Timestamp
  updatedAt: Timestamp
}

/**
 * 官方模型拉取缓存
 * 对应 `official_model_cache` 表
 */
export interface OfficialModelCache {
  providerId: SnowflakeId
  modelsJson: string
  fetchedAt: Timestamp
  expiresAt?: string
  errorMessage?: string
}

// ===== 认证配置（多态联合类型）=====

/**
 * 认证方法枚举
 * 对应 database.md §5.4 中的所有 method 值
 */
export type AuthMethod =
  | 'none'
  | 'api-key'
  | 'oauth2'
  | 'google-vertex-ai-auth'
  | 'antigravity-oauth'
  | 'google-gemini-oauth'
  | 'openai-codex'
  | 'claude-code'
  | 'xai-grok-oauth'
  | 'github-copilot'

/**
 * 认证配置公共字段
 * 所有 method 的认证对象都可包含 label（UI 标签）与 description（UI 描述）。
 */
export interface AuthConfigBase {
  label?: string
  description?: string
}

/** 无认证 */
export interface NoneAuth extends AuthConfigBase {
  method: 'none'
}

/** API Key 认证 */
export interface ApiKeyAuth extends AuthConfigBase {
  method: 'api-key'
  /** API Key 原文或 `$SECRET:{snowflake_id}$` 引用 */
  apiKey?: string
}

/** OAuth 2.0 通用认证 */
export interface OAuth2Auth extends AuthConfigBase {
  method: 'oauth2'
  identityId?: string
  token?: string
  /** OAuth token 过期时间（Unix 秒） */
  expiresAt?: number
  oauth: {
    grantType: 'authorization_code' | 'client_credentials' | 'device_code'
    authorizationUrl?: string
    tokenUrl: string
    deviceAuthorizationUrl?: string
    clientId: string
    clientSecret?: string
    revocationUrl?: string
    scopes?: string[]
    pkce?: boolean
    redirectUri?: string
  }
}

/** Google Vertex AI 认证 */
export interface GoogleVertexAiAuth extends AuthConfigBase {
  method: 'google-vertex-ai-auth'
  subType: 'adc' | 'service-account' | 'api-key'
  projectId?: string
  location?: string
  keyFilePath?: string
  apiKey?: string
}

/** Google Antigravity OAuth */
export interface AntigravityOAuth extends AuthConfigBase {
  method: 'antigravity-oauth'
  identityId?: string
  token?: string
  /** OAuth token 过期时间（Unix 秒） */
  expiresAt?: number
  projectId?: string
  managedProjectId?: string
  tier?: 'free' | 'paid'
  tierId?: string
  email?: string
}

/** Google Gemini CLI OAuth */
export interface GoogleGeminiOAuth extends AuthConfigBase {
  method: 'google-gemini-oauth'
  identityId?: string
  token?: string
  /** OAuth token 过期时间（Unix 秒） */
  expiresAt?: number
  projectId?: string
  oauthType?: 'code_assist' | 'ai_studio' | 'google_one'
  managedProjectId?: string
  tier?: 'free' | 'paid'
  tierId?: string
  email?: string
}

/** OpenAI Codex 认证 */
export interface OpenAiCodexAuth extends AuthConfigBase {
  method: 'openai-codex'
  identityId?: string
  token?: string
  /** OAuth token 过期时间（Unix 秒） */
  expiresAt?: number
  accountId?: string
  email?: string
}

/** Claude Code 认证 */
export interface ClaudeCodeAuth extends AuthConfigBase {
  method: 'claude-code'
  identityId?: string
  token?: string
  /** OAuth token 过期时间（Unix 秒） */
  expiresAt?: number
  email?: string
}

/** xAI Grok OAuth */
export interface XaiGrokOAuth extends AuthConfigBase {
  method: 'xai-grok-oauth'
  identityId?: string
  token?: string
  /** OAuth token 过期时间（Unix 秒） */
  expiresAt?: number
  email?: string
}

/** GitHub Copilot 认证 */
export interface GitHubCopilotAuth extends AuthConfigBase {
  method: 'github-copilot'
  identityId?: string
  token?: string
  /** OAuth token 过期时间（Unix 秒） */
  expiresAt?: number
  enterpriseUrl?: string
  email?: string
}

/** Device Code 授权初始响应 */
export interface DeviceCodeInfo {
  deviceCode: string
  userCode: string
  verificationUri: string
  verificationUriComplete?: string
  expiresIn?: number
  interval: number
}

/** OAuth 浏览器授权启动结果 */
export interface OAuthStartResult {
  /** 浏览器授权 URL */
  authorizationUrl: string
  /** PKCE code_verifier，用于后续手动换 token */
  codeVerifier: string
  /** OAuth state，用于验证回调合法性 */
  state: string
  /** 回调服务器实际监听的 redirect_uri */
  redirectUri: string
}

/** OAuth 回调事件 payload */
export interface OAuthCallbackEvent {
  /** 供应商 ID */
  providerId: string
  /** 授权码 */
  code?: string
  /** OAuth state */
  state?: string
  /** 错误码（授权失败时） */
  error?: string
  /** 错误描述 */
  errorDescription?: string
}

/** Device Code 轮询状态 */
export type DeviceCodePollStatus = 'pending' | 'success'

/** Device Code 轮询结果 */
export interface DeviceCodePollResult {
  status: DeviceCodePollStatus
  provider?: Provider
}

/** 认证配置联合类型 */
export type AuthConfig =
  | NoneAuth
  | ApiKeyAuth
  | OAuth2Auth
  | GoogleVertexAiAuth
  | AntigravityOAuth
  | GoogleGeminiOAuth
  | OpenAiCodexAuth
  | ClaudeCodeAuth
  | XaiGrokOAuth
  | GitHubCopilotAuth

// ===== 配置类型（JSON 字段）=====

/**
 * 供应商级代理配置
 * 对应 database.md §5.3
 *
 * 与全局代理 `GlobalProxyConfig` 区分：供应商代理支持「使用全局代理」「直连」
 * 「SOCKS 代理」「HTTP 代理」四种策略。
 */
export interface ProxyConfig {
  type: 'global' | 'direct' | 'socks' | 'http'
  /** 代理 URL（仅 `socks` / `http` 类型生效） */
  url?: string
}

/**
 * 超时配置
 * 对应 database.md §5.7
 */
export interface TimeoutConfig {
  /** TCP 连接超时（毫秒） */
  connection: number
  /** SSE 流响应超时（毫秒），每次收到数据块后重置 */
  response: number
}

/**
 * 重试配置
 * 对应 database.md §5.8
 */
export interface RetryConfig {
  maxRetries: number
  initialDelayMs: number
  maxDelayMs: number
  backoffMultiplier: number
  jitterFactor: number
  statusCodes: number[]
}

/**
 * 上下文缓存配置
 * 对应 database.md §5.9
 */
export interface ContextCacheConfig {
  type: 'only-free' | 'allow-paid'
  ttl: number
}

/**
 * 模型能力
 * 对应 database.md §5.6
 */
export interface ModelCapabilities {
  toolCalling?: boolean | number
  imageInput?: boolean
  editTools?: 'find-replace' | 'multi-find-replace' | 'apply-patch' | 'code-rewrite'
}

/**
 * 思考/推理配置
 * 对应 database.md §5.11
 */
export interface ThinkingConfig {
  type?: 'enabled' | 'disabled' | 'auto'
  budgetTokens?: number
  effort?: 'max' | 'xhigh' | 'high' | 'medium' | 'low' | 'minimal' | 'none'
  summary?: 'none' | 'auto' | 'concise' | 'detailed'
  mode?: 'standard' | 'pro'
  context?: 'auto' | 'current_turn' | 'all_turns'
}

// ===== 内置数据 =====

/**
 * 内置供应商预设
 * 对应 `builtin_providers` 表
 */
export interface BuiltinProvider {
  id: string
  displayName: string
  /** 中文展示名称，无中文名时为空串 */
  displayCnName: string
  category: string
  providerType: ProviderType
  baseUrl: string
  useRawBaseUrl: boolean
  defaultAuthJson?: string
  defaultBalanceProviderJson?: string
  extraHeadersJson?: string
  extraBodyJson?: string
  autoFetchOfficialModels: boolean
  sortOrder: number
  createdAt: Timestamp
}

/**
 * 内置模型
 * 对应 `builtin_models` 表
 */
export interface BuiltinModel {
  id: string
  displayName: string
  family?: string
  /** 适配的供应商类型列表（来自 builtin-models.json `providerTypes`） */
  providerTypes?: ProviderType[]
  maxInputTokens?: number
  maxOutputTokens?: number
  tokenizer?: string
  tokenCountMultiplier: number
  stream?: boolean
  temperature?: number
  topK?: number
  topP?: number
  frequencyPenalty?: number
  presencePenalty?: number
  parallelToolCalling?: boolean
  serviceTier?: string
  verbosity?: 'low' | 'medium' | 'high'
  capabilities?: ModelCapabilities
  thinking?: ModelThinkingConfig
  capabilitiesJson?: string
  thinkingJson?: string
  multiAgentJson?: string
  webSearchJson?: string
  memoryTool?: boolean
  presetTemplatesJson?: string
  sortOrder: number
  createdAt: Timestamp
}

/**
 * 内置供应商 × 内置模型关联
 * 对应 `builtin_provider_models` 表
 */
export interface BuiltinProviderModel {
  builtinProviderId: string
  builtinModelId: string
  declaredModelId?: string
  sortOrder: number
  createdAt: Timestamp
}

/**
 * 内置模型别名
 * 对应 `builtin_model_aliases` 表
 */
export interface BuiltinModelAlias {
  builtinModelId: string
  alias: string
  createdAt: Timestamp
}

/**
 * 内置模型覆盖配置
 * 对应 `builtin_model_overrides` 表
 */
export interface BuiltinModelOverride {
  id: SnowflakeId
  builtinModelId: string
  matcherType: 'provider_type' | 'name' | 'pattern'
  matcherValue: string
  overrideConfigJson: string
  sortOrder: number
  createdAt: Timestamp
}

/**
 * 内置供应商支持的认证方式
 * 对应 `builtin_provider_auth_types` 表
 */
export interface BuiltinProviderAuthType {
  builtinProviderId: string
  authMethod: AuthMethod
  isDefault: boolean
  sortOrder: number
  createdAt: Timestamp
}

/**
 * 解析供应商的 authJson 字符串为 AuthConfig 对象
 *
 * 后端将 AuthConfig 序列化为 JSON 字符串存储在 auth_json 列中。
 * 前端读取 Provider 时 authJson 为原始 JSON 字符串，
 * 需通过此函数解析后方可访问 method、apiKey 等字段。
 */
export function parseAuthConfig(provider: Provider): AuthConfig | undefined {
  if (!provider.authJson) return undefined
  try {
    return JSON.parse(provider.authJson) as AuthConfig
  } catch {
    return undefined
  }
}

/**
 * 获取供应商的认证方法
 */
export function getAuthMethod(provider: Provider): AuthMethod {
  const config = parseAuthConfig(provider)
  return config?.method ?? 'none'
}

// ===== 暴露模型 DTO =====

/**
 * 对外暴露的网关模型列表项
 * 对应后端 `ExposedModel`，由 `gateway_exposed_models` 命令返回。
 * 对外路由 ID：`{provider_slug}/{model_id}`。
 */
export interface ExposedModel {
  id: string
  providerSlug: string
  modelId: string
  displayName: string
  family?: string
}

// ===== 分享配置 DTO =====

// ===== 导出/导入输入类型 =====

/**
 * 导出供应商的输入参数
 * 对应后端 `ExportProviderInput`
 */
export interface ExportProviderInput {
  providerId: string
  includeSecrets: boolean
}

/**
 * 导入供应商的输入参数
 * 对应后端 `ImportProviderInput`
 */
export interface ImportProviderInput {
  data: string
  conflictStrategy?: 'auto_rename' | 'fail'
}

// ===== 写入输入类型 =====

/**
 * 创建供应商的输入参数
 * 对应后端 `CreateProviderInput`
 */
export interface CreateProviderInput {
  slug: string
  displayName: string
  providerType: string
  baseUrl: string
  useRawBaseUrl?: boolean
  auth?: AuthConfig
  autoFetchOfficialModels?: boolean
  isEnabled?: boolean
  sortOrder?: number
  /** 额度监控配置 JSON */
  balanceProviderJson?: string
  /** 供应商级超时配置 JSON */
  timeoutJson?: string
  /** 供应商级重试配置 JSON */
  retryJson?: string
  /** 供应商级代理配置 JSON */
  proxyJson?: string
  /** 供应商扩展模板变量 JSON */
  scriptVariablesJson?: string
}

export interface UpdateProviderInput {
  slug?: string
  displayName?: string
  baseUrl?: string
  useRawBaseUrl?: boolean
  auth?: AuthConfig
  autoFetchOfficialModels?: boolean
  isEnabled?: boolean
  sortOrder?: number
  /** 额度监控配置 JSON */
  balanceProviderJson?: string
  /** 供应商级超时配置 JSON */
  timeoutJson?: string
  /** 供应商级重试配置 JSON */
  retryJson?: string
  /** 供应商级代理配置 JSON */
  proxyJson?: string
  /** 供应商扩展模板变量 JSON */
  scriptVariablesJson?: string | null
}

/**
 * 创建模型配置的输入参数
 * 对应后端 `CreateModelConfigInput`
 */
export interface CreateModelConfigInput {
  name: string
  family?: string
  maxInputTokens?: number
  maxOutputTokens?: number
  tokenizer?: string
  tokenCountMultiplier?: number
  /** 每百万 token 单价（元 / 1M tokens） */
  pricePer1mTokens?: number
  stream?: boolean
  temperature?: number
  topP?: number
  parallelToolCalling?: boolean
  capabilitiesJson?: string
  thinkingJson?: string
  balanceProviderJson?: string
  timeoutJson?: string
  retryJson?: string
  proxyJson?: string
}

/**
 * 更新模型配置的输入参数
 * 对应后端 `UpdateModelConfigInput`
 */
export interface UpdateModelConfigInput {
  name?: string
  family?: string
  maxInputTokens?: number
  maxOutputTokens?: number
  tokenizer?: string
  tokenCountMultiplier?: number
  /** 每百万 token 单价（元 / 1M tokens） */
  pricePer1mTokens?: number
  stream?: boolean
  temperature?: number
  topK?: number
  topP?: number
  frequencyPenalty?: number
  presencePenalty?: number
  parallelToolCalling?: boolean
  serviceTier?: string
  verbosity?: string
  capabilitiesJson?: string
  thinkingJson?: string
  multiAgentJson?: string
  webSearchJson?: string
  memoryTool?: boolean
  presetTemplatesJson?: string
}

/**
 * 创建网关模型的输入参数
 * 对应后端 `CreateGatewayModelInput`
 */
export interface CreateGatewayModelInput {
  providerId: SnowflakeId
  modelConfigId: SnowflakeId
  modelId: string
  displayName?: string
  family?: string
  isExposed?: boolean
  /** 模型来源：manual | builtin | official */
  source?: string
}

/**
 * 更新网关模型的输入参数
 * 对应后端 `UpdateGatewayModelInput`
 */
export interface UpdateGatewayModelInput {
  /** 真实模型 ID，如 `gpt-4.1` */
  modelId?: string
  displayName?: string
  family?: string
  isExposed?: boolean
}

// ===== 扩展模板变量类型 =====

/**
 * 保留名列表（禁止作为模板变量 key，避免与系统注入常量冲突）
 */
export const SCRIPT_VARIABLE_RESERVED_NAMES = [
  'api_key', 'now_ms', 'provider', 'auth', 'template', 'variables', 'pi', 'e',
]

/**
 * 供应商扩展模板变量容器
 * 对应 `providers.script_variables_json` 列的 JSON 结构
 */
export interface ProviderScriptVariables {
  version: number
  items: ProviderScriptVariable[]
}

/**
 * 单个供应商扩展模板变量
 * key 为脚本中取用键（`variables["cookie"]` 或顶层别名 `cookie`）
 */
export interface ProviderScriptVariable {
  key: string
  value: string
  isSecret?: boolean
  label?: string
  allowedHosts?: string[]
}

/**
 * 导出的供应商信息（去除本地运行时标识）
 * 对应后端 `ExportedProvider`
 */
export interface ExportedProvider extends Omit<Provider, 'id' | 'createdAt' | 'updatedAt'> {
  extraHeaders?: Record<string, string>
  extraBody?: Record<string, unknown>
}

/**
 * 导出的网关模型（去除本地运行时标识）
 * 对应后端 `ExportedGatewayModel`
 */
export interface ExportedGatewayModel extends Omit<GatewayModel, 'id' | 'providerId' | 'modelConfigId' | 'createdAt' | 'updatedAt'> {}

/**
 * 导出的模型配置（去除本地运行时标识）
 * 对应后端 `ExportedModelConfig`
 */
export interface ExportedModelConfig extends Omit<ModelConfig, 'id' | 'createdAt' | 'updatedAt'> {}

/**
 * 导出的模型项
 * 对应后端 `ExportedModel`
 */
export interface ExportedModelItem {
  gatewayModel: ExportedGatewayModel
  modelConfig: ExportedModelConfig
  extraHeaders?: Record<string, string>
  extraBody?: Record<string, unknown>
}

/**
 * 供应商分享配置 DTO
 * 用于导出/导入供应商配置（base64 JSON 编码）
 */
export interface ProviderShareConfig {
  version: '1.0'
  exportedAt: string
  provider: ExportedProvider
  models: ExportedModelItem[]
}

// ===== 网关设置 =====

/**
 * 网关设置（单例行）
 * 对应 `gateway_settings` 表
 */
export interface GatewaySettings {
  id: string
  gatewayHost: string
  gatewayPort: number
  /**
   * 默认 Gateway API Key
   * 兼容 Secret 引用（裸雪花 ID / `$SECRET:{snowflake_id}$`）与明文 key
   */
  defaultApiKeySecretId?: string | null
  isEnabled: boolean
  createdAt: string
  updatedAt: string
}

/**
 * 更新网关设置的输入参数
 */
export interface UpdateGatewaySettingsInput {
  gatewayHost?: string
  gatewayPort?: number
  defaultApiKeySecretId?: string | null
  isEnabled?: boolean
}

// ===== 网关认证 API Key =====

/**
 * 网关认证 API Key
 * 对应 `gateway_auth_keys` 表
 * `apiKeySecretId` 当前按业务约定保存明文 key，便于网关反查
 */
export interface GatewayAuthKey {
  id: string
  name: string
  description?: string
  /** API Key 明文值 */
  apiKeySecretId?: string
  isEnabled: boolean
  expiresAt?: string
  sortOrder: number
  lastUsedAt?: string
  createdAt: string
  updatedAt: string
}

/**
 * 创建网关认证 API Key 的输入参数
 */
export interface CreateGatewayAuthKeyInput {
  name: string
  description?: string
  /** API Key 明文值 */
  apiKeySecretId?: string
  isEnabled?: boolean
  expiresAt?: string
  sortOrder?: number
}

/**
 * 更新网关认证 API Key 的输入参数
 */
export interface UpdateGatewayAuthKeyInput {
  name?: string
  description?: string | null
  /** API Key 明文值；传 `null` 表示清空 */
  apiKeySecretId?: string | null
  isEnabled?: boolean
  expiresAt?: string | null
  sortOrder?: number
}

// ===== 供应商网络检测 =====

/** 网络检测模式 */
export type PingMode = 'direct' | 'proxy'

/** 单个供应商网络检测结果 */
export interface PingProviderResult {
  providerId: string
  displayName: string
  slug: string
  baseUrl: string
  success: boolean
  statusCode?: number
  latencyMs?: number
  error?: string
}

/** 网络检测汇总结果（由 provider:ping-done 事件推送） */
export interface PingDonePayload {
  mode: string
  total: number
  success: number
  failed: number
}
