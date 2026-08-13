import { createFileRoute } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { useTheme } from '@/modules/theme/use-theme'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'
import { cn } from '@/lib/utils'
import { toast } from 'sonner'
import { getSettings, getLogDir, getConfigDir, updateSettings } from '@/hooks/use-settings'
import { clearCallStats } from '@/hooks/use-call-records-mutation'
import { UniversalPasswordCard } from '@/modules/secret/ui/universal-password-card'
import type { AppSettingsDto, GlobalProxyConfig, LogLevel, TitleBarInfoConfig } from '@/modules/settings/types'
import { DEFAULT_TITLEBAR_INFO_CONFIG, LOG_LEVEL_OPTIONS } from '@/modules/settings/types'
import type { BackupSettings } from '@/modules/backup/types'
import { ScrollPage } from '@/components/ui/scroll-page'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { enable as autostartEnable, disable as autostartDisable } from '@tauri-apps/plugin-autostart'
import { DateTimeRangePicker } from '@/components/ui/date-time-range-picker'
import type { DateRange } from 'react-day-picker'
import { UpdateCheck } from '@/modules/settings/ui/update-check'

/**
 * 设置页面
 *
 * 集成主题、语言、网关监听等应用级设置。
 */
function SettingsPage() {
  const { t, i18n } = useTranslation()
  const { theme, setTheme } = useTheme()

  const [settings, setSettings] = useState<AppSettingsDto | null>(null)

  const [globalProxyEnabled, setGlobalProxyEnabled] = useState(false)
  const [proxyType, setProxyType] = useState<'direct' | 'system' | 'http' | 'socks'>('http')
  const [proxyUrl, setProxyUrl] = useState('')
  // 已保存的代理配置，用于 dirty 检测（决定保存按钮是否可用）
  const [savedProxyType, setSavedProxyType] = useState<'direct' | 'system' | 'http' | 'socks'>('http')
  const [savedProxyUrl, setSavedProxyUrl] = useState('')

  const [titlebarInfo, setTitlebarInfo] = useState<TitleBarInfoConfig>(DEFAULT_TITLEBAR_INFO_CONFIG)
  const [autoStartEnabled, setAutoStartEnabled] = useState(false)
  const [logLevel, setLogLevel] = useState<LogLevel>('info')
  const [logDir, setLogDir] = useState('')
  const [configDir, setConfigDir] = useState('')
  const [clearDateRange, setClearDateRange] = useState<DateRange | undefined>(undefined)

  // 测量页面可用高度
  const [pageHeight, pageRef] = useAvailableHeight()
  // 标题区域约占 50px（mb-3=12px + h1+subtitle≈38px）
  const headerOffset = 50
  // 内容区可用高度 = 页面高度 - 标题 - 上下 padding(24px×2)
  const contentHeight = pageHeight > 0 ? Math.max(0, pageHeight - headerOffset) : undefined
  

  // 加载应用设置
  useEffect(() => {
    let cancelled = false
    getSettings()
      .then((s) => {
        if (cancelled) return
        setSettings(s)
        setGlobalProxyEnabled(s.globalProxyEnabled ?? false)
        setProxyType(s.globalProxy?.type ?? 'http')
        setProxyUrl(s.globalProxy?.url ?? '')
        setSavedProxyType(s.globalProxy?.type ?? 'http')
        setSavedProxyUrl(s.globalProxy?.url ?? '')
        setTitlebarInfo(s.titlebarInfo ?? DEFAULT_TITLEBAR_INFO_CONFIG)
        setAutoStartEnabled(s.autoStartEnabled ?? false)
        setLogLevel(s.logLevel ?? 'info')
      })
      .catch(() => {
        // web 预览模式下 Tauri 命令不可用，使用空默认值
        if (cancelled) return
        setSettings({
          theme: 'dark',
          locale: 'zh-CN',
          globalProxyEnabled: false,
          storeSecretsInKeychain: false,
          titlebarInfo: DEFAULT_TITLEBAR_INFO_CONFIG,
          titlebarInfoJson: '',
          backupSettings: {
            defaultFormat: 'zip',
            enableSafetyBackupBeforeRestore: true,
          } as BackupSettings,
          autoStartEnabled: false,
          gatewayLastRunning: false,
          logLevel: 'info',
        })
      })

    // 单独获取日志目录；web 预览模式下该命令不可用，静默忽略
    getLogDir()
      .then((dir) => {
        if (!cancelled) setLogDir(dir)
      })
      .catch(() => {
        // web 预览模式下 Tauri 命令不可用，保持为空
      })

    // 单独获取应用配置目录（与数据库同目录）；web 预览模式下静默忽略
    getConfigDir()
      .then((dir) => {
        if (!cancelled) setConfigDir(dir)
      })
      .catch(() => {
        // web 预览模式下 Tauri 命令不可用，保持为空
      })

    return () => { cancelled = true }
  }, [])

  // 提交部分更新（configKey 允许传 null 表示清空）
  // 返回 boolean 表示是否成功，供调用方决定后续动作（如更新 dirty 状态）
  const patchSettings = async (patch: Omit<Partial<AppSettingsDto>, 'configKey'> & { configKey?: string | null }): Promise<boolean> => {
    if (!settings) return false
    try {
      const updated = await updateSettings(patch)
      setSettings(updated)
      return true
    } catch (err) {
      // eslint-disable-next-line no-console
      console.error('更新设置失败', err)
      toast.error(String(err))
      return false
    }
  }

  // 切换开机自启开关：同步 DB 用户意图与系统实际注册状态
  // - 关闭时若遇到"系统找不到指定的文件 (os error 2)"类错误，说明系统侧已无注册项，
  //   与期望状态一致，静默处理不报错（修复软件更新后路径变更导致的报错）
  // - 启用失败时回滚 UI 与 DB，避免状态不一致
  const handleToggleAutoStart = async (v: boolean) => {
    setAutoStartEnabled(v)
    await patchSettings({ autoStartEnabled: v })
    try {
      if (v) {
        await autostartEnable()
      } else {
        await autostartDisable()
      }
    } catch (e) {
      const msg = String(e)
      // 跨平台"文件/注册表值不存在"特征：
      //   Windows: "系统找不到指定的文件。 (os error 2)"
      //   Linux:   "No such file or directory (os error 2)"
      const isNotFound =
        msg.includes('os error 2') ||
        msg.includes('系统找不到') ||
        msg.includes('cannot find') ||
        msg.includes('No such file') ||
        msg.includes('not found')
      if (v) {
        // 启用失败：回滚 UI 与 DB 到关闭状态
        setAutoStartEnabled(false)
        await patchSettings({ autoStartEnabled: false })
        toast.error(t('settings.autoStartEnableFailed', { error: msg }))
      } else if (!isNotFound) {
        // 关闭失败且非"已无注册项"类错误，正常提示
        toast.error(t('settings.autoStartDisableFailed', { error: msg }))
      }
      // 关闭时遇到"已无注册项"错误：静默处理（系统状态已与期望一致）
    }
  }

  // 保存代理配置（统一提交，避免每次输入都触发保存）
  // 仅当本地与已保存值不一致（dirty）时才可点击
  const handleSaveProxy = async () => {
    const ok = await patchSettings({ globalProxy: { type: proxyType, url: proxyUrl } as GlobalProxyConfig })
    if (ok) {
      setSavedProxyType(proxyType)
      setSavedProxyUrl(proxyUrl)
      toast.success(t('settings.proxySaveSuccess'))
    }
  }

  // 切换标题栏信息项展示
  const toggleTitlebarInfo = async (key: keyof TitleBarInfoConfig) => {
    const next = { ...titlebarInfo, [key]: !titlebarInfo[key] }
    setTitlebarInfo(next)
    await patchSettings({ titlebarInfo: next })
  }

  const themes = [
    { key: 'light', label: t('theme.light') },
    { key: 'dark', label: t('theme.dark') },
    { key: 'claude-light', label: t('theme.claudeLight') },
    { key: 'claude-dark', label: t('theme.claudeDark') },
    { key: 'deepseek-light', label: t('theme.deepseekLight') },
    { key: 'deepseek-dark', label: t('theme.deepseekDark') },
    { key: 'nvidia-light', label: t('theme.nvidiaLight') },
    { key: 'nvidia-dark', label: t('theme.nvidiaDark') },
  ] as const

  const languages = [
    { key: 'zh-CN', label: t('locale.zhCN') },
    { key: 'zh-TW', label: t('locale.zhTW') },
    { key: 'en', label: t('locale.en') },
    { key: 'ja', label: t('locale.ja') },
  ] as const

  const currentLanguage = i18n.language ?? 'zh-CN'

  const handleLanguageChange = (lang: string) => {
    void i18n.changeLanguage(lang)
    void patchSettings({ locale: lang as AppSettingsDto['locale'] })
  }

  return (
    <div ref={pageRef} className="h-full p-6">
      <div className="mb-3">
        <h1 className="text-lg font-semibold">{t('settings.title')}</h1>
        <p className="text-muted-foreground text-sm">{t('settings.subtitle')}</p>
      </div>

      <ScrollPage style={{ height: contentHeight || undefined }} scrollbarVisible="auto" variant="borderless" >
      <div className="mx-auto space-y-6 pb-10">
        {/* 外观设置 */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">
              <i className={cn('fa-solid fa-paintbrush mr-2 text-muted-foreground')} />
              {t('settings.appearance')}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label className="text-sm">{t('theme.title')}</Label>
              <div className="grid grid-cols-4 gap-2">
                {themes.map(({ key, label }) => (
                  <button
                    key={key}
                    type="button"
                    onClick={() => {
                      setTheme(key)
                      void patchSettings({ theme: key as AppSettingsDto['theme'] })
                    }}
                    className={cn(
                      'rounded-md border px-3 py-2 text-xs transition-colors',
                      theme === key
                        ? 'border-primary bg-primary/5 text-primary'
                        : 'hover:bg-muted/50'
                    )}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>

            <Separator />

            <div className="space-y-2">
              <Label className="text-sm">{t('locale.title')}</Label>
              <div className="flex gap-2">
                {languages.map(({ key, label }) => (
                  <button
                    key={key}
                    type="button"
                    onClick={() => handleLanguageChange(key)}
                    className={cn(
                      'rounded-md border px-3 py-2 text-xs transition-colors',
                      currentLanguage === key
                        ? 'border-primary bg-primary/5 text-primary'
                        : 'hover:bg-muted/50'
                    )}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>
          </CardContent>
        </Card>

        {/* 本地网络设置 */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">
              <i className={cn('fa-solid fa-network-wired mr-2 text-muted-foreground')} />
              {t('settings.network.title')}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label className="text-sm">{t('settings.globalProxy')}</Label>
                <p className="text-muted-foreground text-xs">{t('settings.globalProxyDescription')}</p>
              </div>
              <Switch
                checked={globalProxyEnabled}
                onCheckedChange={(v) => {
                  setGlobalProxyEnabled(v)
                  void patchSettings({ globalProxyEnabled: v })
                }}
              />
            </div>
            {globalProxyEnabled && (
              <>
                <Separator />
                <div className="space-y-3">
                  <div className="flex items-center gap-3">
                    <Label className="text-sm shrink-0">{t('settings.proxyType')}</Label>
                    <Select value={proxyType} onValueChange={(v) => {
                      setProxyType(v as 'direct' | 'system' | 'http' | 'socks')
                    }}>
                      <SelectTrigger className="h-8 w-32 text-xs">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="direct">{t('settings.proxyTypeDirect')}</SelectItem>
                        <SelectItem value="system">{t('settings.proxyTypeSystem')}</SelectItem>
                        <SelectItem value="http">{t('settings.proxyTypeHttp')}</SelectItem>
                        <SelectItem value="socks">{t('settings.proxyTypeSocks')}</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                  {(proxyType === 'http' || proxyType === 'socks') && (
                    <div className="flex items-center gap-3">
                      <Label className="text-sm shrink-0">{t('settings.proxyUrl')}</Label>
                      <Input
                        className="h-8 text-xs"
                        placeholder={proxyType === 'socks' ? 'socks5://user:pass@127.0.0.1:1080' : 'http://user:pass@127.0.0.1:7890'}
                        value={proxyUrl}
                        onChange={(e) => setProxyUrl(e.target.value)}
                      />
                    </div>
                  )}
                  {/* 保存按钮：仅当本地配置与已保存值不一致（dirty）时可用 */}
                  <div className="flex justify-end">
                    <Button
                      size="sm"
                      className="h-7 text-xs"
                      disabled={proxyType === savedProxyType && proxyUrl === savedProxyUrl}
                      onClick={() => void handleSaveProxy()}
                    >
                      <i className="fa-solid fa-floppy-disk mr-1.5 text-xs" />
                      {t('common.save')}
                    </Button>
                  </div>
                </div>
              </>
            )}
          </CardContent>
        </Card>

        {/* 日志级别设置 */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">
              <i className={cn('fa-solid fa-file-lines mr-2 text-muted-foreground')} />
              {t('settings.logLevel.title')}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label className="text-sm">{t('settings.logLevel.label')}</Label>
                <p className="text-muted-foreground text-xs">{t('settings.logLevel.description')}</p>
              </div>
              <Select
                value={logLevel}
                onValueChange={(v) => {
                  const next = v as LogLevel
                  setLogLevel(next)
                  void patchSettings({ logLevel: next })
                }}
              >
                <SelectTrigger className="h-8 w-32 text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {LOG_LEVEL_OPTIONS.map((opt) => (
                    <SelectItem key={opt.value} value={opt.value}>
                      {t(opt.labelKey)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            {logDir && (
              <div className="rounded-md bg-muted/50 px-3 py-2">
                <p className="text-muted-foreground text-xs break-all">
                  {t('settings.logLevel.fileLocation', { path: logDir })}
                </p>
              </div>
            )}
          </CardContent>
        </Card>

        {/* 通用密码 */}
        <UniversalPasswordCard
          configKey={settings?.configKey}
          onChange={async (configKey) => {
            await patchSettings({ configKey })
          }}
        />

        {/* 标题栏信息配置 */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">
              <i className={cn('fa-solid fa-window-maximize mr-2 text-muted-foreground')} />
              {t('settings.titlebarInfo.title')}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <p className="text-muted-foreground text-xs">
              {t('settings.titlebarInfo.description')}
            </p>
            <div className="grid grid-cols-2 gap-3">
              {([
                { key: 'showTokens' as const, labelKey: 'settings.titlebarInfo.tokens', icon: 'fa-coins' },
                { key: 'showRpm' as const, labelKey: 'settings.titlebarInfo.rpm', icon: 'fa-bolt' },
                { key: 'showLatency' as const, labelKey: 'settings.titlebarInfo.latency', icon: 'fa-clock' },
                { key: 'showMemory' as const, labelKey: 'settings.titlebarInfo.memory', icon: 'fa-memory' },
                { key: 'showGatewayStatus' as const, labelKey: 'settings.titlebarInfo.gatewayStatus', icon: 'fa-server' },
              ]).map(({ key, labelKey, icon }) => (
                <div key={key} className="flex items-center justify-between rounded-md border p-2">
                  <Label className={cn('flex items-center gap-2 text-xs', !titlebarInfo[key] && 'opacity-50')}>
                    <i className={cn('fa-solid', icon, 'size-3.5')} />
                    {t(labelKey)}
                  </Label>
                  <Switch
                    checked={titlebarInfo[key]}
                    onCheckedChange={() => void toggleTitlebarInfo(key)}
                  />
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        {/* 开机自启 */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">
              <i className={cn('fa-solid fa-power-off mr-2 text-muted-foreground')} />
              {t('settings.autoStart.title')}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label className="text-sm">{t('settings.autoStart.label')}</Label>
                <p className="text-muted-foreground text-xs">{t('settings.autoStart.description')}</p>
              </div>
              <Switch
                checked={autoStartEnabled}
                onCheckedChange={(v) => void handleToggleAutoStart(v)}
              />
            </div>
          </CardContent>
        </Card>

        {/* 数据管理 */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">
              <i className={cn('fa-solid fa-trash-can mr-2 text-muted-foreground')} />
              {t('settings.dataManagement.title')}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <p className="text-muted-foreground text-xs">
              {t('settings.dataManagement.description')}
            </p>

            {/* 一键清空全部 */}
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label className="text-sm">{t('settings.dataManagement.clearAll')}</Label>
                <p className="text-muted-foreground text-xs">{t('settings.dataManagement.clearAllDescription')}</p>
              </div>
              <AlertDialog>
                <AlertDialogTrigger asChild>
                  <Button variant="destructive" size="sm" className="text-xs">
                    {t('settings.dataManagement.clearAll')}
                  </Button>
                </AlertDialogTrigger>
                <AlertDialogContent>
                  <AlertDialogHeader>
                    <AlertDialogTitle>{t('settings.dataManagement.confirmTitle')}</AlertDialogTitle>
                    <AlertDialogDescription>
                      {t('settings.dataManagement.confirmAllDescription')}
                    </AlertDialogDescription>
                  </AlertDialogHeader>
                  <AlertDialogFooter>
                    <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
                    <AlertDialogAction
                      onClick={() => {
                        void clearCallStats().then((count) => {
                          toast.success(t('settings.dataManagement.clearSuccess', { count }))
                        }).catch((err) => {
                          toast.error(String(err))
                        })
                      }}
                    >
                      {t('common.confirm')}
                    </AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            </div>

            <Separator />

            {/* 按时间范围清空 */}
            <div className="space-y-3">
              <Label className="text-sm">{t('settings.dataManagement.clearByRange')}</Label>
              <div className="flex items-center gap-3">
                <DateTimeRangePicker
                  value={clearDateRange}
                  onChange={setClearDateRange}
                  placeholder={t('settings.dataManagement.startAt')}
                />
                <AlertDialog>
                  <AlertDialogTrigger asChild>
                    <Button variant="outline" size="sm" className="text-xs" disabled={!clearDateRange?.from || !clearDateRange?.to}>
                      {t('settings.dataManagement.clearRange')}
                    </Button>
                  </AlertDialogTrigger>
                  <AlertDialogContent>
                    <AlertDialogHeader>
                      <AlertDialogTitle>{t('settings.dataManagement.confirmTitle')}</AlertDialogTitle>
                      <AlertDialogDescription>
                        {t('settings.dataManagement.confirmRangeDescription')}
                      </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                      <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
                      <AlertDialogAction
                        onClick={() => {
                          const startAt = clearDateRange?.from?.toISOString()
                          const endAt = clearDateRange?.to?.toISOString()
                          if (!startAt || !endAt) return
                          void clearCallStats({ startAt, endAt }).then((count) => {
                            toast.success(t('settings.dataManagement.clearSuccess', { count }))
                            setClearDateRange(undefined)
                          }).catch((err) => {
                            toast.error(String(err))
                          })
                        }}
                      >
                        {t('common.confirm')}
                      </AlertDialogAction>
                    </AlertDialogFooter>
                  </AlertDialogContent>
                </AlertDialog>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* 关于 */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">
              <i className={cn('fa-solid fa-circle-info mr-2 text-muted-foreground')} />
              {t('settings.about.title')}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-center justify-between">
              <Label className="text-sm">i-code</Label>
              <span className="text-xs text-muted-foreground">{t('settings.about.description')}</span>
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <Label className="text-sm">{t('settings.about.version')}</Label>
              <div className="flex items-center gap-1">
                <span className="text-xs text-muted-foreground tabular-nums">0.2.1</span>
                <UpdateCheck />
              </div>
            </div>
            <div className="flex items-center justify-between">
              <Label className="text-sm">{t('settings.about.basedOn')}</Label>
              <span className="text-xs text-muted-foreground">Tauri 2.x + React 19</span>
            </div>
            <div className="flex items-center justify-between">
              <Label className="text-sm">{t('settings.about.license')}</Label>
              <span className="text-xs text-muted-foreground">{t('settings.about.licenseValue')}</span>
            </div>
            <div className="flex items-center justify-between">
              <Label className="text-sm">{t('settings.about.github')}</Label>
              <a
                href="https://github.com/xucux/i-code"
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-1 text-xs text-primary hover:underline"
                title={t('settings.about.githubHint')}
              >
                <i className="fa-brands fa-github text-sm" />
                {t('settings.about.githubValue')}
                <i className="fa-solid fa-arrow-up-right-from-square text-[10px]" />
              </a>
            </div>
            <div className="flex items-center justify-between gap-3">
              <Label className="shrink-0 text-sm">{t('settings.about.configDir')}</Label>
              <div className="flex min-w-0 items-center gap-1">
                <span
                  className="truncate text-xs text-muted-foreground tabular-nums"
                  title={configDir || t('settings.about.configDirEmpty')}
                >
                  {configDir || t('settings.about.configDirEmpty')}
                </span>
                {configDir && (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-6 shrink-0 px-1.5 text-[11px]"
                    onClick={() => {
                      navigator.clipboard?.writeText(configDir)
                      toast.success(t('settings.about.configDirCopied'))
                    }}
                    title={t('settings.about.configDirCopy')}
                  >
                    <i className="fa-regular fa-copy" />
                  </Button>
                )}
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
      </ScrollPage>
    </div>
  )
}

export const Route = createFileRoute('/settings')({
  component: SettingsPage,
})
