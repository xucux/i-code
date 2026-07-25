import { useCallback, useEffect, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { ScrollPage } from '@/components/ui/scroll-page'
import { DeleteConfirmDialog } from '@/components/ui/delete-confirm-dialog'
import { LoadingOverlay } from '@/components/ui/loading-overlay'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  deleteWebDavBackup,
  deleteWebDavConfig,
  listWebDavBackups,
  listWebDavConfigs,
  pushWebDavBackup,
  restoreWebDavBackup,
  saveWebDavConfig,
} from '@/hooks/use-backup'
import { formatDateTime, isTauri } from '@/core/utils'
import { toast } from 'sonner'
import type { BackupListItem, BackupSettings, WebDavConfig, WebDavConfigRecord, WebDavPreset } from '@/modules/backup/types'

export interface WebDavBackupPanelProps {
  height: number
  settings: BackupSettings
  /** 通用密码（与「设置 → 安全」共享），存在时 WebDAV 备份自动加密 */
  configKey?: string
}

const PRESET_OPTIONS: { value: WebDavPreset; labelKey: string }[] = [
  { value: 'jianguoyun', labelKey: 'backup.webdav.preset.jianguoyun' },
  { value: 'koofr', labelKey: 'backup.webdav.preset.koofr' },
  { value: 'nextcloud', labelKey: 'backup.webdav.preset.nextcloud' },
  { value: 'custom', labelKey: 'backup.webdav.preset.custom' },
]

const PRESET_URLS: Record<WebDavPreset, string> = {
  jianguoyun: 'https://dav.jianguoyun.com/dav/',
  koofr: 'https://app.koofr.net/dav/Koofr/',
  nextcloud: 'https://example.com/remote.php/dav/files/{username}/',
  custom: '',
}

const EMPTY_DRAFT: Partial<WebDavConfigRecord> = {
  name: '',
  url: '',
  username: '',
  password: '',
  remotePath: '/i-code-backups/',
  strictSsl: true,
  preset: 'custom',
}

/**
 * WebDAV 备份面板
 *
 * 支持从 `webdav_configs` 表中选择、新建、编辑、删除已保存的 WebDAV 配置，
 * 并基于选中配置执行列出/上传/恢复/删除远程备份。
 * 配置密码以明文存储在数据库中，前端表单直接展示与编辑。
 */
