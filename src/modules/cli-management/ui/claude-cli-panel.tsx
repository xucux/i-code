import { useCallback, useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { CodeEditor } from '@/components/ui/code-editor'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { DeleteConfirmDialog } from '@/components/ui/delete-confirm-dialog'
import { ScrollPage } from '@/components/ui/scroll-page'
import { Switch } from '@/components/ui/switch'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { useCliModelMappings, useCliProviders } from '@/hooks/use-cli-profiles'
import {
  applyClaudeConfig,
  createCliModelMapping,
  createCliProvider,
  deleteCliModelMapping,
  deleteCliProvider,
  updateCliModelMapping,
  updateCliProvider,
} from '@/hooks/use-cli-mutation'
import { useCatalogModels, useCatalogProviders, resolveDefaultGatewayKey } from '@/hooks/use-catalog'
import { useTranslation } from '@/modules/i18n/use-translation'
import type { CliModelMapping, CliProfile, CliProvider } from '@/modules/cli-management/types'
import {
  ClaudeModelMapping,
  DEFAULT_FALLBACK_MODEL,
  type ClaudeModelMappingItem,
} from './claude-model-mapping'
import { ProviderBindingForm } from './provider-binding-form'

/** 删除目标类型 */
type DeleteTarget =
  | { type: 'provider'; item: CliProvider }
  | { type: 'mapping'; item: CliModelMapping }

/** Claude CLI 开关配置项 */
interface ClaudeSwitches {
  /** 隐藏 AI 署名（默认关闭时 includeCoAuthoredBy 为 false） */
  hideCoAuthor: boolean
  /** Teammates 模式 (env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS: "1") */
  agentTeams: boolean
  /** 启用 Tool Search (env.ENABLE_TOOL_SEARCH: "true") */
  toolSearch: boolean
  /** 最大强度思考 (env.CLAUDE_CODE_EFFORT_LEVEL: "max") */
  maxEffort: boolean
  /** 禁用自动升级 (env.DISABLE_AUTOUPDATER: "1") */
  disableAutoupdater: boolean
}

export interface ClaudeCliPanelProps {
  profile?: CliProfile
  height: number
}

/**
 * Claude CLI 专属面板
 *
 * 左侧：供应商列表（带"已应用"标识）
 * 右侧：模型映射编辑器 + Claude 开关 + 预览/应用操作
 */
export function ClaudeCliPanel({ profile, height }: ClaudeCliPanelProps) {
  const { t } = useTranslation()
  const { providers: gatewayProviders } = useCatalogProviders()
  const { models: exposedModels } = useCatalogModels()

  // ── 选中与已应用状态 ──
  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(null)
  const [appliedProviderId, setAppliedProviderId] = useState<string | null>(null)

  // ── 数据加载 ──
  const { providers, loading: providersLoading, refetch: refetchProviders } = useCliProviders(
    profile?.id ?? null
  )
  const { mappings, loading: mappingsLoading, refetch: refetchMappings } =
    useCliModelMappings(selectedProviderId)

  // ── 表单弹窗 ──
  const [providerFormOpen, setProviderFormOpen] = useState(false)
  const [editingProvider, setEditingProvider] = useState<CliProvider | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<DeleteTarget | null>(null)

  // ── 预览弹窗 ──
  const [previewOpen, setPreviewOpen] = useState(false)

  // ── Claude 开关配置 ──
  const [switches, setSwitches] = useState<ClaudeSwitches>({
    hideCoAuthor: false,
    agentTeams: false,
    toolSearch: false,
    maxEffort: false,
    disableAutoupdater: false,
  })

  // ── Claude CLI API Key（按供应商明文存储在 authJson 中） ──
  const [apiKey, setApiKey] = useState('')

  // ── 本地编辑态映射（ClaudeModelMappingEditor 回调写入） ──
  const [localMappings, setLocalMappings] = useState<ClaudeModelMappingItem[]>([])
  const [localFallbackModel, setLocalFallbackModel] = useState(DEFAULT_FALLBACK_MODEL)

  // 记录每个固定角色对应的后端 mapping id，用于应用时判断是创建还是更新
  const [mappingIdByRole, setMappingIdByRole] = useState<Record<string, string | undefined>>({})

  // 切换 profile 时重置选中
  useEffect(() => {
    setSelectedProviderId(null)
    setAppliedProviderId(null)
  }, [profile?.id])

  // providers 变化时自动选中默认或第一个
  useEffect(() => {
    if (providers.length === 0) {
      setSelectedProviderId(null)
      return
    }
    if (!providers.some((p) => p.id === selectedProviderId)) {
      const preferred = providers.find((p) => p.isDefault) ?? providers[0]
      setSelectedProviderId(preferred.id)
    }
  }, [providers, selectedProviderId])

  const selectedProvider = providers.find((p) => p.id === selectedProviderId)
  const selectedGatewayProvider = gatewayProviders.find((p) => p.id === selectedProvider?.providerId)
  const enabledGatewayProviders = gatewayProviders.filter((p) => p.isEnabled)

  // 可用模型列表（网关路由模式下按供应商筛选）
  const availableModels = useMemo(() => {
    if (!selectedProvider?.providerId || selectedProvider.routeMode !== 1) return []
    const gw = gatewayProviders.find((p) => p.id === selectedProvider.providerId)
    if (!gw) return []
    return exposedModels
      .filter((m) => m.providerSlug === gw.slug)
      .map((m) => `${m.providerSlug}/${m.modelId}`)
  }, [exposedModels, gatewayProviders, selectedProvider])

  // mappings 加载完成后转换为编辑器所需格式，并记录后端 mapping id
  useEffect(() => {
    const ids: Record<string, string | undefined> = {}
    const items: ClaudeModelMappingItem[] = mappings.map((m) => {
      const role = m.cliModelAlias as ClaudeModelMappingItem['role']
      if (m.cliModelAlias) {
        ids[m.cliModelAlias] = m.id
      }
      return {
        id: m.id,
        role,
        displayName: role,
        actualModel: m.gatewayModelId ?? m.rawModelId ?? '',
        supports1M: false,
      }
    })
    setLocalMappings(items)
    setMappingIdByRole(ids)
  }, [mappings])

  // 选中供应商变化时重置开关、API Key 和 fallback
  useEffect(() => {
    setSwitches({
      hideCoAuthor: false,
      agentTeams: false,
      toolSearch: false,
      maxEffort: false,
      disableAutoupdater: false,
    })
    setApiKey('')
    setLocalFallbackModel(DEFAULT_FALLBACK_MODEL)
  }, [selectedProviderId])

  // 从供应商 authJson 解析 API Key
  useEffect(() => {
    if (!selectedProvider?.authJson) {
      setApiKey('')
      return
    }
    try {
      const parsed = JSON.parse(selectedProvider.authJson) as { apiKey?: string }
      setApiKey(parsed.apiKey ?? '')
    } catch {
      setApiKey('')
    }
  }, [selectedProvider?.authJson])

  // 虚拟供应商无独立凭证：选中时预填网关默认授权 Key 明文
  useEffect(() => {
    // 仅当绑定的是虚拟供应商（providerId 以 virtual: 前缀标识）时触发
    if (!selectedProvider?.providerId?.startsWith('virtual:')) return
    let cancelled = false
    void resolveDefaultGatewayKey().then((key) => {
      if (!cancelled && key) setApiKey(key)
    })
    return () => {
      cancelled = true
    }
  }, [selectedProvider?.providerId])

  // ── 高度计算 ──
  const listHeight = Math.max(0, height - 76)

  // ── 生成 settings.json 预览 ──
  const generateSettingsJson = useCallback(() => {
    if (!selectedProvider) return '{}'

    const env: Record<string, string> = {}

    // 基础 URL：网关模式直接用网关根地址，Anthropic SDK 会自行追加 /v1/messages
    const baseUrl =
      selectedProvider.routeMode === 1
        ? (selectedProvider.gatewayBaseUrl || 'http://127.0.0.1:54321')
        : (selectedProvider.directBaseUrl || '')
    env.ANTHROPIC_BASE_URL = baseUrl

    // 模型角色 → 环境变量映射
    const roleEnvMap: Record<string, { modelKey: string; nameKey: string }> = {
      Sonnet: { modelKey: 'ANTHROPIC_DEFAULT_SONNET_MODEL', nameKey: 'ANTHROPIC_DEFAULT_SONNET_MODEL_NAME' },
      Opus: { modelKey: 'ANTHROPIC_DEFAULT_OPUS_MODEL', nameKey: 'ANTHROPIC_DEFAULT_OPUS_MODEL_NAME' },
      Fable: { modelKey: 'ANTHROPIC_DEFAULT_FABLE_MODEL', nameKey: 'ANTHROPIC_DEFAULT_FABLE_MODEL_NAME' },
      Haiku: { modelKey: 'ANTHROPIC_DEFAULT_HAIKU_MODEL', nameKey: 'ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME' },
    }

    // 遍历当前编辑态的映射，生成对应环境变量
    for (const item of localMappings) {
      const mapping = roleEnvMap[item.role]
      if (!mapping || !item.actualModel) continue

      // 模型名称去掉 [1M] 后缀
      const modelName = item.actualModel.replace(/\[1M\]$/i, '')
      // 若支持 1M 则追加 [1M]
      const modelValue = item.supports1M ? `${modelName}[1M]` : modelName

      env[mapping.modelKey] = modelValue
      env[mapping.nameKey] = modelName
    }

    // 兜底模型：fallbackModel > 第一个映射的 actualModel
    const fallback = localFallbackModel.trim() || localMappings[0]?.actualModel?.replace(/\[1M\]$/i, '') || ''
    if (fallback) {
      env.ANTHROPIC_MODEL = fallback
    }

    // Auth Token：使用当前供应商编辑态的 API Key（明文写入 Claude 官方 settings.json）
    env.ANTHROPIC_AUTH_TOKEN = apiKey.trim() || 'sk-daeafweeeeeeeeeeeeeeee'

    // 开关联动
    if (switches.agentTeams) env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS = '1'
    if (switches.toolSearch) env.ENABLE_TOOL_SEARCH = 'true'
    if (switches.maxEffort) env.CLAUDE_CODE_EFFORT_LEVEL = 'max'
    if (switches.disableAutoupdater) env.DISABLE_AUTOUPDATER = '1'

    const settings: Record<string, unknown> = {
      env,
      includeCoAuthoredBy: switches.hideCoAuthor,
      model: 'haiku',
    }

    return JSON.stringify(settings, null, 2)
  }, [selectedProvider, localMappings, localFallbackModel, switches, apiKey])

  // ── 操作处理 ──

  const openCreateProvider = () => {
    setEditingProvider(null)
    setProviderFormOpen(true)
  }

  const openEditProvider = (provider: CliProvider) => {
    setEditingProvider(provider)
    setProviderFormOpen(true)
  }

  const handleProviderSubmit = async (values: {
    displayName: string
    providerId?: string
    routeMode: number
    gatewayBaseUrl?: string
    directBaseUrl?: string
    sortOrder?: number
    isDefault?: boolean
  }) => {
    if (!profile) return
    const result = editingProvider
      ? await updateCliProvider(editingProvider.id, values)
      : await createCliProvider({
          cliProfileId: profile.id,
          ...values,
          sortOrder: values.sortOrder ?? 0,
          isDefault: values.isDefault ?? false,
        })
    if (!result) {
      toast.error(t('cli.messages.saveFailed'))
      return
    }
    toast.success(t(editingProvider ? 'cli.messages.bindingUpdated' : 'cli.messages.bindingCreated'))
    setProviderFormOpen(false)
    setSelectedProviderId(result.id)
    void refetchProviders()
  }

  const handleDelete = async () => {
    if (!deleteTarget) return
    const deleted =
      deleteTarget.type === 'provider'
        ? await deleteCliProvider(deleteTarget.item.id)
        : await deleteCliModelMapping(deleteTarget.item.id)
    if (!deleted) {
      toast.error(t('cli.messages.deleteFailed'))
      return
    }
    if (deleteTarget.type === 'provider') {
      // 若删除的是已应用的供应商，清除应用状态
      if (appliedProviderId === deleteTarget.item.id) {
        setAppliedProviderId(null)
      }
      setSelectedProviderId(null)
      void refetchProviders()
    } else {
      void refetchMappings()
    }
    setDeleteTarget(null)
    toast.success(t('cli.messages.deleted'))
  }

  /** 保存本地映射变更 */
  const handleMappingsChange = (updated: ClaudeModelMappingItem[]) => {
    setLocalMappings(updated)
  }

  /** 保存兜底模型变更 */
  const handleFallbackChange = (value: string) => {
    setLocalFallbackModel(value)
  }

  /** 切换开关 */
  const handleSwitchToggle = (key: keyof ClaudeSwitches) => {
    setSwitches((prev) => ({ ...prev, [key]: !prev[key] }))
  }

  /** 保存当前供应商配置（持久化 4 个固定角色映射与 API Key） */
  const handleSave = async () => {
    if (!selectedProviderId || !selectedProvider) return

    const inputMode: 'select' | 'manual' = selectedProvider.routeMode === 1 ? 'select' : 'manual'
    const saveResults: Promise<unknown>[] = []

    for (const item of localMappings) {
      const mappingId = mappingIdByRole[item.role]
      const values = {
        cliModelAlias: item.role,
        gatewayModelId: inputMode === 'select' ? item.actualModel : undefined,
        rawModelId: inputMode === 'manual' ? item.actualModel : undefined,
        inputMode,
      }

      if (mappingId) {
        saveResults.push(updateCliModelMapping(mappingId, values))
      } else {
        saveResults.push(
          createCliModelMapping({ cliProviderId: selectedProviderId, ...values })
        )
      }
    }

    // 同时把 API Key 明文保存到供应商 authJson（Claude 官方 settings.json 需要）
    saveResults.push(
      updateCliProvider(selectedProviderId, {
        authJson: JSON.stringify({ apiKey }),
      })
    )

    try {
      await Promise.all(saveResults)
      setAppliedProviderId(selectedProviderId)
      toast.success(t('cli.claude.saveProviderSuccess', { name: selectedProvider.displayName }))
      void refetchMappings()
      void refetchProviders()
    } catch {
      toast.error(t('cli.messages.saveFailed'))
    }
  }

  /** 应用当前供应商配置（生成并写入 Claude Code settings.json） */
  const handleApply = async () => {
    if (!selectedProviderId || !selectedProvider) return

    const result = await applyClaudeConfig({
      cliProviderId: selectedProviderId,
      mappings: localMappings.map((item) => ({
        role: item.role,
        displayName: item.displayName,
        actualModel: item.actualModel,
        supports1M: item.supports1M,
      })),
      fallbackModel: localFallbackModel,
      apiKey,
      switches,
    })

    if (result) {
      setAppliedProviderId(selectedProviderId)
      toast.success(t('cli.claude.applyProviderSuccess', { name: selectedProvider.displayName }))
    } else {
      toast.error(t('cli.claude.applyProviderFailed', { name: selectedProvider.displayName }))
    }
  }

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
    <>
      <TooltipProvider delayDuration={200}>
        <div
          style={{ height: height || undefined }}
          className="grid min-h-0 grid-cols-[minmax(200px,0.38fr)_minmax(0,1fr)] gap-3 overflow-hidden"
        >
          {/* ── 左侧：供应商列表 ── */}
          <Card className="min-h-0 overflow-hidden">
            <CardHeader className="flex flex-row items-center justify-between gap-2 pb-2.5">
              <div className="min-w-0">
                <CardTitle className="text-sm">{t('cli.providers.title')}</CardTitle>
                <CardDescription className="mt-0.5 text-[11px]">
                  {t('cli.providers.count', { count: providers.length })}
                </CardDescription>
              </div>
              <Button size="icon" className="size-7 shrink-0" onClick={openCreateProvider}>
                <i className="fa-solid fa-plus" />
                <span className="sr-only">{t('cli.providers.add')}</span>
              </Button>
            </CardHeader>
            <CardContent className="p-0">
              <ScrollPage
                style={{ height: listHeight || undefined }}
                variant="borderless"
                scrollbarVisible="auto"
                className='w-full'
              >
                <div className="flex flex-col gap-1.5 px-2.5 pb-2.5 min-w-0">
                  {!providersLoading && providers.length === 0 && (
                    <div className="py-6 text-center text-xs text-muted-foreground">
                      {t('cli.providers.empty')}
                    </div>
                  )}
                  {providers.map((provider) => {
                    const isApplied = appliedProviderId === provider.id
                    return (
                      <div
                        key={provider.id}
                        role="button"
                        tabIndex={0}
                        onClick={() => setSelectedProviderId(provider.id)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter' || e.key === ' ') {
                            e.preventDefault()
                            setSelectedProviderId(provider.id)
                          }
                        }}
                        className={` min-w-0 rounded-md border px-2.5 py-2 text-left transition-colors ${
                          selectedProviderId === provider.id
                            ? 'border-primary bg-primary/5'
                            : 'hover:bg-muted/50'
                        }`}
                      >
                        <div className="flex min-w-0 items-center justify-between gap-2">
                          <div className="min-w-0 flex-1">
                            <div className="flex min-w-0 items-center gap-1.5">
                              <span className="min-w-0 flex-1 truncate text-xs font-medium">
                                {provider.displayName}
                              </span>
                              {provider.isDefault && (
                                <Badge variant="outline" className="px-1 py-0 text-[9px]">
                                  {t('cli.providers.default')}
                                </Badge>
                              )}
                              {isApplied && (
                                <Badge className="px-1 py-0 text-[9px]">
                                  {t('cli.providers.applied')}
                                </Badge>
                              )}
                            </div>
                            <p className="mt-0.5 truncate text-[11px] text-muted-foreground">
                              {provider.routeMode === 1
                                ? provider.gatewayBaseUrl || t('cli.providers.defaultGateway')
                                : provider.directBaseUrl}
                            </p>
                          </div>
                          <Badge
                            variant={provider.routeMode === 1 ? 'default' : 'secondary'}
                            className="shrink-0 px-1 py-0 text-[9px]"
                          >
                            {t(
                              provider.routeMode === 1
                                ? 'cli.route.gatewayShort'
                                : 'cli.route.directShort'
                            )}
                          </Badge>
                        </div>
                        <div className="mt-1.5 flex justify-end gap-0.5">
                          <Tooltip>
                            <TooltipTrigger asChild>
                              <Button
                                type="button"
                                variant="ghost"
                                size="icon"
                                className="size-6"
                                onClick={(e) => {
                                  e.stopPropagation()
                                  openEditProvider(provider)
                                }}
                              >
                                <i className="fa-solid fa-pen text-[10px]" />
                              </Button>
                            </TooltipTrigger>
                            <TooltipContent side="top" className="text-[11px]">
                              {t('common.edit')}
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
                                  setDeleteTarget({ type: 'provider', item: provider })
                                }}
                              >
                                <i className="fa-solid fa-trash text-[10px]" />
                              </Button>
                            </TooltipTrigger>
                            <TooltipContent side="top" className="text-[11px]">
                              {t('common.delete')}
                            </TooltipContent>
                          </Tooltip>
                        </div>
                      </div>
                    )
                  })}
                </div>
              </ScrollPage>
            </CardContent>
          </Card>

          {/* ── 右侧：模型映射 + 开关 + 操作 ── */}
          <Card className="min-h-0 overflow-hidden">
            <CardHeader className="flex flex-row items-center justify-between gap-2 pb-2.5">
              <div className="min-w-0">
                <CardTitle className="text-sm">{t('cli.mappings.title')}</CardTitle>
                <CardDescription className="mt-0.5 truncate text-[11px]">
                  {selectedProvider
                    ? t('cli.mappings.currentProvider', { name: selectedProvider.displayName })
                    : t('cli.mappings.selectProvider')}
                </CardDescription>
              </div>
            </CardHeader>
            <CardContent className="min-h-0 p-0">
              {!selectedProvider ? (
                <div className="flex items-center justify-center px-6 py-8 text-xs text-muted-foreground">
                  {t('cli.mappings.selectProvider')}
                </div>
              ) : mappingsLoading ? (
                <div className="flex items-center justify-center px-6 py-8 text-xs text-muted-foreground">
                  <i className="fa-solid fa-spinner fa-spin mr-2" />
                  {t('common.loading')}
                </div>
              ) : (
                <ScrollPage
                  style={{ height: listHeight || undefined }}
                  variant="borderless"
                  scrollbarVisible="auto"
                >
                  <div className="space-y-3 px-6 py-3 ">
                    {/* Claude CLI 模型映射编辑器（已包含 API Key 输入） */}
                    <ClaudeModelMapping
                      mappings={localMappings}
                      fallbackModel={localFallbackModel}
                      availableModels={availableModels}
                      gatewayProvider={selectedGatewayProvider}
                      routeMode={selectedProvider.routeMode}
                      apiKey={apiKey}
                      onMappingsChange={handleMappingsChange}
                      onFallbackChange={handleFallbackChange}
                      onApiKeyChange={setApiKey}
                      className="border-0 shadow-none"
                    />

                    {/* Claude CLI 开关区域 */}
                    <div className="rounded-md border bg-background/50 px-3 py-2.5">
                      <p className="mb-2 text-xs font-medium text-muted-foreground">{t('cli.claude.optionsTitle')}</p>
                      <div className="grid grid-cols-2 gap-x-4 gap-y-2">
                        {/* 隐藏 AI 署名 */}
                        <label className="flex items-center gap-2">
                          <Switch
                            checked={switches.hideCoAuthor}
                            onCheckedChange={() => handleSwitchToggle('hideCoAuthor')}
                            className="data-[state=checked]:bg-primary"
                          />
                          <span className="text-xs">{t('cli.claude.hideCoAuthor')}</span>
                        </label>

                        {/* Teammates 模式 */}
                        <label className="flex items-center gap-2">
                          <Switch
                            checked={switches.agentTeams}
                            onCheckedChange={() => handleSwitchToggle('agentTeams')}
                            className="data-[state=checked]:bg-primary"
                          />
                          <span className="text-xs">{t('cli.claude.agentTeams')}</span>
                        </label>

                        {/* 启用 Tool Search */}
                        <label className="flex items-center gap-2">
                          <Switch
                            checked={switches.toolSearch}
                            onCheckedChange={() => handleSwitchToggle('toolSearch')}
                            className="data-[state=checked]:bg-primary"
                          />
                          <span className="text-xs">{t('cli.claude.toolSearch')}</span>
                        </label>

                        {/* 最大强度思考 */}
                        <label className="flex items-center gap-2">
                          <Switch
                            checked={switches.maxEffort}
                            onCheckedChange={() => handleSwitchToggle('maxEffort')}
                            className="data-[state=checked]:bg-primary"
                          />
                          <span className="text-xs">{t('cli.claude.effortMax')}</span>
                        </label>

                        {/* 禁用自动升级 */}
                        <label className="flex items-center gap-2">
                          <Switch
                            checked={switches.disableAutoupdater}
                            onCheckedChange={() => handleSwitchToggle('disableAutoupdater')}
                            className="data-[state=checked]:bg-primary"
                          />
                          <span className="text-xs">{t('cli.claude.disableAutoupdater')}</span>
                        </label>
                      </div>
                    </div>

                    {/* 底部操作按钮 */}
                    <div className="flex items-center justify-end gap-2 pt-1">
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        className="h-7 gap-1.5 px-3 text-xs"
                        onClick={() => setPreviewOpen(true)}
                      >
                        <i className="fa-solid fa-eye text-[10px]" />
                        {t('cli.claude.previewSettings')}
                      </Button>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        className="h-7 gap-1.5 px-3 text-xs"
                        onClick={() => void handleSave()}
                      >
                        <i className="fa-solid fa-floppy-disk text-[10px]" />
                        {t('common.save')}
                      </Button>
                      <Button
                        type="button"
                        size="sm"
                        className="h-7 gap-1.5 px-3 text-xs"
                        onClick={() => void handleApply()}
                      >
                        <i className="fa-solid fa-check text-[10px]" />
                        {t('common.apply')}
                      </Button>
                    </div>
                  </div>
                </ScrollPage>
              )}
            </CardContent>
          </Card>
        </div>
      </TooltipProvider>

      {/* ── 供应商绑定表单 ── */}
      <ProviderBindingForm
        open={providerFormOpen}
        onOpenChange={setProviderFormOpen}
        profileId={profile.id}
        binding={editingProvider}
        providers={enabledGatewayProviders}
        onSubmit={handleProviderSubmit}
      />

      {/* ── 删除确认对话框 ── */}
      <DeleteConfirmDialog
        open={deleteTarget !== null}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null)
        }}
        title={t(
          deleteTarget?.type === 'provider'
            ? 'cli.delete.providerTitle'
            : 'cli.delete.mappingTitle'
        )}
        description={
          deleteTarget
            ? t('cli.delete.description', {
                name:
                  deleteTarget.type === 'provider'
                    ? deleteTarget.item.displayName
                    : deleteTarget.item.cliModelAlias,
              })
            : ''
        }
        onConfirm={() => void handleDelete()}
      />

      {/* ── settings.json 预览对话框 ── */}
      <Dialog open={previewOpen} onOpenChange={setPreviewOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle className="text-base">{t('cli.claude.previewTitle')}</DialogTitle>
          </DialogHeader>
          <CodeEditor
            value={generateSettingsJson()}
            language="json"
            readOnly
            className="min-h-[280px]"
          />
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-8 text-xs"
              onClick={() => setPreviewOpen(false)}
            >
              {t('common.close')}
            </Button>
            <Button
              type="button"
              size="sm"
              className="h-8 gap-1.5 text-xs"
              onClick={() => {
                navigator.clipboard.writeText(generateSettingsJson())
                toast.success(t('common.copied'))
              }}
            >
              <i className="fa-solid fa-copy text-[10px]" />
              {t('common.copy')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
