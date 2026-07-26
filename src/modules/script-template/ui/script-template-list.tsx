/**
 * 脚本模板列表（网关总览 Tab）
 */

import { useMemo, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { useScriptTemplateList } from '@/hooks/use-script-templates'
import {
  deleteScriptTemplate,
  setScriptTemplateStatus,
} from '@/hooks/use-script-template-mutation'
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
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { ScrollableTable } from '@/components/ui/scrollable-table'
import { DeleteConfirmDialog } from '@/components/ui/delete-confirm-dialog'
import { toast } from 'sonner'
import { toIcodeError } from '@/core/errors'
import type { ScriptTemplate } from '@/modules/script-template/types'
import { ScriptTemplateStatusBadge } from './script-template-status-badge'
import { ScriptTemplateEditor } from './script-template-editor'

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

export function ScriptTemplateList() {
  const { t } = useTranslation('scriptTemplate')
  const [statusFilter, setStatusFilter] = useState<string>('all')
  const [keyword, setKeyword] = useState('')
  const [pageHeight, pageRef] = useAvailableHeight()
  const [toolbarHeight, toolbarRef] = useAvailableHeight()

  const filter = useMemo(
    () => ({
      kind: 'balance',
      status: statusFilter === 'all' ? undefined : statusFilter,
      keyword: keyword.trim() || undefined,
    }),
    [statusFilter, keyword]
  )

  const { templates, loading, refetch } = useScriptTemplateList(filter)

  const [editorOpen, setEditorOpen] = useState(false)
  const [editing, setEditing] = useState<ScriptTemplate | null>(null)
  const [deleteOpen, setDeleteOpen] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<ScriptTemplate | null>(null)

  const contentHeight = Math.max(0, pageHeight - toolbarHeight - 8)

  const openCreate = () => {
    setEditing(null)
    setEditorOpen(true)
  }

  const openEdit = (item: ScriptTemplate) => {
    setEditing(item)
    setEditorOpen(true)
  }

  const handleStatus = async (item: ScriptTemplate, action: 'publish' | 'disable') => {
    try {
      await setScriptTemplateStatus(item.id, action)
      toast.success(t('statusUpdated'))
      refetch()
    } catch (err) {
      toast.error(toIcodeError(err).message)
    }
  }

  const handleDelete = async () => {
    if (!deleteTarget) return
    try {
      await deleteScriptTemplate(deleteTarget.id)
      toast.success(t('deleteSuccess'))
      setDeleteOpen(false)
      setDeleteTarget(null)
      refetch()
    } catch (err) {
      toast.error(toIcodeError(err).message)
    }
  }

  return (
    <div ref={pageRef} className="flex h-full min-h-0 flex-col gap-2">
      <div ref={toolbarRef} className="flex flex-wrap items-center gap-2">
        <div className="text-sm font-medium">{t('title')}</div>
        <div className="ml-auto flex flex-wrap items-center gap-2">
          <Select value={statusFilter} onValueChange={setStatusFilter}>
            <SelectTrigger className="h-7 w-[110px] text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all" className="text-xs">
                {t('filter.allStatus')}
              </SelectItem>
              <SelectItem value="draft" className="text-xs">
                {t('status.draft')}
              </SelectItem>
              <SelectItem value="active" className="text-xs">
                {t('status.active')}
              </SelectItem>
              <SelectItem value="disabled" className="text-xs">
                {t('status.disabled')}
              </SelectItem>
            </SelectContent>
          </Select>
          <Input
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            placeholder={t('filter.search')}
            className="h-7 w-[140px] text-xs"
          />
          <Button size="sm" className="h-7 text-xs" onClick={openCreate}>
            <i className="fa-solid fa-plus mr-1.5" />
            {t('create')}
          </Button>
        </div>
      </div>

      <ScrollableTable
        style={{ height: contentHeight || undefined }}
        loading={loading}
        density="compact"
      >
        <Table>
          <TableHeader className="sticky top-0 z-10 bg-muted">
            <TableRow>
              <TableHead className="text-xs">{t('columns.name')}</TableHead>
              <TableHead className="text-xs">slug</TableHead>
              <TableHead className="text-xs">{t('columns.status')}</TableHead>
              <TableHead className="text-xs">{t('columns.lastTest')}</TableHead>
              <TableHead className="text-xs">{t('columns.updatedAt')}</TableHead>
              <TableHead className="text-xs text-right">{t('columns.actions')}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {templates.length === 0 && !loading ? (
              <TableRow>
                <TableCell colSpan={6} className="text-muted-foreground py-8 text-center text-xs">
                  {t('empty')}
                  <Button
                    variant="link"
                    size="sm"
                    className="ml-1 h-auto p-0 text-xs"
                    onClick={openCreate}
                  >
                    {t('createFromExample')}
                  </Button>
                </TableCell>
              </TableRow>
            ) : (
              templates.map((item) => (
                <TableRow key={item.id}>
                  <TableCell className="text-xs font-medium">{item.name}</TableCell>
                  <TableCell className="font-mono text-[11px]">{item.slug}</TableCell>
                  <TableCell>
                    <ScriptTemplateStatusBadge
                      status={item.status}
                      labels={{
                        draft: t('status.draft'),
                        active: t('status.active'),
                        disabled: t('status.disabled'),
                      }}
                    />
                  </TableCell>
                  <TableCell className="text-[11px]">
                    {item.lastTestOk == null ? (
                      '—'
                    ) : (
                      <span className="inline-flex items-center gap-1">
                        <i
                          className={
                            item.lastTestOk
                              ? 'fa-solid fa-check text-emerald-500'
                              : 'fa-solid fa-xmark text-destructive'
                          }
                        />
                        <span className="text-muted-foreground tabular-nums">
                          {formatTime(item.lastTestAt)}
                        </span>
                      </span>
                    )}
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
                        onClick={() => openEdit(item)}
                      >
                        {t('edit')}
                      </Button>
                      {item.status !== 'active' ? (
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-6 px-1.5 text-[11px]"
                          onClick={() => void handleStatus(item, 'publish')}
                        >
                          {t('publish')}
                        </Button>
                      ) : (
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-6 px-1.5 text-[11px]"
                          onClick={() => void handleStatus(item, 'disable')}
                        >
                          {t('disable')}
                        </Button>
                      )}
                      <Button
                        variant="ghost"
                        size="sm"
                        className="text-destructive h-6 px-1.5 text-[11px]"
                        onClick={() => {
                          setDeleteTarget(item)
                          setDeleteOpen(true)
                        }}
                      >
                        {t('delete')}
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </ScrollableTable>

      <ScriptTemplateEditor
        open={editorOpen}
        onOpenChange={setEditorOpen}
        template={editing}
        onSaved={() => refetch()}
      />

      <DeleteConfirmDialog
        open={deleteOpen}
        onOpenChange={setDeleteOpen}
        title={t('delete')}
        description={t('deleteConfirm', { name: deleteTarget?.name ?? '' })}
        onConfirm={() => void handleDelete()}
      />
    </div>
  )
}
