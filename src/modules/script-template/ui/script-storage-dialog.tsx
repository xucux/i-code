/**
 * 脚本公共存储浏览器对话框
 *
 * 浏览 / 编辑应用数据目录下 `script-storage.json` 的全部键值，
 * 支持新建、编辑、删除、清空与 TTL 设置（与脚本 `storage::*` 函数共用同一存储）。
 */

import { useCallback, useEffect, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Dialog, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { WideDialogContent } from '@/components/ui/wide-dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { ScrollableTable } from '@/components/ui/scrollable-table'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { DeleteConfirmDialog } from '@/components/ui/delete-confirm-dialog'
import { toast } from 'sonner'
import { toIcodeError } from '@/core/errors'
import type { ScriptStorageEntry } from '@/modules/script-template/types'
import {
  clearScriptStorage,
  deleteScriptStorage,
  setScriptStorage,
  viewScriptStorage,
} from '@/hooks/use-script-template-mutation'

export interface ScriptStorageDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

/** 值预览：任意 JSON 值 → 单行文本 */
function formatValue(v: unknown): string {
  if (v === null || v === undefined) return 'null'
  if (typeof v === 'string') return v.length > 60 ? `${v.slice(0, 60)}…` : v
  try {
    const s = JSON.stringify(v)
    return s.length > 60 ? `${s.slice(0, 60)}…` : s
  } catch {
    return String(v)
  }
}

/** 剩余 TTL 文案（毫秒 → 可读文本） */
function formatTtl(expiresAt: number | null): string {
  if (!expiresAt) return '∞'
  const remain = expiresAt - Date.now()
  if (remain <= 0) return 'expired'
  if (remain < 60_000) return `${Math.ceil(remain / 1000)}s`
  if (remain < 3_600_000) return `${Math.ceil(remain / 60_000)}m`
  if (remain < 86_400_000) return `${Math.ceil(remain / 3_600_000)}h`
  return `${Math.ceil(remain / 86_400_000)}d`
}

export function ScriptStorageDialog({ open, onOpenChange }: ScriptStorageDialogProps) {
  const { t } = useTranslation('scriptTemplate')
  const [entries, setEntries] = useState<ScriptStorageEntry[]>([])
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<ScriptStorageEntry | null>(null)
  const [clearConfirm, setClearConfirm] = useState(false)
  // 编辑表单（editingKey 为 null 且 showForm=true 表示新建）
  const [showForm, setShowForm] = useState(false)
  const [editingKey, setEditingKey] = useState<string | null>(null)
  const [formKey, setFormKey] = useState('')
  const [formValue, setFormValue] = useState('')
  const [formTtl, setFormTtl] = useState('')

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const data = await viewScriptStorage()
      setEntries(data)
    } catch (err) {
      toast.error(toIcodeError(err).message)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    if (open) {
      void load()
      setShowForm(false)
      setEditingKey(null)
    }
  }, [open, load])

  const openNew = () => {
    setEditingKey(null)
    setFormKey('')
    setFormValue('')
    setFormTtl('')
    setShowForm(true)
  }

  const openEdit = (entry: ScriptStorageEntry) => {
    setEditingKey(entry.key)
    setFormKey(entry.key)
    setFormValue(JSON.stringify(entry.value, null, 2))
    setFormTtl(entry.expiresAt ? String(Math.max(0, entry.expiresAt - Date.now())) : '')
    setShowForm(true)
  }

  const handleSave = async () => {
    const key = formKey.trim()
    if (!key) {
      toast.error(t('storage.keyRequired'))
      return
    }
    let parsed: unknown
    try {
      parsed = formValue.trim() ? JSON.parse(formValue) : null
    } catch {
      toast.error(t('storage.valueInvalid'))
      return
    }
    const ttl = formTtl.trim() ? Number(formTtl.trim()) : null
    if (ttl != null && (!Number.isFinite(ttl) || ttl <= 0)) {
      toast.error(t('storage.ttlInvalid'))
      return
    }
    setSaving(true)
    try {
      await setScriptStorage(key, parsed, ttl)
      toast.success(t('storage.saved'))
      setShowForm(false)
      setEditingKey(null)
      await load()
    } catch (err) {
      toast.error(toIcodeError(err).message)
    } finally {
      setSaving(false)
    }
  }

  const handleDelete = async () => {
    if (!deleteTarget) return
    try {
      await deleteScriptStorage(deleteTarget.key)
      toast.success(t('storage.deleted'))
      setDeleteTarget(null)
      await load()
    } catch (err) {
      toast.error(toIcodeError(err).message)
    }
  }

  const handleClear = async () => {
    try {
      await clearScriptStorage()
      toast.success(t('storage.cleared'))
      setClearConfirm(false)
      await load()
    } catch (err) {
      toast.error(toIcodeError(err).message)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <WideDialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 text-sm">
            <i className="fa-solid fa-database text-primary" />
            {t('storage.title')}
          </DialogTitle>
        </DialogHeader>

        <p className="text-muted-foreground -mt-1 px-1 text-[11px] leading-relaxed">
          {t('storage.hint')}
        </p>

        <div className="flex items-center gap-2">
          <Button size="sm" className="h-7 text-xs" onClick={openNew} disabled={showForm}>
            <i className="fa-solid fa-plus mr-1.5" />
            {t('storage.new')}
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-7 text-xs"
            onClick={() => void load()}
            disabled={loading}
          >
            <i className="fa-solid fa-rotate mr-1.5" />
            {t('storage.refresh')}
          </Button>
          <div className="ml-auto flex items-center gap-2">
            <span className="text-muted-foreground text-[11px] tabular-nums">
              {entries.length} keys
            </span>
            {entries.length > 0 && (
              <Button
                variant="destructive"
                size="sm"
                className="h-7 text-xs"
                onClick={() => setClearConfirm(true)}
              >
                <i className="fa-solid fa-trash-can mr-1.5" />
                {t('storage.clear')}
              </Button>
            )}
          </div>
        </div>

        {showForm && (
          <div className="border-muted flex flex-col gap-2 rounded-md border p-2">
            <div className="flex items-center gap-2">
              <Input
                value={formKey}
                onChange={(e) => setFormKey(e.target.value)}
                placeholder={t('storage.keyPlaceholder')}
                className="h-7 font-mono text-xs"
                disabled={editingKey != null}
              />
              <Input
                value={formTtl}
                onChange={(e) => setFormTtl(e.target.value)}
                placeholder={t('storage.ttlPlaceholder')}
                className="h-7 w-[150px] text-xs tabular-nums"
              />
            </div>
            <Textarea
              value={formValue}
              onChange={(e) => setFormValue(e.target.value)}
              placeholder={t('storage.valuePlaceholder')}
              className="h-20 font-mono text-xs"
            />
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                className="h-7 text-xs"
                onClick={() => void handleSave()}
                disabled={saving}
              >
                <i className="fa-solid fa-check mr-1.5" />
                {t('storage.save')}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 text-xs"
                onClick={() => {
                  setShowForm(false)
                  setEditingKey(null)
                }}
              >
                {t('storage.cancel')}
              </Button>
            </div>
          </div>
        )}

        <ScrollableTable
          style={{ height: Math.min(320, Math.max(160, entries.length * 34 + 40)) }}
          loading={loading}
          density="compact"
        >
          <Table>
            <TableHeader className="sticky top-0 z-10 bg-muted">
              <TableRow>
                <TableHead className="w-[30%] text-xs">key</TableHead>
                <TableHead className="text-xs">{t('storage.value')}</TableHead>
                <TableHead className="w-[70px] text-xs">{t('storage.ttl')}</TableHead>
                <TableHead className="w-[100px] text-right text-xs">
                  {t('columns.actions')}
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {entries.length === 0 && !loading ? (
                <TableRow>
                  <TableCell
                    colSpan={4}
                    className="text-muted-foreground py-8 text-center text-xs"
                  >
                    {t('storage.empty')}
                  </TableCell>
                </TableRow>
              ) : (
                entries.map((entry) => (
                  <TableRow key={entry.key}>
                    <TableCell className="font-mono text-[11px]">{entry.key}</TableCell>
                    <TableCell className="text-muted-foreground font-mono text-[11px]">
                      {formatValue(entry.value)}
                    </TableCell>
                    <TableCell className="text-[11px] tabular-nums">
                      {formatTtl(entry.expiresAt)}
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="inline-flex items-center gap-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-6 px-1.5 text-[11px]"
                          onClick={() => openEdit(entry)}
                        >
                          {t('edit')}
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="text-destructive h-6 px-1.5 text-[11px]"
                          onClick={() => setDeleteTarget(entry)}
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
      </WideDialogContent>

      <DeleteConfirmDialog
        open={deleteTarget != null}
        onOpenChange={(v) => {
          if (!v) setDeleteTarget(null)
        }}
        onConfirm={() => void handleDelete()}
        title={t('storage.deleteTitle')}
        description={t('storage.deleteConfirm', {
          key: deleteTarget?.key ?? '',
        })}
        confirmText={t('delete')}
      />

      <DeleteConfirmDialog
        open={clearConfirm}
        onOpenChange={setClearConfirm}
        onConfirm={() => void handleClear()}
        title={t('storage.clearTitle')}
        description={t('storage.clearConfirm')}
        confirmText={t('storage.clear')}
      />
    </Dialog>
  )
}
