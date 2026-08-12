import { useState, useMemo, useEffect } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { useProviderList, useBuiltinProviders } from '@/hooks/use-provider-list'
import { useBalanceSnapshots, refreshProviderBalance } from '@/hooks/use-balance-snapshots'
import {
  createProvider,
  updateProvider,
  deleteProvider,
  exportProvider,
  importProvider,
  pingProviders,
} from '@/hooks/use-ai-gateway-mutation'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Textarea } from '@/components/ui/textarea'
import { DeleteConfirmDialog } from '@/components/ui/delete-confirm-dialog'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { ProviderForm } from './provider-form'
import { cn } from '@/lib/utils'
import { toast } from 'sonner'
import { listen } from '@tauri-apps/api/event'
import type { Provider, AuthConfig, BuiltinProvider, BuiltinProviderDefaultModel, PingProviderResult, PingDonePayload } from '@/modules/ai-gateway/types'
import type { IcodeError } from '@/core/errors'
import { extractBalanceListDisplay } from '@/modules/balance/types'
import type { BalanceMetric, BalanceSnapshot } from '@/modules/balance/types'

/**
 * 从内置预设的 displayName 自动生成 slug
 * 如 "Open AI" → "open-ai"，"Anthropic" → "anthropic"
 */
function generateSlug(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, '')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
}

/**
 * 从内置预设的 defaultAuthJson 推断前端使用的认证方式
 *
 * v0.1 只支持 none / api-key；更复杂的 OAuth 统一降级为 none，由用户手动补充。
 */
function inferAuthMethod(defaultAuthJson?: string): 'none' | 'api-key' {
  if (!defaultAuthJson) return 'none'
  try {
    const parsed = JSON.parse(defaultAuthJson) as { method?: string }
    if (parsed.method === 'api-key') return 'api-key'
    return 'none'
  } catch {
    return 'none'
  }
}

/**
 * 处理 Tauri 命令错误，返回用户可读的消息
 */
function getErrorMessage(error: unknown): string {
  if (error && typeof error === 'object') {
    const icodeError = error as IcodeError
    if (icodeError.code && icodeError.message) {
      return `[${icodeError.code}] ${icodeError.message}`
    }
    if ('message' in error) return String((error as Error).message)
  }
  return String(error)
}

/**
 * AI Gateway 供应商列表组件
 *
 * 展示已配置的供应商，支持搜索、新增、编辑、删除。
 * 新增时可选择「手动新增」或「从内置预设选择」。
 * 删除前会弹出二次确认对话框。
 */
