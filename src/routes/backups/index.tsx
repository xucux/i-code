import { createFileRoute } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { BackupManager } from '@/modules/backup/ui/backup-manager'
import { getSettings, updateSettings } from '@/hooks/use-settings'
import { toast } from 'sonner'
import type { AppSettingsDto } from '@/modules/settings/types'

/**
 * 备份首页
 *
 * 提供本地备份、WebDAV 备份与备份设置三个 Tab，所有操作依赖应用设置中的 backupSettings。
 */
function BackupsIndexPage() {
  const [height, pageRef] = useAvailableHeight()
  const [settings, setSettings] = useState<AppSettingsDto | null>(null)

  useEffect(() => {
    let cancelled = false
    getSettings()
      .then((s) => {
        if (cancelled) return
        setSettings(s)
      })
      .catch((err) => {
        if (cancelled) return
        toast.error('加载设置失败', { description: String(err) })
      })
    return () => { cancelled = true }
  }, [])

  const handleSettingsChange = async (next: AppSettingsDto) => {
    try {
      const updated = await updateSettings({ backupSettings: next.backupSettings })
      setSettings(updated)
    } catch (err) {
      toast.error('保存设置失败', { description: String(err) })
      throw err
    }
  }

  if (!settings) {
    return (
      <div ref={pageRef} className="flex h-full items-center justify-center text-muted-foreground text-sm">
        <i className="fa-solid fa-circle-notch fa-spin mr-2" />
        加载中
      </div>
    )
  }

  return (
    <div ref={pageRef} className="flex h-full flex-col p-6">
      <div className="min-h-0 flex-1">
        <BackupManager height={height - 80} settings={settings} onSettingsChange={handleSettingsChange} />
      </div>
    </div>
  )
}

export const Route = createFileRoute('/backups/')({
  component: BackupsIndexPage,
})
