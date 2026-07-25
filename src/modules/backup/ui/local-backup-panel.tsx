import { useCallback, useEffect, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { ScrollPage } from '@/components/ui/scroll-page'
import { DeleteConfirmDialog } from '@/components/ui/delete-confirm-dialog'
import { LoadingOverlay } from '@/components/ui/loading-overlay'
import { createBackup, deleteBackup, listLocalBackups, restoreBackup } from '@/hooks/use-backup'
import { formatDateTime, isTauri } from '@/core/utils'
import { toast } from 'sonner'
import type { BackupListItem } from '@/modules/backup/types'

export interface LocalBackupPanelProps {
  height: number
  localDirectory?: string
}

/**
 * 本地备份面板
 *
 * 展示本地备份文件列表，支持创建、恢复与删除。
 */
export function LocalBackupPanel({ height, localDirectory }: LocalBackupPanelProps) {
  const { t } = useTranslation()
  const [backups, setBackups] = useState<BackupListItem[]>([])
  const [loading, setLoading] = useState(false)
  const [creating, setCreating] = useState(false)
  const [restoreTarget, setRestoreTarget] = useState<BackupListItem | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<BackupListItem | null>(null)
  const [restoring, setRestoring] = useState(false)

  const load = useCallback(async () => {
    if (!isTauri()) return
    setLoading(true)
    try {
      const items = await listLocalBackups(localDirectory)
      setBackups(items)
    } catch (err) {
      toast.error(t('backup.messages.loadFailed'), { description: String(err) })
    } finally {
      setLoading(false)
    }
  }, [localDirectory])

  useEffect(() => {
    let cancelled = false
    void load().then(() => {
      if (cancelled) {
        // 组件已卸载或 effect 重新触发时忽略旧结果
      }
    })
    return () => { cancelled = true }
  }, [load])

  const handleCreate = async () => {
    setCreating(true)
    try {
      const result = await createBackup({ format: 'zip' })
      toast.success(t('backup.messages.createSuccess'), {
        description: result.path,
      })
      await load()
    } catch (err) {
      toast.error(t('backup.messages.createFailed'), { description: String(err) })
    } finally {
      setCreating(false)
    }
  }

  const handleRestore = async (item: BackupListItem) => {
    setRestoreTarget(null)
    setRestoring(true)
    try {
      const result = await restoreBackup(item.path)
      if (result.success) {
        // 需要重启时由后端自动调用 process::restart，前端保持遮罩直到应用退出
        if (!result.needsRestart) {
          toast.success(t('backup.messages.restoreSuccess'))
          setRestoring(false)
        }
      } else {
        toast.error(t('backup.messages.restoreFailed'), {
          description: result.errorMessage ?? result.errorCode ?? '',
        })
        setRestoring(false)
      }
    } catch (err) {
      toast.error(t('backup.messages.restoreFailed'), { description: String(err) })
      setRestoring(false)
    }
  }

  const handleDelete = async (item: BackupListItem) => {
    setDeleteTarget(null)
    try {
      await deleteBackup('local', item.path)
      toast.success(t('backup.messages.deleteSuccess'))
      await load()
    } catch (err) {
      toast.error(t('backup.messages.deleteFailed'), { description: String(err) })
    }
  }

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / 1024 / 1024).toFixed(2)} MB`
  }

  return (
    <div className="flex h-full flex-col gap-3">
      <div className="flex items-center justify-between">
        <p className="text-muted-foreground text-xs">{t('backup.local.description')}</p>
        <Button size="sm" onClick={handleCreate} disabled={creating || loading}>
          <i className="fa-solid fa-plus mr-1.5" />
          {t('backup.local.create')}
        </Button>
      </div>

      <ScrollPage style={{ height }} variant="borderless" scrollbarVisible="auto">
        {backups.length === 0 ? (
          <div className="flex h-32 flex-col items-center justify-center text-muted-foreground text-sm">
            <i className="fa-solid fa-box-open mb-2 text-lg" />
            {t('backup.empty')}
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            {backups.map((item) => (
              <div
                key={item.path}
                className="flex items-center justify-between rounded-md border p-2.5"
              >
                <div className="flex flex-col gap-0.5">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium">
                      {formatDateTime(item.createdAt)}
                    </span>
                    {item.encrypted && (
                      <Badge variant="secondary" className="text-[10px] h-4 px-1">
                        {t('backup.encrypted')}
                      </Badge>
                    )}
                  </div>
                  <span className="text-muted-foreground text-xs">{formatSize(item.sizeBytes)}</span>
                </div>
                <div className="flex items-center gap-1.5">
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7 px-2 text-xs"
                    onClick={() => setRestoreTarget(item)}
                  >
                    <i className="fa-solid fa-rotate-left mr-1" />
                    {t('backup.restore')}
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7 px-2 text-xs text-destructive hover:text-destructive"
                    onClick={() => setDeleteTarget(item)}
                  >
                    <i className="fa-solid fa-trash mr-1" />
                    {t('common.delete')}
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
      </ScrollPage>

      <DeleteConfirmDialog
        open={restoreTarget !== null}
        onOpenChange={(open) => !open && setRestoreTarget(null)}
        title={t('backup.restoreConfirmTitle')}
        description={t('backup.restoreConfirmDescription')}
        onConfirm={() => restoreTarget && void handleRestore(restoreTarget)}
        confirmText={t('backup.restore')}
        confirmVariant="default"
      />

      <DeleteConfirmDialog
        open={deleteTarget !== null}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        title={t('backup.deleteConfirmTitle')}
        description={t('backup.deleteConfirmDescription')}
        onConfirm={() => deleteTarget && void handleDelete(deleteTarget)}
      />

      <LoadingOverlay open={restoring} message={t('backup.restarting')} />
    </div>
  )
}
