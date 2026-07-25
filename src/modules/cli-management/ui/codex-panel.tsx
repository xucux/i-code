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
  createCliProvider,
  deleteCliModelMapping,
  deleteCliProvider,
  updateCliProvider,
} from '@/hooks/use-cli-mutation'
import { useProviderList } from '@/hooks/use-provider-list'
import { useExposedModels } from '@/hooks/use-virtual-provider'
import { useTranslation } from '@/modules/i18n/use-translation'
import type { CliModelMapping, CliProfile, CliProvider } from '@/modules/cli-management/types'
import { ProviderBindingForm } from './provider-binding-form'

/** 删除目标类型 */
type DeleteTarget =
  | { type: 'provider'; item: CliProvider }
  | { type: 'mapping'; item: CliModelMapping }

export interface CodexPanelProps {
  profile?: CliProfile
  height: number
}

/**
 * Codex CLI 专属面板
 *
 * 左侧：供应商列表（带"已应用"标识）
 * 右侧：模型映射编辑器 + 预览 config.toml / 应用操作
 */
export function CodexPanel({ profile, height }: CodexPanelProps) {
  const { t } = useTranslation()
  const { providers: gatewayProviders } = useProviderList()
  const { models: exposedModels } = useExposedModels()

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

  // 将 CliModelMapping[] 转换为 ModelMappingItem[]
  const editorMappings = useMemo<ModelMappingItem[]>(() => {
    return mappings.map((m) => ({
      id: m.id,
      role: m.cliModelAlias,
      displayName: m.cliModelAlias,
      actualModel: m.gatewayModelId ?? m.rawModelId ?? '',
      supports1M: false,
    }))
  }, [mappings])

  // mappings 加载完成后同步到本地编辑态
  useEffect(() => {
    setLocalMappings(editorMappings)
  }, [editorMappings])

  // 选中供应商变化时重置 fallback
  useEffect(() => {
    setLocalFallbackModel('')
  }, [selectedProviderId])

  // ── 高度计算 ──
  const listHeight = Math.max(0, height - 76)

  // ── 生成 config.toml 预览 ──
  const generateConfigToml = useCallback(() => {
    if (!selectedProvider) return ''

    // 基础 URL：网关模式用 gatewayBaseUrl，直连模式用 directBaseUrl
    const baseUrl =
      selectedProvider.routeMode === 1
        ? (selectedProvider.gatewayBaseUrl || 'http://127.0.0.1:54321')
        : (selectedProvider.directBaseUrl || '')

    // 优先使用 fallback，其次取第一个映射的实际模型
    const modelName = localFallbackModel.trim() || localMappings[0]?.actualModel || ''

    const lines: string[] = []

    // [model] 段
    if (modelName) {
      lines.push('[model]')
      lines.push(`name = "${modelName}"`)
      lines.push('')
    }

    // [providers.custom] 段
    lines.push('[providers.custom]')
    lines.push(`name = "${selectedProvider.displayName}"`)
    lines.push(`base_url = "${baseUrl}"`)
    // API Key 占位（安全原因不在前端暴露真实 key）
    lines.push('api_key = ""')
    lines.push('')

    return lines.join('\n')
  }, [selectedProvider, localMappings, localFallbackModel])

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

  /** 保存本地映射变更到编辑态 */
  const handleMappingsChange = (updated: ModelMappingItem[]) => {
    setLocalMappings(updated)
  }

  /** 保存兜底模型变更 */
  const handleFallbackChange = (value: string) => {
    setLocalFallbackModel(value)
  }

  /** 应用当前供应商配置 */
  const handleApply = () => {
    if (!selectedProviderId || !selectedProvider) return
    setAppliedProviderId(selectedProviderId)
    toast.success(`已应用供应商「${selectedProvider.displayName}」的配置`)
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

          {/* ── 右侧：模型映射 + 操作按钮 ── */}
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
                    {/* 模型映射编辑器 */}
                    <ModelMappingEditor
                      mappings={localMappings}
                      fallbackModel={localFallbackModel}
                      availableModels={availableModels}
                      onMappingsChange={handleMappingsChange}
                      onFallbackChange={handleFallbackChange}
                      className="border-0 shadow-none"
                    />

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
                        预览 config.toml
                      </Button>
                      <Button
                        type="button"
                        size="sm"
                        className="h-7 gap-1.5 px-3 text-xs"
                        onClick={handleApply}
                      >
                        <i className="fa-solid fa-check text-[10px]" />
                        应用
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
            <DialogTitle className="text-base">预览 config.toml</DialogTitle>
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
              关闭
            </Button>
            <Button
              type="button"
              size="sm"
              className="h-8 gap-1.5 text-xs"
              onClick={() => {
                navigator.clipboard.writeText(generateConfigToml())
                toast.success('已复制到剪贴板')
              }}
            >
              <i className="fa-solid fa-copy text-[10px]" />
              复制
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