export function ProviderList() {
  const { t } = useTranslation()
  const { providers, loading, refetch } = useProviderList()
  const { builtinProviders, loading: builtinLoading, refetch: refetchBuiltin } = useBuiltinProviders()

  const [searchQuery, setSearchQuery] = useState('')
  const [formOpen, setFormOpen] = useState(false)
  const [editingProvider, setEditingProvider] = useState<Provider | null>(null)
  const [initialFormValues, setInitialFormValues] = useState<Partial<{
    slug: string
    displayName: string
    providerType: string
    baseUrl: string
    remark?: string
    useRawBaseUrl: boolean
    authMethod: 'none' | 'api-key'
    isEnabled: boolean
    sortOrder: number
    extraHeaders?: Record<string, string>
    defaultModels?: BuiltinProviderDefaultModel[]
  }> | undefined>(undefined)

  const [builtinOpen, setBuiltinOpen] = useState(false)
  const [builtinSearchQuery, setBuiltinSearchQuery] = useState('')

  // 内置预设推荐项（来源于预设的协议类型/认证方式，用于表单下拉项右侧显示拇指图标）
  const [builtinProviderTypes, setBuiltinProviderTypes] = useState<string[] | undefined>()
  const [builtinAuthMethods, setBuiltinAuthMethods] = useState<string[] | undefined>()

  const [deleteOpen, setDeleteOpen] = useState(false)
  const [deletingProvider, setDeletingProvider] = useState<Provider | null>(null)

  const [exportOpen, setExportOpen] = useState(false)
  const [exportedData, setExportedData] = useState('')
  const [exportingProvider, setExportingProvider] = useState<Provider | null>(null)

  const [importOpen, setImportOpen] = useState(false)
  const [importData, setImportData] = useState('')
  const [importLoading, setImportLoading] = useState(false)

  // 额度详情对话框状态
  const [detailOpen, setDetailOpen] = useState(false)
  const [detailProvider, setDetailProvider] = useState<Provider | null>(null)

  // 额度快照与刷新状态
  const { snapshots, refetch: refetchBalance } = useBalanceSnapshots()
  const [refreshingId, setRefreshingId] = useState<string | null>(null)

  // 网络检测状态
  const [pinging, setPinging] = useState(false)
  const [pingResultOpen, setPingResultOpen] = useState(false)
  const [pingMode, setPingMode] = useState<'direct' | 'proxy' | 'config'>('direct')
  const [pingDone, setPingDone] = useState<PingDonePayload | null>(null)
  const [pingResults, setPingResults] = useState<PingProviderResult[]>([])

  // 按名称搜索过滤
  const filteredProviders = useMemo(() => {
    if (!searchQuery.trim()) return providers
    const q = searchQuery.toLowerCase()
    return providers.filter(
      (p) =>
        p.displayName.toLowerCase().includes(q) ||
        p.slug.toLowerCase().includes(q) ||
        p.providerType.toLowerCase().includes(q)
    )
  }, [providers, searchQuery])

  // 内置预设搜索过滤：支持 id、displayName、displayCnName 模糊匹配
  const filteredBuiltinProviders = useMemo(() => {
    if (!builtinSearchQuery.trim()) return builtinProviders
    const q = builtinSearchQuery.toLowerCase()
    return builtinProviders.filter(
      (b) =>
        b.id.toLowerCase().includes(q) ||
        b.displayName.toLowerCase().includes(q) ||
        b.displayCnName.toLowerCase().includes(q)
    )
  }, [builtinProviders, builtinSearchQuery])

  const openCreate = () => {
    setEditingProvider(null)
    setInitialFormValues(undefined)
    setBuiltinProviderTypes(undefined)
    setBuiltinAuthMethods(undefined)
    setFormOpen(true)
  }

  const openCreateFromBuiltin = (builtin: BuiltinProvider) => {
    setEditingProvider(null)
    setInitialFormValues({
      slug: generateSlug(builtin.displayName),
      displayName: builtin.displayName,
      providerType: builtin.providerType,
      baseUrl: builtin.baseUrl,
      remark: builtin.remark,
      useRawBaseUrl: builtin.useRawBaseUrl,
      authMethod: inferAuthMethod(builtin.defaultAuthJson),
      isEnabled: true,
      sortOrder: 0,
      extraHeaders: builtin.defaultExtraHeaders,
      defaultModels: builtin.defaultModels,
    })
    setBuiltinProviderTypes(builtin.providerTypes)
    setBuiltinAuthMethods(builtin.authMethods)
    setBuiltinOpen(false)
    setFormOpen(true)
  }

  const openBuiltinDialog = () => {
    setBuiltinSearchQuery('')
    void refetchBuiltin()
    setBuiltinOpen(true)
  }

  const openEdit = (provider: Provider) => {
    setEditingProvider(provider)
    setInitialFormValues(undefined)
    // 编辑时尝试匹配内置预设（按 baseUrl + providerType 精确匹配，回退到 providerType）
    const matched = builtinProviders.find(
      (b) => b.baseUrl === provider.baseUrl && b.providerType === provider.providerType,
    ) ?? builtinProviders.find((b) => b.providerType === provider.providerType)
    setBuiltinProviderTypes(matched?.providerTypes)
    setBuiltinAuthMethods(matched?.authMethods)
    setFormOpen(true)
  }

  const openDelete = (provider: Provider) => {
    setDeletingProvider(provider)
    setDeleteOpen(true)
  }

  // 刷新单个供应商的额度
  const handleRefreshBalance = async (provider: Provider) => {
    setRefreshingId(provider.id)
    try {
      await refreshProviderBalance(provider.id)
      toast.success(t('aiGateway.providerList.balanceRefreshSuccess'))
      void refetchBalance()
    } catch (err) {
      toast.error(getErrorMessage(err) || t('aiGateway.providerList.balanceRefreshFailed'))
    } finally {
      setRefreshingId(null)
    }
  }

  // 网络检测：立即弹窗，逐条接收事件推送结果
  const handlePingProviders = async (mode: 'direct' | 'proxy' | 'config') => {
    if (providers.length === 0) {
      toast.info(t('aiGateway.providerList.networkCheckEmpty'))
      return
    }
    // 重置状态并立即打开弹窗
    setPingMode(mode)
    setPingResults([])
    setPingDone(null)
    setPinging(true)
    setPingResultOpen(true)
    try {
      await pingProviders(mode)
    } catch (err) {
      toast.error(getErrorMessage(err))
      setPinging(false)
    }
    // pinging 会在收到 ping-done 事件后设为 false
  }

  // 监听后端逐条推送的检测事件
  useEffect(() => {
    const unlistenResult = listen<PingProviderResult>('provider:ping-result', (event) => {
      setPingResults((prev) => [...prev, event.payload])
    })
    const unlistenDone = listen<PingDonePayload>('provider:ping-done', (event) => {
      setPingDone(event.payload)
      setPinging(false)
    })
    return () => {
      void unlistenResult.then((fn) => fn())
      void unlistenDone.then((fn) => fn())
    }
  }, [])

  const handleExport = async (provider: Provider, includeSecrets: boolean) => {
    try {
      setExportingProvider(provider)
      const data = await exportProvider({ providerId: provider.id, includeSecrets })
      setExportedData(data)
      setExportOpen(true)
      if (includeSecrets) {
        toast.success(t('aiGateway.providerList.exportWithSecretsSuccess'))
      } else {
        toast.success(t('aiGateway.providerList.exportSuccess'))
      }
    } catch (err) {
      toast.error(getErrorMessage(err))
    }
  }

  const handleCopyExport = async () => {
    try {
      await navigator.clipboard.writeText(exportedData)
      toast.success(t('aiGateway.providerList.exportCopied'))
    } catch {
      toast.error(t('aiGateway.providerList.exportCopyFailed'))
    }
  }

  const handleImport = async () => {
    if (!importData.trim()) {
      toast.error(t('aiGateway.providerList.importEmptyError'))
      return
    }
    setImportLoading(true)
    try {
      await importProvider({ data: importData.trim() })
      toast.success(t('aiGateway.providerList.importSuccess'))
      setImportOpen(false)
      setImportData('')
      void refetch()
    } catch (err) {
      toast.error(getErrorMessage(err))
    } finally {
      setImportLoading(false)
    }
  }

  const handleSubmit = async (values: {
    slug: string
    displayName: string
    providerType: string
    baseUrl: string
    remark?: string
    useRawBaseUrl: boolean
    transport?: 'auto' | 'sse' | 'websocket'
    auth?: AuthConfig
    isEnabled: boolean
    sortOrder?: number
    balanceProviderJson?: string
    proxyJson?: string
    timeoutJson?: string
    retryJson?: string
    scriptVariablesJson?: string
    extraHeaders?: Record<string, string>
  }) => {
    try {
      if (editingProvider) {
        const result = await updateProvider(editingProvider.id, {
          displayName: values.displayName,
          baseUrl: values.baseUrl,
          remark: values.remark,
          useRawBaseUrl: values.useRawBaseUrl,
          transport: values.transport,
          auth: values.auth,
          isEnabled: values.isEnabled,
          sortOrder: values.sortOrder,
          balanceProviderJson: values.balanceProviderJson,
          proxyJson: values.proxyJson,
          timeoutJson: values.timeoutJson,
          retryJson: values.retryJson,
          scriptVariablesJson: values.scriptVariablesJson,
          extraHeaders: values.extraHeaders,
        })
        if (result) {
          toast.success(t('aiGateway.providerList.updateSuccess'))
          setFormOpen(false)
          void refetch()
        } else {
          toast.error(t('aiGateway.providerList.updateEmptyError'))
        }
      } else {
        const result = await createProvider({
          slug: values.slug,
          displayName: values.displayName,
          providerType: values.providerType,
          baseUrl: values.baseUrl,
          remark: values.remark,
          useRawBaseUrl: values.useRawBaseUrl,
          transport: values.transport,
          auth: values.auth,
          isEnabled: values.isEnabled,
          sortOrder: values.sortOrder,
          balanceProviderJson: values.balanceProviderJson,
          proxyJson: values.proxyJson,
          timeoutJson: values.timeoutJson,
          retryJson: values.retryJson,
          scriptVariablesJson: values.scriptVariablesJson,
          extraHeaders: values.extraHeaders,
          // 从内置预设创建时自动关联默认模型（后端按 matchModelId 匹配内置模型创建）
          defaultModels: initialFormValues?.defaultModels,
        })
        if (result) {
          toast.success(t('aiGateway.providerList.createSuccess'))
          setFormOpen(false)
          void refetch()
        } else {
          toast.error(t('aiGateway.providerList.createEmptyError'))
        }
      }
    } catch (err) {
      toast.error(getErrorMessage(err))
    }
  }

  const handleConfirmDelete = async () => {
    if (!deletingProvider) return
    try {
      await deleteProvider(deletingProvider.id)
      toast.success(t('aiGateway.providerList.deleteSuccess'))
      setDeleteOpen(false)
      void refetch()
    } catch (err) {
      toast.error(getErrorMessage(err))
    }
  }

  return (
    <>
      <Card className="h-[calc(100%-1rem)]">
        <CardHeader className="pb-2">
          <div className="flex items-center justify-between">
            <CardTitle className="text-base">{t('aiGateway.providers')}</CardTitle>
            <div className="flex items-center gap-2">
              <Button variant="outline" size="sm" className="h-7 text-xs" onClick={openBuiltinDialog} disabled={builtinLoading}>
                <i className={cn('fa-solid fa-book', builtinLoading && 'animate-spin', 'mr-1.5')} />
                {t('aiGateway.providerList.fromBuiltin')}
              </Button>
              <Button variant="outline" size="sm" className="h-7 text-xs" onClick={() => setImportOpen(true)}>
                <i className="fa-solid fa-file-import mr-1.5" />
                {t('aiGateway.providerList.import')}
              </Button>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline" size="sm" className="h-7 text-xs" disabled={pinging}>
                    <i className={cn('fa-solid fa-heart-pulse mr-1.5', pinging && 'animate-pulse')} />
                    {t('aiGateway.providerList.networkCheck')}
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="w-44">
                  <DropdownMenuItem
                    disabled={pinging}
                    onClick={() => handlePingProviders('direct')}
                  >
                    <i className="fa-solid fa-wifi size-4" />
                    <span className="text-xs">{t('aiGateway.providerList.networkCheckDirect')}</span>
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    disabled={pinging}
                    onClick={() => handlePingProviders('proxy')}
                  >
                    <i className="fa-solid fa-network-wired size-4" />
                    <span className="text-xs">{t('aiGateway.providerList.networkCheckProxy')}</span>
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    disabled={pinging}
                    onClick={() => handlePingProviders('config')}
                  >
                    <i className="fa-solid fa-sliders size-4" />
                    <span className="text-xs">{t('aiGateway.providerList.networkCheckConfig')}</span>
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
              <Button size="sm" className="h-7 text-xs" onClick={openCreate}>
                <i className={cn('fa-solid fa-plus', 'mr-1.5')} />
                {t('aiGateway.addProvider')}
              </Button>
            </div>
          </div>
          <CardDescription className="text-xs">{t('aiGateway.providerList.count', { filtered: filteredProviders.length, total: providers.length })}</CardDescription>
          {/* 搜索输入框 */}
          <div className="relative mt-2">
            <i className="fa-solid fa-search text-muted-foreground absolute left-2.5 top-1/2 -translate-y-1/2 text-xs" />
            <Input
              placeholder={t('aiGateway.providerList.searchPlaceholder')}
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="h-8 pl-7 text-xs"
            />
          </div>
        </CardHeader>
        <CardContent className="p-0 h-[calc(100%-8rem)] overflow-auto custom-scrollbar">
        
            <div className="h-30 space-y-2 p-4 pt-0">
              {filteredProviders.length === 0 && !loading && (
                <p className="text-muted-foreground py-4 text-center text-sm">
                  {searchQuery ? t('aiGateway.providerList.noMatches') : t('empty', { ns: 'common' })}
                </p>
              )}
              {filteredProviders.map((provider) => {
                const snapshotRow = snapshots.get(provider.id)
                // 仅当额度监控方法非 none 时才展示刷新/详情 UI
                const hasBalanceConfig = (() => {
                  if (!provider.balanceProviderJson) return false
                  try {
                    const config = JSON.parse(provider.balanceProviderJson) as { method?: string }
                    return config.method !== 'none'
                  } catch {
                    return false
                  }
                })()
                const balanceDisplay = extractBalanceListDisplay(snapshotRow?.snapshot)
                const isRefreshing = refreshingId === provider.id
                const currency = balanceDisplay?.currencySymbol ?? ''
                return (
                <div
                  key={provider.id}
                  className="flex items-center justify-between rounded-md border p-3 transition-colors hover:bg-muted/50"
                >
                  <div className="min-w-0 flex-1 space-y-0.5">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-sm font-medium">{provider.displayName}</span>
                      <Badge variant="outline" className="text-[10px]">
                        {provider.providerType}
                      </Badge>
                    </div>
                    <p className="text-muted-foreground truncate text-xs font-mono">{provider.baseUrl}</p>
                    {/* 额度信息：百分比 / 总额 / 已花费 */}
                    {hasBalanceConfig && (
                      <div className="flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[10px] text-muted-foreground tabular-nums">
                        {isRefreshing && (
                          <span className="text-primary">
                            <i className="fa-solid fa-circle-notch fa-spin mr-1" />
                            {t('aiGateway.providerList.balanceRefreshing')}
                          </span>
                        )}
                        {!isRefreshing && balanceDisplay && (
                          <>
                            {balanceDisplay.percent !== undefined && (
                              <span>
                                {t('aiGateway.providerList.balancePercent')}
                                {Math.round(balanceDisplay.percent)}%
                              </span>
                            )}
                            {balanceDisplay.limit !== undefined && (
                              <span>
                                {t('aiGateway.providerList.balanceLimit')}
                                {currency}
                                {balanceDisplay.limit}
                              </span>
                            )}
                            {balanceDisplay.used !== undefined && (
                              <span>
                                {t('aiGateway.providerList.balanceUsed')}
                                {currency}
                                {balanceDisplay.used}
                              </span>
                            )}
                          </>
                        )}
                        {!isRefreshing && !balanceDisplay && (
                          <span>{t('aiGateway.providerList.balanceNoData')}</span>
                        )}
                      </div>
                    )}
                  </div>
                  <div className="ml-3 flex items-center gap-1">
                    <Badge variant={provider.isEnabled ? 'default' : 'secondary'} className="shrink-0 text-[10px]">
                      {provider.isEnabled ? t('common.enabled') : t('common.disabled')}
                    </Badge>
                    {hasBalanceConfig && (
                      <Button
                        variant="ghost"
                        size="icon"
                        className="size-7"
                        title={t('aiGateway.providerList.balanceRefresh')}
                        disabled={isRefreshing}
                        onClick={() => handleRefreshBalance(provider)}
                      >
                        <i className={cn('fa-solid fa-gauge-high text-xs', isRefreshing && 'animate-spin')} />
                      </Button>
                    )}
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-7"
                      onClick={() => openEdit(provider)}
                    >
                      <i className="fa-solid fa-pen text-xs" />
                    </Button>
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button variant="ghost" size="icon" className="size-7">
                          <i className="fa-solid fa-ellipsis-vertical text-xs" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end" className="w-44">
                        {hasBalanceConfig && (
                          <>
                            <DropdownMenuItem
                              disabled={isRefreshing}
                              onClick={() => handleRefreshBalance(provider)}
                            >
                              <i className={cn('fa-solid fa-gauge-high size-4', isRefreshing && 'animate-spin')} />
                              <span className="text-xs">{t('aiGateway.providerList.balanceRefresh')}</span>
                            </DropdownMenuItem>
                            <DropdownMenuItem
                              onClick={() => {
                                setDetailProvider(provider)
                                setDetailOpen(true)
                              }}
                            >
                              <i className="fa-solid fa-chart-pie size-4" />
                              <span className="text-xs">{t('aiGateway.providerList.balanceDetail')}</span>
                            </DropdownMenuItem>
                            <DropdownMenuSeparator />
                          </>
                        )}
                        <DropdownMenuItem onClick={() => handleExport(provider, false)}>
                          <i className="fa-solid fa-share-nodes size-4" />
                          <span className="text-xs">{t('aiGateway.providerList.exportWithoutSecrets')}</span>
                        </DropdownMenuItem>
                        <DropdownMenuItem onClick={() => handleExport(provider, true)}>
                          <i className="fa-solid fa-key size-4" />
                          <span className="text-xs">{t('aiGateway.providerList.exportWithSecrets')}</span>
                        </DropdownMenuItem>
                        <DropdownMenuSeparator />
                        <DropdownMenuItem
                          className="text-destructive focus:text-destructive"
                          onClick={() => openDelete(provider)}
                        >
                          <i className="fa-solid fa-trash size-4" />
                          <span className="text-xs">{t('common.delete')}</span>
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                </div>
                )
              })}
            </div>

        </CardContent>
      </Card>

      <ProviderForm
        open={formOpen}
        onOpenChange={setFormOpen}
        provider={editingProvider}
        initialValues={initialFormValues}
        builtinProviderTypes={builtinProviderTypes}
        builtinAuthMethods={builtinAuthMethods}
        onSubmit={handleSubmit}
        onProviderUpdated={(provider) => {
          setEditingProvider(provider)
          void refetch()
        }}
      />

      <DeleteConfirmDialog
        open={deleteOpen}
        onOpenChange={setDeleteOpen}
        title={t('aiGateway.providerList.deleteTitle')}
        description={deletingProvider ? t('aiGateway.providerList.deleteDescription', { name: deletingProvider.displayName }) : ''}
        onConfirm={handleConfirmDelete}
      />

      {/* 内置预设选择对话框 */}
      <ProviderBuiltinDialog
        open={builtinOpen}
        onOpenChange={setBuiltinOpen}
        builtinProviders={filteredBuiltinProviders}
        searchQuery={builtinSearchQuery}
        onSearchChange={setBuiltinSearchQuery}
        onSelect={openCreateFromBuiltin}
      />

      {/* 导出结果对话框 */}
      <Dialog open={exportOpen} onOpenChange={setExportOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle className="text-base">
              {t('aiGateway.providerList.exportDialogTitle', { name: exportingProvider?.displayName ?? '' })}
            </DialogTitle>
            <DialogDescription className="text-xs">
              {t('aiGateway.providerList.exportDialogDescription')}
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
                {t('aiGateway.providerList.exportCopy')}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      {/* 导入对话框 */}
      <Dialog open={importOpen} onOpenChange={setImportOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle className="text-base">{t('aiGateway.providerList.importDialogTitle')}</DialogTitle>
            <DialogDescription className="text-xs">
              {t('aiGateway.providerList.importDialogDescription')}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <Textarea
              placeholder={t('aiGateway.providerList.importPlaceholder')}
              value={importData}
              onChange={(e) => setImportData(e.target.value)}
              className="min-h-[160px] font-mono text-xs break-all"
            />
            <div className="flex justify-end gap-2">
              <Button variant="outline" size="sm" className="h-8 text-xs" onClick={() => setImportOpen(false)} disabled={importLoading}>
                {t('common.cancel')}
              </Button>
              <Button size="sm" className="h-8 text-xs" onClick={handleImport} disabled={importLoading}>
                {importLoading && <i className="fa-solid fa-circle-notch fa-spin mr-1.5" />}
                {t('aiGateway.providerList.importConfirm')}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      {/* 额度详情对话框 */}
      <BalanceDetailDialog
        open={detailOpen}
        onOpenChange={setDetailOpen}
        provider={detailProvider}
        snapshot={detailProvider ? snapshots.get(detailProvider.id)?.snapshot : undefined}
      />

      {/* 网络检测结果对话框 */}
      <PingResultDialog
        open={pingResultOpen}
        onOpenChange={setPingResultOpen}
        mode={pingMode}
        results={pingResults}
        done={pingDone}
        loading={pinging}
      />
    </>
  )
}

interface ProviderBuiltinDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  builtinProviders: BuiltinProvider[]
  searchQuery: string
  onSearchChange: (q: string) => void
  onSelect: (builtin: BuiltinProvider) => void
}

/**
 * 内置供应商预设选择对话框
 * 支持搜索过滤
 */
function ProviderBuiltinDialog({
  open,
  onOpenChange,
  builtinProviders,
  searchQuery,
  onSearchChange,
  onSelect,
}: ProviderBuiltinDialogProps) {
  const { t } = useTranslation('aiGateway')

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle className="text-base">{t('providerList.builtinDialogTitle')}</DialogTitle>
          <DialogDescription className="text-xs">{t('providerList.builtinDialogDescription')}</DialogDescription>
        </DialogHeader>
        <div className="relative mb-2">
          <i className="fa-solid fa-search text-muted-foreground absolute left-2.5 top-1/2 -translate-y-1/2 text-xs" />
          <Input
            placeholder={t('providerList.builtinSearchPlaceholder')}
            value={searchQuery}
            onChange={(e) => onSearchChange(e.target.value)}
            className="h-8 pl-7 text-xs"
          />
        </div>
        <ScrollArea className="max-h-[360px]">
          <div className="space-y-1.5">
            {builtinProviders.length === 0 && (
              <p className="text-muted-foreground py-4 text-center text-sm">
                {searchQuery ? t('providerList.builtinNoMatches', { query: searchQuery }) : t('aiGateway.providerList.builtinEmpty')}
              </p>
            )}
            {builtinProviders.map((builtin) => (
              <button
                key={builtin.id}
                type="button"
                onClick={() => onSelect(builtin)}
                className="w-full rounded-md border p-2 text-left transition-colors hover:bg-muted/50"
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="truncate text-xs font-medium">
                    {builtin.displayName}
                    {builtin.displayCnName && (
                      <span className="text-muted-foreground ml-1.5 text-[10px] font-normal">
                        {builtin.displayCnName}
                      </span>
                    )}
                  </span>
                  <Badge variant="outline" className="shrink-0 text-[10px] px-1 py-0">{builtin.providerType}</Badge>
                </div>
                <p className="text-muted-foreground mt-0.5 truncate text-[10px] font-mono leading-tight">{builtin.baseUrl}</p>
              </button>
            ))}
          </div>
        </ScrollArea>
      </DialogContent>
    </Dialog>
  )
}

