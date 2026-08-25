/**
 * 脚本模板市场对话框
 *
 * 浏览公共仓 catalog，一键应用为本地 draft 模板。
 * 源：https://github.com/xucux/i-code-script-templates
 */

import { useMemo, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import {
  Dialog,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { WideDialogContent } from '@/components/ui/wide-dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { ScrollableTable } from '@/components/ui/scrollable-table'
import { CodeEditor } from '@/components/ui/code-editor'
import { useScriptTemplateMarketplace } from '@/hooks/use-script-template-marketplace'
import {
  applyMarketplaceTemplate,
} from '@/hooks/use-script-template-mutation'
import { previewMarketplaceScript } from '@/hooks/use-script-template-marketplace'
import { toIcodeError } from '@/core/errors'
import { toast } from 'sonner'
import type {
  MarketplaceItemSummary,
  ScriptTemplate,
} from '@/modules/script-template/types'
import { cn } from '@/lib/utils'

export interface ScriptTemplateMarketplaceDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 应用成功：返回本地新建的模板 */
  onApplied: (created: ScriptTemplate) => void
}

function formatTime(iso?: string): string {
  if (!iso) return '—'
  try {
    const d = new Date(iso)
    if (Number.isNaN(d.getTime())) return iso
    return d.toLocaleString()
  } catch {
    return iso
  }
}

export function ScriptTemplateMarketplaceDialog({
  open,
  onOpenChange,
  onApplied,
}: ScriptTemplateMarketplaceDialogProps) {
  const { t } = useTranslation('scriptTemplate')
  const [kindFilter, setKindFilter] = useState<string>('all')
  const [keyword, setKeyword] = useState('')
  const [selected, setSelected] = useState<MarketplaceItemSummary | null>(null)
  const [applying, setApplying] = useState(false)
  const [previewOpen, setPreviewOpen] = useState(false)
  const [previewBody, setPreviewBody] = useState('')
  const [previewLoading, setPreviewLoading] = useState(false)

  const filter = useMemo(
    () => ({
      kind: kindFilter === 'all' ? undefined : kindFilter,
      keyword: keyword.trim() || undefined,
    }),
    [kindFilter, keyword]
  )

  const { items, loading, error, result, refetch } = useScriptTemplateMarketplace(
    open ? filter : { kind: '__skip__' }
  )

  // 对话框关闭时不持续请求：open=false 时用空列表
  const displayItems = open ? items : []
  const displayLoading = open && loading

  const handleApply = async (item: MarketplaceItemSummary) => {
    setApplying(true)
    try {
      const created = await applyMarketplaceTemplate({
        id: item.id,
        conflictStrategy: 'rename',
        publishAfterCreate: false,
      })
      toast.success(t('marketplaceApplySuccess', { name: created.name }))
      onApplied(created)
      onOpenChange(false)
    } catch (err) {
      toast.error(toIcodeError(err).message)
    } finally {
      setApplying(false)
    }
  }

  const handlePreview = async (item: MarketplaceItemSummary) => {
    setSelected(item)
    setPreviewLoading(true)
    setPreviewOpen(true)
    setPreviewBody('')
    try {
      const preview = await previewMarketplaceScript(item.id)
      setPreviewBody(preview.scriptBody)
    } catch (err) {
      toast.error(toIcodeError(err).message)
      setPreviewOpen(false)
    } finally {
      setPreviewLoading(false)
    }
  }

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <WideDialogContent className="flex max-h-[min(640px,90vh)] flex-col gap-3 p-4">
          <DialogHeader className="space-y-1">
            <DialogTitle className="text-sm">{t('marketplaceTitle')}</DialogTitle>
            <p className="text-muted-foreground text-[11px]">
              {t('marketplaceSourceHint')}
              {result?.source ? (
                <span className="ml-1 font-mono text-[10px]">{result.source}</span>
              ) : null}
              {result?.fromCache ? (
                <span className="text-muted-foreground ml-1">({t('marketplaceCached')})</span>
              ) : null}
            </p>
          </DialogHeader>

          <div className="flex flex-wrap items-center gap-2">
            <Select value={kindFilter} onValueChange={setKindFilter}>
              <SelectTrigger className="h-7 w-[120px] text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all" className="text-xs">
                  {t('marketplaceFilter.allKinds')}
                </SelectItem>
                <SelectItem value="balance" className="text-xs">
                  {t('kind.balance')}
                </SelectItem>
              </SelectContent>
            </Select>
            <Input
              value={keyword}
              onChange={(e) => setKeyword(e.target.value)}
              placeholder={t('marketplaceFilter.search')}
              className="h-7 w-[160px] text-xs"
            />
            <Button
              variant="outline"
              size="sm"
              className="h-7 text-xs"
              disabled={displayLoading}
              onClick={() => refetch(true)}
            >
              <i className={cn('fa-solid fa-rotate mr-1.5', displayLoading && 'animate-spin')} />
              {t('marketplaceRefresh')}
            </Button>
          </div>

          {error ? (
            <div className="border-destructive/40 bg-destructive/5 text-destructive rounded-md border px-3 py-2 text-xs">
              {t('marketplaceLoadFailed')}: {error}
              <Button
                variant="link"
                size="sm"
                className="ml-1 h-auto p-0 text-xs"
                onClick={() => refetch(true)}
              >
                {t('marketplaceRefresh')}
              </Button>
            </div>
          ) : null}

          <div className="min-h-0 flex-1 overflow-hidden">
            <ScrollableTable
              style={{ height: 280 }}
              loading={displayLoading}
              density="compact"
            >
              <TableHeader className="sticky top-0 z-10 bg-muted">
                  <TableRow>
                    <TableHead className="text-xs">{t('marketplaceColumns.name')}</TableHead>
                    <TableHead className="text-xs">{t('marketplaceColumns.author')}</TableHead>
                    <TableHead className="text-xs">slug</TableHead>
                    <TableHead className="text-xs">{t('marketplaceColumns.kind')}</TableHead>
                    <TableHead className="text-xs">{t('marketplaceColumns.version')}</TableHead>
                    <TableHead className="text-xs">{t('marketplaceColumns.updatedAt')}</TableHead>
                    <TableHead className="text-xs text-right">
                      {t('marketplaceColumns.actions')}
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {!displayLoading && displayItems.length === 0 ? (
                    <TableRow>
                      <TableCell
                        colSpan={7}
                        className="text-muted-foreground py-8 text-center text-xs"
                      >
                        {t('marketplaceEmpty')}
                      </TableCell>
                    </TableRow>
                  ) : (
                    displayItems.map((item) => (
                      <TableRow
                        key={item.id}
                        className={cn(
                          'cursor-pointer',
                          selected?.id === item.id && 'bg-muted/60'
                        )}
                        onClick={() => setSelected(item)}
                      >
                        <TableCell className="text-xs font-medium">{item.name}</TableCell>
                        <TableCell className="text-[11px]">{item.author}</TableCell>
                        <TableCell className="font-mono text-[11px]">{item.slug}</TableCell>
                        <TableCell className="text-[11px]">
                          {item.kind === 'balance' ? t('kind.balance') : item.kind}
                        </TableCell>
                        <TableCell className="font-mono text-[11px] tabular-nums">
                          {item.version ?? '—'}
                        </TableCell>
                        <TableCell className="text-muted-foreground text-[11px] tabular-nums">
                          {formatTime(item.updatedAt)}
                        </TableCell>
                        <TableCell className="text-right">
                          <div className="inline-flex items-center gap-1">
                            <Button
                              variant="ghost"
                              size="sm"
                              className="h-6 px-1.5 text-[11px]"
                              onClick={(e) => {
                                e.stopPropagation()
                                void handlePreview(item)
                              }}
                            >
                              {t('marketplacePreview')}
                            </Button>
                            <Button
                              variant="default"
                              size="sm"
                              className="h-6 px-1.5 text-[11px]"
                              disabled={applying}
                              onClick={(e) => {
                                e.stopPropagation()
                                void handleApply(item)
                              }}
                            >
                              {applying ? t('marketplaceApplying') : t('marketplaceApply')}
                            </Button>
                          </div>
                        </TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
            </ScrollableTable>
          </div>

          {selected ? (
            <div className="bg-muted/40 rounded-md border px-3 py-2 text-[11px]">
              <div className="mb-1 font-medium text-xs">{selected.name}</div>
              <div className="text-muted-foreground space-y-0.5">
                <div>
                  {t('marketplaceColumns.author')}: {selected.author}
                  {selected.version ? ` · v${selected.version}` : ''}
                  {' · '}
                  {t('marketplaceColumns.createdAt')}: {formatTime(selected.createdAt)}
                </div>
                {selected.description ? (
                  <p className="line-clamp-3 whitespace-pre-wrap">{selected.description}</p>
                ) : null}
                {selected.varList && selected.varList.length > 0 ? (
                  <div className="pt-1">
                    <div className="text-muted-foreground/80 mb-0.5 text-[10px]">
                      {t('marketplaceVars')}
                    </div>
                    <div className="space-y-0.5">
                      {selected.varList.map((v) => (
                        <div
                          key={v.name}
                          className="flex flex-wrap items-center gap-x-1.5 gap-y-0.5"
                        >
                          <code className="bg-background rounded border px-1 py-0 font-mono text-[10px]">
                            {v.name}
                          </code>
                          <span
                            className={cn(
                              'rounded border px-1 py-0 text-[10px]',
                              v.source === 'system'
                                ? 'border-blue-500/30 text-blue-600 dark:text-blue-400'
                                : 'border-amber-500/30 text-amber-600 dark:text-amber-400'
                            )}
                          >
                            {t(
                              v.source === 'system'
                                ? 'marketplaceVarSource.system'
                                : 'marketplaceVarSource.custom'
                            )}
                          </span>
                          <span
                            className={cn(
                              'rounded border px-1 py-0 text-[10px]',
                              v.required
                                ? 'border-destructive/30 text-destructive'
                                : 'border-muted-foreground/30 text-muted-foreground'
                            )}
                          >
                            {t(
                              v.required
                                ? 'marketplaceVarRequired'
                                : 'marketplaceVarOptional'
                            )}
                          </span>
                          {v.description ? (
                            <span className="text-muted-foreground/80 text-[10px]">
                              {v.description}
                            </span>
                          ) : null}
                        </div>
                      ))}
                    </div>
                  </div>
                ) : (
                  <p className="text-muted-foreground/60 pt-1 text-[10px]">
                    {t('marketplaceNoVars')}
                  </p>
                )}
                <p className="text-muted-foreground/80 pt-1">{t('marketplaceApplyHint')}</p>
              </div>
            </div>
          ) : null}

          <DialogFooter className="gap-2 sm:justify-between">
            <Button
              variant="ghost"
              size="sm"
              className="h-7 text-xs"
              onClick={() => onOpenChange(false)}
            >
              {t('marketplaceClose')}
            </Button>
            <Button
              size="sm"
              className="h-7 text-xs"
              disabled={!selected || applying}
              onClick={() => selected && void handleApply(selected)}
            >
              <i className="fa-solid fa-download mr-1.5" />
              {applying ? t('marketplaceApplying') : t('marketplaceApply')}
            </Button>
          </DialogFooter>
        </WideDialogContent>
      </Dialog>

      <Dialog open={previewOpen} onOpenChange={setPreviewOpen}>
        <WideDialogContent className="flex max-h-[min(640px,90vh)] flex-col gap-3 p-4">
          <DialogHeader>
            <DialogTitle className="text-sm">
              {t('marketplacePreview')}
              {selected ? ` — ${selected.name}` : ''}
            </DialogTitle>
          </DialogHeader>
          <div className="min-h-0 overflow-hidden rounded-md border">
            {previewLoading ? (
              <div className="text-muted-foreground flex h-[320px] items-center justify-center text-xs">
                <i className="fa-solid fa-spinner fa-spin mr-2" />
                {t('marketplaceLoading')}
              </div>
            ) : (
              <CodeEditor
                value={previewBody}
                onChange={() => undefined}
                language="javascript"
                readOnly
                autoHeight
                className="max-h-[320px] overflow-auto text-xs"
              />
            )}
          </div>
          <DialogFooter>
            <Button
              size="sm"
              className="h-7 text-xs"
              disabled={!selected || applying}
              onClick={() => selected && void handleApply(selected)}
            >
              {t('marketplaceApply')}
            </Button>
            <Button
              variant="outline"
              size="sm"
              className="h-7 text-xs"
              onClick={() => setPreviewOpen(false)}
            >
              {t('marketplaceClose')}
            </Button>
          </DialogFooter>
        </WideDialogContent>
      </Dialog>
    </>
  )
}