export function WebDavBackupPanel({ height, settings: _settings, configKey }: WebDavBackupPanelProps) {
  const { t } = useTranslation()
  const [records, setRecords] = useState<WebDavConfigRecord[]>([])
  const [selectedId, setSelectedId] = useState<string>('new')
  const [draft, setDraft] = useState<Partial<WebDavConfigRecord>>(EMPTY_DRAFT)
  const [loading, setLoading] = useState(false)
  const [pushing, setPushing] = useState(false)
  const [saving, setSaving] = useState(false)
  const [restoreTarget, setRestoreTarget] = useState<BackupListItem | null>(null)
  const [deleteBackupTarget, setDeleteBackupTarget] = useState<BackupListItem | null>(null)
  const [deleteConfigTarget, setDeleteConfigTarget] = useState<WebDavConfigRecord | null>(null)
  const [backups, setBackups] = useState<BackupListItem[]>([])
  const [restoring, setRestoring] = useState(false)

  const encryptEnabled = Boolean(configKey && configKey.length >= 1 && configKey.length <= 20)

  /** 加载已保存的 WebDAV 配置列表 */
  const loadRecords = useCallback(async () => {
    if (!isTauri()) return
    try {
      const items = await listWebDavConfigs()
      setRecords(items)
      // 首次加载且存在记录时，默认选中第一条
      if (selectedId === 'new' && items.length > 0 && !draft.id) {
        setSelectedId(items[0].id)
        setDraft(items[0])
      }
    } catch (err) {
      toast.error(t('backup.webdav.configLoadFailed'), { description: String(err) })
    }
  }, [selectedId, draft.id])

  useEffect(() => {
    void loadRecords()
  }, [loadRecords])

  /** 根据当前表单构造运行时 WebDavConfig */
  const buildRuntimeConfig = (): WebDavConfig | null => {
    if (!draft.url || !draft.username) {
      toast.error(t('backup.webdav.validation.required'))
      return null
    }
    if (!draft.password) {
      toast.error(t('backup.webdav.validation.passwordRequired'))
      return null
    }

    return {
      url: draft.url,
      username: draft.username,
      passwordSecretId: `$PLAIN:${draft.password}$`,
      remotePath: draft.remotePath || '/i-code-backups/',
      strictSsl: draft.strictSsl ?? true,
    }
  }

  /** 选中配置变更 */
  const handleSelectChange = (value: string) => {
    if (value === 'new') {
      setSelectedId('new')
      setDraft({ ...EMPTY_DRAFT })
      return
    }
    const found = records.find((r) => r.id === value)
    if (found) {
      setSelectedId(found.id)
      setDraft({ ...found })
    }
  }

  /** 服务预设变更 */
  const handlePresetChange = (value: WebDavPreset) => {
    setDraft((prev) => ({
      ...prev,
      preset: value,
      url: PRESET_URLS[value],
      remotePath: '/i-code-backups/',
    }))
  }

  /** 保存当前表单为 WebDAV 配置 */
  const handleSaveConfig = async () => {
    if (!draft.name || !draft.url || !draft.username || !draft.password) {
      toast.error(t('backup.webdav.validation.saveRequired'))
      return
    }

    setSaving(true)
    try {
      const saved = await saveWebDavConfig({
        id: draft.id,
        name: draft.name,
        url: draft.url,
        username: draft.username,
        password: draft.password,
        remotePath: draft.remotePath || '/i-code-backups/',
        strictSsl: draft.strictSsl ?? true,
        preset: draft.preset ?? 'custom',
      })
      toast.success(t('backup.webdav.configSaveSuccess'))
      const updated = await listWebDavConfigs()
      setRecords(updated)
      setSelectedId(saved.id)
      setDraft(saved)
    } catch (err) {
      toast.error(t('backup.webdav.configSaveFailed'), { description: String(err) })
    } finally {
      setSaving(false)
    }
  }

  /** 新建配置 */
  const handleNewConfig = () => {
    setSelectedId('new')
    setDraft({ ...EMPTY_DRAFT })
  }

  /** 删除配置确认 */
  const handleDeleteConfig = async (record: WebDavConfigRecord) => {
    setDeleteConfigTarget(null)
    try {
      await deleteWebDavConfig(record.id)
      toast.success(t('backup.webdav.configDeleteSuccess'))
      const updated = await listWebDavConfigs()
      setRecords(updated)
      if (updated.length > 0) {
        setSelectedId(updated[0].id)
        setDraft(updated[0])
      } else {
        setSelectedId('new')
        setDraft({ ...EMPTY_DRAFT })
      }
    } catch (err) {
      toast.error(t('backup.webdav.configDeleteFailed'), { description: String(err) })
    }
  }

  /** 列出远程备份 */
  const load = useCallback(async () => {
    const cfg = buildRuntimeConfig()
    if (!cfg) return

    setLoading(true)
    try {
      const items = await listWebDavBackups(cfg)
      setBackups(items)
    } catch (err) {
      toast.error(t('backup.messages.loadFailed'), { description: String(err) })
    } finally {
      setLoading(false)
    }
  }, [draft])

  /** 上传备份 */
  const handlePush = async () => {
    const cfg = buildRuntimeConfig()
    if (!cfg) return

    setPushing(true)
    try {
      const result = await pushWebDavBackup({
        config: cfg,
        encrypt: encryptEnabled,
      })
      toast.success(t('backup.messages.pushSuccess'), { description: result.path })
      await load()
    } catch (err) {
      toast.error(t('backup.messages.pushFailed'), { description: String(err) })
    } finally {
      setPushing(false)
    }
  }

  /** 恢复备份 */
  const handleRestore = async (item: BackupListItem) => {
    const cfg = buildRuntimeConfig()
    if (!cfg) return

    setRestoreTarget(null)
    setRestoring(true)
    try {
      const result = await restoreWebDavBackup({
        config: cfg,
        remotePath: item.path,
        encrypted: item.encrypted ?? false,
      })
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

  /** 删除远程备份 */
  const handleDeleteBackup = async (item: BackupListItem) => {
    const cfg = buildRuntimeConfig()
    if (!cfg) return

    setDeleteBackupTarget(null)
    try {
      await deleteWebDavBackup(cfg, item.path)
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
      {/* 配置选择器：底部对齐，使按钮与 Select 输入框同一基线 */}
      <div className="flex items-end gap-2">
        <div className="min-w-0 flex-1 space-y-1.5">
          <Label className="text-xs">{t('backup.webdav.configLabel')}</Label>
          <Select value={selectedId} onValueChange={handleSelectChange}>
            <SelectTrigger className="h-8 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="new">{t('backup.webdav.configNew')}</SelectItem>
              {records.map((record) => (
                <SelectItem key={record.id} value={record.id}>
                  {record.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex shrink-0 gap-1.5">
          <Button
            variant="outline"
            size="sm"
            className="h-8 text-xs"
            onClick={handleNewConfig}
          >
            <i className="fa-solid fa-plus mr-1" />
            {t('backup.webdav.configNew')}
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-8 text-xs text-destructive hover:text-destructive"
            onClick={() => {
              const found = records.find((r) => r.id === selectedId)
              if (found) setDeleteConfigTarget(found)
            }}
            disabled={selectedId === 'new'}
          >
            <i className="fa-solid fa-trash mr-1" />
            {t('backup.webdav.configDelete')}
          </Button>
        </div>
      </div>

      {/* 配置表单 */}
      <div className="grid grid-cols-2 gap-3">
        <div className="col-span-2 space-y-1.5">
          <Label className="text-xs">{t('backup.webdav.configName')}</Label>
          <Input
            className="h-8 text-xs"
            placeholder={t('backup.webdav.configName')}
            value={draft.name ?? ''}
            onChange={(e) => setDraft((prev) => ({ ...prev, name: e.target.value }))}
          />
        </div>
        <div className="space-y-1.5">
          <Label className="text-xs">{t('backup.webdav.presetLabel')}</Label>
          <Select value={draft.preset ?? 'custom'} onValueChange={(v) => handlePresetChange(v as WebDavPreset)}>
            <SelectTrigger className="h-8 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {PRESET_OPTIONS.map((opt) => (
                <SelectItem key={opt.value} value={opt.value}>
                  {t(opt.labelKey)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1.5">
          <Label className="text-xs">{t('backup.webdav.url')}</Label>
          <Input
            className="h-8 text-xs"
            placeholder="https://dav.example.com"
            value={draft.url ?? ''}
            onChange={(e) => setDraft((prev) => ({ ...prev, url: e.target.value }))}
          />
        </div>
        <div className="space-y-1.5">
          <Label className="text-xs">{t('backup.webdav.username')}</Label>
          <Input
            className="h-8 text-xs"
            value={draft.username ?? ''}
            onChange={(e) => setDraft((prev) => ({ ...prev, username: e.target.value }))}
          />
        </div>
        <div className="space-y-1.5">
          <Label className="text-xs">{t('backup.webdav.password')}</Label>
          <Input
            type="password"
            className="h-8 text-xs"
            value={draft.password ?? ''}
            onChange={(e) => setDraft((prev) => ({ ...prev, password: e.target.value }))}
          />
        </div>
        <div className="space-y-1.5">
          <Label className="text-xs">{t('backup.webdav.remotePath')}</Label>
          <Input
            className="h-8 text-xs"
            value={draft.remotePath ?? '/i-code-backups/'}
            onChange={(e) => setDraft((prev) => ({ ...prev, remotePath: e.target.value }))}
          />
        </div>
      </div>

      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1.5">
            <Switch
              id="strict-ssl"
              checked={draft.strictSsl ?? true}
              onCheckedChange={(v) => setDraft((prev) => ({ ...prev, strictSsl: v }))}
            />
            <Label htmlFor="strict-ssl" className="text-xs">
              {t('backup.webdav.strictSsl')}
            </Label>
          </div>
          <div className="flex items-center gap-1.5">
            <Switch
              id="encrypt-push"
              checked={encryptEnabled}
              disabled
            />
            <Label htmlFor="encrypt-push" className="text-xs">
              {t('backup.webdav.encrypt')}
            </Label>
          </div>
          {!encryptEnabled && (
            <span className="text-muted-foreground text-xs">{t('backup.webdav.encryptHint')}</span>
          )}
        </div>
        <div className="flex items-center gap-1.5">
          <Button
            variant="outline"
            size="sm"
            className="h-8 text-xs"
            onClick={() => void handleSaveConfig()}
            disabled={saving}
          >
            <i className="fa-solid fa-save mr-1" />
            {t('backup.webdav.configSave')}
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-8 text-xs"
            onClick={() => void load()}
            disabled={loading}
          >
            <i className="fa-solid fa-rotate mr-1" />
            {t('backup.webdav.list')}
          </Button>
          <Button size="sm" className="h-8 text-xs" onClick={() => void handlePush()} disabled={pushing}>
            <i className="fa-solid fa-cloud-arrow-up mr-1" />
            {t('backup.webdav.push')}
          </Button>
        </div>
      </div>

      <ScrollPage style={{ height: height - 240 }} variant="borderless" scrollbarVisible="auto">
        {backups.length === 0 ? (
          <div className="flex h-32 flex-col items-center justify-center text-muted-foreground text-sm">
            <i className="fa-solid fa-cloud mb-2 text-lg" />
            {t('backup.webdav.empty')}
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
                    <span className="text-sm font-medium">{formatDateTime(item.createdAt)}</span>
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
                    onClick={() => setDeleteBackupTarget(item)}
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

      <Dialog open={restoreTarget !== null} onOpenChange={(open) => !open && setRestoreTarget(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle className="text-base">{t('backup.restoreConfirmTitle')}</DialogTitle>
            <DialogDescription className="text-xs">
              {restoreTarget?.encrypted
                ? t('backup.webdav.restoreEncryptedDescription')
                : t('backup.restoreConfirmDescription')}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="ghost"
              size="sm"
              className="h-8 text-xs"
              onClick={() => setRestoreTarget(null)}
            >
              {t('common.cancel')}
            </Button>
            <Button
              size="sm"
              className="h-8 text-xs"
              onClick={() => restoreTarget && void handleRestore(restoreTarget)}
            >
              {t('backup.restore')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <DeleteConfirmDialog
        open={deleteBackupTarget !== null}
        onOpenChange={(open) => !open && setDeleteBackupTarget(null)}
        title={t('backup.deleteConfirmTitle')}
        description={t('backup.deleteConfirmDescription')}
        onConfirm={() => deleteBackupTarget && void handleDeleteBackup(deleteBackupTarget)}
      />

      <DeleteConfirmDialog
        open={deleteConfigTarget !== null}
        onOpenChange={(open) => !open && setDeleteConfigTarget(null)}
        title={t('backup.webdav.deleteConfirmTitle')}
        description={t('backup.webdav.deleteConfirmDescription')}
        onConfirm={() => deleteConfigTarget && void handleDeleteConfig(deleteConfigTarget)}
      />

      <LoadingOverlay open={restoring} message={t('backup.restarting')} />
    </div>
  )
}
