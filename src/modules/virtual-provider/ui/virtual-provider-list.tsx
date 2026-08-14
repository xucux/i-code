import { useMemo, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { useProviderList } from '@/hooks/use-provider-list'
import {
  useVirtualProviderList,
  useVirtualModels,
  useVirtualRoutesByProvider,
} from '@/hooks/use-virtual-provider'
import {
  deleteVirtualProvider,
  deleteVirtualModel,
  updateVirtualProvider,
  generatePreset,
} from '@/hooks/use-virtual-provider-mutation'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { ScrollPage } from '@/components/ui/scroll-page'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { DeleteConfirmDialog } from '@/components/ui/delete-confirm-dialog'
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
import { VirtualProviderDialog } from './virtual-provider-create-dialog'
import { VirtualModelDialog } from './virtual-model-form'
import { RouteHistoryTab } from './route-history-tab'
import {
  VirtualModelGraph,
  type VirtualModelTargetNode,
} from '@/components/ui/virtual-model-graph'
import { cn } from '@/lib/utils'
import { toast } from 'sonner'
import { toIcodeError } from '@/core/errors'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import type { VirtualProvider, VirtualModel } from '@/modules/virtual-provider/types'
import type { Provider } from '@/modules/ai-gateway/types'

type DeleteTarget =
  | { type: 'provider'; item: VirtualProvider }
  | { type: 'model'; item: VirtualModel }

/**
 * 虚拟供应商页面
 *
 * 页面结构：
 * 1. 顶部工具栏：供应商选择器、新建/编辑供应商、删除供应商、新建虚拟模型。
 * 2. 供应商信息卡：展示当前供应商基础信息，并提供启用开关。
 * 3. 父级模型列表：使用 VirtualModelGraph 展示该供应商下的全部虚拟模型及其子级路由；
 *    点击父级节点可打开编辑弹窗，调整模型信息或子级路由。
 */
export function VirtualProviderList() {
  const { t } = useTranslation('virtualProvider')
  const { providers, loading: providersLoading, refetch: refetchProviders } =
    useVirtualProviderList()
  const { providers: realProviders } = useProviderList()

  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(null)
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null)

  const { models, loading: modelsLoading, refetch: refetchModels } =
    useVirtualModels(selectedProviderId)
  const { routes, loading: routesLoading, refetch: refetchRoutes } =
    useVirtualRoutesByProvider(selectedProviderId)

  // ===== 供应商弹窗状态 =====
  const [providerDialogOpen, setProviderDialogOpen] = useState(false)
  const [editingProvider, setEditingProvider] = useState<VirtualProvider | null>(null)

  // ===== 虚拟模型弹窗状态 =====
  const [modelDialogOpen, setModelDialogOpen] = useState(false)
  const [editingModel, setEditingModel] = useState<VirtualModel | null>(null)

  // ===== 删除确认状态 =====
  const [deleteOpen, setDeleteOpen] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<DeleteTarget | null>(null)

  // ===== 一键生成状态 =====
  const [presetDialogOpen, setPresetDialogOpen] = useState(false)
  const [generating, setGenerating] = useState(false)

  const selectedProvider = providers.find((p) => p.id === selectedProviderId)

  const providerById = useMemo(() => {
    const map = new Map<string, Provider>()
    for (const provider of realProviders) {
      map.set(provider.id, provider)
    }
    return map
  }, [realProviders])

  const openCreateProvider = () => {
    setEditingProvider(null)
    setProviderDialogOpen(true)
  }

  const openEditProvider = () => {
    if (!selectedProvider) return
    setEditingProvider(selectedProvider)
    setProviderDialogOpen(true)
  }

  const openCreateModel = () => {
    if (!selectedProviderId) return
    setEditingModel(null)
    setModelDialogOpen(true)
  }

  const openEditModel = (model: VirtualModel) => {
    setEditingModel(model)
    setModelDialogOpen(true)
  }

  const openDelete = (target: DeleteTarget) => {
    setDeleteTarget(target)
    setDeleteOpen(true)
  }

  const handleProviderChange = (value: string) => {
    setSelectedProviderId(value || null)
    setSelectedModelId(null)
  }

  const handleToggleProviderEnabled = async (enabled: boolean) => {
    if (!selectedProvider) return
    try {
      await updateVirtualProvider(selectedProvider.id, { isEnabled: enabled })
      toast.success(enabled ? t('providerEnabled') : t('providerDisabled'))
      void refetchProviders()
    } catch (err) {
      toast.error(toIcodeError(err).message)
    }
  }

  const handleConfirmDelete = async () => {
    if (!deleteTarget) return
    try {
      if (deleteTarget.type === 'provider') {
        await deleteVirtualProvider(deleteTarget.item.id)
        if (selectedProviderId === deleteTarget.item.id) {
          setSelectedProviderId(null)
          setSelectedModelId(null)
        }
        void refetchProviders()
      } else {
        await deleteVirtualModel(deleteTarget.item.id)
        if (selectedModelId === deleteTarget.item.id) {
          setSelectedModelId(null)
        }
        void refetchModels()
        void refetchRoutes()
      }
      toast.success(t('deleteSuccess'))
      setDeleteOpen(false)
    } catch (err) {
      toast.error(toIcodeError(err).message)
    }
  }

  const deleteTitle = deleteTarget
    ? deleteTarget.type === 'provider'
      ? t('deleteProvider')
      : t('deleteModel')
    : ''
  const deleteDescription = deleteTarget
    ? t('deleteConfirmDescription', {
        name:
          deleteTarget.type === 'provider'
            ? deleteTarget.item.name
            : deleteTarget.item.modelId,
      })
    : ''

  /** 一键生成虚拟供应商 + 三个虚拟模型 */
  const handleGeneratePreset = async () => {
    setGenerating(true)
    try {
      const result = await generatePreset({})
      // 刷新列表并自动选中新生成的供应商
      await refetchProviders()
      setSelectedProviderId(result.provider.id)
      setSelectedModelId(null)
      // 构造结果消息
      const slotMsgs = result.slots
        .map((s) => {
          const status = s.empty ? t('generatePresetSlotEmpty', { modelId: s.modelId }) : `${s.routeCount} 条路由`
          return `${s.displayName || s.modelId}（${status}）`
        })
        .join('、')
      toast.success(t('generatePresetSuccess', { name: result.provider.name, details: slotMsgs }))
      setPresetDialogOpen(false)
    } catch (err) {
      const icodeErr = toIcodeError(err)
      // alias 冲突单独提示
      if (icodeErr.code === 'CONFLICT' || String(icodeErr.message).includes('已存在')) {
        toast.error(t('generatePresetAliasConflict'))
      } else {
        toast.error(icodeErr.message)
      }
    } finally {
      setGenerating(false)
    }
  }

  /** 把路由数据转换为 VirtualModelGraph 节点 */
  const graphTargets: VirtualModelTargetNode[] = useMemo(() => {
    const parents: VirtualModelTargetNode[] = models.map((model) => ({
      id: model.id,
      provider: selectedProvider?.name ?? '',
      model: model.displayName ?? model.modelId,
      priority: 0,
      enabled: model.isEnabled,
    }))

    const children: VirtualModelTargetNode[] = routes.map((route) => {
      const realProvider = providerById.get(route.targetProviderId)
      return {
        id: route.id,
        parentId: route.virtualModelId,
        provider: realProvider?.displayName ?? route.targetProviderId,
        // 展示模型 ID 全称：{provider_slug}/{model_id}
        model: realProvider
          ? `${realProvider.slug}/${route.targetModelId}`
          : route.targetModelId,
        priority: Number(route.priority),
        enabled: route.enabled,
        healthy: route.isHealthy,
        lastHealthyAt: route.lastHealthyAt,
        lastCheckAt: route.lastCheckAt,
        consecutiveFailures: route.consecutiveFailures,
        lastErrorText: route.lastErrorText,
        lastCheckDurationMs: route.lastCheckDurationMs,
        weight: route.weight,
      }
    })

    return [...parents, ...children]
  }, [models, routes, selectedProvider, providerById])

  const handleSelectParent = (id: string) => {
    const model = models.find((m) => m.id === id)
    if (model) {
      openEditModel(model)
    }
  }

  return (
    <div className="flex h-full flex-col gap-3">
      {/* 顶部工具栏 */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Select value={selectedProviderId ?? ''} onValueChange={handleProviderChange}>
            <SelectTrigger className="h-8 w-56 text-xs">
              <SelectValue placeholder={t('selectProvider')} />
            </SelectTrigger>
            <SelectContent>
              {providers.map((provider) => (
                <SelectItem key={provider.id} value={provider.id} className="text-xs">
                  <span className="truncate">{provider.name}</span>
                  <span className="text-muted-foreground ml-1 text-[10px]">({provider.alias})</span>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          <TooltipProvider delayDuration={200}>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button size="icon" className="size-8" onClick={openCreateProvider}>
                  <i className="fa-solid fa-plus" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="bottom" className="text-[11px]">
                {t('newProvider')}
              </TooltipContent>
            </Tooltip>

            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  size="icon"
                  variant="outline"
                  className="size-8"
                  disabled={!selectedProvider}
                  onClick={openEditProvider}
                >
                  <i className="fa-solid fa-pen" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="bottom" className="text-[11px]">
                {t('editProvider')}
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>

          <Button
            size="sm"
            variant="ghost"
            className="h-8 text-xs text-destructive hover:text-destructive"
            disabled={!selectedProvider}
            onClick={() => selectedProvider && openDelete({ type: 'provider', item: selectedProvider })}
          >
            <i className="fa-solid fa-trash" />
          </Button>
        </div>

        <div className="flex items-center gap-2">
          <Button
            size="sm"
            variant="outline"
            className="h-8 text-xs"
            onClick={() => setPresetDialogOpen(true)}
          >
            <i className={cn('fa-solid fa-wand-magic-sparkles', 'mr-1.5')} />
            {t('generatePreset')}
          </Button>

          <Button
            size="sm"
            variant="secondary"
            className="h-8 text-xs"
            disabled={!selectedProvider}
            onClick={openCreateModel}
          >
            <i className={cn('fa-solid fa-plus', 'mr-1.5')} />
            {t('newModel')}
          </Button>
        </div>
      </div>

      {/* 供应商信息卡 */}
      {selectedProvider && (
        <Card>
          <CardContent className="flex flex-wrap items-center gap-3 py-2 text-xs">
            <span className="font-medium">{selectedProvider.name}</span>
            <Badge variant={selectedProvider.isEnabled ? 'default' : 'secondary'} className="text-[10px]">
              {selectedProvider.isEnabled ? t('enabled') : t('disabled')}
            </Badge>
            <span className="text-muted-foreground font-mono">{selectedProvider.alias}</span>
            <span className="text-muted-foreground">
              {t('strategy')}：{selectedProvider.strategy}
            </span>
            <div className="ml-auto flex items-center gap-2">
              <span className="text-muted-foreground">{t('enabledVirtualProvider')}</span>
              <Switch
                checked={selectedProvider.isEnabled}
                onCheckedChange={handleToggleProviderEnabled}
                className="data-[state=checked]:bg-primary"
              />
            </div>
          </CardContent>
        </Card>
      )}

      {/* 主内容区：未选择供应商时展示占位；选中后展示「模型列表 / 路由历史」Tab */}
      <div className="min-h-0 flex-1">
        {!selectedProvider && !providersLoading && (
          <div className="text-muted-foreground flex h-40 items-center justify-center text-sm">
            {t('noProvider')}
          </div>
        )}

        {(providersLoading || modelsLoading || routesLoading) && !selectedProvider && (
          <div className="text-muted-foreground py-4 text-center text-xs">{t('loading')}</div>
        )}

        {selectedProvider && (
          <Tabs defaultValue="models" className="flex h-full flex-col gap-2">
            <TabsList className="h-8 w-fit">
              <TabsTrigger value="models" className="text-xs">{t('tabModelList')}</TabsTrigger>
              <TabsTrigger value="history" className="text-xs">{t('tabRouteHistory')}</TabsTrigger>
            </TabsList>

            {/* 模型列表 Tab */}
            <TabsContent value="models" className="min-h-0 flex-1">
              <ScrollPage variant="borderless" scrollbarVisible="auto" className="h-full">
                <div className="space-y-4 p-4">
                  <Card>
                    <CardHeader className="pb-2">
                      <CardTitle className="text-xs font-medium">{t('modelList')}</CardTitle>
                    </CardHeader>
                    <CardContent>
                      <VirtualModelGraph
                        virtualModel={selectedProvider.name}
                        targets={graphTargets}
                        selectedId={selectedModelId ?? undefined}
                        editMode
                        onSelectParent={handleSelectParent}
                        renderParentActions={(target) => {
                          const model = models.find((m) => m.id === target.id)
                          if (!model) return null
                          return (
                            <>
                              <Button
                                type="button"
                                variant="ghost"
                                size="icon"
                                className="size-6"
                                onClick={() => openEditModel(model)}
                              >
                                <i className="fa-solid fa-pen text-[10px]" />
                              </Button>
                              <Button
                                type="button"
                                variant="ghost"
                                size="icon"
                                className="size-6 text-destructive hover:text-destructive"
                                onClick={() => openDelete({ type: 'model', item: model })}
                              >
                                <i className="fa-solid fa-trash text-[10px]" />
                              </Button>
                            </>
                          )
                        }}
                      />
                    </CardContent>
                  </Card>

                  {models.length === 0 && !modelsLoading && (
                    <div className="text-muted-foreground py-6 text-center text-xs">
                      {t('noModels')}
                    </div>
                  )}

                  {(modelsLoading || routesLoading) && (
                    <div className="text-muted-foreground py-4 text-center text-xs">{t('loading')}</div>
                  )}
                </div>
              </ScrollPage>
            </TabsContent>

            {/* 路由历史 Tab */}
            <TabsContent value="history" className="min-h-0 flex-1">
              <RouteHistoryTab
                virtualProviderId={selectedProviderId}
                providerMap={providerById}
              />
            </TabsContent>
          </Tabs>
        )}
      </div>

      <VirtualProviderDialog
        open={providerDialogOpen}
        onOpenChange={setProviderDialogOpen}
        provider={editingProvider}
        onSuccess={() => {
          void refetchProviders()
          if (editingProvider) {
            // 编辑后保持当前选中
            void refetchModels()
            void refetchRoutes()
          } else {
            // 新建后列表会刷新，用户手动选择
            setSelectedProviderId(null)
            setSelectedModelId(null)
          }
        }}
      />

      <VirtualModelDialog
        open={modelDialogOpen}
        onOpenChange={setModelDialogOpen}
        virtualProviderId={selectedProviderId ?? ''}
        strategy={selectedProvider?.strategy as ('fallback' | 'on_all' | 'load_balance') | undefined}
        model={editingModel}
        onSuccess={() => {
          void refetchModels()
          void refetchRoutes()
        }}
      />

      <DeleteConfirmDialog
        open={deleteOpen}
        onOpenChange={setDeleteOpen}
        title={deleteTitle}
        description={deleteDescription}
        onConfirm={handleConfirmDelete}
      />

      {/* 一键生成确认弹窗 */}
      <Dialog open={presetDialogOpen} onOpenChange={setPresetDialogOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{t('generatePresetTitle')}</DialogTitle>
            <DialogDescription>{t('generatePresetDesc')}</DialogDescription>
          </DialogHeader>
          <div className="space-y-2 text-xs">
            <p className="text-muted-foreground">{t('generatePresetSlots')}</p>
            <ul className="text-muted-foreground list-inside list-disc space-y-1">
              <li>
                <span className="text-foreground font-mono">virtual_opus</span> — {t('generatePresetSlotOpus')}
              </li>
              <li>
                <span className="text-foreground font-mono">virtual_sonnet</span> — {t('generatePresetSlotSonnet')}
              </li>
              <li>
                <span className="text-foreground font-mono">virtual_haiku</span> — {t('generatePresetSlotHaiku')}
              </li>
            </ul>
            <p className="text-muted-foreground">{t('generatePresetMatchHint')}</p>
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setPresetDialogOpen(false)}
              disabled={generating}
            >
              {t('cancel', { ns: 'common' })}
            </Button>
            <Button size="sm" onClick={() => void handleGeneratePreset()} disabled={generating}>
              {generating ? t('generating') : t('generatePresetConfirm')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
