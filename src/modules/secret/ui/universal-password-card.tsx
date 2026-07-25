import { useEffect, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import { toast } from 'sonner'

export interface UniversalPasswordCardProps {
  /** 当前通用密码（明文或空） */
  configKey?: string
  /** 密码变更回调；保存时传当前值，清除时传 null */
  onChange: (configKey: string | null) => Promise<void>
}

/**
 * 通用密码卡片
 *
 * 用于「设置 → 安全」与「备份 → 备份设置」两个入口。
 * 该密码经 SHA-256 派生为 AES-256-GCM 密钥，同时用于：
 * - Secret 模块加密本地存储的 API Key、Token 等敏感数据
 * - Backup 模块加密 WebDAV 远端备份文件
 */
export function UniversalPasswordCard({ configKey, onChange }: UniversalPasswordCardProps) {
  const { t } = useTranslation()
  const [draft, setDraft] = useState(configKey ?? '')
  const [visible, setVisible] = useState(false)
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    setDraft(configKey ?? '')
  }, [configKey])

  const validate = (value: string) => {
    if (value && (value.length < 1 || value.length > 20)) {
      toast.error(t('settings.configKey.lengthError'))
      return false
    }
    return true
  }

  const handleSave = async () => {
    if (!validate(draft)) return
    setSaving(true)
    try {
      await onChange(draft || null)
      toast.success(t('backup.messages.saveSuccess'))
    } catch (err) {
      toast.error(t('backup.messages.saveFailed'), { description: String(err) })
    } finally {
      setSaving(false)
    }
  }

  const handleClear = async () => {
    setSaving(true)
    try {
      setDraft('')
      setVisible(false)
      await onChange(null)
      toast.success(t('backup.messages.saveSuccess'))
    } catch (err) {
      toast.error(t('backup.messages.saveFailed'), { description: String(err) })
    } finally {
      setSaving(false)
    }
  }

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="text-base">
          <i className={cn('fa-solid fa-key mr-2 text-muted-foreground')} />
          {t('settings.configKey.title')}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <p className="text-muted-foreground text-xs">
          {t('settings.configKey.description')}
        </p>
        <div className="flex items-center gap-2">
          <div className="relative flex-1">
            <Input
              type={visible ? 'text' : 'password'}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              placeholder={t('settings.configKey.placeholder')}
              className="h-8 pr-10 text-xs"
            />
            <button
              type="button"
              onClick={() => setVisible((v) => !v)}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
              title={visible ? t('settings.configKey.hide') : t('settings.configKey.show')}
            >
              <i className={cn('fa-solid', visible ? 'fa-eye-slash' : 'fa-eye')} />
            </button>
          </div>
          <Button
            size="sm"
            className="h-8 px-2.5 text-xs"
            onClick={() => void handleSave()}
            disabled={saving}
          >
            <i className="fa-solid fa-save mr-1.5" />
            {t('settings.configKey.save')}
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-8 px-2.5 text-xs"
            disabled={saving || !configKey}
            onClick={() => void handleClear()}
          >
            <i className="fa-solid fa-eraser mr-1.5" />
            {t('settings.configKey.clear')}
          </Button>
        </div>
        {draft && (
          <p className="text-muted-foreground text-xs">
            {t('settings.configKey.length', { length: draft.length })}
          </p>
        )}
        <p className="text-muted-foreground text-xs">
          {t('settings.configKey.scope')}
        </p>
      </CardContent>
    </Card>
  )
}
