import { useCallback, useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { CodeEditor } from '@/components/ui/code-editor'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { DeleteConfirmDialog } from '@/components/ui/delete-confirm-dialog'
import { ScrollPage } from '@/components/ui/scroll-page'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import {
  ModelMappingEditor,
  type ModelMappingItem,
} from '@/components/ui/model-mapping-editor'
import { useCliModelMappings, useCliProviders } from '@/hooks/use-cli-profiles'
import {
  createCliModelMapping,
  createCliProvider,
  deleteCliModelMapping,
  deleteCliProvider,
  updateCliModelMapping,
  updateCliProvider,
} from '@/hooks/use-cli-mutation'
import { useCatalogModels, useCatalogProviders, resolveDefaultGatewayKey } from '@/hooks/use-catalog'
import { useTranslation } from '@/modules/i18n/use-translation'
import { invokeCommand } from '@/hooks/use-command'
import { toIcodeError } from '@/core/errors'
import type { CliModelMapping, CliProfile, CliProvider } from '@/modules/cli-management/types'
import { ProviderBindingForm } from './provider-binding-form'

/** 删除目标类型 */
type DeleteTarget =
  | { type: 'provider'; item: CliProvider }
  | { type: 'mapping'; item: CliModelMapping }

/** Codex wire API 类型 */
type CodexWireApi = 'chat' | 'responses'

export interface CodexPanelProps {
  profile?: CliProfile
  height: number
}

/**
 * Codex CLI 专属面板
 *
 * 左侧：供应商列表（带"已应用"标识）
 * 右侧：模型映射编辑器 + API Key + wire_api 选择 + 预览 config.toml / 应用操作
 *
 * 参考 cc switch 项目的 Codex config.toml 结构：
 * - [model] name / model_provider
 * - [model_providers.{provider_id}] name / base_url / api_key / wire_api
 * - [profiles.{alias}] model / model_provider
 */
export function CodexPanel({ profile, height }: CodexPanelProps) {
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

  // ── 本地编辑态映射（ModelMappingEditor 回调写入） ──
  const [localMappings, setLocalMappings] = useState<ModelMappingItem[]>([])
  const [localFallbackModel, setLocalFallbackModel] = useState('')

  // 记录每个模型别名对应的后端 mapping id，用于应用时判断是创建还是更新
  const [mappingIdByRole, setMappingIdByRole] = useState<Record<string, string | undefined>>({})

  // ── Codex API Key（按供应商明文存储在 authJson 中） ──
  const [apiKey, setApiKey] = useState('')

  // ── Codex wire_api（默认 chat，本地网关走 /v1/chat/completions） ──
  const [wireApi, setWireApi] = useState<CodexWireApi>('chat')

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
    const items: ModelMappingItem[] = mappings.map((m) => {
      if (m.cliModelAlias) {
        ids[m.cliModelAlias] = m.id
      }
      return {
        id: m.id,
        role: m.cliModelAlias,
        displayName: m.cliModelAlias,
        actualModel: m.gatewayModelId ?? m.rawModelId ?? '',
        supports1M: false,
      }
    })
    setLocalMappings(items)
    setMappingIdByRole(ids)
  }, [mappings])

  // 选中供应商变化时重置 API Key、fallback 和 wire_api
  useEffect(() => {
    setApiKey('')
    setLocalFallbackModel('')
    setWireApi('chat')
  }, [selectedProviderId])

  // 从供应商 authJson 解析 API Key
  useEffect(() => {
    if (!selectedProvider?.authJson) {
      setApiKey('')
      return
    }
    try {
      const parsed = JSON.parse(selectedProvider.authJson) as { apiKey?: string; wireApi?: CodexWireApi }
      setApiKey(parsed.apiKey ?? '')
      setWireApi(parsed.wireApi === 'responses' ? 'responses' : 'chat')
    } catch {
      setApiKey('')
      setWireApi('chat')
    }
  }, [selectedProvider?.authJson])

  // ── 高度计算 ──
  const listHeight = Math.max(0, height - 76)

  /**
   * 生成 Codex config.toml 预览
   *
   * 结构参考 cc switch 项目解析逻辑：
   * - [model]：默认模型与默认 provider
   * - [model_providers.custom]：自定义 provider 配置
   * - [profiles.{alias}]：每个模型映射对应一个 profile，可用 `codex --profile {alias}` 切换
   */
  const generateConfigToml = useCallback(() => {
    if (!selectedProvider) return ''

    // 基础 URL
    const baseUrl =
      selectedProvider.routeMode === 1
        ? (selectedProvider.gatewayBaseUrl || 'http://127.0.0.1:54321')
        : (selectedProvider.directBaseUrl || '')

    // 默认模型：fallback > 第一个映射的实际模型
    const defaultModel = localFallbackModel.trim() || localMappings[0]?.actualModel || ''

    const lines: string[] = []

    // [model] 段
    if (defaultModel) {
      lines.push('[model]')
      lines.push(`name = "${escapeTomlValue(defaultModel)}"`)
      lines.push('model_provider = "custom"')
      lines.push('')
    }

    // [model_providers.custom] 段
    lines.push('[model_providers.custom]')
    lines.push(`name = "${escapeTomlValue(selectedProvider.displayName)}"`)
    if (baseUrl) {
      lines.push(`base_url = "${escapeTomlValue(baseUrl)}"`)
    }
    // API Key 明文写入预览（用户明确操作后才会真正应用）
    lines.push(`api_key = "${escapeTomlValue(apiKey)}"`)
    lines.push(`wire_api = "${wireApi}"`)
    lines.push('')

    // [profiles.{alias}] 段：每个映射对应一个可切换 profile
    for (const item of localMappings) {
      if (!item.role || !item.actualModel) continue
      const sectionKey = item.role.includes('.') || item.role.includes(' ')
        ? `"${escapeTomlValue(item.role)}"`
        : escapeTomlValue(item.role)
      lines.push(`[profiles.${sectionKey}]`)
      lines.push(`model = "${escapeTomlValue(item.actualModel)}"`)
      lines.push('model_provider = "custom"')
      lines.push('')
    }

    return lines.join('\n')
  }, [selectedProvider, localMappings, localFallbackModel, apiKey, wireApi])

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

  /** 保存本地映射变更到编辑态 */
  const handleMappingsChange = (updated: ModelMappingItem[]) => {
    setLocalMappings(updated)
  }

  /** 保存兜底模型变更 */
  const handleFallbackChange = (value: string) => {
    setLocalFallbackModel(value)
  }

  /** 新增一条空映射 */
  const handleAddMapping = () => {
    const index = localMappings.length + 1
    const next: ModelMappingItem[] = [
      ...localMappings,
      {
        id: `new-${Date.now()}`,
        role: `model-${index}`,
        displayName: `model-${index}`,
        actualModel: '',
        supports1M: false,
      },
    ]
    setLocalMappings(next)
  }

  /** 从 Gateway 供应商 authJson 中导入 API Key 并解密为明文 */
  const handleImportApiKey = useCallback(async () => {
    if (!selectedGatewayProvider) {
      toast.error(t('cli.claude.modelMapping.noProvider'))
      return
    }
    // 虚拟供应商无独立凭证，统一使用网关默认授权 Key
    if (selectedGatewayProvider.isVirtual) {
      const key = await resolveDefaultGatewayKey()
      if (!key) {
        toast.error(t('cli.claude.modelMapping.noApiKeyInProvider'))
        return
      }
      setApiKey(key)
      toast.success(t('cli.claude.modelMapping.apiKeyImported'))
      return
    }
    const auth = selectedGatewayProvider.authJson
      ? (JSON.parse(selectedGatewayProvider.authJson) as { method?: string; apiKey?: string } | undefined)
      : undefined
    if (auth?.method !== 'api-key' || !auth.apiKey) {
      toast.error(t('cli.claude.modelMapping.noApiKeyInProvider'))
      return
    }
    try {
      const plaintext = await invokeCommand<string>('secret_decrypt_text', { value: auth.apiKey })
      setApiKey(plaintext)
      toast.success(t('cli.claude.modelMapping.apiKeyImported'))
    } catch (err) {
      toast.error(toIcodeError(err).message)
    }
  }, [selectedGatewayProvider, t])

  /** 应用当前供应商配置：保存映射列表、API Key 和 wire_api */
  const handleApply = async () => {
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

    // 同时把 API Key 与 wire_api 明文保存到供应商 authJson
    saveResults.push(
      updateCliProvider(selectedProviderId, {
        authJson: JSON.stringify({ apiKey, wireApi }),
      })
    )

    try {
      await Promise.all(saveResults)
      setAppliedProviderId(selectedProviderId)
      toast.success(t('cli.codex.applyProvider'))
      void refetchMappings()
      void refetchProviders()
    } catch {
      toast.error(t('cli.messages.saveFailed'))
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
              >
                <div className="flex flex-col gap-1.5 px-2.5 pb-2.5">
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
                        className={`w-full rounded-md border px-2.5 py-2 text-left transition-colors ${
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

          {/* ── 右侧：模型映射 + API Key + 操作 ── */}
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
                  <div className="space-y-3 px-6 py-3">
                    {/* 模型映射编辑器（Codex 不需要 1M 开关） */}
                    <ModelMappingEditor
                      mappings={localMappings}
                      fallbackModel={localFallbackModel}
                      availableModels={availableModels}
                      onMappingsChange={handleMappingsChange}
                      onFallbackChange={handleFallbackChange}
                      showSupports1M={false}
                      onDeleteMapping={(id) => {
                        const target = mappings.find((m) => m.id === id)
                        if (target) {
                          setDeleteTarget({ type: 'mapping', item: target })
                        } else {
                          // 本地新增的映射未入库，直接移除
                          setLocalMappings((prev) => prev.filter((item) => item.id !== id))
                        }
                      }}
                      className="border-0 shadow-none"
                    />

                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      className="h-7 gap-1 px-2 text-xs"
                      onClick={handleAddMapping}
                    >
                      <i className="fa-solid fa-plus text-[10px]" />
                      {t('cli.mappings.add')}
                    </Button>

                    {/* Codex API Key */}
                    <div className="space-y-1.5 rounded-md border bg-background/50 px-3 py-2.5">
                      <div className="flex items-center justify-between">
                        <Label htmlFor="codex-api-key" className="text-xs font-medium text-muted-foreground">
                          {t('cli.codex.apiKeyLabel')}
                        </Label>
                        <Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          className="h-6 gap-1 px-2 text-[11px]"
                          onClick={() => void handleImportApiKey()}
                          disabled={!selectedGatewayProvider}
                        >
                          <i className="fa-solid fa-key text-[10px]" />
                          {t('cli.claude.modelMapping.importApiKey')}
                        </Button>
                      </div>
                      <Input
                        id="codex-api-key"
                        type="password"
                        value={apiKey}
                        onChange={(e) => setApiKey(e.target.value)}
                        placeholder={t('cli.codex.apiKeyPlaceholder')}
                        className="h-8 text-xs"
                      />
                      <p className="text-xs text-muted-foreground">
                        {t('cli.codex.apiKeyDesc')}
                      </p>
                    </div>

                    {/* Codex wire_api */}
                    <div className="space-y-1.5 rounded-md border bg-background/50 px-3 py-2.5">
                      <Label className="text-xs font-medium text-muted-foreground">
                        {t('cli.codex.wireApiLabel')}
                      </Label>
                      <Select
                        value={wireApi}
                        onValueChange={(value) => setWireApi(value as CodexWireApi)}
                      >
                        <SelectTrigger className="h-8 text-xs">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectGroup>
                            <SelectItem value="chat" className="text-xs">
                              {t('cli.codex.wireApiChat')}
                            </SelectItem>
                            <SelectItem value="responses" className="text-xs">
                              {t('cli.codex.wireApiResponses')}
                            </SelectItem>
                          </SelectGroup>
                        </SelectContent>
                      </Select>
                      <p className="text-xs text-muted-foreground">
                        {t('cli.codex.wireApiHint')}
                      </p>
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
                        {t('cli.codex.previewConfig')}
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

      {/* ── config.toml 预览对话框 ── */}
      <Dialog open={previewOpen} onOpenChange={setPreviewOpen}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle className="text-base">{t('cli.codex.previewTitle')}</DialogTitle>
          </DialogHeader>
          <CodeEditor
            value={generateConfigToml()}
            language="toml"
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
                navigator.clipboard.writeText(generateConfigToml())
                toast.success(t('cli.opencode.exportCopied'))
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

/** 转义 TOML 字符串值中的特殊字符 */
function escapeTomlValue(value: string): string {
  return value.replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\n/g, '\\n')
}
