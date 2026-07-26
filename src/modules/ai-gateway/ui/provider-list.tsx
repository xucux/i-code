import { useState, useMemo } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { useProviderList, useBuiltinProviders } from '@/hooks/use-provider-list'
import { useBalanceSnapshots, refreshProviderBalance } from '@/hooks/use-balance-snapshots'
import {
  createProvider,
  updateProvider,
  deleteProvider,
  exportProvider,
  importProvider,
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
import { ProviderForm } from './provider-form'
import { cn } from '@/lib/utils'
import { toast } from 'sonner'
import type { Provider, AuthConfig, BuiltinProvider } from '@/modules/ai-gateway/types'
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
    useRawBaseUrl: boolean
    authMethod: 'none' | 'api-key'
    isEnabled: boolean
    sortOrder: number
  }> | undefined>(undefined)

  const [builtinOpen, setBuiltinOpen] = useState(false)
  const [builtinSearchQuery, setBuiltinSearchQuery] = useState('')

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
    setFormOpen(true)
  }

  const openCreateFromBuiltin = (builtin: BuiltinProvider) => {
    setEditingProvider(null)
    setInitialFormValues({
      slug: generateSlug(builtin.displayName),
      displayName: builtin.displayName,
      providerType: builtin.providerType,
      baseUrl: builtin.baseUrl,
      useRawBaseUrl: builtin.useRawBaseUrl,
      authMethod: inferAuthMethod(builtin.defaultAuthJson),
      isEnabled: true,
      sortOrder: 0,
    })
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
    useRawBaseUrl: boolean
    auth?: AuthConfig
    isEnabled: boolean
    sortOrder?: number
    balanceProviderJson?: string
    proxyJson?: string
    timeoutJson?: string
    retryJson?: string
  }) => {
    try {
      if (editingProvider) {
        const result = await updateProvider(editingProvider.id, {
          displayName: values.displayName,
          baseUrl: values.baseUrl,
          useRawBaseUrl: values.useRawBaseUrl,
          auth: values.auth,
          isEnabled: values.isEnabled,
          sortOrder: values.sortOrder,
          balanceProviderJson: values.balanceProviderJson,
          proxyJson: values.proxyJson,
          timeoutJson: values.timeoutJson,
          retryJson: values.retryJson,
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
          useRawBaseUrl: values.useRawBaseUrl,
          auth: values.auth,
          isEnabled: values.isEnabled,
          sortOrder: values.sortOrder,
          balanceProviderJson: values.balanceProviderJson,
          proxyJson: values.proxyJson,
          timeoutJson: values.timeoutJson,
          retryJson: values.retryJson,
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
              <Button variant="outline" size="sm" className="h-7 text-xs" onClick={refetch} disabled={loading}>
                <i className={cn('fa-solid fa-rotate', loading && 'animate-spin', 'mr-1.5')} />
                {t('aiGateway.refresh')}
              </Button>
              <Button variant="outline" size="sm" className="h-7 text-xs" onClick={openBuiltinDialog} disabled={builtinLoading}>
                <i className={cn('fa-solid fa-book', builtinLoading && 'animate-spin', 'mr-1.5')} />
                {t('aiGateway.providerList.fromBuiltin')}
              </Button>
              <Button variant="outline" size="sm" className="h-7 text-xs" onClick={() => setImportOpen(true)}>
                <i className="fa-solid fa-file-import mr-1.5" />
                {t('aiGateway.providerList.import')}
              </Button>
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
                const hasBalanceConfig = !!provider.balanceProviderJson
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
    case 'percent':
      return `${item.value}%`
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
