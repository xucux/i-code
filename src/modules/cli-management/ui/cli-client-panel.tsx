import { useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { DeleteConfirmDialog } from '@/components/ui/delete-confirm-dialog'
import { ScrollPage } from '@/components/ui/scroll-page'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { useCliModelMappings, useCliProviders } from '@/hooks/use-cli-profiles'
import {
  createCliModelMapping,
  createCliProvider,
  deleteCliModelMapping,
  deleteCliProvider,
  updateCliModelMapping,
  updateCliProvider,
} from '@/hooks/use-cli-mutation'
import { useProviderList } from '@/hooks/use-provider-list'
import { useExposedModels } from '@/hooks/use-virtual-provider'
import { useTranslation } from '@/modules/i18n/use-translation'
import {
  ModelMappingEditor,
  type ModelMappingItem,
} from '@/components/ui/model-mapping-editor'
import type { CliModelMapping, CliProfile, CliProvider } from '@/modules/cli-management/types'
import { ModelMappingForm } from './model-mapping-form'
import { ProviderBindingForm } from './provider-binding-form'

type DeleteTarget =
  | { type: 'provider'; item: CliProvider }
  | { type: 'mapping'; item: CliModelMapping }

export interface CliClientPanelProps {
  profile?: CliProfile
  height: number
}

/** 单个固定 CLI 客户端的供应商与模型映射工作台。 */
export function CliClientPanel({ profile, height }: CliClientPanelProps) {
  const { t } = useTranslation()
  const { providers: gatewayProviders } = useProviderList()
  const { models: exposedModels } = useExposedModels()
  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(null)
  const { providers, loading: providersLoading, refetch: refetchProviders } = useCliProviders(
    profile?.id ?? null
  )
  const { mappings, loading: mappingsLoading, refetch: refetchMappings } =
    useCliModelMappings(selectedProviderId)

  const [providerFormOpen, setProviderFormOpen] = useState(false)
  const [editingProvider, setEditingProvider] = useState<CliProvider | null>(null)
  const [mappingFormOpen, setMappingFormOpen] = useState(false)
  const [editingMapping, setEditingMapping] = useState<CliModelMapping | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<DeleteTarget | null>(null)

  useEffect(() => {
    setSelectedProviderId(null)
  }, [profile?.id])

  useEffect(() => {
    if (providers.length === 0) {
      setSelectedProviderId(null)
      return
    }
    if (!providers.some((provider) => provider.id === selectedProviderId)) {
      const preferred = providers.find((provider) => provider.isDefault) ?? providers[0]
      setSelectedProviderId(preferred.id)
    }
  }, [providers, selectedProviderId])

  const selectedProvider = providers.find((provider) => provider.id === selectedProviderId)
  const enabledGatewayProviders = gatewayProviders.filter((provider) => provider.isEnabled)

  const availableModels = useMemo(() => {
    if (!selectedProvider?.providerId || selectedProvider.routeMode !== 1) return []
    const gatewayProvider = gatewayProviders.find(
      (provider) => provider.id === selectedProvider.providerId
    )
    if (!gatewayProvider) return []
    return exposedModels
      .filter((model) => model.providerSlug === gatewayProvider.slug)
      .map((model) => `${model.providerSlug}/${model.modelId}`)
  }, [exposedModels, gatewayProviders, selectedProvider])

  const editorMappings = useMemo<ModelMappingItem[]>(() => {
    return mappings.map((mapping) => ({
      id: mapping.id,
      role: mapping.cliModelAlias,
      displayName: mapping.cliModelAlias,
      actualModel: mapping.gatewayModelId ?? mapping.rawModelId ?? '',
      supports1M: false,
    }))
  }, [mappings])

  const listHeight = Math.max(0, height - 76)

  const openCreateProvider = () => {
    setEditingProvider(null)
    setProviderFormOpen(true)
  }

  const openEditProvider = (provider: CliProvider) => {
    setEditingProvider(provider)
    setProviderFormOpen(true)
  }

  const openCreateMapping = () => {
    setEditingMapping(null)
    setMappingFormOpen(true)
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

  const handleMappingSubmit = async (values: {
    cliModelAlias: string
    gatewayModelId?: string
    rawModelId?: string
    inputMode: 'select' | 'manual'
  }) => {
    if (!selectedProviderId) return
    const normalized = {
      ...values,
      gatewayModelId: values.inputMode === 'select' ? values.gatewayModelId : undefined,
      rawModelId: values.inputMode === 'manual' ? values.rawModelId : undefined,
    }
    const result = editingMapping
      ? await updateCliModelMapping(editingMapping.id, normalized)
      : await createCliModelMapping({ cliProviderId: selectedProviderId, ...normalized })
    if (!result) {
      toast.error(t('cli.messages.saveFailed'))
      return
    }
    toast.success(t(editingMapping ? 'cli.messages.mappingUpdated' : 'cli.messages.mappingCreated'))
    setMappingFormOpen(false)
    void refetchMappings()
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
      setSelectedProviderId(null)
      void refetchProviders()
    } else {
      void refetchMappings()
    }
    setDeleteTarget(null)
    toast.success(t('cli.messages.deleted'))
  }

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
                  {providers.map((provider) => (
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
                      <div className="flex items-center justify-between gap-2">
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-1.5">
                            <span className="truncate text-xs font-medium">
                              {provider.displayName}
                            </span>
                            {provider.isDefault && (
                              <Badge variant="outline" className="px-1 py-0 text-[9px]">
                                {t('cli.providers.default')}
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
                              onClick={(event) => {
                                event.stopPropagation()
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
                              onClick={(event) => {
                                event.stopPropagation()
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
                  ))}
                </div>
              </ScrollPage>
            </CardContent>
          </Card>

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
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    size="icon"
                    className="size-7 shrink-0"
                    disabled={!selectedProvider}
                    onClick={openCreateMapping}
                  >
                    <i className="fa-solid fa-plus" />
                    <span className="sr-only">{t('cli.mappings.add')}</span>
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="top" className="text-[11px]">
                  {t('cli.mappings.add')}
                </TooltipContent>
              </Tooltip>
            </CardHeader>
            <CardContent className="min-h-0">
              {!selectedProvider ? (
                <div className="flex items-center justify-center py-8 text-xs text-muted-foreground">
                  {t('cli.mappings.selectProvider')}
                </div>
              ) : mappingsLoading ? (
                <div className="flex items-center justify-center py-8 text-xs text-muted-foreground">
                  <i className="fa-solid fa-spinner fa-spin mr-2" />
                  {t('common.loading')}
                </div>
              ) : (
                <ModelMappingEditor
                  mappings={editorMappings}
                  fallbackModel=""
                  availableModels={availableModels}
                  className="border-0 shadow-none"
                />
              )}
            </CardContent>
          </Card>
        </div>
      </TooltipProvider>

      <ProviderBindingForm
        open={providerFormOpen}
        onOpenChange={setProviderFormOpen}
        profileId={profile.id}
        binding={editingProvider}
        providers={enabledGatewayProviders}
        onSubmit={handleProviderSubmit}
      />

      <ModelMappingForm
        open={mappingFormOpen}
        onOpenChange={setMappingFormOpen}
        providerId={selectedProviderId ?? ''}
        mapping={editingMapping}
        routeMode={selectedProvider?.routeMode}
        availableModels={availableModels.map((value) => ({ value, label: value }))}
        onSubmit={handleMappingSubmit}
      />

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
    </>
  )
}
