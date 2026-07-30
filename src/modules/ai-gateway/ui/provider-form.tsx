import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Separator } from '@/components/ui/separator'
import { Badge } from '@/components/ui/badge'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { HelpIcon } from '@/components/ui/help-icon'
import { Checkbox } from '@/components/ui/checkbox'
import { invokeCommand } from '@/hooks/use-command'
import { clearOauthToken } from '@/hooks/use-ai-gateway-mutation'
import { open } from '@tauri-apps/plugin-shell'
import { listen } from '@tauri-apps/api/event'
import { BalanceConfigForm } from '@/modules/balance/ui/balance-config-form'
import { ScriptVariablesEditor } from '@/modules/ai-gateway/ui/script-variables-editor'
import { PortInUseDialog } from '@/modules/ai-gateway/ui/port-in-use-dialog'
import type { Provider, ProviderType, AuthConfig, AuthMethod, BuiltinModel, GatewayModel, ModelConfig, ModelCapabilities, ModelThinkingConfig, ModelEditTool, DeviceCodeInfo, DeviceCodePollResult, OAuthStartResult, OAuthCallbackEvent, ProviderScriptVariable } from '@/modules/ai-gateway/types'
import { parseAuthConfig } from '@/modules/ai-gateway/types'
import { toast } from 'sonner'
import { toIcodeError } from '@/core/errors'
import { formatDateTime } from '@/core/utils'

const providerTypes: ProviderType[] = [
  'anthropic',
  'openai-chat-completion',
  'openai-codex',
  'openai-responses',
  'google-ai-studio',
  'google-vertex-ai',
  'google-gemini-cli',
  'github-copilot',
  'xai-grok-build',
  'ollama',
  'custom',
]

/** 分词器可选项，与参考项目 TokenizerId 对齐（使用 tokenizer 命名空间） */
function getTokenizerOptions(t: (key: string, options?: Record<string, unknown> | string) => string) {
  return [
    { value: 'default', label: t('default', { ns: 'tokenizer' }) },
    { value: 'conservative', label: t('conservative', { ns: 'tokenizer' }) },
    { value: 'char4', label: t('char4', { ns: 'tokenizer' }) },
    { value: 'openai', label: t('openai', { ns: 'tokenizer' }) },
    { value: 'deepseek', label: t('deepseek', { ns: 'tokenizer' }) },
  ]
}

/** 编辑器工具提示可选项（需传入 t 以支持 i18n） */
function getEditToolOptions(t: (key: string) => string): Array<{ value: '' | ModelEditTool; label: string }> {
  return [
    { value: '', label: t('aiGateway.providerForm.modelEdit.editToolsNone') },
    { value: 'find-replace', label: 'find-replace' },
    { value: 'multi-find-replace', label: 'multi-find-replace' },
    { value: 'apply-patch', label: 'apply-patch' },
    { value: 'code-rewrite', label: 'code-rewrite' },
  ]
}

/** 思考模式类型可选项（需传入 t 以支持 i18n） */
function getThinkingTypeOptions(t: (key: string) => string) {
  return [
    { value: 'enabled', label: t('aiGateway.providerForm.modelEdit.thinkingEnabled') },
    { value: 'disabled', label: t('aiGateway.providerForm.modelEdit.thinkingDisabled') },
    { value: 'auto', label: t('aiGateway.providerForm.modelEdit.thinkingAuto') },
  ]
}


/**
 * 获取当前协议类型支持的认证方式列表
 *
 * 目前展示全部认证方式，由用户根据实际供应商选择；
 * 未来可结合 builtin_provider_auth_types 做按类型的动态过滤。
 */
function getAuthMethodOptions(t: (key: string) => string, _providerType: string) {
  return [
    { value: 'none', label: t('aiGateway.providerForm.authMethodNone') },
    { value: 'api-key', label: 'API Key' },
    { value: 'oauth2', label: t('aiGateway.providerForm.authMethodOAuth2') },
    { value: 'google-vertex-ai-auth', label: t('aiGateway.providerForm.authMethodGoogleVertexAi') },
    { value: 'antigravity-oauth', label: t('aiGateway.providerForm.authMethodAntigravity') },
    { value: 'google-gemini-oauth', label: t('aiGateway.providerForm.authMethodGoogleGemini') },
    { value: 'openai-codex', label: t('aiGateway.providerForm.authMethodOpenaiCodex') },
    { value: 'claude-code', label: t('aiGateway.providerForm.authMethodClaudeCode') },
    { value: 'xai-grok-oauth', label: t('aiGateway.providerForm.authMethodXaiGrok') },
    { value: 'github-copilot', label: t('aiGateway.providerForm.authMethodGithubCopilot') },
  ]
}

/**
 * 判断认证方式是否需要 OAuth 浏览器/Device Code 授权流程
 *
 * 与后端 `auth::providers::is_oauth_method` 保持一致。
 * Google Vertex AI 使用 ADC/服务账号/API Key，不走 OAuth 流程。
 */
/** OAuth 浏览器授权流程超时时间（秒） */
const OAUTH_TIMEOUT_SECONDS = 120

function isOAuthMethod(method: AuthMethod): boolean {
  return [
    'oauth2',
    'antigravity-oauth',
    'google-gemini-oauth',
    'openai-codex',
    'claude-code',
    'xai-grok-oauth',
    'github-copilot',
  ].includes(method)
}

const schema = z.object({
  slug: z.string().min(1, 'aiGateway.providerForm.validation.slugRequired').regex(/^[a-z0-9-]+$/, 'aiGateway.providerForm.validation.slugInvalid'),
  displayName: z.string().min(1, 'aiGateway.providerForm.validation.displayNameRequired'),
  providerType: z.string().min(1, 'aiGateway.providerForm.validation.providerTypeRequired'),
  baseUrl: z.string().min(1, 'aiGateway.providerForm.validation.baseUrlRequired'),
  useRawBaseUrl: z.boolean(),
  authMethod: z.enum([
    'none',
    'api-key',
    'oauth2',
    'google-vertex-ai-auth',
    'antigravity-oauth',
    'google-gemini-oauth',
    'openai-codex',
    'claude-code',
    'xai-grok-oauth',
    'github-copilot',
  ]),
  apiKey: z.string().optional(),
  // Google Vertex AI 配置
  googleVertexSubType: z.enum(['adc', 'service-account', 'api-key']).optional(),
  googleVertexProjectId: z.string().optional(),
  googleVertexLocation: z.string().optional(),
  googleVertexKeyFilePath: z.string().optional(),
  // 通用 OAuth 2.0 端点配置
  oauthAuthorizationUrl: z.string().optional(),
  oauthTokenUrl: z.string().optional(),
  oauthClientId: z.string().optional(),
  oauthClientSecret: z.string().optional(),
  oauthScopes: z.string().optional(),
  oauthPkce: z.boolean().optional(),
  isEnabled: z.boolean(),
  sortOrder: z.coerce.number().int(),
})

type FormValues = z.infer<typeof schema>

export interface ProviderFormInitialValues extends Partial<{
  slug: string
  displayName: string
  providerType: string
  baseUrl: string
  useRawBaseUrl: boolean
  authMethod: AuthMethod
  isEnabled: boolean
  sortOrder: number
}> {}

interface ProviderFormProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  provider?: Provider | null
  /** 新增时用于预填充内置预设字段 */
  initialValues?: ProviderFormInitialValues
  onSubmit: (values: {
    slug: string
    displayName: string
    providerType: string
    baseUrl: string
    useRawBaseUrl: boolean
    auth?: AuthConfig
    isEnabled: boolean
    sortOrder?: number
    balanceProviderJson?: string
    proxyJson?: string
    timeoutJson?: string
    retryJson?: string
    scriptVariablesJson?: string
  }) => void
  /** OAuth 授权成功后回传更新后的供应商对象 */
  onProviderUpdated?: (provider: Provider) => void
}

/**
 * 根据表单值构建 AuthConfig
 *
 * - API Key / Vertex AI：使用表单输入值
 * - 通用 OAuth 2.0：使用表单填写的端点配置；编辑时保留已有 token
 * - 具体 OAuth 供应商（Antigravity/Gemini/Codex/Claude/xAI/Copilot）：
 *   编辑时若已有同 method 配置，保留完整对象（端点由后端 preset 填充，token 已存在），
 *   否则仅保存 method，授权时由后端应用预设端点。
 */
function buildAuthConfig(
  values: FormValues,
  provider: Provider | null | undefined,
  existingApiKeyRef?: string,
  existingVertexApiKeyRef?: string,
): AuthConfig | undefined {
  const { authMethod, apiKey } = values
  const isEdit = Boolean(provider)

  switch (authMethod) {
    case 'api-key':
      return {
        method: 'api-key',
        // 编辑时：用户未输入新值则保留原始引用，输入新值则提交明文由后端加密
        apiKey: isEdit && !apiKey ? (existingApiKeyRef || undefined) : apiKey || undefined,
      }

    case 'none':
      return { method: 'none' }

    case 'google-vertex-ai-auth': {
      const existing = preserveExistingAuth('google-vertex-ai-auth', provider) as
        | Extract<AuthConfig, { method: 'google-vertex-ai-auth' }>
        | undefined
      return {
        method: 'google-vertex-ai-auth',
        subType: values.googleVertexSubType ?? existing?.subType ?? 'adc',
        projectId: values.googleVertexProjectId || existing?.projectId,
        location: values.googleVertexLocation || existing?.location,
        keyFilePath: values.googleVertexKeyFilePath || existing?.keyFilePath,
        // 编辑时：用户未输入新值则保留原始引用，输入新值则提交明文由后端加密
        apiKey: isEdit && !values.apiKey
          ? (existingVertexApiKeyRef || existing?.apiKey)
          : (values.apiKey || existing?.apiKey),
      }
    }

    case 'oauth2': {
      const existing = preserveExistingAuth('oauth2', provider) as
        | Extract<AuthConfig, { method: 'oauth2' }>
        | undefined
      const scopes = values.oauthScopes
        ? values.oauthScopes.split(/\s+/).filter(Boolean)
        : existing?.oauth?.scopes
      return {
        method: 'oauth2',
        oauth: {
          grantType: 'authorization_code',
          authorizationUrl: values.oauthAuthorizationUrl || existing?.oauth?.authorizationUrl || '',
          tokenUrl: values.oauthTokenUrl || existing?.oauth?.tokenUrl || '',
          clientId: values.oauthClientId || existing?.oauth?.clientId || '',
          clientSecret: values.oauthClientSecret || existing?.oauth?.clientSecret,
          scopes,
          pkce: values.oauthPkce ?? existing?.oauth?.pkce ?? true,
        },
        token: existing?.token,
      }
    }

    case 'antigravity-oauth':
      return preserveExistingAuth('antigravity-oauth', provider) ?? { method: 'antigravity-oauth' }
    case 'google-gemini-oauth':
      return preserveExistingAuth('google-gemini-oauth', provider) ?? { method: 'google-gemini-oauth' }
    case 'openai-codex':
      return preserveExistingAuth('openai-codex', provider) ?? { method: 'openai-codex' }
    case 'claude-code':
      return preserveExistingAuth('claude-code', provider) ?? { method: 'claude-code' }
    case 'xai-grok-oauth':
      return preserveExistingAuth('xai-grok-oauth', provider) ?? { method: 'xai-grok-oauth' }
    case 'github-copilot':
      return preserveExistingAuth('github-copilot', provider) ?? { method: 'github-copilot' }

    default:
      return { method: 'none' }
  }
}

/**
 * 若 provider 已有同 method 的认证配置，则返回该配置对象
 *
 * 用于 OAuth 等敏感场景：保存时避免覆盖已获取的 token。
 */
function preserveExistingAuth(method: AuthMethod, provider: Provider | null | undefined): AuthConfig | undefined {
  if (!provider?.authJson) return undefined
  try {
    const existing = JSON.parse(provider.authJson) as AuthConfig
    if (existing.method === method) return existing
  } catch {
    // ignore
  }
  return undefined
}

/** 模型编辑弹窗表单校验模式 */
const modelEditSchema = z.object({
  modelId: z.string().min(1, 'aiGateway.providerForm.modelEdit.validation.modelIdRequired'),
  displayName: z.string(),
  family: z.string(),
  maxInputTokens: z.string(),
  maxOutputTokens: z.string(),
  tokenizer: z.string(),
  tokenCountMultiplier: z.coerce.number().min(0).default(1),
  pricePer1mTokens: z.string(),
  stream: z.boolean().default(true),
  temperature: z.string(),
  topP: z.string(),
  toolCalling: z.boolean().default(false),
  imageInput: z.boolean().default(false),
  editTools: z.enum(['', 'find-replace', 'multi-find-replace', 'apply-patch', 'code-rewrite']),
  thinkingType: z.enum(['', 'enabled', 'disabled', 'auto']),
  thinkingEffort: z.string(),
  thinkingBudgetTokens: z.string(),
})

/** 模型编辑弹窗表单值 */
type ModelEditFormValues = z.infer<typeof modelEditSchema>

/** 从 capabilities 对象序列化为 JSON 字符串；空对象返回 undefined */
function buildCapabilitiesJson(values: Pick<ModelEditFormValues, 'toolCalling' | 'imageInput' | 'editTools'>): string | undefined {
  const caps: ModelCapabilities = {}
  if (values.toolCalling) caps.toolCalling = true
  if (values.imageInput) caps.imageInput = true
  if (values.editTools) caps.editTools = values.editTools
  return Object.keys(caps).length > 0 ? JSON.stringify(caps) : undefined
}

