import { useEffect, useState } from 'react'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { DeleteConfirmDialog } from '@/components/ui/delete-confirm-dialog'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import {
  getGatewaySettings,
  updateGatewaySettings,
  listGatewayAuthKeys,
  createGatewayAuthKey,
  updateGatewayAuthKey,
  deleteGatewayAuthKey,
} from '@/hooks/use-ai-gateway-mutation'
import type { GatewaySettings, GatewayAuthKey, CreateGatewayAuthKeyInput, UpdateGatewayAuthKeyInput } from '@/modules/ai-gateway/types'
import { DEFAULT_GATEWAY_HOST, DEFAULT_GATEWAY_PORT } from '@/core/constants'
import { formatDateTime } from '@/core/utils'

/**
 * 生成以 sk-icode- 为前缀的随机 API Key
 */
function generateGatewayApiKey(): string {
  const arr = new Uint8Array(16)
  crypto.getRandomValues(arr)
  const hex = Array.from(arr, (b) => b.toString(16).padStart(2, '0')).join('')
  return `sk-icode-${hex}`
}

/**
 * 生成以 sk-icode-default- 为前缀的默认 Gateway API Key
 */
function generateDefaultGatewayApiKey(): string {
  const arr = new Uint8Array(16)
  crypto.getRandomValues(arr)
  const hex = Array.from(arr, (b) => b.toString(16).padStart(2, '0')).join('')
  return `sk-icode-default-${hex}`
}

/**
 * 复制文本到剪贴板
 */
async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    return false
  }
}

/**
 * 将 API Key 掩码展示：保留前缀与随后 6 位，其余用 `*` 替代
 * 例如 `sk-icode-affefa************************`
 */
function maskApiKey(key: string | undefined): string {
  if (!key) return ''
  const visible = 16
  if (key.length <= visible) return key
  return `${key.slice(0, visible)}${'*'.repeat(key.length - visible)}`
}

/**
 * 网关基础设置卡片
 *
 * 负责加载和保存网关监听地址、端口、启用状态及默认 API Key。
 */