/**
 * 格式化额度指标数值用于展示
 */
function formatMetricValue(item: BalanceMetric): string {
  switch (item.type) {
    case 'amount':
    case 'integer': {
      const v = item.value
      if (v === undefined || v === null) return '-'
      const n = typeof v === 'number' ? v : Number(v)
      if (!Number.isFinite(n)) return String(v)
      if (Math.abs(n) >= 1000) return n.toLocaleString(undefined, { maximumFractionDigits: 2 })
      return Number.isInteger(n) ? String(n) : n.toFixed(2).replace(/\.?0+$/, '')
    }
    case 'percent': {
      const v = item.value
      if (v === undefined || v === null) return '-'
      const n = typeof v === 'number' ? v : Number(v)
      if (!Number.isFinite(n)) return `${item.value}%`
      return `${n.toFixed(2).replace(/\.?0+$/, '')}%`
    }
    case 'token': {
      const parts: string[] = []
      if (item.remaining !== undefined) parts.push(`剩余: ${item.remaining}`)
      if (item.used !== undefined) parts.push(`已用: ${item.used}`)
      if (item.limit !== undefined) parts.push(`上限: ${item.limit}`)
      return parts.length > 0 ? parts.join(' / ') : '-'
    }
    case 'time':
      return item.value || '-'
    case 'status':
      return item.message || item.value || '-'
    default:
      return '-'
  }
}

