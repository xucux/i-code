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
} from '@/hooks/use-virtual-provider-mutation'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { ScrollPage } from '@/components/ui/scroll-page'
import { DeleteConfirmDialog } from '@/components/ui/delete-confirm-dialog'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { VirtualProviderDialog } from './virtual-provider-create-dialog'
import { VirtualModelDialog } from './virtual-model-form'
import {
  VirtualModelGraph,
  type VirtualModelTargetNode,
} from '@/components/ui/virtual-model-graph'
import { cn } from '@/lib/utils'
import { toast } from 'sonner'
import { toIcodeError } from '@/core/errors'
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
        model: route.targetModelId,
        priority: Number(route.priority),
        enabled: route.enabled,
        healthy: route.isHealthy,
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

          <Button size="sm" className="h-8 text-xs" onClick={openCreateProvider}>
            <i className={cn('fa-solid fa-plus', 'mr-1.5')} />
            {t('newProvider')}
          </Button>

          <Button
            size="sm"
            variant="outline"
            className="h-8 text-xs"
            disabled={!selectedProvider}
            onClick={openEditProvider}
          >
            <i className={cn('fa-solid fa-pen', 'mr-1.5')} />
            {t('editProvider')}
          </Button>

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

      {/* 可滚动主内容区 */}
      <ScrollPage variant="borderless" scrollbarVisible="auto" className="flex-1">
        <div className="space-y-4 p-4">
          {!selectedProvider && !providersLoading && (
            <div className="text-muted-foreground flex h-40 items-center justify-center text-sm">
              {t('noProvider')}
            </div>
          )}

          {selectedProvider && (
            <>
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
            </>
          )}

          {(providersLoading || modelsLoading || routesLoading) && (
            <div className="text-muted-foreground py-4 text-center text-xs">{t('loading')}</div>
          )}
        </div>
      </ScrollPage>

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
    </div>
  )
}