export function GatewayBasicSettings() {
  const { t } = useTranslation('aiGateway')
  const [settings, setSettings] = useState<GatewaySettings | null>(null)
  const [showDefaultKey, setShowDefaultKey] = useState(false)

  // 加载网关设置
  const loadSettings = async () => {
    try {
      const s = await getGatewaySettings()
      setSettings(s)
    } catch {
      // web 预览模式下 Tauri 命令不可用，使用默认值
      setSettings({
        id: 'default',
        gatewayHost: DEFAULT_GATEWAY_HOST,
        gatewayPort: DEFAULT_GATEWAY_PORT,
        isEnabled: true,
        createdAt: '',
        updatedAt: '',
      })
    }
  }

  useEffect(() => { void loadSettings() }, [])

  // 实时更新网关设置
  const updateSettings = async (patch: Partial<GatewaySettings>) => {
    if (!settings) return
    try {
      const input: Record<string, unknown> = {}
      if ('gatewayHost' in patch) input.gatewayHost = patch.gatewayHost
      if ('gatewayPort' in patch) input.gatewayPort = patch.gatewayPort
      if ('isEnabled' in patch) input.isEnabled = patch.isEnabled
      if ('defaultApiKeySecretId' in patch) input.defaultApiKeySecretId = patch.defaultApiKeySecretId
      const updated = await updateGatewaySettings(input as Parameters<typeof updateGatewaySettings>[0])
      setSettings(updated)
    } catch (err) {
      toast.error(String(err))
    }
  }

  const handleCopyDefaultKey = async () => {
    const value = settings?.defaultApiKeySecretId
    if (!value) return
    const ok = await copyToClipboard(value)
    toast.success(ok ? t('gatewaySettings.copied') : t('gatewaySettings.copyFailed'))
  }

  if (!settings) return null

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="text-base">{t('gatewaySettings.title')}</CardTitle>
        <CardDescription className="text-xs">{t('gatewaySettings.description')}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex items-center justify-between">
          <Label className="text-xs">{t('gatewaySettings.enabled')}</Label>
          <Switch
            checked={settings.isEnabled}
            onCheckedChange={(v) => updateSettings({ isEnabled: v })}
          />
        </div>
        <div className="grid grid-cols-[1fr_1fr] gap-3">
          <div className="space-y-1">
            <Label className="text-xs">{t('gatewaySettings.host')}</Label>
            <Input
              value={settings.gatewayHost}
              onChange={(e) => updateSettings({ gatewayHost: e.target.value })}
              className="h-8 text-xs"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-xs">{t('gatewaySettings.port')}</Label>
            <Input
              type="number"
              value={settings.gatewayPort}
              onChange={(e) => updateSettings({ gatewayPort: Number(e.target.value) })}
              className="h-8 text-xs"
            />
          </div>
        </div>
        <div className="space-y-1">
          <Label className="text-xs">{t('gatewaySettings.defaultApiKey')}</Label>
          <div className="flex items-center gap-2">
            <Input
              value={settings.defaultApiKeySecretId ?? ''}
              onChange={(e) => updateSettings({ defaultApiKeySecretId: e.target.value || null })}
              className="h-8 text-xs"
              placeholder={t('gatewaySettings.defaultApiKeyPlaceholder')}
              type={showDefaultKey ? 'text' : 'password'}
            />
            <Button
              variant="outline"
              size="icon"
              className="size-8 shrink-0"
              onClick={() => setShowDefaultKey((v) => !v)}
              title={showDefaultKey ? t('authKey.hide') : t('authKey.show')}
            >
              <i className={showDefaultKey ? 'fa-solid fa-eye-slash text-xs' : 'fa-solid fa-eye text-xs'} />
            </Button>
            <Button
              variant="outline"
              size="icon"
              className="size-8 shrink-0"
              onClick={() => updateSettings({ defaultApiKeySecretId: generateDefaultGatewayApiKey() })}
              title={t('authKey.generate')}
            >
              <i className="fa-solid fa-wand-magic-sparkles text-xs" />
            </Button>
            <Button
              variant="outline"
              size="icon"
              className="size-8 shrink-0"
              onClick={handleCopyDefaultKey}
              disabled={!settings.defaultApiKeySecretId}
              title={t('common.copy')}
            >
              <i className="fa-solid fa-copy text-xs" />
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

/**
 * API Key 管理卡片
 *
 * 负责网关认证 API Key 的增删改查。
 */
export function GatewayAuthKeyManager() {
  const { t } = useTranslation('aiGateway')
  const [authKeys, setAuthKeys] = useState<GatewayAuthKey[]>([])

  // API Key 表单状态
  const [authFormOpen, setAuthFormOpen] = useState(false)
  const [editingAuthKey, setEditingAuthKey] = useState<GatewayAuthKey | null>(null)
  const [authFormName, setAuthFormName] = useState('')
  const [authFormDesc, setAuthFormDesc] = useState('')
  const [authFormApiKeyValue, setAuthFormApiKeyValue] = useState('')
  const [authFormShowKey, setAuthFormShowKey] = useState(false)
  const [authFormEnabled, setAuthFormEnabled] = useState(true)
  const [authFormExpires, setAuthFormExpires] = useState('')

  // 删除确认
  const [deleteOpen, setDeleteOpen] = useState(false)
  const [deletingAuthKey, setDeletingAuthKey] = useState<GatewayAuthKey | null>(null)

  // 加载 API Key 列表
  const loadAuthKeys = async () => {
    try {
      const keys = await listGatewayAuthKeys()
      setAuthKeys(keys)
    } catch {
      // web 预览模式下 Tauri 命令不可用，使用空列表
      setAuthKeys([])
    }
  }

  useEffect(() => { void loadAuthKeys() }, [])

  // 打开创建/编辑 API Key 表单
  const openAuthForm = (key?: GatewayAuthKey) => {
    if (key) {
      setEditingAuthKey(key)
      setAuthFormName(key.name)
      setAuthFormDesc(key.description ?? '')
      setAuthFormApiKeyValue('')
      setAuthFormShowKey(false)
      setAuthFormEnabled(key.isEnabled)
      setAuthFormExpires(key.expiresAt ?? '')
    } else {
      setEditingAuthKey(null)
      setAuthFormName('')
      setAuthFormDesc('')
      setAuthFormApiKeyValue('')
      setAuthFormShowKey(false)
      setAuthFormEnabled(true)
      setAuthFormExpires('')
    }
    setAuthFormOpen(true)
  }

  // 生成新的 API Key 值
  const handleGenerateApiKey = () => {
    setAuthFormApiKeyValue(generateGatewayApiKey())
  }

  // 提交 API Key 表单
  const handleAuthSubmit = async () => {
    try {
      if (editingAuthKey) {
        const input: UpdateGatewayAuthKeyInput = {
          name: authFormName,
          description: authFormDesc || null,
          isEnabled: authFormEnabled,
          expiresAt: authFormExpires || null,
        }
        // 仅在用户填写了新 key 时才更新，避免误清空
        if (authFormApiKeyValue) {
          input.apiKeySecretId = authFormApiKeyValue
        }
        await updateGatewayAuthKey(editingAuthKey.id, input)
        toast.success(t('authKey.updateSuccess'))
      } else {
        const input: CreateGatewayAuthKeyInput = {
          name: authFormName,
          description: authFormDesc || undefined,
          apiKeySecretId: authFormApiKeyValue || generateGatewayApiKey(),
          isEnabled: authFormEnabled,
          expiresAt: authFormExpires || undefined,
        }
        await createGatewayAuthKey(input)
        toast.success(t('authKey.createSuccess'))
      }
      setAuthFormOpen(false)
      void loadAuthKeys()
    } catch (err) {
      toast.error(String(err))
    }
  }

  // 删除 API Key
  const handleDelete = async () => {
    if (!deletingAuthKey) return
    try {
      await deleteGatewayAuthKey(deletingAuthKey.id)
      toast.success(t('authKey.deleteSuccess'))
      setDeleteOpen(false)
      void loadAuthKeys()
    } catch (err) {
      toast.error(String(err))
    }
  }

  // 复制单个 API Key
  const handleCopyAuthKey = async (key: GatewayAuthKey) => {
    if (!key.apiKeySecretId) return
    const ok = await copyToClipboard(key.apiKeySecretId)
    toast.success(ok ? t('authKey.copied') : t('authKey.copyFailed'))
  }

  return (
    <>
      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <div>
              <CardTitle className="text-base">{t('authKey.title')}</CardTitle>
              <CardDescription className="text-xs">{t('authKey.description')}</CardDescription>
            </div>
            <Button size="sm" className="h-7 text-xs" onClick={() => openAuthForm()}>
              <i className="fa-solid fa-plus mr-1.5" />
              {t('authKey.add')}
            </Button>
          </div>
        </CardHeader>
        <CardContent className="p-0">
          <ScrollArea className="max-h-[300px]">
            <div className="space-y-1 p-4 pt-0">
              {authKeys.length === 0 && (
                <p className="text-muted-foreground py-4 text-center text-xs">{t('authKey.empty')}</p>
              )}
              {authKeys.map((key) => (
                <div
                  key={key.id}
                  className="group flex items-center justify-between rounded-md border p-2.5 text-xs hover:bg-muted/50"
                >
                  <div className="min-w-0 flex-1 space-y-0.5">
                    <div className="flex items-center gap-2">
                      <span className="truncate font-medium">{key.name}</span>
                      <Badge variant={key.isEnabled ? 'default' : 'secondary'} className="text-[10px]">
                        {key.isEnabled ? t('authKey.enabled') : t('authKey.disabled')}
                      </Badge>
                    </div>
                    {key.apiKeySecretId && (
                      <div className="font-mono text-muted-foreground text-[10px]">
                        {maskApiKey(key.apiKeySecretId)}
                      </div>
                    )}
                    <div className="text-muted-foreground flex items-center gap-2 text-[10px]">
                      {key.description && <span className="truncate">{key.description}</span>}
                      {key.expiresAt && <span>{t('authKey.expiresAt', { value: formatDateTime(key.expiresAt) })}</span>}
                      {key.lastUsedAt && <span>{t('authKey.lastUsedAt', { value: formatDateTime(key.lastUsedAt) })}</span>}
                    </div>
                  </div>
                  <div className="ml-2 flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-6"
                      onClick={() => handleCopyAuthKey(key)}
                      disabled={!key.apiKeySecretId}
                      title={t('common.copy')}
                    >
                      <i className="fa-solid fa-copy text-[10px]" />
                    </Button>
                    <Button variant="ghost" size="icon" className="size-6" onClick={() => openAuthForm(key)}>
                      <i className="fa-solid fa-pen text-[10px]" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-6 text-destructive hover:text-destructive"
                      onClick={() => {
                        setDeletingAuthKey(key)
                        setDeleteOpen(true)
                      }}
                    >
                      <i className="fa-solid fa-trash text-[10px]" />
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          </ScrollArea>
        </CardContent>
      </Card>

      {/* API Key 表单 Dialog */}
      <Dialog open={authFormOpen} onOpenChange={setAuthFormOpen}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle className="text-base">
              {editingAuthKey ? t('authKey.editTitle') : t('authKey.createTitle')}
            </DialogTitle>
            <DialogDescription className="text-xs">
              {t('authKey.formDescription')}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="space-y-1">
              <Label className="text-xs">{t('authKey.name')}</Label>
              <Input
                value={authFormName}
                onChange={(e) => setAuthFormName(e.target.value)}
                className="h-8 text-xs"
                placeholder={t('authKey.namePlaceholder')}
              />
            </div>
            <div className="space-y-1">
              <Label className="text-xs">{t('authKey.description')}</Label>
              <Input
                value={authFormDesc}
                onChange={(e) => setAuthFormDesc(e.target.value)}
                className="h-8 text-xs"
                placeholder={t('authKey.descriptionPlaceholder')}
              />
            </div>
            <div className="space-y-1">
              <Label className="text-xs">{t('authKey.value')}</Label>
              <div className="flex items-center gap-2">
                <Input
                  value={authFormApiKeyValue}
                  onChange={(e) => setAuthFormApiKeyValue(e.target.value)}
                  className="h-8 text-xs"
                  placeholder={editingAuthKey ? t('authKey.valuePlaceholderEdit') : t('authKey.valuePlaceholderCreate')}
                  type={authFormShowKey ? 'text' : 'password'}
                />
                <Button
                  variant="outline"
                  size="icon"
                  className="size-8 shrink-0"
                  onClick={() => setAuthFormShowKey((v) => !v)}
                  title={authFormShowKey ? t('authKey.hide') : t('authKey.show')}
                >
                  <i className={authFormShowKey ? 'fa-solid fa-eye-slash text-xs' : 'fa-solid fa-eye text-xs'} />
                </Button>
                <Button
                  variant="outline"
                  size="icon"
                  className="size-8 shrink-0"
                  onClick={handleGenerateApiKey}
                  title={t('authKey.generate')}
                >
                  <i className="fa-solid fa-wand-magic-sparkles text-xs" />
                </Button>
              </div>
            </div>
            <div className="space-y-1">
              <Label className="text-xs">{t('authKey.expiresAtLabel')}</Label>
              <Input
                type="datetime-local"
                value={authFormExpires}
                onChange={(e) => setAuthFormExpires(e.target.value)}
                className="h-8 text-xs"
              />
            </div>
            <div className="flex items-center gap-2">
              <Switch checked={authFormEnabled} onCheckedChange={setAuthFormEnabled} />
              <Label className="text-xs">{t('authKey.enabled')}</Label>
            </div>
          </div>
          <DialogFooter>
            <Button type="button" size="sm" className="h-8 text-xs" onClick={handleAuthSubmit}>
              <i className="fa-solid fa-check mr-1.5" />
              {editingAuthKey ? t('authKey.confirmUpdate') : t('authKey.confirmCreate')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <DeleteConfirmDialog
        open={deleteOpen}
        onOpenChange={setDeleteOpen}
        title={t('authKey.deleteTitle')}
        description={deletingAuthKey ? t('authKey.deleteConfirm', { name: deletingAuthKey.name }) : ''}
        onConfirm={handleDelete}
      />
    </>
  )
}

/**
 * 网关设置管理组件
 *
 * 组合网关基础设置和 API Key 管理两个区域。
 */
export function GatewaySettingsPanel() {
  return (
    <div className="space-y-4">
      <GatewayBasicSettings />
      <GatewayAuthKeyManager />
    </div>
  )
}