/**
 * 获取指标类型的中文标签
 */
function getMetricTypeLabel(type: string): string {
  const map: Record<string, string> = {
    amount: '金额',
    integer: '整数',
    token: 'Token',
    percent: '百分比',
    time: '时间',
    status: '状态',
  }
  return map[type] || type
}

/**
 * 获取指标方向标签
 */
function getDirectionLabel(direction?: string): string {
  const map: Record<string, string> = {
    remaining: '剩余',
    used: '已用',
    limit: '上限',
  }
  return direction ? (map[direction] || direction) : '-'
}

/**
 * 获取时间类型标签
 */
function getTimeKindLabel(kind?: string): string {
  const map: Record<string, string> = {
    expiresAt: '过期时间',
    resetAt: '重置时间',
  }
  return kind ? (map[kind] || kind) : '-'
}

/**
 * 获取状态值标签和颜色
 */
function getStatusInfo(value: string): { label: string; color: string } {
  const map: Record<string, { label: string; color: string }> = {
    ok: { label: '正常', color: 'text-green-600' },
    unlimited: { label: '无限制', color: 'text-blue-600' },
    exhausted: { label: '已耗尽', color: 'text-red-600' },
    error: { label: '错误', color: 'text-red-600' },
    unavailable: { label: '不可用', color: 'text-muted-foreground' },
  }
  return map[value] || { label: value, color: '' }
}