/** 从 thinking 表单值序列化为 JSON 字符串；未选择类型时返回 undefined */
function buildThinkingJson(values: Pick<ModelEditFormValues, 'thinkingType' | 'thinkingEffort' | 'thinkingBudgetTokens'>): string | undefined {
  if (!values.thinkingType) return undefined
  const thinking: ModelThinkingConfig = {
    type: values.thinkingType as 'enabled' | 'disabled' | 'auto',
  }
  if (values.thinkingEffort.trim()) thinking.effort = values.thinkingEffort.trim()
  if (values.thinkingBudgetTokens.trim()) thinking.budgetTokens = Number(values.thinkingBudgetTokens.trim())
  return JSON.stringify(thinking)
}

/**
 * 解析 capabilities JSON 字符串为表单值
 */
function parseCapabilitiesForm(capabilitiesJson?: string): Pick<ModelEditFormValues, 'toolCalling' | 'imageInput' | 'editTools'> {
  const defaultValues = { toolCalling: false, imageInput: false, editTools: '' as const }
  if (!capabilitiesJson) return defaultValues
  try {
    const parsed = JSON.parse(capabilitiesJson) as ModelCapabilities
    return {
      toolCalling: parsed.toolCalling === true || (typeof parsed.toolCalling === 'number' && parsed.toolCalling > 0),
      imageInput: parsed.imageInput === true,
      editTools: parsed.editTools || '',
    }
  } catch {
    return defaultValues
  }
}

/**
 * 解析 thinking JSON 字符串为表单值
 */
function parseThinkingForm(thinkingJson?: string): Pick<ModelEditFormValues, 'thinkingType' | 'thinkingEffort' | 'thinkingBudgetTokens'> {
  const defaultValues = { thinkingType: '' as const, thinkingEffort: '', thinkingBudgetTokens: '' }
  if (!thinkingJson) return defaultValues
  try {
    const parsed = JSON.parse(thinkingJson) as ModelThinkingConfig
    return {
      thinkingType: parsed.type || '',
      thinkingEffort: parsed.effort || '',
      thinkingBudgetTokens: parsed.budgetTokens?.toString() || '',
    }
  } catch {
    return defaultValues
  }
}

/**
 * 构建 script_variables_json 提交数据
 *
 * 编辑时：isSecret=true 且 value 为空或仍为 $SECRET:...$ 占位 → 保留原引用（未改）
 * isSecret=true 且 value 为新明文 → 原样发给后端加密
 * isSecret=false → 明文原样
 * 过滤掉 key 为空或重复的项；items 为空 → 返回 undefined（不传该字段）
 */
function buildScriptVariablesJson(
  variables: ProviderScriptVariable[],
  isEdit: boolean,
): string | undefined {
  const filtered = variables.filter((v) => v.key.trim() !== '')
  if (filtered.length === 0) return isEdit ? JSON.stringify({ version: 1, items: [] }) : undefined

  // 重复名去重（保留最后一个）
  const seen = new Map<string, ProviderScriptVariable>()
  for (const v of filtered) {
    seen.set(v.key.toLowerCase(), v)
  }
  const items = Array.from(seen.values())

  // 对 secret 变量：编辑模式下，空值或仍是引用的 → 保留原引用
  const processedItems = items.map((v) => ({
    ...v,
    value: v.isSecret && isEdit && (!v.value || v.value.startsWith('$SECRET:'))
      ? v.value // 保留原引用或空
      : v.value, // 新明文或非 secret → 原样
  }))

  return JSON.stringify({ version: 1, items: processedItems })
}

/**
 * 供应商新增/编辑表单
 *
 * 包含基本信息与模型管理两部分：
 * - 基础信息：slug、显示名称、协议类型、Base URL、认证配置
 * - 模型管理：可从内置模型列表选择，或从供应商 API 拉取官方模型
 */
