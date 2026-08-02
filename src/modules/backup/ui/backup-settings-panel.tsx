import { useEffect, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { ScrollPage } from '@/components/ui/scroll-page'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { UniversalPasswordCard } from '@/modules/secret/ui/universal-password-card'
import { toast } from 'sonner'
import type { BackupSettings, BackupFormat } from '@/modules/backup/types'

export interface BackupSettingsPanelProps {
  height: number
  settings: BackupSettings
  /** 通用密码（与「设置 → 安全」共享），用于加密 API Key 与远端备份文件 */
  configKey?: string
  onSave: (settings: BackupSettings) => Promise<void>
  onConfigKeyChange: (configKey: string | null) => Promise<void>
}

/**
 * 备份设置面板
 *
 * 配置本地备份目录、默认格式、保留策略与通用密码。
 * WebDAV 连接配置已迁移到「WebDAV」Tab，此处不再重复。
 * 注意：此处设置的密码与「设置 → 安全」中的配置密钥是同一个密码，
 * 用于 Secret（API Key 等）和 WebDAV 远端备份的 AES 加密/解密。
 */
export function BackupSettingsPanel({
  height,
  settings,
  configKey,
  onSave,
  onConfigKeyChange,
}: BackupSettingsPanelProps) {
  const { t } = useTranslation()
  const [draft, setDraft] = useState<BackupSettings>(settings)
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    setDraft(settings)
  }, [settings])

  const handleSave = async () => {
    setSaving(true)
    try {
      await onSave(draft)
      toast.success(t('backup.messages.saveSuccess'))
    } catch (err) {
      toast.error(t('backup.messages.saveFailed'), { description: String(err) })
    } finally {
      setSaving(false)
    }
  }

  return (
    <ScrollPage style={{ height }} variant="borderless" scrollbarVisible="auto">
      <div className="flex flex-col gap-4 pr-1 pb-3">
        {/* 通用密码 */}
        <UniversalPasswordCard configKey={configKey} onChange={onConfigKeyChange} />

        {/* 本地备份设置：与「通用密码」保持同一 Card 视觉 */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">
              <i className="fa-solid fa-folder-open mr-2 text-muted-foreground" />
              {t('backup.settings.local')}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="space-y-1.5">
              <Label className="text-xs">{t('backup.settings.localDirectory')}</Label>
              <Input
                className="h-8 text-xs"
                value={draft.localDirectory ?? ''}
                onChange={(e) => setDraft((prev) => ({ ...prev, localDirectory: e.target.value }))}
                placeholder={t('backup.settings.localDirectoryPlaceholder')}
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label className="text-xs">{t('backup.settings.defaultFormat')}</Label>
                <Select
                  value={draft.defaultFormat}
                  onValueChange={(v) => setDraft((prev) => ({ ...prev, defaultFormat: v as BackupFormat }))}
                >
                  <SelectTrigger className="h-8 text-xs">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="zip">zip</SelectItem>
                    <SelectItem value="tar-gz">tar.gz</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs">{t('backup.settings.localRetention')}</Label>
                <Input
                  type="number"
                  min={0}
                  className="h-8 text-xs"
                  value={draft.localRetentionCount ?? 0}
                  onChange={(e) =>
                    setDraft((prev) => ({ ...prev, localRetentionCount: parseInt(e.target.value, 10) }))
                  }
                />
              </div>
            </div>
          </CardContent>
        </Card>

        {/* 通用策略设置（不含 WebDAV 连接，连接配置见 WebDAV Tab）：与「通用密码」保持同一 Card 视觉 */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">
              <i className="fa-solid fa-sliders mr-2 text-muted-foreground" />
              {t('backup.settings.policy')}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label className="text-xs">{t('backup.settings.webdavRetention')}</Label>
                <Input
                  type="number"
                  min={0}
                  className="h-8 text-xs"
                  value={draft.webdavRetentionCount ?? 0}
                  onChange={(e) =>
                    setDraft((prev) => ({ ...prev, webdavRetentionCount: parseInt(e.target.value, 10) }))
                  }
                />
              </div>
              <div className="flex items-end gap-2 pb-1">
                <Switch
                  id="safety-backup"
                  checked={draft.enableSafetyBackupBeforeRestore}
                  onCheckedChange={(v) => setDraft((prev) => ({ ...prev, enableSafetyBackupBeforeRestore: v }))}
                />
                <Label htmlFor="safety-backup" className="text-xs">
                  {t('backup.settings.safetyBackup')}
                </Label>
              </div>
            </div>
          </CardContent>
        </Card>

        <div className="flex justify-end">
          <Button size="sm" onClick={() => void handleSave()} disabled={saving}>
            <i className="fa-solid fa-save mr-1.5" />
            {t('common.save')}
          </Button>
        </div>
      </div>
    </ScrollPage>
  )
}