interface BalanceDetailDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  provider: Provider | null
  snapshot?: BalanceSnapshot
}

/**
 * 额度详情对话框
 * 展示 BalanceSnapshot 的所有指标信息，items 以表格形式呈现
 */
function BalanceDetailDialog({
  open,
  onOpenChange,
  provider,
  snapshot,
}: BalanceDetailDialogProps) {
  const { t } = useTranslation()

  const formatTime = (timestamp: number) => {
    try {
      return new Date(timestamp).toLocaleString()
    } catch {
      return String(timestamp)
    }
  }

  const hasData = snapshot && snapshot.items && snapshot.items.length > 0

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="text-base">
            {t('aiGateway.providerList.balanceDetailTitle', { name: provider?.displayName ?? '' })}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {snapshot
              ? `${t('aiGateway.providerList.balanceDetailUpdatedAt')}: ${formatTime(snapshot.updatedAt)}`
              : t('aiGateway.providerList.balanceDetailNoSnapshot')}
          </DialogDescription>
        </DialogHeader>
        {!hasData ? (
          <div className="flex items-center justify-center py-8">
            <p className="text-muted-foreground text-sm">
              {t('aiGateway.providerList.balanceDetailNoData')}
            </p>
          </div>
        ) : (
          <ScrollArea className="flex-1 min-h-0 -mx-6 px-6">
            <Table density="compact" overflow={false}>
              <TableHeader className="sticky top-0 z-10 bg-muted/50">
                <TableRow>
                  <TableHead className="text-xs">指标</TableHead>
                  <TableHead className="text-xs">类型</TableHead>
                  <TableHead className="text-xs">数值</TableHead>
                  <TableHead className="text-xs">方向/基准</TableHead>
                  <TableHead className="text-xs">周期</TableHead>
                  <TableHead className="text-xs">作用域</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {snapshot.items.map((item, index) => (
                  <TableRow
                    key={`${item.id}-${index}`}
                    className={cn(item.primary && 'bg-primary/5')}
                  >
                    <TableCell className="text-xs font-medium">
                      <div className="flex items-center gap-1.5">
                        <span>{item.label || item.id}</span>
                        {item.primary && (
                          <Badge variant="default" className="text-[10px] px-1 py-0">
                            主
                          </Badge>
                        )}
                      </div>
                    </TableCell>
                    <TableCell className="text-xs">
                      <Badge variant="outline" className="text-[10px] px-1 py-0">
                        {getMetricTypeLabel(item.type)}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-xs tabular-nums">
                      {item.type === 'status' ? (
                        <span className={getStatusInfo(item.value).color}>
                          {getStatusInfo(item.value).label}
                        </span>
                      ) : (
                        <span>
                          {item.type === 'amount' && item.currencySymbol && (
                            <span className="text-muted-foreground mr-0.5">{item.currencySymbol}</span>
                          )}
                          {formatMetricValue(item)}
                        </span>
                      )}
                    </TableCell>
                    <TableCell className="text-xs text-muted-foreground">
                      {item.type === 'status' && item.message
                        ? item.message
                        : (item.type === 'amount' || item.type === 'integer') && item.direction
                          ? getDirectionLabel(item.direction)
                          : item.type === 'time' && item.kind
                            ? getTimeKindLabel(item.kind)
                            : item.type === 'percent' && item.basis
                              ? (item.basis === 'remaining' ? '剩余' : '已用')
                              : '-'}
                    </TableCell>
                    <TableCell className="text-xs text-muted-foreground">
                      {item.periodLabel || item.period || '-'}
                    </TableCell>
                    <TableCell className="text-xs text-muted-foreground">
                      {item.scope || '-'}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </ScrollArea>
        )}
      </DialogContent>
    </Dialog>
  )
}

interface PingResultDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: 'direct' | 'proxy' | 'config'
  results: PingProviderResult[]
  done: PingDonePayload | null
  loading: boolean
}

/**
 * 网络检测结果对话框
 *
 * 逐条接收后端推送的检测事件，实时展示在紧凑表格中。
 * 检测进行中时表格逐行追加，标题栏显示进度。
 */
function PingResultDialog({
  open,
  onOpenChange,
  mode,
  results,
  done,
  loading,
}: PingResultDialogProps) {
  const { t } = useTranslation()

  const modeLabel = mode === 'direct'
    ? t('aiGateway.providerList.networkCheckDirect')
    : mode === 'proxy'
      ? t('aiGateway.providerList.networkCheckProxy')
      : t('aiGateway.providerList.networkCheckConfig')

  const summaryText = done
    ? t('aiGateway.providerList.pingResultSummary', { success: done.success, failed: done.failed, total: done.total })
    : loading
      ? t('aiGateway.providerList.pingInProgress', { done: results.length })
      : ''

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col gap-3">
        <DialogHeader>
          <DialogTitle className="text-base">
            {t('aiGateway.providerList.pingResultTitle')}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {modeLabel} — {summaryText}
          </DialogDescription>
        </DialogHeader>
        {/* 滚动容器：使用原生 overflow-auto，避免 Radix ScrollArea 在 Table 上注入 display:table 包裹层导致撑开与高度链断裂 */}
        <div className="min-h-0 flex-1 overflow-auto rounded-md border -mx-1">
          <Table density="compact" overflow={false}>
            <TableHeader className="sticky top-0 z-10 bg-muted/80 backdrop-blur">
              <TableRow>
                <TableHead className="text-xs">供应商</TableHead>
                <TableHead className="text-xs">URL</TableHead>
                <TableHead className="text-xs w-20 whitespace-nowrap">状态</TableHead>
                <TableHead className="text-xs w-20 whitespace-nowrap">延迟</TableHead>
                <TableHead className="text-xs w-20 whitespace-nowrap">状态码</TableHead>
                <TableHead className="text-xs">错误</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {results.map((r) => (
                <TableRow key={r.providerId}>
                  <TableCell className="text-xs font-medium">
                    {r.displayName}
                    <span className="text-muted-foreground ml-1 text-[10px]">({r.slug})</span>
                  </TableCell>
                  <TableCell className="text-xs font-mono text-muted-foreground max-w-[200px] truncate">
                    {r.baseUrl}
                  </TableCell>
                  <TableCell className="text-xs whitespace-nowrap">
                    {r.success ? (
                      <span className="text-emerald-600 whitespace-nowrap"><i className="fa-solid fa-circle-check mr-1" />{t('aiGateway.providerList.pingOk')}</span>
                    ) : (
                      <span className="text-destructive whitespace-nowrap"><i className="fa-solid fa-circle-xmark mr-1" />{t('aiGateway.providerList.pingFail')}</span>
                    )}
                  </TableCell>
                  <TableCell className="text-xs tabular-nums whitespace-nowrap">
                    {r.latencyMs != null ? `${r.latencyMs}ms` : '-'}
                  </TableCell>
                  <TableCell className="text-xs tabular-nums whitespace-nowrap">
                    {r.statusCode ?? '-'}
                  </TableCell>
                  <TableCell className="text-xs text-muted-foreground">
                    <PingErrorCell error={r.error} t={t} />
                  </TableCell>
                </TableRow>
              ))}
              {loading && (
                <TableRow>
                  <TableCell colSpan={6} className="text-center text-xs text-muted-foreground py-3">
                    <i className="fa-solid fa-circle-notch fa-spin mr-1.5" />
                    {t('aiGateway.providerList.pingInProgress', { done: results.length })}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>
      </DialogContent>
    </Dialog>
  )
}

/**
 * 网络检测错误单元格
 *
 * - 无错误：显示占位短横线。
 * - 有错误：默认截断显示，鼠标指针变化提示可点击；点击触发 Popover 弹出完整错误信息（保留换行），
 *   并提供「复制」按钮便于把完整错误外发分析。
 *
 * 单元格不直接使用 `title` 原生 tooltip，因为长错误在原生 tooltip 中不可选中、不可复制，
 * 用户体验差；改用 Popover 后可滚动、可复制。
 */
function PingErrorCell({
  error,
  t,
}: {
  error?: string | null
  t: ReturnType<typeof useTranslation>['t']
}) {
  if (!error) {
    return <span className="text-muted-foreground/60">-</span>
  }

  const handleCopy = () => {
    navigator.clipboard?.writeText(error)
    toast.success(t('aiGateway.providerList.pingErrorCopied'))
  }

  return (
    <Popover>
      <PopoverTrigger asChild>
        <div
          role="button"
          tabIndex={0}
          title={t('aiGateway.providerList.pingErrorClickHint')}
          className="max-w-[220px] truncate cursor-pointer outline-none hover:text-foreground hover:underline focus-visible:ring-1 focus-visible:ring-ring"
        >
          {error}
        </div>
      </PopoverTrigger>
      <PopoverContent side="top" align="end" className="w-80 p-0">
        <div className="flex items-center justify-between border-b px-2 py-1">
          <span className="text-[10px] text-muted-foreground">
            {t('aiGateway.providerList.pingErrorDetail')}
          </span>
          <Button
            size="sm"
            variant="ghost"
            className="h-5 px-1.5 text-[10px]"
            onClick={handleCopy}
          >
            <i className="fa-regular fa-copy mr-1" />
            {t('aiGateway.providerList.pingErrorCopy')}
          </Button>
        </div>
        <pre className="max-h-60 overflow-auto whitespace-pre-wrap break-words p-2 text-[11px] leading-relaxed">
          {error}
        </pre>
      </PopoverContent>
    </Popover>
  )
}
