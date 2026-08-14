import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { invokeCommand } from '@/hooks/use-command'
import { toIcodeError } from '@/core/errors'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ScrollPage } from '@/components/ui/scroll-page'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { useTranslation } from '@/modules/i18n/use-translation'
import type { CliConfigFileContent, CliProfile } from '@/modules/cli-management/types'
import type { BuiltinModel, ProviderShareConfig, ProviderType } from '@/modules/ai-gateway/types'
import { exportProvider } from '@/hooks/use-ai-gateway-mutation'
import type { CatalogProvider } from '@/modules/gateway-runtime/types'
import { resolveDefaultGatewayKey } from '@/hooks/use-catalog'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  OpenCodeAgentDialog,
  type OpenCodeAgent,
  type OpenCodeAgents,
} from '@/modules/cli-management/ui/opencode-agent-dialog'

// ── OpenCode Provider 与 Model 类型定义 ──

/** OpenCode 模型配置 */
interface OpenCodeModel {
  name: string
  limit?: { context: number; output: number }
  modalities?: { input: string[]; output: string[] }
}

/** OpenCode Provider 配置（对应 opencode.json 的 provider 条目） */
interface OpenCodeProvider {
  npm: string
  name: string
  options: {
    baseURL?: string
    apiKey?: string
    [key: string]: unknown
  }
  models: Record<string, OpenCodeModel>
}

/** Oh-My-OpenAgent 配置预设（当前版本暂不展示） */
// interface OpenAgentConfig {
//   id: string
//   name: string
//   agentCount: number
//   isApplied: boolean
// }

/** opencode.json 顶层结构
 *
 * 实际文件中 `providers` 是一个数组，数组每项是 `{ providerId: providerInfo }` 的单键对象。
 * `agent` 字段为 Agent 配置映射，键名为 Agent ID，值为 Agent 配置对象。
 */
interface OpenCodeConfig {
  model?: string,
  plugins?: string[],
  provider?: Record<string, OpenCodeProvider>[] | Record<string, OpenCodeProvider>
  agent?: OpenCodeAgents
}

/** Provider 编辑表单数据 */
interface ProviderFormData {
  id: string
  npm: string
  name: string
  baseURL: string
  apiKey: string
  models: Record<string, OpenCodeModel>
}

/** Model 编辑表单数据 */
interface ModelFormData {
  id: string
  name: string
  contextLimit: string
  outputLimit: string
  inputModalities: string[]
  outputModalities: string[]
}

/** 掩码显示 API Key */
function maskApiKey(key?: string): string {
  if (!key) return '—'
  if (key.length <= 8) return '••••••••'
  return key.slice(0, 4) + '••••' + key.slice(-4)
}

/** 生成唯一 ID（当前版本由 Oh-My-OpenAgent 使用，暂不展示） */
// function uid(): string {
//   return Date.now().toString(36) + Math.random().toString(36).slice(2, 8)
// }

/** 根据 Gateway 供应商类型推荐 OpenCode npm 适配器 */
function getNpmAdapterForProviderType(providerType: ProviderType): string {
  if (providerType === 'anthropic' || providerType === 'claude-code') {
    return '@ai-sdk/anthropic'
  }
  return '@ai-sdk/openai-compatible'
}

/** 判断 npm 字段是否为内置适配器 */
function isBuiltinNpmAdapter(npm: string): boolean {
  return npm === '@ai-sdk/openai-compatible' || npm === '@ai-sdk/anthropic'
}

export interface OpenCodePanelProps {
  profile?: CliProfile
  height: number
}

/**
 * OpenCode CLI 专属面板
 *
 * 两大区域：
 * 1. Provider 配置（opencode.json 的 provider 段）
 * 2. Oh-My-OpenAgent 配置管理
 */
