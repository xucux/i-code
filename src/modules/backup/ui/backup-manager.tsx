import { useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { LocalBackupPanel } from './local-backup-panel'
import { WebDavBackupPanel } from './webdav-backup-panel'
import { BackupSettingsPanel } from './backup-settings-panel'
import type { AppSettingsDto } from '@/modules/settings/types'

export interface BackupManagerProps {
  height: number
  settings: AppSettingsDto
  onSettingsChange: (settings: AppSettingsDto) => void
}

/**
 * 备份管理器
 *
 * 使用 Tabs 组织本地备份、WebDAV 备份与备份设置三个子页面。
 */
export function BackupManager({ height, settings, onSettingsChange }: BackupManagerProps) {
  const { t } = useTranslation()
  const [activeTab, setActiveTab] = useState('local')

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <Tabs value={activeTab} onValueChange={setActiveTab} className="flex min-h-0 flex-1 flex-col">
          <TabsList className="shrink-0 self-start">
            <TabsTrigger value="local" className='text-xs'>{t('backup.tabs.local')}</TabsTrigger>
            <TabsTrigger value="webdav" className='text-xs'>{t('backup.tabs.webdav')}</TabsTrigger>
            <TabsTrigger value="settings" className='text-xs'>{t('backup.tabs.settings')}</TabsTrigger>
          </TabsList>

          <TabsContent value="local" className="mt-3 min-h-0 flex-1">
            <LocalBackupPanel
              height={height}
              localDirectory={settings.backupSettings.localDirectory}
            />
          </TabsContent>

          <TabsContent value="webdav" className="mt-3 min-h-0 flex-1">
            <WebDavBackupPanel
              height={height}
              settings={settings.backupSettings}
              configKey={settings.configKey}
            />
          </TabsContent>

          <TabsContent value="settings" className="mt-3 min-h-0 flex-1">
            <BackupSettingsPanel
              height={height}
              settings={settings.backupSettings}
              configKey={settings.configKey}
              onSave={async (backupSettings) => {
                await onSettingsChange({ ...settings, backupSettings })
              }}
              onConfigKeyChange={async (configKey) => {
                await onSettingsChange({ ...settings, configKey: configKey ?? undefined })
              }}
            />
          </TabsContent>
        </Tabs>
    </div>

  )
}