export function ProviderForm({ open, onOpenChange, provider, initialValues, onSubmit, onProviderUpdated }: ProviderFormProps) {
  const { t } = useTranslation()
  const isEdit = Boolean(provider)

  // 高级设置：JSON 字段，不纳入 zod schema，用 state 管理
  const [balanceProviderJson, setBalanceProviderJson] = useState('')
  const [proxyMode, setProxyMode] = useState<'global' | 'direct' | 'socks' | 'http'>('global')
  const [proxyUrl, setProxyUrl] = useState('')
  const [timeoutConnection, setTimeoutConnection] = useState(5000)
  const [timeoutResponse, setTimeoutResponse] = useState(120000)
  const [maxRetries, setMaxRetries] = useState(3)

  // 扩展模板变量
  const [scriptVariables, setScriptVariables] = useState<ProviderScriptVariable[]>([])

  // OAuth 授权状态
  const [oauthAuthorizing, setOauthAuthorizing] = useState(false)
  const [oauthTimedOut, setOauthTimedOut] = useState(false)
  const [oauthRemainingSeconds, setOauthRemainingSeconds] = useState(OAUTH_TIMEOUT_SECONDS)
  const [deviceCodeInfo, setDeviceCodeInfo] = useState<DeviceCodeInfo | null>(null)
  const [deviceCodePolling, setDeviceCodePolling] = useState(false)

  // OAuth 手动授权码输入状态
  const [oauthStartResult, setOauthStartResult] = useState<OAuthStartResult | null>(null)
  const [manualCode, setManualCode] = useState('')
  const [manualExchanging, setManualExchanging] = useState(false)
  const [showManualInput, setShowManualInput] = useState(false)

  // 端口占用提示弹窗
  const [portInUsePort, setPortInUsePort] = useState<number | null>(null)

  // 重新授权确认弹窗
  const [reauthorizeOpen, setReauthorizeOpen] = useState(false)
  const [reauthorizeDeleteHistory, setReauthorizeDeleteHistory] = useState(true)
  const [reauthorizeClearing, setReauthorizeClearing] = useState(false)
  const reauthorizePendingMethodRef = useRef<AuthMethod | null>(null)

  // API Key 明文显示与解密状态
  const [showApiKey, setShowApiKey] = useState(false)
  const [showVertexApiKey, setShowVertexApiKey] = useState(false)
  const [decryptingApiKey, setDecryptingApiKey] = useState(false)
  const [decryptedApiKeyValue, setDecryptedApiKeyValue] = useState<string | null>(null)
  const [decryptedVertexApiKeyValue, setDecryptedVertexApiKeyValue] = useState<string | null>(null)

  // 编辑模式下保留原始 Secret 引用，用于「留空不修改」场景
  const existingApiKeyRef = useRef<string>('')
  const existingVertexApiKeyRef = useRef<string>('')

  // 用于忽略超时前已发起但尚未返回的授权结果
  const oauthAttemptIdRef = useRef(0)
  const oauthAuthMethodRef = useRef<AuthMethod | null>(null)

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      slug: '',
      displayName: '',
      providerType: 'openai-chat-completion',
      baseUrl: '',
      useRawBaseUrl: false,
      authMethod: 'none',
      apiKey: '',
      isEnabled: true,
      sortOrder: 0,
    },
  })

  // 编辑时回填表单
  useEffect(() => {
    if (provider) {
      const authConfig = parseAuthConfig(provider)
      const authMethod = authConfig?.method ?? 'none'
      const isApiKeyMethod = authMethod === 'api-key'
      const isVertexMethod = authMethod === 'google-vertex-ai-auth'
      const isGenericOAuth = authMethod === 'oauth2'

      const rawApiKey = isApiKeyMethod || isVertexMethod ? (authConfig as Extract<AuthConfig, { method: 'api-key' | 'google-vertex-ai-auth' }>).apiKey ?? '' : ''
      // 编辑时保留原始 Secret 引用，表单字段留空表示「不修改」
      const isApiKeySecretRef = rawApiKey.startsWith('$SECRET:') && rawApiKey.endsWith('$')
      existingApiKeyRef.current = isApiKeyMethod && isApiKeySecretRef ? rawApiKey : ''

      const vertexAuth = isVertexMethod ? authConfig as Extract<AuthConfig, { method: 'google-vertex-ai-auth' }> : undefined
      const rawVertexApiKey = vertexAuth?.apiKey ?? ''
      const isVertexApiKeySecretRef = rawVertexApiKey.startsWith('$SECRET:') && rawVertexApiKey.endsWith('$')
      existingVertexApiKeyRef.current = isVertexMethod && isVertexApiKeySecretRef ? rawVertexApiKey : ''
      const oauthAuth = isGenericOAuth ? authConfig as Extract<AuthConfig, { method: 'oauth2' }> : undefined

      form.reset({
        slug: provider.slug,
        displayName: provider.displayName,
        providerType: provider.providerType,
        baseUrl: provider.baseUrl,
        useRawBaseUrl: provider.useRawBaseUrl,
        authMethod: authMethod as AuthMethod,
        apiKey: isApiKeyMethod || isVertexMethod ? '' : '',
        // Google Vertex AI
        googleVertexSubType: vertexAuth?.subType ?? 'adc',
        googleVertexProjectId: vertexAuth?.projectId ?? '',
        googleVertexLocation: vertexAuth?.location ?? '',
        googleVertexKeyFilePath: vertexAuth?.keyFilePath ?? '',
        // 通用 OAuth 2.0
        oauthAuthorizationUrl: oauthAuth?.oauth?.authorizationUrl ?? '',
        oauthTokenUrl: oauthAuth?.oauth?.tokenUrl ?? '',
        oauthClientId: oauthAuth?.oauth?.clientId ?? '',
        oauthClientSecret: oauthAuth?.oauth?.clientSecret ?? '',
        oauthScopes: oauthAuth?.oauth?.scopes?.join(' ') ?? '',
        oauthPkce: oauthAuth?.oauth?.pkce ?? true,
        isEnabled: provider.isEnabled,
        sortOrder: provider.sortOrder,
      })
      // 回填高级设置
      setBalanceProviderJson(provider.balanceProviderJson ?? '')
      // 解析 proxyJson（供应商级代理：global / direct / socks / http）
      try {
        const proxy = provider.proxyJson ? JSON.parse(provider.proxyJson) as { type?: string; url?: string } : null
        if (proxy?.type === 'socks' || proxy?.type === 'http') {
          setProxyMode(proxy.type)
          setProxyUrl(proxy.url ?? '')
        } else if (proxy?.type === 'direct') {
          setProxyMode('direct')
          setProxyUrl('')
        } else {
          setProxyMode('global')
          setProxyUrl('')
        }
      } catch {
        setProxyMode('global')
        setProxyUrl('')
      }
      // 解析 timeoutJson
      try {
        const timeout = provider.timeoutJson ? JSON.parse(provider.timeoutJson) as { connection?: number; response?: number } : null
        setTimeoutConnection(timeout?.connection ?? 5000)
        setTimeoutResponse(timeout?.response ?? 120000)
      } catch {
        setTimeoutConnection(5000)
        setTimeoutResponse(120000)
      }
      // 解析 retryJson
      try {
        const retry = provider.retryJson ? JSON.parse(provider.retryJson) as { maxRetries?: number } : null
        setMaxRetries(retry?.maxRetries ?? 3)
      } catch {
        setMaxRetries(3)
      }
      // 解析 scriptVariablesJson
      try {
        const svJson = provider.scriptVariablesJson
        if (svJson) {
          const sv = JSON.parse(svJson) as { version: number; items: ProviderScriptVariable[] }
          setScriptVariables(sv.items ?? [])
        } else {
          setScriptVariables([])
        }
      } catch {
        setScriptVariables([])
      }
      // 重置 API Key 解密/显示状态
      setShowApiKey(false)
      setShowVertexApiKey(false)
      setDecryptedApiKeyValue(null)
      setDecryptedVertexApiKeyValue(null)
    } else {
      // 新建模式：清除 Secret 引用与解密状态
      existingApiKeyRef.current = ''
      existingVertexApiKeyRef.current = ''
      setShowApiKey(false)
      setShowVertexApiKey(false)
      setDecryptedApiKeyValue(null)
      setDecryptedVertexApiKeyValue(null)
      form.reset({
        slug: '',
        displayName: '',
        providerType: 'openai-chat-completion',
        baseUrl: '',
        useRawBaseUrl: false,
        authMethod: 'none',
        apiKey: '',
        googleVertexSubType: 'adc',
        googleVertexProjectId: '',
        googleVertexLocation: '',
        googleVertexKeyFilePath: '',
        oauthAuthorizationUrl: '',
        oauthTokenUrl: '',
        oauthClientId: '',
        oauthClientSecret: '',
        oauthScopes: '',
        oauthPkce: true,
        isEnabled: true,
        sortOrder: 0,
      })
      // 重置高级设置
      setBalanceProviderJson('')
      setProxyMode('global')
      setProxyUrl('')
      setTimeoutConnection(5000)
      setTimeoutResponse(120000)
      setMaxRetries(3)
      setScriptVariables([])
      // 应用内置预设预填充值（延迟 setValue 避免与 reset 冲突）
      if (initialValues) {
        window.setTimeout(() => {
          if (initialValues.slug) form.setValue('slug', initialValues.slug)
          if (initialValues.displayName) form.setValue('displayName', initialValues.displayName)
          if (initialValues.providerType) form.setValue('providerType', initialValues.providerType)
          if (initialValues.baseUrl) form.setValue('baseUrl', initialValues.baseUrl)
          if (initialValues.useRawBaseUrl !== undefined) form.setValue('useRawBaseUrl', initialValues.useRawBaseUrl)
          if (initialValues.authMethod) form.setValue('authMethod', initialValues.authMethod)
          if (initialValues.sortOrder !== undefined) form.setValue('sortOrder', initialValues.sortOrder)
        }, 0)
      }
    }
  }, [provider, initialValues, form])

  /**
   * 解密 API Key 引用为明文
   *
   * 点击小眼睛时调用后端 `secret_decrypt_text` 命令解密 `$SECRET:{id}$` 引用。
   * 仅在编辑模式且存在 Secret 引用时有效。
   */
  const handleDecryptApiKey = async (target: 'api-key' | 'vertex-api-key') => {
    const ref = target === 'api-key' ? existingApiKeyRef.current : existingVertexApiKeyRef.current
    if (!ref) return
    setDecryptingApiKey(true)
    try {
      const plaintext = await invokeCommand<string>('secret_decrypt_text', { value: ref })
      if (target === 'api-key') {
        setDecryptedApiKeyValue(plaintext)
      } else {
        setDecryptedVertexApiKeyValue(plaintext)
      }
    } catch (err) {
      const error = toIcodeError(err)
      toast.error(error.message)
    } finally {
      setDecryptingApiKey(false)
    }
  }

  const handleSubmit = (values: FormValues) => {
    const auth: AuthConfig | undefined = buildAuthConfig(
      values,
      provider,
      existingApiKeyRef.current,
      existingVertexApiKeyRef.current,
    )

    // 构建 proxyJson（供应商级代理：始终序列化，确保切换回 global 时能覆盖旧模式）
    // 此前 global 返回 undefined 会被 Tauri invoke 省略，导致后端跳过更新、
    // DB 仍保留旧的 socks/http 配置，表现为「无法切换回全局代理」。
    const proxyJson = JSON.stringify({
      type: proxyMode,
      url: proxyMode === 'socks' || proxyMode === 'http' ? proxyUrl : undefined,
    })

    // 构建 timeoutJson（仅非默认值时提交）
    const timeoutJson = (timeoutConnection !== 5000 || timeoutResponse !== 120000)
      ? JSON.stringify({ connection: timeoutConnection, response: timeoutResponse })
      : undefined

    // 构建 retryJson（仅非默认值时提交）
    const retryJson = maxRetries !== 3
      ? JSON.stringify({ maxRetries })
      : undefined

    onSubmit({
      slug: values.slug,
      displayName: values.displayName,
      providerType: values.providerType,
      baseUrl: values.baseUrl,
      useRawBaseUrl: values.useRawBaseUrl,
      auth,
      isEnabled: values.isEnabled,
      sortOrder: values.sortOrder,
      balanceProviderJson: balanceProviderJson || undefined,
      proxyJson,
      timeoutJson,
      retryJson,
      scriptVariablesJson: buildScriptVariablesJson(scriptVariables, isEdit),
    })
  }

  /**
   * 启动 OAuth 授权流程
   *
   * - 浏览器授权码流程：调用 `gateway_provider_oauth_start` 打开浏览器并返回 PKCE 参数，
   *   调用 `gateway_provider_oauth_complete` 完成换 token。
   *   回调服务器收到浏览器重定向时，会通过 `oauth-callback-result` 事件通知前端，
   *   前端监听事件后自动调用 `gateway_provider_oauth_complete` 完成流程。
   * - Device Code 流程：请求设备码并开启轮询
   */
  /**
   * 发起 OAuth 授权
   *
   * 若供应商已有 token（重新授权场景），先弹窗让用户选择是否删除历史认证信息；
   * 确认后根据勾选状态决定是否先清空旧 token，再走实际授权流程。
   * 首次授权（无 token）直接进入授权流程。
   */
  const startOAuthAuthorize = async (authMethod: AuthMethod) => {
    if (!provider) return
    const existingAuth = parseAuthConfig(provider)
    const hasExistingToken = !!existingAuth
      && existingAuth.method !== 'none'
      && existingAuth.method !== 'api-key'
      && 'token' in existingAuth
      && Boolean(existingAuth.token)
    if (hasExistingToken) {
      reauthorizePendingMethodRef.current = authMethod
      setReauthorizeDeleteHistory(true)
      setReauthorizeOpen(true)
      return
    }
    void doStartOAuthAuthorize(authMethod)
  }

  /**
   * 重新授权确认弹窗：用户确认后根据勾选状态执行
   */
  const handleReauthorizeConfirm = async () => {
    const authMethod = reauthorizePendingMethodRef.current
    if (!authMethod || !provider) return
    const deleteHistory = reauthorizeDeleteHistory
    setReauthorizeOpen(false)
    reauthorizePendingMethodRef.current = null
    if (deleteHistory) {
      setReauthorizeClearing(true)
      try {
        const updated = await clearOauthToken(provider.id)
        onProviderUpdated?.(updated)
      } catch (err) {
        toast.error(toIcodeError(err).message)
      } finally {
        setReauthorizeClearing(false)
      }
    }
    void doStartOAuthAuthorize(authMethod)
  }

  const doStartOAuthAuthorize = async (authMethod: AuthMethod) => {
    if (!provider) return
    // 新的一次授权尝试：递增 attempt id，使旧请求返回的结果被忽略
    oauthAttemptIdRef.current += 1
    const attemptId = oauthAttemptIdRef.current
    oauthAuthMethodRef.current = authMethod
    setOauthTimedOut(false)
    setOauthRemainingSeconds(OAUTH_TIMEOUT_SECONDS)
    setOauthAuthorizing(true)
    setShowManualInput(false)
    setOauthStartResult(null)
    setManualCode('')
    try {
      if (authMethod === 'github-copilot') {
        // Device Code 流程：请求设备码并开启轮询
        const info = await invokeCommand<DeviceCodeInfo>('gateway_provider_oauth_device_code', {
          providerId: provider.id,
          authMethod,
        })
        if (oauthAttemptIdRef.current !== attemptId) return
        setDeviceCodeInfo(info)
        setDeviceCodePolling(true)
      } else {
        // 浏览器授权码流程：使用 start 命令启动授权
        // start 命令会启动回调服务器并打开浏览器，同时返回 PKCE 参数
        // 回调服务器收到浏览器重定向时会通过事件自动完成；
        // 若供应商在浏览器中显示授权码（不自动重定向），则用户手动输入
        const startResult = await invokeCommand<OAuthStartResult>('gateway_provider_oauth_start', {
          providerId: provider.id,
          authMethod,
        })
        if (oauthAttemptIdRef.current !== attemptId) return
        setOauthStartResult(startResult)
        // 显示手动输入区域（同时等待事件自动完成）
        setShowManualInput(true)
      }
    } catch (err) {
      if (oauthAttemptIdRef.current !== attemptId) return
      const error = toIcodeError(err)
      // 检测端口占用错误，弹窗提示清理进程
      if (error.details?.reason === 'port_in_use' && typeof error.details.port === 'number') {
        setPortInUsePort(error.details.port)
      } else {
        toast.error(error.message)
      }
    } finally {
      if (oauthAttemptIdRef.current === attemptId) {
        setOauthAuthorizing(false)
      }
    }
  }

  /**
   * 手动输入授权码完成 OAuth 流程
   *
   * 当浏览器不自动回调（供应商在浏览器中显示授权码），
   * 用户可手动粘贴授权码，调用 `gateway_provider_oauth_complete` 完成换 token。
   */
  const completeOAuthWithCode = async (code: string) => {
    if (manualExchanging) return

    if (!provider || !oauthStartResult || !code.trim()) return
    setManualExchanging(true)
    try {
      // 尝试解析输入：可能是纯授权码，也可能是完整回调 URL（含 ?code=xxx）
      let parsedCode = code.trim()
      try {
        const url = new URL(parsedCode)
        const urlCode = url.searchParams.get('code')
        if (urlCode) parsedCode = urlCode
      } catch {
        // 不是 URL，当作纯授权码处理
      }

      const updated = await invokeCommand<Provider>('gateway_provider_oauth_complete', {
        providerId: provider.id,
        authMethod: form.getValues('authMethod'),
        code: parsedCode,
        codeVerifier: oauthStartResult.codeVerifier,
        redirectUri: oauthStartResult.redirectUri,
      })
      toast.success(t('aiGateway.providerForm.oauthAuthorizeSuccess'))
      onProviderUpdated?.(updated)
      setShowManualInput(false)
      setOauthStartResult(null)
      setManualCode('')
    } catch (err) {
      const error = toIcodeError(err)
      toast.error(error.message)
    } finally {
      setManualExchanging(false)
    }
  }

  /**
   * 取消正在进行的授权码换取流程
   *
   * 由于 Tauri invoke 不支持真正的请求中断，这里仅重置前端状态，
   * 使用户可以重新编辑/提交授权码；后端请求若已发出会继续执行，
   * 但前端不再等待其返回。
   */
  const cancelManualExchange = useCallback(() => {
    setManualExchanging(false)
  }, [])

  // 使用 ref 避免 device code 轮询 effect 因闭包变化而频繁重置
  const onProviderUpdatedRef = useRef(onProviderUpdated)
  onProviderUpdatedRef.current = onProviderUpdated
  const tRef = useRef(t)
  tRef.current = t

  /**
   * 监听 OAuth 回调事件
   *
   * 当回调服务器收到浏览器重定向（自动回调成功）时，
   * 后端通过 `oauth-callback-result` 事件发送授权码，
   * 前端自动调用 `gateway_provider_oauth_complete` 完成流程。
   */
  useEffect(() => {
    if (!oauthStartResult || !provider) return

    const unlisten = listen<OAuthCallbackEvent>('oauth-callback-result', (event) => {
      // 仅处理当前供应商的回调
      if (event.payload.providerId !== provider.id) return
      // 仅处理当前授权尝试的回调
      if (!event.payload.code) return
      if (event.payload.error) {
        const msg = event.payload.errorDescription || event.payload.error
        toast.error(msg)
        return
      }
      // 自动完成授权流程
      void completeOAuthWithCode(event.payload.code)
    })

    return () => {
      unlisten.then((fn) => fn())
    }
  }, [oauthStartResult, provider])

  /**
   * OAuth 浏览器授权倒计时
   *
   * 手动输入区域显示后开始 120s 倒计时（等待自动回调）；
   * Device Code 流程不启用此倒计时。
   * 倒计时归零时提示用户手动输入授权码。
   */
  useEffect(() => {
    if (!showManualInput || deviceCodePolling) return
    if (oauthAuthMethodRef.current === 'github-copilot') return

    setOauthRemainingSeconds(OAUTH_TIMEOUT_SECONDS)
    const timer = setInterval(() => {
      setOauthRemainingSeconds((prev) => {
        if (prev <= 1) {
          clearInterval(timer)
          setOauthTimedOut(true)
          return 0
        }
        return prev - 1
      })
    }, 1000)

    return () => clearInterval(timer)
  }, [showManualInput, deviceCodePolling])

  /**
   * Device Code 轮询
   *
   * 首次 interval 延迟后发起轮询；若状态为 pending 则按 interval 继续等待，
   * 成功或出错时停止轮询。
   */
  useEffect(() => {
    if (!deviceCodeInfo || !deviceCodePolling || !provider) return

    let cancelled = false
    const authMethod = form.getValues('authMethod') as AuthMethod

    const poll = async () => {
      try {
        const result = await invokeCommand<DeviceCodePollResult>('gateway_provider_oauth_poll_device_token', {
          providerId: provider.id,
          authMethod,
          deviceCode: deviceCodeInfo.deviceCode,
        })
        if (cancelled) return
        if (result.status === 'success' && result.provider) {
          setDeviceCodePolling(false)
          setDeviceCodeInfo(null)
          toast.success(tRef.current('aiGateway.providerForm.oauthAuthorizeSuccess'))
          onProviderUpdatedRef.current?.(result.provider)
        } else {
          setTimeout(poll, deviceCodeInfo.interval * 1000)
        }
      } catch (err) {
        if (cancelled) return
        setDeviceCodePolling(false)
        const error = toIcodeError(err)
        toast.error(error.message)
      }
    }

    const timeoutId = setTimeout(poll, deviceCodeInfo.interval * 1000)
    return () => {
      cancelled = true
      clearTimeout(timeoutId)
    }
  }, [deviceCodeInfo, deviceCodePolling, provider, form])

  const errors = form.formState.errors

  return (
    <>
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto custom-scrollbar">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-1.5 text-base">
            {isEdit ? t('aiGateway.providerForm.editTitle') : t('aiGateway.addProvider')}
            <Popover>
              <PopoverTrigger asChild>
                <i className="fa-regular fa-circle-question inline-block cursor-help text-muted-foreground text-xs" />
              </PopoverTrigger>
              <PopoverContent side="bottom" align="start" className="w-64 text-xs">
                <ul className="space-y-1">
                  <li>• {t('aiGateway.providerForm.help.slug')}</li>
                  <li>• {t('aiGateway.providerForm.help.providerType')}</li>
                  <li>• {t('aiGateway.providerForm.help.baseUrl')}</li>
                  <li>• {t('aiGateway.providerForm.help.apiKey')}</li>
                  <li>• {t('aiGateway.providerForm.help.addModels')}</li>
                  <li>• {t('aiGateway.providerForm.help.visibility')}</li>
                </ul>
              </PopoverContent>
            </Popover>
          </DialogTitle>
          <DialogDescription className="text-xs">
            {isEdit ? t('aiGateway.providerForm.editDescription') : t('aiGateway.providerForm.createDescription')}
          </DialogDescription>
        </DialogHeader>

        <Tabs defaultValue="basic" className="w-full">
          <TabsList className="mb-3">
            <TabsTrigger value="basic" className="text-xs">{t('aiGateway.providerForm.tabs.basic')}</TabsTrigger>
            <TabsTrigger value="models" className="text-xs">{t('aiGateway.providerForm.tabs.models')}</TabsTrigger>
            <TabsTrigger value="advanced" className="text-xs">{t('aiGateway.providerForm.tabs.advanced')}</TabsTrigger>
            <TabsTrigger value="extension" className="text-xs">{t('aiGateway.providerForm.tabs.extension')}</TabsTrigger>
          </TabsList>

          <form onSubmit={form.handleSubmit(handleSubmit)} id="provider-form" className="contents">
            <TabsContent value="basic" className="space-y-4">
              <div className="grid grid-cols-2 gap-4">
                {/* slug */}
                <div className="space-y-1.5" data-invalid={errors.slug ? '' : undefined}>
                  <Label className="text-xs" htmlFor="slug">slug</Label>
                  <Input
                    id="slug"
                    {...form.register('slug')}
                    disabled={isEdit}
                    className="h-8 text-xs"
                    aria-invalid={errors.slug ? true : undefined}
                  />
                  {errors.slug && (
                    <p className="text-destructive text-[10px]">{t(errors.slug.message || '')}</p>
                  )}
                </div>
                {/* 显示名称 */}
                <div className="space-y-1.5" data-invalid={errors.displayName ? '' : undefined}>
                  <Label className="text-xs" htmlFor="displayName">{t('aiGateway.providerForm.displayName')}</Label>
                  <Input
                    id="displayName"
                    {...form.register('displayName')}
                    className="h-8 text-xs"
                    aria-invalid={errors.displayName ? true : undefined}
                  />
                  {errors.displayName && (
                    <p className="text-destructive text-[10px]">{t(errors.displayName.message || '')}</p>
                  )}
                </div>
              </div>

              <div className="mt-4 grid grid-cols-2 gap-4">
                {/* 协议类型 */}
                <div className="space-y-1.5">
                  <Label className="text-xs">{t('aiGateway.providerForm.providerType')}</Label>
                  <Select
                    value={form.watch('providerType')}
                    onValueChange={(v) => form.setValue('providerType', v)}
                  >
                    <SelectTrigger className="h-8 text-xs">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {providerTypes.map((type) => (
                        <SelectItem key={type} value={type} className="text-xs">
                          {type}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                {/* 排序 */}
                <div className="space-y-1.5">
                  <Label className="text-xs" htmlFor="sortOrder">{t('aiGateway.providerForm.sortOrder')}</Label>
                  <Input
                    id="sortOrder"
                    type="number"
                    {...form.register('sortOrder')}
                    className="h-8 text-xs"
                  />
                </div>
              </div>

              {/* Base URL */}
              <div className="mt-4 space-y-1.5" data-invalid={errors.baseUrl ? '' : undefined}>
                <Label className="text-xs" htmlFor="baseUrl">{t('aiGateway.providerForm.baseUrl')}</Label>
                <Input
                  id="baseUrl"
                  {...form.register('baseUrl')}
                  className="h-8 text-xs"
                  aria-invalid={errors.baseUrl ? true : undefined}
                />
                {errors.baseUrl && (
                  <p className="text-destructive text-[10px]">{t(errors.baseUrl.message || '')}</p>
                )}
              </div>

              <div className="mt-4 grid grid-cols-2 gap-4">
                {/* 认证方式 */}
                <div className="space-y-1.5">
                  <Label className="text-xs">{t('aiGateway.providerForm.authMethod')}</Label>
                  <Select
                    value={form.watch('authMethod')}
                    onValueChange={(v) => form.setValue('authMethod', v as AuthMethod)}
                  >
                    <SelectTrigger className="h-8 text-xs">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {getAuthMethodOptions(t, form.watch('providerType')).map((opt) => (
                        <SelectItem key={opt.value} value={opt.value} className="text-xs">
                          {opt.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                <div className="flex items-end pb-1.5">
                  <label className="flex items-center gap-2 text-xs">
                    <Switch checked={form.watch('useRawBaseUrl')} onCheckedChange={(v) => form.setValue('useRawBaseUrl', v)} />
                    {t('aiGateway.providerForm.useRawBaseUrl')}
                    <TooltipProvider delayDuration={200}>
                      <Tooltip>
                        <TooltipTrigger asChild>
                          <i className="fa-solid fa-circle-info text-muted-foreground cursor-help" />
                        </TooltipTrigger>
                        <TooltipContent side="top" className="max-w-[260px] text-[11px] leading-relaxed whitespace-pre-line">
                          {t('aiGateway.providerForm.useRawBaseUrlHelp')}
                        </TooltipContent>
                      </Tooltip>
                    </TooltipProvider>
                  </label>
                </div>
              </div>

              {/* API Key */}
              {form.watch('authMethod') === 'api-key' && (
                <div className="mt-4 space-y-1.5">
                  <Label className="text-xs" htmlFor="apiKey">{t('aiGateway.providerForm.apiKey')}</Label>
                  <div className="relative">
                    <Input
                      id="apiKey"
                      type={showApiKey ? 'text' : 'password'}
                      {...form.register('apiKey')}
                      className="h-8 text-xs pr-8"
                      placeholder={isEdit
                        ? (existingApiKeyRef.current
                          ? t('aiGateway.providerForm.apiKeyEditPlaceholder')
                          : t('aiGateway.providerForm.apiKeyPlaceholder'))
                        : t('aiGateway.providerForm.apiKeyPlaceholder')
                      }
                    />
                    <button
                      type="button"
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                      title={showApiKey ? t('aiGateway.providerForm.apiKeyHide') : t('aiGateway.providerForm.apiKeyShow')}
                      onClick={async () => {
                        if (!showApiKey && isEdit && existingApiKeyRef.current && decryptedApiKeyValue === null) {
                          await handleDecryptApiKey('api-key')
                        }
                        setShowApiKey(!showApiKey)
                      }}
                      disabled={decryptingApiKey}
                    >
                      <i className={cn(
                        decryptingApiKey ? 'fa-solid fa-spinner fa-spin' : showApiKey ? 'fa-solid fa-eye-slash' : 'fa-solid fa-eye',
                        'text-xs',
                      )} />
                    </button>
                  </div>
                  {/* 解密后的明文预览 */}
                  {showApiKey && isEdit && decryptedApiKeyValue !== null && !form.watch('apiKey') && (
                    <p className="text-muted-foreground text-[10px] font-mono break-all">
                      {t('aiGateway.providerForm.apiKeyCurrentValue')}: {decryptedApiKeyValue}
                    </p>
                  )}
                </div>
              )}

              {/* Google Vertex AI 配置 */}
              {form.watch('authMethod') === 'google-vertex-ai-auth' && (
                <div className="mt-4 space-y-3">
                  <div className="space-y-1.5">
                    <Label className="text-xs">{t('aiGateway.providerForm.googleVertexSubType')}</Label>
                    <Select
                      value={form.watch('googleVertexSubType') ?? 'adc'}
                      onValueChange={(v) => form.setValue('googleVertexSubType', v as 'adc' | 'service-account' | 'api-key')}
                    >
                      <SelectTrigger className="h-8 text-xs">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="adc" className="text-xs">{t('aiGateway.providerForm.googleVertexSubTypeAdc')}</SelectItem>
                        <SelectItem value="service-account" className="text-xs">{t('aiGateway.providerForm.googleVertexSubTypeServiceAccount')}</SelectItem>
                        <SelectItem value="api-key" className="text-xs">{t('aiGateway.providerForm.googleVertexSubTypeApiKey')}</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>

                  {form.watch('googleVertexSubType') === 'adc' && (
                    <div className="grid grid-cols-2 gap-3">
                      <div className="space-y-1.5">
                        <Label className="text-xs" htmlFor="googleVertexProjectId">{t('aiGateway.providerForm.googleVertexProjectId')}</Label>
                        <Input
                          id="googleVertexProjectId"
                          {...form.register('googleVertexProjectId')}
                          className="h-8 text-xs"
                          placeholder={t('aiGateway.providerForm.googleVertexProjectIdPlaceholder')}
                        />
                      </div>
                      <div className="space-y-1.5">
                        <Label className="text-xs" htmlFor="googleVertexLocation">{t('aiGateway.providerForm.googleVertexLocation')}</Label>
                        <Input
                          id="googleVertexLocation"
                          {...form.register('googleVertexLocation')}
                          className="h-8 text-xs"
                          placeholder={t('aiGateway.providerForm.googleVertexLocationPlaceholder')}
                        />
                      </div>
                    </div>
                  )}

                  {form.watch('googleVertexSubType') === 'service-account' && (
                    <div className="space-y-1.5">
                      <Label className="text-xs" htmlFor="googleVertexKeyFilePath">{t('aiGateway.providerForm.googleVertexKeyFilePath')}</Label>
                      <Input
                        id="googleVertexKeyFilePath"
                        {...form.register('googleVertexKeyFilePath')}
                        className="h-8 text-xs"
                        placeholder={t('aiGateway.providerForm.googleVertexKeyFilePathPlaceholder')}
                      />
                    </div>
                  )}

                  {form.watch('googleVertexSubType') === 'api-key' && (
                    <div className="space-y-1.5">
                      <Label className="text-xs" htmlFor="apiKey">{t('aiGateway.providerForm.apiKey')}</Label>
                      <div className="relative">
                        <Input
                          id="apiKey"
                          type={showVertexApiKey ? 'text' : 'password'}
                          {...form.register('apiKey')}
                          className="h-8 text-xs pr-8"
                          placeholder={isEdit
                            ? (existingVertexApiKeyRef.current
                              ? t('aiGateway.providerForm.apiKeyEditPlaceholder')
                              : t('aiGateway.providerForm.apiKeyPlaceholder'))
                            : t('aiGateway.providerForm.apiKeyPlaceholder')
                          }
                        />
                        <button
                          type="button"
                          className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                          title={showVertexApiKey ? t('aiGateway.providerForm.apiKeyHide') : t('aiGateway.providerForm.apiKeyShow')}
                          onClick={async () => {
                            if (!showVertexApiKey && isEdit && existingVertexApiKeyRef.current && decryptedVertexApiKeyValue === null) {
                              await handleDecryptApiKey('vertex-api-key')
                            }
                            setShowVertexApiKey(!showVertexApiKey)
                          }}
                          disabled={decryptingApiKey}
                        >
                          <i className={cn(
                            decryptingApiKey ? 'fa-solid fa-spinner fa-spin' : showVertexApiKey ? 'fa-solid fa-eye-slash' : 'fa-solid fa-eye',
                            'text-xs',
                          )} />
                        </button>
                      </div>
                      {/* 解密后的明文预览 */}
                      {showVertexApiKey && isEdit && decryptedVertexApiKeyValue !== null && !form.watch('apiKey') && (
                        <p className="text-muted-foreground text-[10px] font-mono break-all">
                          {t('aiGateway.providerForm.apiKeyCurrentValue')}: {decryptedVertexApiKeyValue}
                        </p>
                      )}
                    </div>
                  )}
                </div>
              )}

              {/* 通用 OAuth 2.0 端点配置 */}
              {form.watch('authMethod') === 'oauth2' && (
                <div className="mt-4 space-y-3">
                  <div className="grid grid-cols-2 gap-3">
                    <div className="space-y-1.5">
                      <Label className="text-xs" htmlFor="oauthAuthorizationUrl">{t('aiGateway.providerForm.oauthAuthorizationUrl')}</Label>
                      <Input
                        id="oauthAuthorizationUrl"
                        {...form.register('oauthAuthorizationUrl')}
                        className="h-8 text-xs"
                        placeholder={t('aiGateway.providerForm.oauthAuthorizationUrlPlaceholder')}
                      />
                    </div>
                    <div className="space-y-1.5">
                      <Label className="text-xs" htmlFor="oauthTokenUrl">{t('aiGateway.providerForm.oauthTokenUrl')}</Label>
                      <Input
                        id="oauthTokenUrl"
                        {...form.register('oauthTokenUrl')}
                        className="h-8 text-xs"
                        placeholder={t('aiGateway.providerForm.oauthTokenUrlPlaceholder')}
                      />
                    </div>
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <div className="space-y-1.5">
                      <Label className="text-xs" htmlFor="oauthClientId">{t('aiGateway.providerForm.oauthClientId')}</Label>
                      <Input
                        id="oauthClientId"
                        {...form.register('oauthClientId')}
                        className="h-8 text-xs"
                        placeholder={t('aiGateway.providerForm.oauthClientIdPlaceholder')}
                      />
                    </div>
                    <div className="space-y-1.5">
                      <Label className="text-xs" htmlFor="oauthClientSecret">{t('aiGateway.providerForm.oauthClientSecret')}</Label>
                      <Input
                        id="oauthClientSecret"
                        type="password"
                        {...form.register('oauthClientSecret')}
                        className="h-8 text-xs"
                        placeholder={t('aiGateway.providerForm.oauthClientSecretPlaceholder')}
                      />
                    </div>
                  </div>
                  <div className="space-y-1.5">
                    <Label className="text-xs" htmlFor="oauthScopes">{t('aiGateway.providerForm.oauthScopes')}</Label>
                    <Input
                      id="oauthScopes"
                      {...form.register('oauthScopes')}
                      className="h-8 text-xs"
                      placeholder={t('aiGateway.providerForm.oauthScopesPlaceholder')}
                    />
                  </div>
                  <label className="flex items-center gap-2 text-xs">
                    <Switch
                      checked={form.watch('oauthPkce') ?? true}
                      onCheckedChange={(v) => form.setValue('oauthPkce', v)}
                    />
                    {t('aiGateway.providerForm.oauthPkce')}
                  </label>
                </div>
              )}

              {/* OAuth 授权区 */}
              {isOAuthMethod(form.watch('authMethod')) && isEdit && (
                <OAuthAuthorizeSection
                  provider={provider!}
                  authMethod={form.watch('authMethod')}
                  authorizing={oauthAuthorizing}
                  timedOut={oauthTimedOut}
                  remainingSeconds={oauthRemainingSeconds}
                  deviceCodeInfo={deviceCodeInfo}
                  deviceCodePolling={deviceCodePolling}
                  showManualInput={showManualInput}
                  manualCode={manualCode}
                  manualExchanging={manualExchanging}
                  onStartAuthorize={() => void startOAuthAuthorize(form.watch('authMethod'))}
                  onStopDeviceCode={() => {
                    setDeviceCodeInfo(null)
                    setDeviceCodePolling(false)
                  }}
                  onManualCodeChange={setManualCode}
                  onCompleteWithCode={() => void completeOAuthWithCode(manualCode)}
                  onCancelManualExchange={cancelManualExchange}
                />
              )}

              {isOAuthMethod(form.watch('authMethod')) && !isEdit && (
                <div className="mt-4 rounded-md border border-dashed p-3 text-xs text-muted-foreground">
                  <i className="fa-solid fa-info-circle mr-1.5" />
                  {t('aiGateway.providerForm.oauthSaveFirst')}
                </div>
              )}

              {/* 启用开关 */}
              <div className="mt-4 flex items-center gap-2">
                <Switch
                  id="isEnabled"
                  checked={form.watch('isEnabled')}
                  onCheckedChange={(v) => form.setValue('isEnabled', v)}
                />
                <Label className="text-xs" htmlFor="isEnabled">{t('aiGateway.providerForm.enableProvider')}</Label>
              </div>
          </TabsContent>

          <TabsContent value="models" className="min-h-[200px]">
            <ModelManagementSection provider={provider} />
          </TabsContent>

          <TabsContent value="advanced" className="space-y-4">
            {/* 额度监控 */}
            <div>
              <div className="mb-2 text-xs font-medium">{t('aiGateway.providerForm.balanceMonitoring')}</div>
              <BalanceConfigForm
                value={balanceProviderJson}
                onChange={setBalanceProviderJson}
              />
            </div>

            {/* 代理配置 */}
            <div>
              <div className="mb-2 text-xs font-medium">{t('aiGateway.providerForm.proxyConfig')}</div>
              <div className="space-y-2">
                <Select value={proxyMode} onValueChange={(v) => setProxyMode(v as 'global' | 'direct' | 'socks' | 'http')}>
                  <SelectTrigger className="h-8 text-xs">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="global" className="text-xs">{t('aiGateway.providerForm.proxyGlobal')}</SelectItem>
                    <SelectItem value="direct" className="text-xs">{t('aiGateway.providerForm.proxyDirect')}</SelectItem>
                    <SelectItem value="socks" className="text-xs">{t('aiGateway.providerForm.proxySocks')}</SelectItem>
                    <SelectItem value="http" className="text-xs">{t('aiGateway.providerForm.proxyHttp')}</SelectItem>
                  </SelectContent>
                </Select>
                {(proxyMode === 'socks' || proxyMode === 'http') && (
                  <Input
                    value={proxyUrl}
                    onChange={(e) => setProxyUrl(e.target.value)}
                    className="h-8 text-xs font-mono"
                    placeholder={t('aiGateway.providerForm.proxyUrlPlaceholder')}
                  />
                )}
              </div>
            </div>

            {/* 超时与重试 */}
            <div>
              <div className="mb-2 text-xs font-medium">{t('aiGateway.providerForm.timeoutRetry')}</div>
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1.5">
                  <Label className="text-[11px]">{t('aiGateway.providerForm.timeoutConnection')}</Label>
                  <Input
                    type="number"
                    value={timeoutConnection}
                    onChange={(e) => setTimeoutConnection(Number(e.target.value) || 0)}
                    className="h-8 text-xs"
                  />
                </div>
                <div className="space-y-1.5">
                  <Label className="text-[11px]">{t('aiGateway.providerForm.timeoutResponse')}</Label>
                  <Input
                    type="number"
                    value={timeoutResponse}
                    onChange={(e) => setTimeoutResponse(Number(e.target.value) || 0)}
                    className="h-8 text-xs"
                  />
                </div>
              </div>
              <div className="mt-2 space-y-1.5">
                <Label className="text-[11px]">{t('aiGateway.providerForm.maxRetries')}</Label>
                <Input
                  type="number"
                  value={maxRetries}
                  onChange={(e) => setMaxRetries(Number(e.target.value) || 0)}
                  className="h-8 text-xs"
                />
              </div>
            </div>
          </TabsContent>

          <TabsContent value="extension" className="space-y-4">
            <ScriptVariablesEditor
              variables={scriptVariables}
              onChange={setScriptVariables}
              isEdit={isEdit}
            />
          </TabsContent>
          </form>
        </Tabs>

        <Separator className="my-2" />

        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-8 text-xs"
            onClick={() => onOpenChange(false)}
          >
            {t('common.cancel')}
          </Button>
          <Button
            type="submit"
            form="provider-form"
            size="sm"
            className="h-8 text-xs"
          >
            <i className="fa-solid fa-check mr-1.5" data-icon="inline-start" />
            {t('common.save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    {/* 端口占用提示弹窗 */}
    <PortInUseDialog
      open={portInUsePort !== null}
      port={portInUsePort}
      onOpenChange={(v) => !v && setPortInUsePort(null)}
    />

    {/* 重新授权确认弹窗 */}
    <Dialog open={reauthorizeOpen} onOpenChange={setReauthorizeOpen}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 text-base">
            <i className="fa-solid fa-triangle-exclamation text-amber-500" />
            {t('aiGateway.providerForm.reauthorizeTitle')}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {t('aiGateway.providerForm.reauthorizeDescription')}
          </DialogDescription>
        </DialogHeader>
        <label className="flex cursor-pointer items-start gap-2 rounded-md border p-3 text-xs hover:bg-muted/50">
          <Checkbox
            checked={reauthorizeDeleteHistory}
            onCheckedChange={(v) => setReauthorizeDeleteHistory(v === true)}
            className="mt-0.5"
          />
          <div className="space-y-1">
            <span className="font-medium">{t('aiGateway.providerForm.reauthorizeDeleteHistory')}</span>
            <p className="text-muted-foreground text-[11px]">
              {t('aiGateway.providerForm.reauthorizeDeleteHistoryHint')}
            </p>
          </div>
        </label>
        <DialogFooter>
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-8 text-xs"
            onClick={() => setReauthorizeOpen(false)}
            disabled={reauthorizeClearing}
          >
            {t('common.cancel')}
          </Button>
          <Button
            type="button"
            size="sm"
            className="h-8 text-xs"
            onClick={() => void handleReauthorizeConfirm()}
            disabled={reauthorizeClearing}
          >
            {reauthorizeClearing ? (
              <i className="fa-solid fa-spinner fa-spin mr-1.5" />
            ) : (
              <i className="fa-solid fa-check mr-1.5" />
            )}
            {t('aiGateway.providerForm.reauthorizeConfirm')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
    </>
  )
}

// ===== 模型管理子组件 =====

interface ModelManagementSectionProps {
  provider?: Provider | null
}

/**
 * 模型管理子组件
 *
 * 在供应商表单中嵌入，提供：
 * - 展示当前供应商已添加的模型列表
 * - 支持标记模型为公开/隐藏
 * - 支持删除模型
 * - 支持从内置模型列表中选择添加
 * - 支持从供应商 API 拉取官方模型后选择添加
 *
 * 仅编辑模式下可用（需要已保存的 provider ID）。
 */
function ModelManagementSection({ provider }: ModelManagementSectionProps) {
  const [existingModels, setExistingModels] = useState<GatewayModel[]>([])
  const [modelsLoading, setModelsLoading] = useState(false)
  const [builtinModels, setBuiltinModels] = useState<BuiltinModel[]>([])
  const [builtinLoading, setBuiltinLoading] = useState(false)
  const [officialModels, setOfficialModels] = useState<string[]>([])
  const [officialLoading, setOfficialLoading] = useState(false)
  const [officialError, setOfficialError] = useState<string | null>(null)

  // 批量添加的选中状态
  const [selectedBuiltinIds, setSelectedBuiltinIds] = useState<Set<string>>(new Set())
  const [selectedOfficialIds, setSelectedOfficialIds] = useState<Set<string>>(new Set())

  // 模糊搜索关键词（列表超过6个时展示搜索框）
  const [builtinSearch, setBuiltinSearch] = useState('')
  const [officialSearch, setOfficialSearch] = useState('')

  // 内置模型 providerType 筛选（默认展示全部）
  const [builtinTypeFilter, setBuiltinTypeFilter] = useState<'all' | ProviderType>('all')

  // 添加进度
  const [addingModels, setAddingModels] = useState(false)

  // 弹窗编辑状态
  const [editDialogOpen, setEditDialogOpen] = useState(false)
  const [editingGatewayModel, setEditingGatewayModel] = useState<GatewayModel | null>(null)
  const [editingModelConfig, setEditingModelConfig] = useState<ModelConfig | null>(null)

  const { t, i18n } = useTranslation()

  // 全部内置模型（用于官方模型添加时的模糊匹配预填充）
  const [allBuiltinModels, setAllBuiltinModels] = useState<BuiltinModel[]>([])

  // 模型编辑表单
  const modelEditForm = useForm<ModelEditFormValues>({
    resolver: zodResolver(modelEditSchema),
    defaultValues: {
      modelId: '',
      displayName: '',
      family: '',
      maxInputTokens: '',
      maxOutputTokens: '',
      tokenizer: '',
      tokenCountMultiplier: 1,
      pricePer1mTokens: '',
      stream: true,
      temperature: '',
      topP: '',
      toolCalling: false,
      imageInput: false,
      editTools: '',
      thinkingType: '',
      thinkingEffort: '',
      thinkingBudgetTokens: '',
    },
  })

  const existingModelIds = new Set(existingModels.map((m) => m.modelId))

  /** 内置模型过滤：先按 providerType 筛选，再按关键词模糊搜索 */
  const filteredBuiltinModels = useMemo(() => {
    let result = builtinModels
    if (builtinTypeFilter !== 'all') {
      result = result.filter((m) => m.providerTypes?.includes(builtinTypeFilter))
    }
    if (builtinModels.length > 6 && builtinSearch.trim()) {
      const q = builtinSearch.toLowerCase()
      result = result.filter(
        (m) =>
          m.id.toLowerCase().includes(q) ||
          m.displayName.toLowerCase().includes(q) ||
          (m.family && m.family.toLowerCase().includes(q))
      )
    }
    return result
  }, [builtinModels, builtinSearch, builtinTypeFilter])

  /** 官方模型模糊搜索过滤（匹配 modelId） */
  const filteredOfficialModels = useMemo(() => {
    if (officialModels.length <= 6 || !officialSearch.trim()) return officialModels
    const q = officialSearch.toLowerCase()
    return officialModels.filter((id) => id.toLowerCase().includes(q))
  }, [officialModels, officialSearch])

  // 加载已添加模型
  const loadExistingModels = async () => {
    if (!provider) return
    setModelsLoading(true)
    try {
      const result = await invokeCommand<GatewayModel[]>('gateway_model_list_by_provider', {
        providerId: provider.id,
      })
      setExistingModels(result)
    } catch {
      setExistingModels([])
    } finally {
      setModelsLoading(false)
    }
  }

  // 加载内置模型列表（默认展示全部，前端再按 providerType 筛选）
  const loadBuiltinModels = async () => {
    if (!provider) return
    setBuiltinLoading(true)
    try {
      const result = await invokeCommand<BuiltinModel[]>('gateway_builtin_models_list')
      setBuiltinModels(result)
    } catch (err) {
      setBuiltinModels([])
      const error = toIcodeError(err)
      toast.error(t('aiGateway.providerForm.modelManagement.loadBuiltinFailed', { code: error.code, message: error.message }))
      void invokeCommand('log_message', {
        level: 'ERROR',
        message: `加载内置模型预设失败: ${error.message}`,
        fileName: 'provider-form.tsx',
      }).catch(() => {})
    } finally {
      setBuiltinLoading(false)
    }
  }

  // 加载全部内置模型（用于官方模型预填充）
  const loadAllBuiltinModels = async () => {
    try {
      const result = await invokeCommand<BuiltinModel[]>('gateway_builtin_models_list')
      setAllBuiltinModels(result)
    } catch (err) {
      setAllBuiltinModels([])
      const error = toIcodeError(err)
      toast.error(t('aiGateway.providerForm.modelManagement.loadBuiltinFailed', { code: error.code, message: error.message }))
      void invokeCommand('log_message', {
        level: 'ERROR',
        message: `加载内置模型预设失败: ${error.message}`,
        fileName: 'provider-form.tsx',
      }).catch(() => {})
    }
  }

  useEffect(() => {
    void loadAllBuiltinModels()
  }, [])

  useEffect(() => {
    if (provider) {
      void loadExistingModels()
      void loadBuiltinModels()
    }
  }, [provider])

  // 拉取官方模型
  const handleFetchOfficialModels = async () => {
    if (!provider) return
    setOfficialLoading(true)
    setOfficialError(null)
    try {
      const models = await invokeCommand<string[]>('gateway_fetch_official_models', {
        providerId: provider.id,
      })
      setOfficialModels(models)
    } catch (err) {
      setOfficialError(String(err))
      setOfficialModels([])
    } finally {
      setOfficialLoading(false)
    }
  }

  // 切换模型公开/隐藏状态
  const handleToggleVisibility = async (model: GatewayModel) => {
    try {
      await invokeCommand<GatewayModel>('gateway_model_update', {
        id: model.id,
        input: { isExposed: !model.isExposed },
      })
      // 更新本地状态
      setExistingModels((prev) =>
        prev.map((m) => (m.id === model.id ? { ...m, isExposed: !m.isExposed } : m))
      )
    } catch (err) {
      toast.error(String(err))
    }
  }

  /** 开始弹窗编辑模型参数 */
  const startEditModel = async (model: GatewayModel) => {
    setEditingGatewayModel(model)

    // 加载关联的 ModelConfig；加载失败则不允许编辑
    try {
      const config = await invokeCommand<ModelConfig>('gateway_model_config_get', { id: model.modelConfigId })
      setEditingModelConfig(config)
      setEditDialogOpen(true)
    } catch {
      toast.error(t('aiGateway.providerForm.modelEdit.loadConfigFailed', '加载模型配置失败'))
      setEditingGatewayModel(null)
    }
  }

  // 弹窗打开时根据当前模型配置回显表单
  useEffect(() => {
    if (editingGatewayModel && editingModelConfig) {
      const caps = parseCapabilitiesForm(editingModelConfig.capabilitiesJson)
      const thinking = parseThinkingForm(editingModelConfig.thinkingJson)
      modelEditForm.reset({
        modelId: editingGatewayModel.modelId,
        displayName: editingGatewayModel.displayName ?? editingModelConfig.name ?? '',
        family: editingGatewayModel.family ?? editingModelConfig.family ?? '',
        maxInputTokens: editingModelConfig.maxInputTokens?.toString() ?? '',
        maxOutputTokens: editingModelConfig.maxOutputTokens?.toString() ?? '',
        tokenizer: editingModelConfig.tokenizer ?? '',
        tokenCountMultiplier: editingModelConfig.tokenCountMultiplier ?? 1,
        pricePer1mTokens: editingModelConfig.pricePer1mTokens?.toString() ?? '',
        stream: editingModelConfig.stream ?? true,
        temperature: editingModelConfig.temperature?.toString() ?? '',
        topP: editingModelConfig.topP?.toString() ?? '',
        ...caps,
        ...thinking,
      })
    }
  }, [editingGatewayModel, editingModelConfig, modelEditForm])

  /** 保存弹窗编辑的模型参数 */
  const saveEditModel = async (values: ModelEditFormValues) => {
    if (!editingGatewayModel || !editingModelConfig) return
    try {
      const displayName = values.displayName.trim() || undefined
      const family = values.family.trim() || undefined

      // 1. 更新 GatewayModel（modelId、displayName、family）
      await invokeCommand<GatewayModel>('gateway_model_update', {
        id: editingGatewayModel.id,
        input: {
          modelId: values.modelId.trim(),
          displayName,
          family,
        },
      })

      // 2. 更新 ModelConfig
      const capabilities = buildCapabilitiesJson(values)
      const thinking = buildThinkingJson(values)
      await invokeCommand<ModelConfig>('gateway_model_config_update', {
        id: editingModelConfig.id,
        input: {
          name: displayName ?? values.modelId.trim(),
          family,
          maxInputTokens: values.maxInputTokens ? Number(values.maxInputTokens) : undefined,
          maxOutputTokens: values.maxOutputTokens ? Number(values.maxOutputTokens) : undefined,
          tokenizer: values.tokenizer || undefined,
          tokenCountMultiplier: values.tokenCountMultiplier,
          pricePer1mTokens: values.pricePer1mTokens ? Number(values.pricePer1mTokens) : undefined,
          stream: values.stream,
          temperature: values.temperature ? Number(values.temperature) : undefined,
          topP: values.topP ? Number(values.topP) : undefined,
          capabilitiesJson: capabilities,
          thinkingJson: thinking,
        },
      })

      // 更新本地状态
      setExistingModels((prev) =>
        prev.map((m) =>
          m.id === editingGatewayModel.id
            ? { ...m, modelId: values.modelId.trim(), displayName, family }
            : m
        )
      )
      setEditDialogOpen(false)
      setEditingGatewayModel(null)
      setEditingModelConfig(null)
      toast.success(t('aiGateway.providerForm.modelEdit.saveSuccess', '模型已保存'))
    } catch (err) {
      toast.error(String(err))
    }
  }

  /** 取消弹窗编辑 */
  const cancelEditModel = () => {
    setEditDialogOpen(false)
    setEditingGatewayModel(null)
    setEditingModelConfig(null)
  }

  // 删除模型
  const handleDeleteModel = async (modelId: string) => {
    try {
      await invokeCommand<void>('gateway_model_delete', { id: modelId })
      setExistingModels((prev) => prev.filter((m) => m.id !== modelId))
      toast.success(t('aiGateway.providerForm.modelManagement.deleteSuccess'))
    } catch (err) {
      toast.error(String(err))
    }
  }

  // 根据模型 ID 在全部内置模型中做匹配，返回首个匹配项
  // 匹配优先级：精确匹配 > 前缀匹配（modelId 以 builtin.id 开头） > 包含匹配
  const findBuiltinByModelId = (modelId: string): BuiltinModel | undefined => {
    const lower = modelId.toLowerCase()

    // 1. 精确匹配
    const exact = allBuiltinModels.find((b) => b.id.toLowerCase() === lower)
    if (exact) return exact

    // 2. modelId 以某个 builtin.id 开头（如 "gpt-4o-2024-01-01" 匹配 "gpt-4o"）
    const prefix = allBuiltinModels.find((b) => lower.startsWith(b.id.toLowerCase()))
    if (prefix) return prefix

    // 3. builtin.id 以 modelId 开头（如 "mimo-v2.5" 匹配 "mimo-v2.5-pro" 时优先选最短的）
    const suffixMatches = allBuiltinModels
      .filter((b) => b.id.toLowerCase().startsWith(lower))
      .sort((a, b) => a.id.length - b.id.length)
    if (suffixMatches.length > 0) return suffixMatches[0]

    // 4. 兜底：双向包含，优先选最短匹配
    const containsMatches = allBuiltinModels
      .filter((b) => {
        const bl = b.id.toLowerCase()
        return lower.includes(bl) || bl.includes(lower)
      })
      .sort((a, b) => a.id.length - b.id.length)
    return containsMatches[0]
  }

  // 快速创建模型配置并关联到供应商
  const addModels = async (
    modelsToAdd: Array<{
      modelId: string
      displayName?: string
      family?: string
      source: string
      builtin?: BuiltinModel
    }>
  ) => {
    if (!provider || modelsToAdd.length === 0) return
    setAddingModels(true)
    let added = 0
    for (const m of modelsToAdd) {
      try {
        const builtin = m.builtin
        // 1. 创建 model_config（优先使用内置预设配置）
        const config = await invokeCommand<ModelConfig>('gateway_model_config_create', {
          input: {
            name: m.displayName ?? m.modelId,
            family: m.family,
            maxInputTokens: builtin?.maxInputTokens,
            maxOutputTokens: builtin?.maxOutputTokens,
            tokenizer: builtin?.tokenizer,
            tokenCountMultiplier: builtin?.tokenCountMultiplier ?? 1,
            stream: builtin?.stream,
            temperature: builtin?.temperature,
            topP: builtin?.topP,
            capabilitiesJson: builtin?.capabilities ? JSON.stringify(builtin.capabilities) : undefined,
            thinkingJson: builtin?.thinking ? JSON.stringify(builtin.thinking) : undefined,
          },
        })
        // 2. 创建 gateway_model
        await invokeCommand<GatewayModel>('gateway_model_create', {
          input: {
            providerId: provider.id,
            modelConfigId: config.id,
            modelId: m.modelId,
            displayName: m.displayName,
            family: m.family,
            isExposed: true,
            source: m.source,
          },
        })
        added++
      } catch (err: any) {
        console.error('Failed to add model:', m.modelId, err)
      }
    }
    if (added > 0) {
      toast.success(t('aiGateway.providerForm.modelAdd.addSuccess', { count: added }))
      void loadExistingModels()
    }
    setAddingModels(false)
  }

  // 从内置模型添加
  const handleAddBuiltinModels = async () => {
    if (selectedBuiltinIds.size === 0) return
    const modelsToAdd = builtinModels
      .filter((m) => selectedBuiltinIds.has(m.id))
      .map((m) => ({
        modelId: m.id,
        displayName: m.displayName,
        family: m.family,
        source: 'builtin' as const,
        builtin: m,
      }))
    await addModels(modelsToAdd)
    setSelectedBuiltinIds(new Set())
  }

  // 从官方模型添加
  const handleAddOfficialModels = async () => {
    if (selectedOfficialIds.size === 0) return
    const modelsToAdd = Array.from(selectedOfficialIds).map((modelId) => {
      const builtin = findBuiltinByModelId(modelId)
      return {
        modelId,
        displayName: builtin?.displayName,
        family: builtin?.family,
        source: 'official' as const,
        builtin,
      }
    })
    await addModels(modelsToAdd)
    setSelectedOfficialIds(new Set())
  }

  if (!provider) {
    return (
      <div className="flex flex-col items-center justify-center py-8 text-sm text-muted-foreground">
        <i className="fa-solid fa-info-circle mb-2 text-lg" />
        <p>{t('aiGateway.providerForm.modelManagement.saveFirst')}</p>
      </div>
    )
  }

  return (
    <div className="space-y-4">
      {/* 已添加的模型列表 */}
      <div>
        <div className="mb-2 flex items-center justify-between">
          <span className="text-xs font-medium">
            {t('aiGateway.providerForm.modelManagement.addedModels', { count: existingModels.length })}
          </span>
          {modelsLoading && <i className="fa-solid fa-spinner fa-spin text-muted-foreground text-xs" />}
        </div>
        <ScrollArea className="h-[160px] rounded-md border">
          {existingModels.length === 0 && !modelsLoading && (
            <p className="text-muted-foreground py-16 text-center text-xs">
              {t('aiGateway.providerForm.modelManagement.noModels')}
            </p>
          )}
          <div className="space-y-1 p-2">
            {existingModels.map((model) => (
              <div key={model.id} className="rounded text-xs">
                {/* 模型行：展示 + 操作 */}
                <div className="flex items-center justify-between px-2 py-1.5 hover:bg-muted/50">
                  <div className="flex min-w-0 flex-1 items-center gap-2">
                    <span className="truncate font-medium">{model.displayName || model.modelId}</span>
                    <span className="text-muted-foreground shrink-0 truncate font-mono text-[10px]">{model.modelId}</span>
                    <Badge variant="outline" className="shrink-0 text-[10px]">
                      {model.source === 'builtin'
                        ? t('aiGateway.providerForm.source.builtin', '内置')
                        : model.source === 'official'
                          ? t('aiGateway.providerForm.source.official', '官方')
                          : t('aiGateway.providerForm.source.manual', '手动')}
                    </Badge>
                  </div>
                  <div className="ml-2 flex shrink-0 items-center gap-1">
                    <button
                      type="button"
                      className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] transition-colors hover:bg-muted"
                      onClick={() => handleToggleVisibility(model)}
                      title={model.isExposed ? t('aiGateway.providerForm.hideModel', '点击隐藏') : t('aiGateway.providerForm.exposeModel', '点击公开')}
                    >
                      <i
                        className={cn(
                          'fa-solid',
                          model.isExposed ? 'fa-eye text-primary' : 'fa-eye-slash text-muted-foreground'
                        )}
                      />
                      <span className={model.isExposed ? 'text-primary' : 'text-muted-foreground'}>
                        {model.isExposed ? t('aiGateway.providerForm.exposed', '公开') : t('aiGateway.providerForm.hidden', '隐藏')}
                      </span>
                    </button>
                    {/* 编辑按钮：弹窗编辑 */}
                    <button
                      type="button"
                      className="rounded px-1 py-0.5 text-[10px] text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                      onClick={() => startEditModel(model)}
                      title={t('aiGateway.providerForm.editModel', '编辑参数')}
                    >
                      <i className="fa-solid fa-pen" />
                    </button>
                    <button
                      type="button"
                      className="rounded px-1 py-0.5 text-[10px] text-destructive transition-colors hover:bg-destructive/10"
                      onClick={() => handleDeleteModel(model.id)}
                      title={t('aiGateway.providerForm.deleteModel', '删除模型')}
                    >
                      <i className="fa-solid fa-trash" />
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </ScrollArea>
      </div>

      {/* 添加模型区域 */}
      <Tabs defaultValue="builtin" className="w-full">
        <TabsList className="mb-3">
          <TabsTrigger value="builtin" className="text-xs">
            <i className="fa-solid fa-book mr-1" data-icon="inline-start" />
            {t('aiGateway.providerForm.modelManagement.builtinModels')}
          </TabsTrigger>
          <TabsTrigger value="official" className="text-xs">
            <i className="fa-solid fa-cloud-arrow-down mr-1" data-icon="inline-start" />
            {t('aiGateway.providerForm.modelManagement.officialModels')}
          </TabsTrigger>
        </TabsList>

        {/* 内置模型列表 */}
        <TabsContent value="builtin">
          {/* 内置模型超过6个时展示搜索框与类型筛选 */}
          {builtinModels.length > 6 && (
            <div className="mb-2 flex items-center gap-2">
              <Select
                value={builtinTypeFilter}
                onValueChange={(v) => setBuiltinTypeFilter(v as 'all' | ProviderType)}
              >
                <SelectTrigger className="h-7 w-[140px] text-xs">
                  <SelectValue placeholder={t('aiGateway.providerForm.modelManagement.type')} />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all" className="text-xs">{t('aiGateway.providerForm.modelManagement.all')}</SelectItem>
                  {providerTypes.map((type) => (
                    <SelectItem key={type} value={type} className="text-xs">
                      {type}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <div className="relative flex-1">
                <i className="fa-solid fa-search text-muted-foreground absolute left-2 top-1/2 -translate-y-1/2 text-[10px]" />
                <Input
                  placeholder={t('aiGateway.providerForm.modelManagement.searchBuiltin')}
                  value={builtinSearch}
                  onChange={(e) => setBuiltinSearch(e.target.value)}
                  className="h-7 pl-6 text-xs"
                />
              </div>
            </div>
          )}
          <ScrollArea className="h-[160px] rounded-md border">
            {builtinLoading ? (
              <div className="flex items-center justify-center py-8">
                <i className="fa-solid fa-spinner fa-spin text-muted-foreground mr-2" />
                <span className="text-xs text-muted-foreground">{t('loading', { ns: 'common' })}</span>
              </div>
            ) : (
              <div className="space-y-1 p-2">
                {filteredBuiltinModels.map((model) => {
                  const alreadyAdded = existingModelIds.has(model.id)
                  const selected = selectedBuiltinIds.has(model.id)
                  return (
                    <div
                      key={model.id}
                      className={cn(
                        'flex items-center justify-between rounded px-2 py-1.5 text-xs',
                        alreadyAdded ? 'opacity-40' : 'hover:bg-muted/50 cursor-pointer'
                      )}
                      onClick={() => {
                        if (alreadyAdded) return
                        setSelectedBuiltinIds((prev) => {
                          const next = new Set(prev)
                          if (next.has(model.id)) next.delete(model.id)
                          else next.add(model.id)
                          return next
                        })
                      }}
                    >
                      <div className="flex items-center gap-2">
                        {!alreadyAdded && (
                          <input
                            type="checkbox"
                            className="size-3 accent-primary"
                            checked={selected}
                            onChange={() => {}}
                          />
                        )}
                        <span className="font-medium">{model.displayName}</span>
                        <Badge variant="outline" className="text-[10px]">{model.id}</Badge>
                      </div>
                      <div className="flex items-center gap-2">
                        {alreadyAdded && <span className="text-[10px] text-muted-foreground">{t('aiGateway.providerForm.modelManagement.added')}</span>}
                        {model.maxInputTokens && (
                          <span className="text-muted-foreground">{Math.round(model.maxInputTokens / 1000)}K ctx</span>
                        )}
                      </div>
                    </div>
                  )
                })}
              </div>
            )}
          </ScrollArea>
          {selectedBuiltinIds.size > 0 && (
            <div className="mt-2 flex justify-end">
              <Button
                type="button"
                size="sm"
                className="h-7 text-xs"
                onClick={handleAddBuiltinModels}
                disabled={addingModels}
              >
                {addingModels ? (
                  <i className="fa-solid fa-spinner fa-spin mr-1.5" />
                ) : (
                  <i className="fa-solid fa-plus mr-1.5" />
                )}
                {t('aiGateway.providerForm.modelManagement.addToProvider', { count: selectedBuiltinIds.size })}
              </Button>
            </div>
          )}
        </TabsContent>

        {/* 官方模型拉取 */}
        <TabsContent value="official">
          <div className="mb-3">
            <Button
              type="button"
              size="sm"
              className="h-7 text-xs"
              onClick={handleFetchOfficialModels}
              disabled={officialLoading}
            >
              <i
                className={cn('fa-solid fa-cloud-arrow-down mr-1.5', officialLoading && 'animate-bounce')}
              />
              {officialLoading ? t('aiGateway.providerForm.modelManagement.fetching') : t('aiGateway.providerForm.modelManagement.fetchOfficial')}
            </Button>
          </div>

          {officialError && (
            <div className="mb-2 rounded-md border border-destructive/30 bg-destructive/5 p-2 text-xs text-destructive">
              <i className="fa-solid fa-circle-exclamation mr-1" />
              {officialError}
            </div>
          )}

          {/* 官方模型超过6个时展示搜索框 */}
          {officialModels.length > 6 && (
            <div className="relative mb-2">
              <i className="fa-solid fa-search text-muted-foreground absolute left-2 top-1/2 -translate-y-1/2 text-[10px]" />
              <Input
                placeholder={t('aiGateway.providerForm.modelManagement.searchOfficial')}
                value={officialSearch}
                onChange={(e) => setOfficialSearch(e.target.value)}
                className="h-7 pl-6 text-xs"
              />
            </div>
          )}

          <ScrollArea className="h-[160px] rounded-md border">
            {officialModels.length === 0 ? (
              <p className="text-muted-foreground py-8 text-center text-xs">
                {officialError ? t('aiGateway.providerForm.modelManagement.fetchFailedHint') : t('aiGateway.providerForm.modelManagement.fetchHint')}
              </p>
            ) : filteredOfficialModels.length === 0 ? (
              <p className="text-muted-foreground py-8 text-center text-xs">
                {t('aiGateway.providerForm.modelManagement.noOfficialMatches')}
              </p>
            ) : (
              <div className="space-y-1 p-2">
                {filteredOfficialModels.map((modelId) => {
                  const alreadyAdded = existingModelIds.has(modelId)
                  const selected = selectedOfficialIds.has(modelId)
                  return (
                    <div
                      key={modelId}
                      className={cn(
                        'flex items-center justify-between rounded px-2 py-1.5 text-xs',
                        alreadyAdded ? 'opacity-40' : 'hover:bg-muted/50 cursor-pointer'
                      )}
                      onClick={() => {
                        if (alreadyAdded) return
                        setSelectedOfficialIds((prev) => {
                          const next = new Set(prev)
                          if (next.has(modelId)) next.delete(modelId)
                          else next.add(modelId)
                          return next
                        })
                      }}
                    >
                      <div className="flex items-center gap-2">
                        {!alreadyAdded && (
                          <input
                            type="checkbox"
                            className="size-3 accent-primary"
                            checked={selected}
                            onChange={() => {}}
                          />
                        )}
                        <span className="font-mono">{modelId}</span>
                        <Badge variant="secondary" className="text-[10px]">{t('aiGateway.providerForm.source.official')}</Badge>
                      </div>
                      {alreadyAdded && <span className="text-[10px] text-muted-foreground">{t('aiGateway.providerForm.modelManagement.added')}</span>}
                    </div>
                  )
                })}
              </div>
            )}
          </ScrollArea>
          {selectedOfficialIds.size > 0 && (
            <div className="mt-2 flex justify-end">
              <Button
                type="button"
                size="sm"
                className="h-7 text-xs"
                onClick={handleAddOfficialModels}
                disabled={addingModels}
              >
                {addingModels ? (
                  <i className="fa-solid fa-spinner fa-spin mr-1.5" />
                ) : (
                  <i className="fa-solid fa-plus mr-1.5" />
                )}
                {t('aiGateway.providerForm.modelManagement.addToProvider', { count: selectedOfficialIds.size })}
              </Button>
            </div>
          )}
        </TabsContent>
      </Tabs>

      {/* 模型编辑弹窗 */}
      <Dialog open={editDialogOpen} onOpenChange={(open) => { if (!open) cancelEditModel() }}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle className="text-base">{t('aiGateway.providerForm.modelEdit.title', '编辑模型')}</DialogTitle>
            <DialogDescription className="text-xs">{t('aiGateway.providerForm.modelEdit.description', '修改模型参数，所有字段均为可选，留空则使用默认值')}</DialogDescription>
          </DialogHeader>
          <form onSubmit={modelEditForm.handleSubmit(saveEditModel)} id="model-edit-form">
            <Tabs defaultValue="basic" className="w-full">
              <TabsList className="mb-3">
                <TabsTrigger value="basic" className="text-xs">{t('aiGateway.providerForm.modelEdit.tabBasic', '基础')}</TabsTrigger>
                <TabsTrigger value="capabilities" className="text-xs">{t('aiGateway.providerForm.modelEdit.tabCapabilities', '能力')}</TabsTrigger>
                <TabsTrigger value="thinking" className="text-xs">{t('aiGateway.providerForm.modelEdit.tabThinking', '思考')}</TabsTrigger>
              </TabsList>
              <ScrollArea className="h-[360px] pr-2">
                <TabsContent value="basic" className="space-y-3">
                  <div className="grid grid-cols-2 gap-3">
                    <div className="space-y-1">
                      <Label className="text-xs" htmlFor="model-edit-modelId">{t('aiGateway.providerForm.modelEdit.modelId', '模型 ID')}</Label>
                      <Input id="model-edit-modelId" {...modelEditForm.register('modelId')} className="h-8 text-xs" />
                      {modelEditForm.formState.errors.modelId && <p className="text-destructive text-[10px]">{t(modelEditForm.formState.errors.modelId.message || '')}</p>}
                    </div>
                    <div className="space-y-1">
                      <Label className="text-xs" htmlFor="model-edit-displayName">{t('aiGateway.providerForm.modelEdit.displayName', '展示名称')}</Label>
                      <Input id="model-edit-displayName" {...modelEditForm.register('displayName')} className="h-8 text-xs" />
                    </div>
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <div className="space-y-1">
                      <Label className="text-xs" htmlFor="model-edit-family">{t('aiGateway.providerForm.modelEdit.family', '模型族')}</Label>
                      <Input id="model-edit-family" {...modelEditForm.register('family')} className="h-8 text-xs" placeholder={t('aiGateway.providerForm.modelEdit.familyPlaceholder')} />
                    </div>
                    <div className="space-y-1">
                      <Label className="text-xs">{t('aiGateway.providerForm.modelEdit.tokenizer', '分词器')}</Label>
                      <Select value={modelEditForm.watch('tokenizer')} onValueChange={(v) => modelEditForm.setValue('tokenizer', v)}>
                        <SelectTrigger className="h-8 text-xs"><SelectValue placeholder={t('aiGateway.providerForm.modelEdit.tokenizerPlaceholder', '不指定')} /></SelectTrigger>
                        <SelectContent>
                          <SelectItem value="" className="text-xs">{t('aiGateway.providerForm.modelEdit.tokenizerNone', '不指定')}</SelectItem>
                          {getTokenizerOptions(t).map((opt) => (<SelectItem key={opt.value} value={opt.value} className="text-xs">{opt.label}</SelectItem>))}
                        </SelectContent>
                      </Select>
                    </div>
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <div className="space-y-1">
                      <Label className="text-xs" htmlFor="model-edit-maxInputTokens">{t('aiGateway.providerForm.modelEdit.maxInputTokens', '最大输入 Token')}</Label>
                      <Input id="model-edit-maxInputTokens" type="number" {...modelEditForm.register('maxInputTokens')} className="h-8 text-xs" />
                    </div>
                    <div className="space-y-1">
                      <Label className="text-xs" htmlFor="model-edit-maxOutputTokens">{t('aiGateway.providerForm.modelEdit.maxOutputTokens', '最大输出 Token')}</Label>
                      <Input id="model-edit-maxOutputTokens" type="number" {...modelEditForm.register('maxOutputTokens')} className="h-8 text-xs" />
                    </div>
                  </div>
                  <div className="grid grid-cols-3 gap-3">
                    <div className="space-y-1">
                      <Label className="text-xs" htmlFor="model-edit-tokenCountMultiplier">{t('aiGateway.providerForm.modelEdit.tokenCountMultiplier', '成本倍率')}</Label>
                      <Input id="model-edit-tokenCountMultiplier" type="number" step="0.1" {...modelEditForm.register('tokenCountMultiplier', { valueAsNumber: true })} className="h-8 text-xs" />
                    </div>
                    <div className="space-y-1">
                      <Label className="text-xs" htmlFor="model-edit-pricePer1mTokens">{t('aiGateway.providerForm.modelEdit.pricePer1mTokens', i18n.language === 'zh-CN' ? '¥ / 百万 tokens' : '$ / million tokens')}</Label>
                      <Input id="model-edit-pricePer1mTokens" type="number" step="0.0001" {...modelEditForm.register('pricePer1mTokens')} className="h-8 text-xs" />
                    </div>
                    <div className="flex items-end pb-1.5">
                      <label className="flex items-center gap-2 text-xs">
                        <Switch checked={modelEditForm.watch('stream')} onCheckedChange={(v) => modelEditForm.setValue('stream', v)} />
                        {t('aiGateway.providerForm.modelEdit.stream', '流式')}
                      </label>
                    </div>
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <div className="space-y-1">
                      <Label className="text-xs" htmlFor="model-edit-temperature">{t('aiGateway.providerForm.modelEdit.temperature', 'Temperature')}</Label>
                      <Input id="model-edit-temperature" type="number" step="0.1" {...modelEditForm.register('temperature')} className="h-8 text-xs" />
                    </div>
                    <div className="space-y-1">
                      <Label className="text-xs" htmlFor="model-edit-topP">{t('aiGateway.providerForm.modelEdit.topP', 'Top P')}</Label>
                      <Input id="model-edit-topP" type="number" step="0.1" {...modelEditForm.register('topP')} className="h-8 text-xs" />
                    </div>
                  </div>
                </TabsContent>
                <TabsContent value="capabilities" className="space-y-3">
                  <div className="space-y-2">
                    <label className="flex items-center justify-between rounded border p-2 text-xs">
                      <span>{t('aiGateway.providerForm.modelEdit.toolCalling', '工具调用')}</span>
                      <Switch checked={modelEditForm.watch('toolCalling')} onCheckedChange={(v) => modelEditForm.setValue('toolCalling', v)} />
                    </label>
                    <label className="flex items-center justify-between rounded border p-2 text-xs">
                      <span>{t('aiGateway.providerForm.modelEdit.imageInput', '图片输入')}</span>
                      <Switch checked={modelEditForm.watch('imageInput')} onCheckedChange={(v) => modelEditForm.setValue('imageInput', v)} />
                    </label>
                    <div className="space-y-1">
                      <Label className="text-xs">{t('aiGateway.providerForm.modelEdit.editTools', '编辑器工具')}</Label>
                      <Select value={modelEditForm.watch('editTools')} onValueChange={(v) => modelEditForm.setValue('editTools', v as ModelEditFormValues['editTools'])}>
                        <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
                        <SelectContent>
                          {getEditToolOptions(t).map((opt) => (<SelectItem key={opt.value} value={opt.value} className="text-xs">{opt.label}</SelectItem>))}
                        </SelectContent>
                      </Select>
                    </div>
                  </div>
                </TabsContent>
                <TabsContent value="thinking" className="space-y-3">
                  <div className="space-y-2">
                    <div className="space-y-1">
                      <Label className="text-xs">{t('aiGateway.providerForm.modelEdit.thinkingType', '思考模式')}</Label>
                      <Select value={modelEditForm.watch('thinkingType')} onValueChange={(v) => modelEditForm.setValue('thinkingType', v as ModelEditFormValues['thinkingType'])}>
                        <SelectTrigger className="h-8 text-xs"><SelectValue placeholder={t('aiGateway.providerForm.modelEdit.thinkingTypePlaceholder', '不启用')} /></SelectTrigger>
                        <SelectContent>
                          {getThinkingTypeOptions(t).map((opt) => (<SelectItem key={opt.value} value={opt.value} className="text-xs">{opt.label}</SelectItem>))}
                        </SelectContent>
                      </Select>
                    </div>
                    <div className="grid grid-cols-2 gap-3">
                      <div className="space-y-1">
                        <Label className="text-xs" htmlFor="model-edit-thinkingEffort">{t('aiGateway.providerForm.modelEdit.thinkingEffort', '努力程度')}</Label>
                        <Input id="model-edit-thinkingEffort" {...modelEditForm.register('thinkingEffort')} className="h-8 text-xs" />
                      </div>
                      <div className="space-y-1">
                        <Label className="text-xs" htmlFor="model-edit-thinkingBudgetTokens">{t('aiGateway.providerForm.modelEdit.thinkingBudgetTokens', '预算 Token')}</Label>
                        <Input id="model-edit-thinkingBudgetTokens" type="number" {...modelEditForm.register('thinkingBudgetTokens')} className="h-8 text-xs" />
                      </div>
                    </div>
                  </div>
                </TabsContent>
              </ScrollArea>
            </Tabs>
          </form>
          <DialogFooter>
            <Button type="button" variant="ghost" size="sm" className="h-8 text-xs" onClick={cancelEditModel}>{t('common.cancel')}</Button>
            <Button type="submit" form="model-edit-form" size="sm" className="h-8 text-xs">{t('common.save')}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}

// ===== OAuth 授权 UI 子组件 =====

interface OAuthAuthorizeSectionProps {
  provider: Provider
  authMethod: AuthMethod
  authorizing: boolean
  timedOut: boolean
  remainingSeconds: number
  deviceCodeInfo: DeviceCodeInfo | null
  deviceCodePolling: boolean
  showManualInput: boolean
  manualCode: string
  manualExchanging: boolean
  onStartAuthorize: () => void
  onStopDeviceCode: () => void
  onManualCodeChange: (code: string) => void
  onCompleteWithCode: () => void
  onCancelManualExchange: () => void
}

/**
 * OAuth 授权操作区
 *
 * - 浏览器授权码流程：展示授权按钮与当前授权状态，120s 倒计时后允许重新授权；
 *   若自动回调超时，提供手动输入授权码的备选方案
 * - Device Code 流程：展示用户码、验证 URL，并支持打开浏览器与停止轮询
 */
function OAuthAuthorizeSection({
  provider,
  authMethod,
  authorizing,
  timedOut,
  remainingSeconds,
  deviceCodeInfo,
  deviceCodePolling,
  showManualInput,
  manualCode,
  manualExchanging,
  onStartAuthorize,
  onStopDeviceCode,
  onManualCodeChange,
  onCompleteWithCode,
  onCancelManualExchange,
}: OAuthAuthorizeSectionProps) {
  const { t } = useTranslation()
  const isDeviceCode = authMethod === 'github-copilot'
  const authConfig = useMemo(() => parseAuthConfig(provider), [provider])
  const { hasToken, expiresAt, isExpired, hasExpiry, githubLogin, email } = useMemo(() => {
    if (!authConfig || authConfig.method === 'none' || authConfig.method === 'api-key') {
      return { hasToken: false, expiresAt: undefined as number | undefined, isExpired: false, hasExpiry: false, githubLogin: undefined as string | undefined, email: undefined as string | undefined }
    }
    if (!('token' in authConfig) || !authConfig.token) {
      return { hasToken: false, expiresAt: undefined as number | undefined, isExpired: false, hasExpiry: false, githubLogin: undefined as string | undefined, email: undefined as string | undefined }
    }
    const exp = 'expiresAt' in authConfig ? authConfig.expiresAt : undefined
    const hasExp = exp !== undefined && exp !== null && typeof exp === 'number'
    const expired = hasExp ? exp! * 1000 < Date.now() : false
    const gl = 'githubLogin' in authConfig ? authConfig.githubLogin : undefined
    const em = 'email' in authConfig ? authConfig.email : undefined
    return { hasToken: true, expiresAt: exp, isExpired: expired, hasExpiry: hasExp, githubLogin: gl, email: em }
  }, [authConfig])

  const verificationUrl = deviceCodeInfo?.verificationUriComplete || deviceCodeInfo?.verificationUri

  return (
    <div className="mt-4 rounded-md border p-3 space-y-3">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1">
          <span className="text-xs font-medium">{t('aiGateway.providerForm.oauthTitle')}</span>
          <HelpIcon type="popover" side="bottom" align="start" contentClassName="max-w-sm text-xs">
            <div className="space-y-1.5">
              <p className="font-medium">{t('aiGateway.callbackServer.helpTitle')}</p>
              <p className="whitespace-pre-line text-muted-foreground">{t('aiGateway.callbackServer.helpContent')}</p>
            </div>
          </HelpIcon>
        </div>
        {hasToken && (
          isExpired ? (
            <Badge variant="outline" className="text-[10px] text-amber-600 border-amber-200 bg-amber-50 dark:bg-amber-950/30">
              <i className="fa-solid fa-clock mr-1" />
              {t('aiGateway.providerForm.oauthExpired')}
            </Badge>
          ) : (
            <Badge variant="outline" className="text-[10px] text-green-600 border-green-200 bg-green-50 dark:bg-green-950/30">
              <i className="fa-solid fa-check mr-1" />
              {t('aiGateway.providerForm.oauthAuthorized')}
            </Badge>
          )
        )}
      </div>

      {/* 过期时间展示 */}
      {hasToken && (
        <div className="text-[11px] text-muted-foreground flex items-center gap-1.5">
          {hasExpiry ? (
            <>
              <i className="fa-regular fa-clock text-[10px]" />
              <span>{t('aiGateway.providerForm.oauthExpiresAt')}:</span>
              <span className={isExpired ? 'text-amber-600 font-medium' : 'text-foreground/80'}>
                {formatDateTime(new Date(expiresAt! * 1000).toISOString())}
              </span>
              {isExpired && (
                <span className="text-amber-600">({t('aiGateway.providerForm.oauthExpired')})</span>
              )}
            </>
          ) : (
            <>
              <i className="fa-regular fa-clock text-[10px]" />
              <span>{t('aiGateway.providerForm.oauthExpiryNotProvided')}</span>
            </>
          )}
        </div>
      )}

      {/* GitHub 账户信息展示 */}
      {hasToken && isDeviceCode && (githubLogin || email) && (
        <div className="text-[11px] text-muted-foreground flex items-center gap-1.5 flex-wrap">
          {githubLogin && (
            <>
              <i className="fa-brands fa-github text-[10px]" />
              <span>{t('aiGateway.providerForm.oauthGithubAccount')}:</span>
              <span className="text-foreground/80 font-medium">{githubLogin}</span>
            </>
          )}
          {githubLogin && email && <span className="text-muted-foreground/50">·</span>}
          {email && (
            <>
              <i className="fa-regular fa-envelope text-[10px]" />
              <span className="text-foreground/80">{email}</span>
            </>
          )}
        </div>
      )}

      {!isDeviceCode ? (
        <div className="space-y-2">
          <p className="text-[11px] text-muted-foreground">
            {showManualInput
              ? timedOut
                ? t('aiGateway.providerForm.oauthAuthorizeTimeout')
                : t('aiGateway.providerForm.oauthManualHint')
              : t('aiGateway.providerForm.oauthAuthorizeHint')}
          </p>
          {!showManualInput && (
            <Button
              type="button"
              size="sm"
              variant="secondary"
              className="h-8 text-xs"
              onClick={onStartAuthorize}
              disabled={authorizing}
            >
              {authorizing ? (
                <i className="fa-solid fa-spinner fa-spin mr-1.5" />
              ) : (
                <i className={`fa-solid ${hasToken || timedOut ? 'fa-rotate' : 'fa-external-link-alt'} mr-1.5`} />
              )}
              {authorizing
                ? t('aiGateway.providerForm.oauthAuthorizingWithCountdown', { seconds: remainingSeconds })
                : hasToken || timedOut
                  ? t('aiGateway.providerForm.oauthRetryAuthorize')
                  : t('aiGateway.providerForm.oauthAuthorize')}
            </Button>
          )}
          {showManualInput && (
            <div className="space-y-2">
              <div className="space-y-1.5">
                <Label className="text-xs">{t('aiGateway.providerForm.oauthManualCodeLabel')}</Label>
                <Input
                  value={manualCode}
                  onChange={(e) => onManualCodeChange(e.target.value)}
                  placeholder={t('aiGateway.providerForm.oauthManualCodePlaceholder')}
                  className="h-8 text-xs font-mono"
                />
                <p className="text-[10px] text-muted-foreground">{t('aiGateway.providerForm.oauthManualCodeHint')}</p>
              </div>
              <div className="flex flex-wrap gap-2">
                {manualExchanging ? (
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    className="h-8 text-xs"
                    onClick={onCancelManualExchange}
                  >
                    <i className="fa-solid fa-xmark mr-1.5" />
                    {t('aiGateway.providerForm.oauthManualCancel')}
                  </Button>
                ) : (
                  <Button
                    type="button"
                    size="sm"
                    variant="secondary"
                    className="h-8 text-xs"
                    onClick={onCompleteWithCode}
                    disabled={!manualCode.trim()}
                  >
                    <i className="fa-solid fa-check mr-1.5" />
                    {t('aiGateway.providerForm.oauthManualSubmit')}
                  </Button>
                )}
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="h-8 text-xs"
                  onClick={onStartAuthorize}
                  disabled={authorizing || manualExchanging}
                >
                  <i className="fa-solid fa-rotate mr-1.5" />
                  {t('aiGateway.providerForm.oauthRetryAuthorize')}
                </Button>
              </div>
            </div>
          )}
        </div>
      ) : (
        <div className="space-y-2">
          {!deviceCodeInfo ? (
            <>
              <p className="text-[11px] text-muted-foreground">{t('aiGateway.providerForm.oauthDeviceCodeHint')}</p>
              <Button
                type="button"
                size="sm"
                variant="secondary"
                className="h-8 text-xs"
                onClick={onStartAuthorize}
                disabled={authorizing}
              >
                {authorizing ? (
                  <i className="fa-solid fa-spinner fa-spin mr-1.5" />
                ) : (
                  <i className="fa-solid fa-keyboard mr-1.5" />
                )}
                {authorizing
                  ? t('aiGateway.providerForm.oauthRequestingDeviceCode')
                  : t('aiGateway.providerForm.oauthRequestDeviceCode')}
              </Button>
            </>
          ) : (
            <div className="space-y-2">
              <div className="rounded bg-muted p-2">
                <div className="text-[11px] text-muted-foreground">{t('aiGateway.providerForm.oauthUserCode')}</div>
                <div className="font-mono text-sm font-semibold tracking-wide">{deviceCodeInfo.userCode}</div>
              </div>
              <div className="flex flex-wrap gap-2">
                <Button
                  type="button"
                  size="sm"
                  variant="secondary"
                  className="h-8 text-xs"
                  onClick={() => {
                    if (verificationUrl) void open(verificationUrl)
                  }}
                >
                  <i className="fa-solid fa-external-link-alt mr-1.5" />
                  {t('aiGateway.providerForm.oauthOpenVerificationUrl')}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="h-8 text-xs"
                  onClick={onStopDeviceCode}
                >
                  {t('aiGateway.providerForm.oauthStopPolling')}
                </Button>
              </div>
              {deviceCodePolling && (
                <p className="text-[11px] text-muted-foreground">
                  <i className="fa-solid fa-spinner fa-spin mr-1" />
                  {t('aiGateway.providerForm.oauthPollingDeviceCode')}
                </p>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

/** 本地 cn 工具，避免循环依赖 */
function cn(...inputs: (string | boolean | undefined | null)[]): string {
  return inputs.filter(Boolean).join(' ')
}