export function OpenCodePanel({ profile, height }: OpenCodePanelProps) {
  const { t } = useTranslation()

  // ── Provider 数据（本地状态） ──
  const [providers, setProviders] = useState<Record<string, OpenCodeProvider>>({})
  const [primaryModel, setPrimaryModel] = useState<string>('')
  const [expandedProviders, setExpandedProviders] = useState<Record<string, boolean>>({})
  const [configLoading, setConfigLoading] = useState(false)

  // 保留原始 opencode.json 中的其它字段（如 plugins），避免保存时丢失
  const [baseConfig, setBaseConfig] = useState<OpenCodeConfig | null>(null)
  // 记录原始 provider 是数组还是对象，回写时保持相同格式
  const [providerIsArray, setProviderIsArray] = useState(false)
  // 配置文件是否存在；不存在时提示初始化
  const [fileExists, setFileExists] = useState(true)
  const [initDialogOpen, setInitDialogOpen] = useState(false)
  // 是否有未保存的本地改动
  const isDirtyRef = useRef(false)

  // ── Agent 配置（本地状态，从 opencode.json 的 agent 字段解析） ──
  const [agents, setAgents] = useState<OpenCodeAgents>({})
  const [agentDialogOpen, setAgentDialogOpen] = useState(false)

  // ── Oh-My-OpenAgent 配置（本地状态，初始为空）── 当前版本暂不展示 ──
  // const [agentConfigs, setAgentConfigs] = useState<OpenAgentConfig[]>([])
  // const [appliedConfigId, setAppliedConfigId] = useState<string | null>(null)

  /** 从后端读取真实 opencode.json，解析 providers 与主模型 */
  useEffect(() => {
    if (!profile?.id) return

    const loadConfig = async () => {
      setConfigLoading(true)
      try {
        const result = await invokeCommand<{ content: string }>('cli_config_read', {
          cliType: 'opencode',
          configuredPath: profile.configFilePath,
        })
        const parsed = JSON.parse(result.content) as OpenCodeConfig
        console.log("获取opencode.json配置", parsed)
        const normalizedProviders: Record<string, OpenCodeProvider> = {}
        if (Array.isArray(parsed.provider)) {
          setProviderIsArray(true)
          for (const item of parsed.provider) {
            if (item && typeof item === 'object') {
              Object.assign(normalizedProviders, item)
            }
          }
        } else if (parsed.provider && typeof parsed.provider === 'object') {
          setProviderIsArray(false)
          Object.assign(normalizedProviders, parsed.provider)
        }
        setBaseConfig(parsed)
        setFileExists(true)
        setProviders(normalizedProviders)
        setPrimaryModel(parsed.model ?? '')
        // 解析 agent 字段：若文件中没有则为空对象
        const parsedAgents: OpenCodeAgents = {}
        if (parsed.agent && typeof parsed.agent === 'object' && !Array.isArray(parsed.agent)) {
          for (const [agentKey, agentVal] of Object.entries(parsed.agent)) {
            if (agentVal && typeof agentVal === 'object') {
              parsedAgents[agentKey] = agentVal as OpenCodeAgent
            }
          }
        }
        setAgents(parsedAgents)
        // 默认展开第一个 Provider
        const firstProviderId = Object.keys(normalizedProviders)[0]
        setExpandedProviders(firstProviderId ? { [firstProviderId]: true } : {})
        isDirtyRef.current = false
      } catch (err) {
        const error = toIcodeError(err)
        // 文件不存在时提示初始化，而非静默空状态
        if (error.code === 'NOT_FOUND') {
          setBaseConfig({ provider: [] })
          setProviderIsArray(true)
          setFileExists(false)
          setInitDialogOpen(true)
          setProviders({})
          setPrimaryModel('')
          setAgents({})
          setExpandedProviders({})
          isDirtyRef.current = false
        } else {
          toast.error(`读取 opencode.json 失败: ${error.message}`)
        }
      } finally {
        setConfigLoading(false)
      }
    }

    loadConfig()
  }, [profile?.id, profile?.configFilePath])

  // ── 自动保存到 opencode.json ──

  /** 根据当前本地状态构造待写入的 opencode.json 对象 */
  const buildOpenCodeConfig = useCallback(
    (
      currentProviders: Record<string, OpenCodeProvider>,
      currentPrimaryModel: string,
      currentAgents?: OpenCodeAgents
    ): OpenCodeConfig => {
      const providerValue = providerIsArray
        ? Object.entries(currentProviders).map(([id, p]) => ({ [id]: p }))
        : currentProviders
      // agent 字段：传入则覆盖，否则保留 baseConfig 中的原值
      const agentValue = currentAgents !== undefined ? currentAgents : baseConfig?.agent
      return {
        ...baseConfig,
        model: currentPrimaryModel || baseConfig?.model,
        provider: providerValue,
        ...(agentValue && Object.keys(agentValue).length > 0 ? { agent: agentValue } : {}),
      } as OpenCodeConfig
    },
    [baseConfig, providerIsArray]
  )

  /** 将当前配置写回 opencode.json */
  const saveConfig = useCallback(async () => {
    if (!profile?.id || configLoading || !isDirtyRef.current) return
    try {
      const nextConfig = buildOpenCodeConfig(providers, primaryModel, agents)
      const content = JSON.stringify(nextConfig, null, 2)
      await invokeCommand<CliConfigFileContent>('cli_config_save', {
        cliType: 'opencode',
        configuredPath: profile.configFilePath,
        content,
      })
      setBaseConfig(nextConfig)
      isDirtyRef.current = false
    } catch (err) {
      const error = toIcodeError(err)
      toast.error(t('cli.opencode.saveConfigFailed', { message: error.message }))
    }
  }, [profile, configLoading, providers, primaryModel, agents, buildOpenCodeConfig, t])

  /** 有未保存改动时自动写回文件 */
  useEffect(() => {
    if (!profile?.id || configLoading || !fileExists) return
    if (!isDirtyRef.current) return
    const timer = setTimeout(() => {
      void saveConfig()
    }, 300)
    return () => clearTimeout(timer)
  }, [profile?.id, configLoading, fileExists, providers, primaryModel, agents, saveConfig])

  /** 配置文件不存在时，初始化创建 opencode.json */
  const handleInitConfirm = useCallback(async () => {
    if (!profile?.id) return
    try {
      const nextConfig = buildOpenCodeConfig(providers, primaryModel, agents)
      const content = JSON.stringify(nextConfig, null, 2)
      await invokeCommand<CliConfigFileContent>('cli_config_save', {
        cliType: 'opencode',
        configuredPath: profile.configFilePath,
        content,
      })
      setBaseConfig(nextConfig)
      setFileExists(true)
      setInitDialogOpen(false)
      isDirtyRef.current = false
      toast.success(t('cli.opencode.saveConfigSuccess'))
    } catch (err) {
      const error = toIcodeError(err)
      toast.error(t('cli.opencode.saveConfigFailed', { message: error.message }))
    }
  }, [profile, providers, primaryModel, agents, buildOpenCodeConfig, t])

  // ── 弹窗状态 ──
  const [providerDialogOpen, setProviderDialogOpen] = useState(false)
  const [editingProviderId, setEditingProviderId] = useState<string | null>(null)
  const [providerForm, setProviderForm] = useState<ProviderFormData>({
    id: '',
    npm: '',
    name: '',
    baseURL: '',
    apiKey: '',
    models: {},
  })

  // ── 已创建的 Gateway Provider（用于快速导入到 OpenCode） ──
  const [gatewayProviders, setGatewayProviders] = useState<CatalogProvider[]>([])
  const [gatewayProvidersLoading, setGatewayProvidersLoading] = useState(false)
  const [selectedGatewayProviderId, setSelectedGatewayProviderId] = useState<string>('')

  const [modelDialogOpen, setModelDialogOpen] = useState(false)
  const [modelDialogProviderId, setModelDialogProviderId] = useState<string | null>(null)
  const [editingModelId, setEditingModelId] = useState<string | null>(null)
  const [modelForm, setModelForm] = useState<ModelFormData>({
    id: '',
    name: '',
    contextLimit: '',
    outputLimit: '',
    inputModalities: ['text'],
    outputModalities: ['text'],
  })

  // ── 内置模型预设（用于添加模型时快速填充） ──
  const [builtinModels, setBuiltinModels] = useState<BuiltinModel[]>([])
  const [builtinModelsLoading, setBuiltinModelsLoading] = useState(false)
  const [selectedBuiltinModelId, setSelectedBuiltinModelId] = useState<string>('')

  // Oh-My-OpenAgent 弹窗状态（当前版本暂不展示）
  // const [agentDialogOpen, setAgentDialogOpen] = useState(false)
  // const [editingAgentId, setEditingAgentId] = useState<string | null>(null)
  // const [agentFormName, setAgentFormName] = useState('')

  // ── Provider 配置导入/导出弹窗状态 ──
  const [exportDialogOpen, setExportDialogOpen] = useState(false)
  const [exportedData, setExportedData] = useState('')
  const [importDialogOpen, setImportDialogOpen] = useState(false)
  const [importData, setImportData] = useState('')
  const [importLoading, setImportLoading] = useState(false)

  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<{
    type: 'provider' | 'model' | 'agent'
    providerId?: string
    modelId?: string
    agentId?: string
  } | null>(null)

  // ── Provider 操作 ──

  /** 切换折叠状态 */
  const toggleExpand = useCallback((providerId: string) => {
    setExpandedProviders((prev) => ({ ...prev, [providerId]: !prev[providerId] }))
  }, [])

  /** 导出全部 Provider 配置为 base64 */
  const handleExportProviders = useCallback(() => {
    try {
      const payload = JSON.stringify(providers, null, 2)
      const base64 = btoa(unescape(encodeURIComponent(payload)))
      setExportedData(base64)
      setExportDialogOpen(true)
      toast.success(t('cli.opencode.exportProvidersSuccess'))
    } catch (err) {
      const error = toIcodeError(err)
      toast.error(t('cli.opencode.exportProvidersFailed', { message: error.message }))
    }
  }, [providers, t])

  /** 复制导出内容到剪贴板 */
  const handleCopyExport = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(exportedData)
      toast.success(t('cli.opencode.exportCopied'))
    } catch {
      toast.error(t('cli.opencode.exportCopyFailed'))
    }
  }, [exportedData, t])

  /** 从 base64 导入全部 Provider 配置 */
  const handleImportProviders = useCallback(async () => {
    const trimmed = importData.trim()
    if (!trimmed) {
      toast.error(t('cli.opencode.importProvidersEmptyError'))
      return
    }
    setImportLoading(true)
    try {
      let decoded: string
      try {
        decoded = decodeURIComponent(escape(atob(trimmed)))
      } catch {
        toast.error(t('cli.opencode.importProvidersDecodeError'))
        return
      }
      const parsed = JSON.parse(decoded) as unknown
      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
        toast.error(t('cli.opencode.importProvidersFormatError'))
        return
      }
      const importedProviders = parsed as Record<string, OpenCodeProvider>
      for (const [id, provider] of Object.entries(importedProviders)) {
        if (!id.trim() || !provider || typeof provider !== 'object' || !provider.npm || !provider.name) {
          toast.error(t('cli.opencode.importProvidersInvalidEntry', { id }))
          return
        }
      }
      setProviders(importedProviders)
      setPrimaryModel('')
      setExpandedProviders({})
      isDirtyRef.current = true
      toast.success(t('cli.opencode.importProvidersSuccess'))
      setImportDialogOpen(false)
      setImportData('')
    } catch (err) {
      const error = toIcodeError(err)
      toast.error(t('cli.opencode.importProvidersFailed', { message: error.message }))
    } finally {
      setImportLoading(false)
    }
  }, [importData, t])

  /** 加载目录供应商列表（真实 + 生效虚拟供应商） */
  const loadGatewayProviders = useCallback(async () => {
    setGatewayProvidersLoading(true)
    try {
      const result = await invokeCommand<CatalogProvider[]>('gateway_catalog_providers')
      setGatewayProviders(result)
    } catch (err) {
      setGatewayProviders([])
      const error = toIcodeError(err)
      toast.error(t('cli.opencode.loadGatewayProvidersFailed', { message: error.message }))
    } finally {
      setGatewayProvidersLoading(false)
    }
  }, [t])

  /**
   * 从选中的 Gateway Provider 导入配置到 OpenCode Provider 表单
   *
   * 真实供应商：通过 gateway_provider_export 获取完整配置。
   * 虚拟供应商：直接从目录条目构造基础配置（无独立模型，apiKey 用网关默认 Key）。
   */
  const applyGatewayProviderToForm = useCallback(async (providerId: string) => {
    setSelectedGatewayProviderId(providerId)
    if (!providerId) {
      // 选择「手动创建」时重置表单，保留用户当前输入不被覆盖
      return
    }

    // 虚拟供应商处理
    const selected = gatewayProviders.find((p) => p.id === providerId)
    if (selected?.isVirtual) {
      let apiKey = ''
      try {
        const key = await resolveDefaultGatewayKey()
        if (key) apiKey = key
      } catch {
        // 无默认 Key 时 apiKey 留空
      }
      setProviderForm({
        id: selected.slug,
        name: selected.displayName,
        npm: 'openai', // 虚拟供应商统一走 OpenAI 兼容协议
        baseURL: 'http://127.0.0.1:54321',
        apiKey,
        models: {},
      })
      toast.success(t('cli.opencode.importProviderSuccess', { name: selected.displayName }))
      return
    }

    try {
      const exportedBase64 = await exportProvider({
        providerId,
        includeSecrets: true,
      })
      const decoded = JSON.parse(atob(exportedBase64)) as ProviderShareConfig
      const exportedProvider = decoded.provider

      // 从已解密的 authJson 中提取 API Key
      let apiKey = ''
      if (exportedProvider.authJson) {
        const authConfig = JSON.parse(exportedProvider.authJson) as { method?: string; apiKey?: string }
        if (authConfig.method === 'api-key' && authConfig.apiKey) {
          apiKey = authConfig.apiKey
        }
      }

      // 将导出的模型列表转换为 OpenCode 模型结构
      const importedModels: Record<string, OpenCodeModel> = {}
      for (const item of decoded.models) {
        const modelId = item.gatewayModel.modelId
        const contextLimit = item.modelConfig.maxInputTokens
        const outputLimit = item.modelConfig.maxOutputTokens
        const inputModalities: string[] = ['text']
        if (item.modelConfig.capabilitiesJson) {
          try {
            const caps = JSON.parse(item.modelConfig.capabilitiesJson) as { imageInput?: boolean }
            if (caps.imageInput) {
              inputModalities.push('image')
            }
          } catch {
            // 忽略 capabilities 解析失败，默认仅 text
          }
        }
        importedModels[modelId] = {
          name: item.gatewayModel.displayName || item.modelConfig.name || modelId,
          limit:
            (contextLimit && contextLimit > 0) || (outputLimit && outputLimit > 0)
              ? { context: contextLimit || 0, output: outputLimit || 0 }
              : undefined,
          modalities: { input: inputModalities, output: ['text'] },
        }
      }

      setProviderForm({
        id: exportedProvider.slug,
        name: exportedProvider.displayName,
        npm: getNpmAdapterForProviderType(exportedProvider.providerType as ProviderType),
        baseURL: exportedProvider.baseUrl,
        apiKey,
        models: importedModels,
      })
      toast.success(t('cli.opencode.importProviderSuccess', { name: exportedProvider.displayName }))
    } catch (err) {
      const error = toIcodeError(err)
      toast.error(t('cli.opencode.importProviderFailed', { message: error.message }))
    }
  }, [t, gatewayProviders])

  /** 打开添加 Provider 弹窗 */
  const openAddProvider = useCallback(() => {
    setEditingProviderId(null)
    setSelectedGatewayProviderId('')
    setProviderForm({ id: '', npm: '', name: '', baseURL: '', apiKey: '', models: {} })
    void loadGatewayProviders()
    setProviderDialogOpen(true)
  }, [loadGatewayProviders])

  /** 打开编辑 Provider 弹窗 */
  const openEditProvider = useCallback(
    (providerId: string) => {
      const provider = providers[providerId]
      if (!provider) return
      setEditingProviderId(providerId)
      setProviderForm({
        id: providerId,
        npm: provider.npm,
        name: provider.name,
        baseURL: provider.options.baseURL ?? '',
        apiKey: provider.options.apiKey ?? '',
        models: { ...provider.models },
      })
      setProviderDialogOpen(true)
    },
    [providers]
  )

  /** 快速选择 npm 适配器，自定义值保留当前输入 */
  const applyNpmAdapter = useCallback((value: string) => {
    if (value === 'custom') {
      return
    }
    setProviderForm((prev) => ({ ...prev, npm: value }))
  }, [])

  /** 提交 Provider 表单 */
  const handleProviderSubmit = useCallback(() => {
    const { id, npm, name, baseURL, apiKey, models } = providerForm
    const trimmedId = id.trim()
    if (!trimmedId) {
      toast.error('Provider ID 不能为空')
      return
    }
    if (!name.trim()) {
      toast.error('Provider 名称不能为空')
      return
    }

    // 编辑模式：若 ID 变更，需删除旧键
    if (editingProviderId && editingProviderId !== trimmedId) {
      setProviders((prev) => {
        const next = { ...prev }
        delete next[editingProviderId]
        next[trimmedId] = {
          npm,
          name: name.trim(),
          options: { baseURL: baseURL.trim() || undefined, apiKey: apiKey.trim() || undefined },
          models,
        }
        return next
      })
    } else {
      setProviders((prev) => ({
        ...prev,
        [trimmedId]: {
          npm,
          name: name.trim(),
          options: { baseURL: baseURL.trim() || undefined, apiKey: apiKey.trim() || undefined },
          models,
        },
      }))
    }

    isDirtyRef.current = true
    toast.success(editingProviderId ? 'Provider 已更新' : 'Provider 已添加')
    setProviderDialogOpen(false)
  }, [providerForm, editingProviderId])

  /** 删除 Provider */
  const handleDeleteProvider = useCallback((providerId: string) => {
    setProviders((prev) => {
      const next = { ...prev }
      delete next[providerId]
      return next
    })
    // 若主模型属于该 provider，清除主模型设置
    setPrimaryModel((prev) => (prev.startsWith(`${providerId}/`) ? '' : prev))
    isDirtyRef.current = true
    toast.success('Provider 已删除')
  }, [])

  // ── Model 操作 ──

  /** 加载内置模型预设列表 */
  const loadBuiltinModels = useCallback(async () => {
    setBuiltinModelsLoading(true)
    try {
      const result = await invokeCommand<BuiltinModel[]>('gateway_builtin_models_list')
      setBuiltinModels(result)
    } catch (err) {
      setBuiltinModels([])
      const error = toIcodeError(err)
      toast.error(t('cli.opencode.loadBuiltinModelsFailed', { message: error.message }))
    } finally {
      setBuiltinModelsLoading(false)
    }
  }, [])

  /** 打开添加 Model 弹窗 */
  const openAddModel = useCallback((providerId: string) => {
    setModelDialogProviderId(providerId)
    setEditingModelId(null)
    setSelectedBuiltinModelId('')
    setModelForm({
      id: '',
      name: '',
      contextLimit: '',
      outputLimit: '',
      inputModalities: ['text'],
      outputModalities: ['text'],
    })
    void loadBuiltinModels()
    setModelDialogOpen(true)
  }, [loadBuiltinModels])

  /** 将选中的内置模型预设填充到表单（字段仍可手动编辑） */
  const applyBuiltinModelToForm = useCallback((builtinModelId: string) => {
    setSelectedBuiltinModelId(builtinModelId)
    if (!builtinModelId) {
      return
    }
    const model = builtinModels.find((m) => m.id === builtinModelId)
    if (!model) return

    const inputModalities: string[] = ['text']
    if (model.capabilities?.imageInput) {
      inputModalities.push('image')
    }

    setModelForm((prev) => ({
      ...prev,
      id: model.id,
      name: model.displayName,
      contextLimit: model.maxInputTokens?.toString() ?? '',
      outputLimit: model.maxOutputTokens?.toString() ?? '',
      inputModalities,
      outputModalities: ['text'],
    }))
  }, [builtinModels])

  /** 打开编辑 Model 弹窗 */
  const openEditModel = useCallback(
    (providerId: string, modelId: string) => {
      const model = providers[providerId]?.models[modelId]
      if (!model) return
      setModelDialogProviderId(providerId)
      setEditingModelId(modelId)
      setSelectedBuiltinModelId('')
      setModelForm({
        id: modelId,
        name: model.name,
        contextLimit: model.limit?.context?.toString() ?? '',
        outputLimit: model.limit?.output?.toString() ?? '',
        inputModalities: model.modalities?.input ?? ['text'],
        outputModalities: model.modalities?.output ?? ['text'],
      })
      setModelDialogOpen(true)
    },
    [providers]
  )

  /** 提交 Model 表单 */
  const handleModelSubmit = useCallback(() => {
    if (!modelDialogProviderId) return
    const { id, name, contextLimit, outputLimit, inputModalities, outputModalities } = modelForm
    const trimmedId = id.trim()
    if (!trimmedId) {
      toast.error('模型 ID 不能为空')
      return
    }

    const newModel: OpenCodeModel = {
      name: name.trim() || trimmedId,
      limit:
        contextLimit || outputLimit
          ? {
              context: parseInt(contextLimit) || 0,
              output: parseInt(outputLimit) || 0,
            }
          : undefined,
      modalities: { input: inputModalities, output: outputModalities },
    }

    setProviders((prev) => {
      const provider = prev[modelDialogProviderId!]
      if (!provider) return prev
      const nextModels = { ...provider.models }
      // 编辑模式：若 ID 变更，需删除旧键
      if (editingModelId && editingModelId !== trimmedId) {
        delete nextModels[editingModelId]
      }
      nextModels[trimmedId] = newModel
      return {
        ...prev,
        [modelDialogProviderId!]: { ...provider, models: nextModels },
      }
    })

    isDirtyRef.current = true
    toast.success(editingModelId ? '模型已更新' : '模型已添加')
    setModelDialogOpen(false)
  }, [modelDialogProviderId, modelForm, editingModelId])

  /** 删除 Model */
  const handleDeleteModel = useCallback((providerId: string, modelId: string) => {
    setProviders((prev) => {
      const provider = prev[providerId]
      if (!provider) return prev
      const nextModels = { ...provider.models }
      delete nextModels[modelId]
      return { ...prev, [providerId]: { ...provider, models: nextModels } }
    })
    // 清除主模型指向
    setPrimaryModel((prev) => (prev === `${providerId}/${modelId}` ? '' : prev))
    isDirtyRef.current = true
    toast.success('模型已删除')
  }, [])

  /** 设置主模型 */
  const handleSetPrimaryModel = useCallback((providerId: string, modelId: string) => {
    setPrimaryModel(`${providerId}/${modelId}`)
    isDirtyRef.current = true
    toast.success(`主模型已设为 ${providerId}/${modelId}`)
  }, [])

  // ── Agent 配置操作 ──

  /** 计算当前所有可用模型（`providerId/modelId` 格式），供 Agent 模型选择使用 */
  const availableModelsForAgents = useMemo(() => {
    const list: string[] = []
    for (const [providerId, provider] of Object.entries(providers)) {
      for (const modelId of Object.keys(provider.models)) {
        list.push(`${providerId}/${modelId}`)
      }
    }
    return list
  }, [providers])

  /** Agent 弹窗保存回调：更新本地 agents 状态并标记 dirty 触发自动保存 */
  const handleAgentsSave = useCallback((nextAgents: OpenCodeAgents) => {
    setAgents(nextAgents)
    isDirtyRef.current = true
  }, [])

  // ── Oh-My-OpenAgent 操作 ── 当前版本暂不展示 ──
  //
  // /** 打开添加配置弹窗 */
  // const openAddAgentConfig = useCallback(() => {
  //   setEditingAgentId(null)
  //   setAgentFormName('')
  //   setAgentDialogOpen(true)
  // }, [])
  //
  // /** 打开编辑配置弹窗 */
  // const openEditAgentConfig = useCallback((config: OpenAgentConfig) => {
  //   setEditingAgentId(config.id)
  //   setAgentFormName(config.name)
  //   setAgentDialogOpen(true)
  // }, [])
  //
  // /** 提交配置表单 */
  // const handleAgentSubmit = useCallback(() => {
  //   const trimmedName = agentFormName.trim()
  //   if (!trimmedName) {
  //     toast.error('配置名称不能为空')
  //     return
  //   }
  //   if (editingAgentId) {
  //     setAgentConfigs((prev) =>
  //       prev.map((c) => (c.id === editingAgentId ? { ...c, name: trimmedName } : c))
  //     )
  //     toast.success('配置已更新')
  //   } else {
  //     setAgentConfigs((prev) => [
  //       ...prev,
  //       { id: uid(), name: trimmedName, agentCount: 0, isApplied: false },
  //     ])
  //     toast.success('配置已添加')
  //   }
  //   setAgentDialogOpen(false)
  // }, [agentFormName, editingAgentId])
  //
  // /** 复制配置 */
  // const handleCopyAgentConfig = useCallback((config: OpenAgentConfig) => {
  //   setAgentConfigs((prev) => [
  //     ...prev,
  //     { id: uid(), name: `${config.name} (副本)`, agentCount: config.agentCount, isApplied: false },
  //   ])
  //   toast.success('配置已复制')
  // }, [])
  //
  // /** 删除配置 */
  // const handleDeleteAgentConfig = useCallback((configId: string) => {
  //   setAgentConfigs((prev) => prev.filter((c) => c.id !== configId))
  //   setAppliedConfigId((prev) => (prev === configId ? null : prev))
  //   toast.success('配置已删除')
  // }, [])
  //
  // /** 应用/取消应用配置 */
  // const handleToggleApplyConfig = useCallback(
  //   (configId: string) => {
  //     if (appliedConfigId === configId) {
  //       setAppliedConfigId(null)
  //       toast.success('已取消应用')
  //     } else {
  //       setAppliedConfigId(configId)
  //       toast.success('配置已应用')
  //     }
  //   },
  //   [appliedConfigId]
  // )

  // ── 删除确认 ──
  const openDeleteConfirm = useCallback(
    (target: { type: 'provider' | 'model' | 'agent'; providerId?: string; modelId?: string; agentId?: string }) => {
      setDeleteTarget(target)
      setDeleteConfirmOpen(true)
    },
    []
  )

  const handleDeleteConfirm = useCallback(() => {
    if (!deleteTarget) return
    if (deleteTarget.type === 'provider' && deleteTarget.providerId) {
      handleDeleteProvider(deleteTarget.providerId)
    } else if (deleteTarget.type === 'model' && deleteTarget.providerId && deleteTarget.modelId) {
      handleDeleteModel(deleteTarget.providerId, deleteTarget.modelId)
    }
    // Oh-My-OpenAgent 删除逻辑暂不展示
    // else if (deleteTarget.type === 'agent' && deleteTarget.agentId) {
    //   handleDeleteAgentConfig(deleteTarget.agentId)
    // }
    setDeleteConfirmOpen(false)
    setDeleteTarget(null)
  }, [deleteTarget, handleDeleteProvider, handleDeleteModel])

  // ── Modalities 切换辅助 ──
  const toggleModality = useCallback(
    (field: 'inputModalities' | 'outputModalities', value: string) => {
      setModelForm((prev) => {
        const current = prev[field]
        const next = current.includes(value)
          ? current.filter((m) => m !== value)
          : [...current, value]
        return { ...prev, [field]: next.length > 0 ? next : ['text'] }
      })
    },
    []
  )

  // ── Provider 列表（排序保持稳定） ──
  const providerEntries = useMemo(() => Object.entries(providers), [providers])

  // ── 高度分配：当前仅展示 Provider 区，占满可用空间 ──
  const providerSectionHeight = Math.max(0, height - 80)
  // OpenAgent 区暂不展示，保留变量名以便后续启用
  // const agentSectionHeight = Math.max(0, height - Math.floor(height * 0.6) - 100)

  // ── 未加载 profile 的占位 ──
  if (!profile) {
    return (
      <div
        style={{ height: height || undefined }}
        className="flex items-center justify-center rounded-md border text-sm text-muted-foreground"
      >
        {t('cli.loadingProfile')}
      </div>
    )
  }

  return (
    <TooltipProvider delayDuration={200}>
      <div style={{ height: height || undefined }} className="flex min-h-0 flex-col gap-3 overflow-hidden">
        {/* ── Section 1: Provider 配置 ── */}
        <Card className="min-h-0 flex-1 overflow-hidden">
          <CardHeader className="flex flex-row items-center justify-between gap-2 pb-2.5">
            <div className="min-w-0">
              <CardTitle className="text-sm">Provider 配置</CardTitle>
              <CardDescription className="mt-0.5 text-[11px]">
                opencode.json · {providerEntries.length} 个 Provider
              </CardDescription>
            </div>
            <div className="flex items-center gap-1.5">
              {primaryModel && (
                <Badge variant="outline" className="gap-1 px-1.5 py-0 text-[9px]">
                  <i className="fa-solid fa-star text-[8px] text-amber-500" />
                  {primaryModel}
                </Badge>
              )}
              <Button
                type="button"
                variant="outline"
                size="icon"
                className="size-7 shrink-0"
                onClick={() => setImportDialogOpen(true)}
              >
                <i className="fa-solid fa-file-import" />
                <span className="sr-only">{t('cli.opencode.importProviders')}</span>
              </Button>
              <Button
                type="button"
                variant="outline"
                size="icon"
                className="size-7 shrink-0"
                onClick={handleExportProviders}
                disabled={providerEntries.length === 0}
              >
                <i className="fa-solid fa-file-export" />
                <span className="sr-only">{t('cli.opencode.exportProviders')}</span>
              </Button>
              <Button size="icon" className="size-7 shrink-0" onClick={openAddProvider}>
                <i className="fa-solid fa-plus" />
                <span className="sr-only">添加 Provider</span>
              </Button>
              <Separator orientation="vertical" className="mx-0.5 h-5" />
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    className="h-7 gap-1.5 px-2 text-xs"
                    onClick={() => setAgentDialogOpen(true)}
                  >
                    <i className="fa-solid fa-user-gear text-[10px]" />
                    {t('cli.opencode.agent.openDialog')}
                    {Object.keys(agents).length > 0 && (
                      <Badge variant="secondary" className="ml-0.5 px-1 py-0 text-[9px]">
                        {Object.keys(agents).length}
                      </Badge>
                    )}
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom" className="text-[11px]">
                  {t('cli.opencode.agent.openDialogTooltip')}
                </TooltipContent>
              </Tooltip>
            </div>
          </CardHeader>
          <CardContent className="min-h-0 p-0">
            {configLoading ? (
              <div
                style={{ height: providerSectionHeight || undefined }}
                className="flex items-center justify-center text-xs text-muted-foreground"
              >
                <i className="fa-solid fa-spinner fa-spin mr-2" />
                正在读取 opencode.json…
              </div>
            ) : (
              <ScrollPage
                style={{ height: providerSectionHeight || undefined }}
                variant="borderless"
                scrollbarVisible="auto"
              >
                <div className="flex flex-col gap-2 px-3 pb-3">
                  {providerEntries.length === 0 && (
                    <div className="py-6 text-center text-xs text-muted-foreground">
                      暂无 Provider，点击右上角添加
                    </div>
                  )}
                {providerEntries.map(([providerId, provider]) => {
                  const isExpanded = expandedProviders[providerId] ?? false
                  const modelEntries = Object.entries(provider.models)
                  return (
                    <Collapsible
                      key={providerId}
                      open={isExpanded}
                      onOpenChange={() => toggleExpand(providerId)}
                      className="rounded-md border"
                    >
                      {/* Provider 折叠头 */}
                      <div className="flex items-center gap-2 px-3 py-2">
                        <CollapsibleTrigger asChild>
                          <Button type="button" variant="ghost" size="icon" className="size-5 shrink-0">
                            <i
                              className={`fa-solid fa-chevron-right text-[10px] transition-transform ${
                                isExpanded ? 'rotate-90' : ''
                              }`}
                            />
                          </Button>
                        </CollapsibleTrigger>
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-1.5">
                            <span className="truncate text-xs font-medium">{provider.name}</span>
                            <code className="shrink-0 rounded bg-muted px-1 py-0 text-[9px] text-muted-foreground">
                              {providerId}
                            </code>
                            {provider.npm && (
                              <Badge variant="secondary" className="shrink-0 px-1 py-0 text-[8px]">
                                npm
                              </Badge>
                            )}
                          </div>
                          <p className="mt-0.5 truncate text-[11px] text-muted-foreground">
                            {provider.options.baseURL || '未设置 baseURL'}
                          </p>
                        </div>
                        <div className="flex items-center gap-0.5">
                          <Tooltip>
                            <TooltipTrigger asChild>
                              <Button
                                type="button"
                                variant="ghost"
                                size="icon"
                                className="size-6"
                                onClick={(e) => {
                                  e.stopPropagation()
                                  openEditProvider(providerId)
                                }}
                              >
                                <i className="fa-solid fa-pen text-[10px]" />
                              </Button>
                            </TooltipTrigger>
                            <TooltipContent side="top" className="text-[11px]">
                              编辑
                            </TooltipContent>
                          </Tooltip>
                          <Tooltip>
                            <TooltipTrigger asChild>
                              <Button
                                type="button"
                                variant="ghost"
                                size="icon"
                                className="size-6 text-destructive hover:text-destructive"
                                onClick={(e) => {
                                  e.stopPropagation()
                                  openDeleteConfirm({ type: 'provider', providerId })
                                }}
                              >
                                <i className="fa-solid fa-trash text-[10px]" />
                              </Button>
                            </TooltipTrigger>
                            <TooltipContent side="top" className="text-[11px]">
                              删除
                            </TooltipContent>
                          </Tooltip>
                        </div>
                      </div>

                      {/* Provider 折叠内容：模型列表 */}
                      <CollapsibleContent>
                        <div className="border-t px-3 pb-2.5 pt-2">
                          {/* API Key 显示 */}
                          {provider.options.apiKey && (
                            <div className="mb-2 flex items-center gap-2 text-[11px]">
                              <span className="text-muted-foreground">API Key:</span>
                              <code className="rounded bg-muted px-1 py-0 text-[10px]">
                                {maskApiKey(provider.options.apiKey)}
                              </code>
                            </div>
                          )}

                          {/* 模型表格 */}
                          <div className="mb-1.5 flex items-center justify-between">
                            <span className="text-[11px] font-medium text-muted-foreground">
                              模型 ({modelEntries.length})
                            </span>
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              className="h-5 gap-1 px-1.5 text-[10px]"
                              onClick={() => openAddModel(providerId)}
                            >
                              <i className="fa-solid fa-plus text-[8px]" />
                              添加模型
                            </Button>
                          </div>

                          {modelEntries.length === 0 ? (
                            <div className="py-3 text-center text-[11px] text-muted-foreground">
                              暂无模型
                            </div>
                          ) : (
                            <div className="overflow-x-auto rounded border text-[11px]">
                              <table className="w-full">
                                <thead>
                                  <tr className="border-b bg-muted/30">
                                    <th className="px-2 py-1 text-left font-medium">ID</th>
                                    <th className="px-2 py-1 text-left font-medium">名称</th>
                                    <th className="px-2 py-1 text-right font-medium">上下文</th>
                                    <th className="px-2 py-1 text-right font-medium">输出</th>
                                    <th className="px-2 py-1 text-center font-medium">输入</th>
                                    <th className="px-2 py-1 text-center font-medium">输出</th>
                                    <th className="px-2 py-1 text-center font-medium">操作</th>
                                  </tr>
                                </thead>
                                <tbody>
                                  {modelEntries.map(([modelId, model]) => {
                                    const isPrimary = primaryModel === `${providerId}/${modelId}`
                                    return (
                                      <tr key={modelId} className="border-b last:border-b-0">
                                        <td className="px-2 py-1">
                                          <div className="flex items-center gap-1">
                                            {isPrimary && (
                                              <i className="fa-solid fa-star text-[8px] text-amber-500" />
                                            )}
                                            <code className="text-[10px]">{modelId}</code>
                                          </div>
                                        </td>
                                        <td className="max-w-[100px] truncate px-2 py-1">
                                          {model.name}
                                        </td>
                                        <td className="px-2 py-1 text-right tabular-nums">
                                          {model.limit?.context
                                            ? `${(model.limit.context / 1024).toFixed(0)}K`
                                            : '—'}
                                        </td>
                                        <td className="px-2 py-1 text-right tabular-nums">
                                          {model.limit?.output
                                            ? `${(model.limit.output / 1024).toFixed(0)}K`
                                            : '—'}
                                        </td>
                                        <td className="px-2 py-1 text-center">
                                          {model.modalities?.input?.join(', ') ?? 'text'}
                                        </td>
                                        <td className="px-2 py-1 text-center">
                                          {model.modalities?.output?.join(', ') ?? 'text'}
                                        </td>
                                        <td className="px-2 py-1">
                                          <div className="flex items-center justify-center gap-0.5">
                                            <Tooltip>
                                              <TooltipTrigger asChild>
                                                <Button
                                                  type="button"
                                                  variant="ghost"
                                                  size="icon"
                                                  className="size-5"
                                                  onClick={() => openEditModel(providerId, modelId)}
                                                >
                                                  <i className="fa-solid fa-pen text-[8px]" />
                                                </Button>
                                              </TooltipTrigger>
                                              <TooltipContent side="top" className="text-[11px]">
                                                编辑
                                              </TooltipContent>
                                            </Tooltip>
                                            {!isPrimary && (
                                              <Tooltip>
                                                <TooltipTrigger asChild>
                                                  <Button
                                                    type="button"
                                                    variant="ghost"
                                                    size="icon"
                                                    className="size-5"
                                                    onClick={() =>
                                                      handleSetPrimaryModel(providerId, modelId)
                                                    }
                                                  >
                                                    <i className="fa-solid fa-star text-[8px]" />
                                                  </Button>
                                                </TooltipTrigger>
                                                <TooltipContent side="top" className="text-[11px]">
                                                  设为主模型
                                                </TooltipContent>
                                              </Tooltip>
                                            )}
                                            <Tooltip>
                                              <TooltipTrigger asChild>
                                                <Button
                                                  type="button"
                                                  variant="ghost"
                                                  size="icon"
                                                  className="size-5 text-destructive hover:text-destructive"
                                                  onClick={() =>
                                                    openDeleteConfirm({
                                                      type: 'model',
                                                      providerId,
                                                      modelId,
                                                    })
                                                  }
                                                >
                                                  <i className="fa-solid fa-trash text-[8px]" />
                                                </Button>
                                              </TooltipTrigger>
                                              <TooltipContent side="top" className="text-[11px]">
                                                删除
                                              </TooltipContent>
                                            </Tooltip>
                                          </div>
                                        </td>
                                      </tr>
                                    )
                                  })}
                                </tbody>
                              </table>
                            </div>
                          )}
                        </div>
                      </CollapsibleContent>
                    </Collapsible>
                  )
                })}
              </div>
              </ScrollPage>
            )}
          </CardContent>
        </Card>

        {/* ── Section 2: Oh-My-OpenAgent 配置管理 ── 当前版本暂不展示 ── */}
      </div>

      {/* ── Provider 编辑弹窗 ── */}
      <Dialog open={providerDialogOpen} onOpenChange={setProviderDialogOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle className="text-base">
              {editingProviderId ? '编辑 Provider' : '添加 Provider'}
            </DialogTitle>
            <DialogDescription className="text-xs">
              配置 opencode.json 中的 provider 条目
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3 py-2">
            {!editingProviderId && (
              <div className="space-y-1.5">
                <Label className="text-xs">{t('cli.opencode.selectGatewayProvider')}</Label>
                <Select
                  value={selectedGatewayProviderId}
                  onValueChange={applyGatewayProviderToForm}
                  disabled={gatewayProvidersLoading}
                >
                  <SelectTrigger className="h-8 text-xs">
                    <SelectValue placeholder={gatewayProvidersLoading ? t('common.loading') : t('cli.opencode.selectGatewayProviderPlaceholder')} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="" className="text-xs">{t('cli.opencode.manualCreate')}</SelectItem>
                    {gatewayProviders.map((provider) => (
                      <SelectItem key={provider.id} value={provider.id} className="text-xs">
                        {provider.displayName}
                        <span className="ml-1 text-muted-foreground">({provider.slug})</span>
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}
            <div className="space-y-1.5">
              <Label className="text-xs">ID</Label>
              <Input
                value={providerForm.id}
                onChange={(e) => setProviderForm((p) => ({ ...p, id: e.target.value }))}
                placeholder="joy_agent"
                className="h-8 text-xs"
                disabled={!!editingProviderId}
              />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs">名称</Label>
              <Input
                value={providerForm.name}
                onChange={(e) => setProviderForm((p) => ({ ...p, name: e.target.value }))}
                placeholder="JoyAgent"
                className="h-8 text-xs"
              />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs">npm 包</Label>
              <div className="flex items-center gap-2">
                <Select
                  value={isBuiltinNpmAdapter(providerForm.npm) ? providerForm.npm : 'custom'}
                  onValueChange={applyNpmAdapter}
                >
                  <SelectTrigger className="h-8 w-auto min-w-[140px] text-xs">
                    <SelectValue placeholder={t('cli.opencode.selectNpmAdapter')} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="@ai-sdk/openai-compatible" className="text-xs">@ai-sdk/openai-compatible</SelectItem>
                    <SelectItem value="@ai-sdk/anthropic" className="text-xs">@ai-sdk/anthropic</SelectItem>
                    <SelectItem value="custom" className="text-xs">{t('cli.opencode.manualInput')}</SelectItem>
                  </SelectContent>
                </Select>
                <Input
                  value={providerForm.npm}
                  onChange={(e) => setProviderForm((p) => ({ ...p, npm: e.target.value }))}
                  placeholder="@anthropic-ai/opencode-provider-xxx"
                  className="h-8 flex-1 text-xs"
                />
              </div>
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs">Base URL</Label>
              <Input
                value={providerForm.baseURL}
                onChange={(e) => setProviderForm((p) => ({ ...p, baseURL: e.target.value }))}
                placeholder="https://api.example.com/v1"
                className="h-8 text-xs"
              />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs">API Key</Label>
              <Input
                type="password"
                value={providerForm.apiKey}
                onChange={(e) => setProviderForm((p) => ({ ...p, apiKey: e.target.value }))}
                placeholder="sk-xxxx"
                className="h-8 text-xs"
              />
            </div>
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-8 text-xs"
              onClick={() => setProviderDialogOpen(false)}
            >
              取消
            </Button>
            <Button
              type="button"
              size="sm"
              className="h-8 gap-1.5 text-xs"
              onClick={handleProviderSubmit}
            >
              <i className="fa-solid fa-check text-[10px]" />
              {editingProviderId ? '保存' : '添加'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* ── Model 编辑弹窗 ── */}
      <Dialog open={modelDialogOpen} onOpenChange={setModelDialogOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle className="text-base">
              {editingModelId ? '编辑模型' : '添加模型'}
            </DialogTitle>
            <DialogDescription className="text-xs">
              {modelDialogProviderId
                ? `Provider: ${providers[modelDialogProviderId]?.name ?? modelDialogProviderId}`
                : ''}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3 py-2">
            {!editingModelId && (
              <div className="space-y-1.5">
                <Label className="text-xs">{t('cli.opencode.selectBuiltinModel')}</Label>
                <Select
                  value={selectedBuiltinModelId}
                  onValueChange={applyBuiltinModelToForm}
                  disabled={builtinModelsLoading}
                >
                  <SelectTrigger className="h-8 text-xs">
                    <SelectValue placeholder={builtinModelsLoading ? t('common.loading') : t('cli.opencode.selectBuiltinModelPlaceholder')} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="" className="text-xs">{t('cli.opencode.manualInput')}</SelectItem>
                    {builtinModels.map((model) => (
                      <SelectItem key={model.id} value={model.id} className="text-xs">
                        {model.displayName}
                        <span className="ml-1 text-muted-foreground">({model.id})</span>
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}
            <div className="space-y-1.5">
              <Label className="text-xs">模型 ID</Label>
              <Input
                value={modelForm.id}
                onChange={(e) => setModelForm((p) => ({ ...p, id: e.target.value }))}
                placeholder="deepseek-v4-flash"
                className="h-8 text-xs"
                disabled={!!editingModelId}
              />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs">模型名称</Label>
              <Input
                value={modelForm.name}
                onChange={(e) => setModelForm((p) => ({ ...p, name: e.target.value }))}
                placeholder="DeepSeek-V4-Flash"
                className="h-8 text-xs"
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label className="text-xs">上下文 Limit</Label>
                <Input
                  value={modelForm.contextLimit}
                  onChange={(e) => setModelForm((p) => ({ ...p, contextLimit: e.target.value }))}
                  placeholder="65536"
                  className="h-8 text-xs tabular-nums"
                />
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs">输出 Limit</Label>
                <Input
                  value={modelForm.outputLimit}
                  onChange={(e) => setModelForm((p) => ({ ...p, outputLimit: e.target.value }))}
                  placeholder="8192"
                  className="h-8 text-xs tabular-nums"
                />
              </div>
            </div>

            <Separator />

            {/* 输入 Modalities */}
            <div className="space-y-1.5">
              <Label className="text-xs">输入 Modalities</Label>
              <div className="flex items-center gap-3">
                <label className="flex items-center gap-1.5">
                  <Switch
                    checked={modelForm.inputModalities.includes('text')}
                    onCheckedChange={() => toggleModality('inputModalities', 'text')}
                    className="data-[state=checked]:bg-primary"
                  />
                  <span className="text-xs">text</span>
                </label>
                <label className="flex items-center gap-1.5">
                  <Switch
                    checked={modelForm.inputModalities.includes('image')}
                    onCheckedChange={() => toggleModality('inputModalities', 'image')}
                    className="data-[state=checked]:bg-primary"
                  />
                  <span className="text-xs">image</span>
                </label>
              </div>
            </div>

            {/* 输出 Modalities */}
            <div className="space-y-1.5">
              <Label className="text-xs">输出 Modalities</Label>
              <div className="flex items-center gap-3">
                <label className="flex items-center gap-1.5">
                  <Switch
                    checked={modelForm.outputModalities.includes('text')}
                    onCheckedChange={() => toggleModality('outputModalities', 'text')}
                    className="data-[state=checked]:bg-primary"
                  />
                  <span className="text-xs">text</span>
                </label>
              </div>
            </div>
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-8 text-xs"
              onClick={() => setModelDialogOpen(false)}
            >
              取消
            </Button>
            <Button
              type="button"
              size="sm"
              className="h-8 gap-1.5 text-xs"
              onClick={handleModelSubmit}
            >
              <i className="fa-solid fa-check text-[10px]" />
              {editingModelId ? '保存' : '添加'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* ── 导出 Provider 配置弹窗 ── */}
      <Dialog open={exportDialogOpen} onOpenChange={setExportDialogOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle className="text-base">
              {t('cli.opencode.exportProvidersDialogTitle')}
            </DialogTitle>
            <DialogDescription className="text-xs">
              {t('cli.opencode.exportProvidersDialogDescription')}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <Textarea
              value={exportedData}
              readOnly
              className="min-h-[160px] font-mono text-xs break-all"
            />
            <div className="flex justify-end gap-2">
              <Button variant="outline" size="sm" className="h-8 text-xs" onClick={handleCopyExport}>
                <i className="fa-solid fa-copy mr-1.5" />
                {t('cli.opencode.exportCopy')}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      {/* ── 导入 Provider 配置弹窗 ── */}
      <Dialog open={importDialogOpen} onOpenChange={setImportDialogOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle className="text-base">
              {t('cli.opencode.importProvidersDialogTitle')}
            </DialogTitle>
            <DialogDescription className="text-xs">
              {t('cli.opencode.importProvidersDialogDescription')}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <Textarea
              placeholder={t('cli.opencode.importProvidersPlaceholder')}
              value={importData}
              onChange={(e) => setImportData(e.target.value)}
              className="min-h-[160px] font-mono text-xs break-all"
            />
            <div className="flex justify-end gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-8 text-xs"
                onClick={() => setImportDialogOpen(false)}
                disabled={importLoading}
              >
                {t('common.cancel')}
              </Button>
              <Button
                type="button"
                size="sm"
                className="h-8 text-xs"
                onClick={handleImportProviders}
                disabled={importLoading}
              >
                {importLoading && <i className="fa-solid fa-circle-notch fa-spin mr-1.5" />}
                {t('cli.opencode.importProvidersConfirm')}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      {/* ── 删除确认弹窗 ── */}
      <Dialog open={deleteConfirmOpen} onOpenChange={setDeleteConfirmOpen}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle className="text-base">确认删除</DialogTitle>
            <DialogDescription className="text-xs">
              {deleteTarget?.type === 'provider'
                ? `确定删除 Provider「${deleteTarget.providerId}」？其下所有模型也将被删除。`
                : deleteTarget?.type === 'model'
                  ? `确定删除模型「${deleteTarget.modelId}」？`
                  : deleteTarget?.type === 'agent'
                    ? '确定删除此配置预设？'
                    : ''}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-8 text-xs"
              onClick={() => {
                setDeleteConfirmOpen(false)
                setDeleteTarget(null)
              }}
            >
              取消
            </Button>
            <Button
              type="button"
              variant="destructive"
              size="sm"
              className="h-8 gap-1.5 text-xs"
              onClick={handleDeleteConfirm}
            >
              <i className="fa-solid fa-trash text-[10px]" />
              删除
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* ── 初始化 opencode.json 提示 ── */}
      <Dialog open={initDialogOpen} onOpenChange={setInitDialogOpen}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle className="text-base">{t('cli.opencode.initConfigTitle')}</DialogTitle>
            <DialogDescription className="text-xs">
              {t('cli.opencode.initConfigDescription')}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-8 text-xs"
              onClick={() => setInitDialogOpen(false)}
            >
              {t('cli.opencode.initConfigCancel')}
            </Button>
            <Button type="button" size="sm" className="h-8 text-xs" onClick={handleInitConfirm}>
              {t('cli.opencode.initConfigConfirm')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* ── Agent 配置弹窗（全屏） ── */}
      <OpenCodeAgentDialog
        open={agentDialogOpen}
        onOpenChange={setAgentDialogOpen}
        agents={agents}
        availableModels={availableModelsForAgents}
        onSave={handleAgentsSave}
      />
    </TooltipProvider>
  )
}
