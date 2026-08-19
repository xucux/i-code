import { createFileRoute } from '@tanstack/react-router'
import { useCallback, useEffect, useMemo, useState } from 'react'
import type { DateRange } from 'react-day-picker'
import { useTranslation } from '@/modules/i18n/use-translation'

import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import { LogViewer } from '@/components/ui/log-viewer'
import { AutoRefreshSelect } from '@/components/ui/auto-refresh'
import { ScrollPage } from '@/components/ui/scroll-page'
import { DateTimeRangePicker } from '@/components/ui/date-time-range-picker'
import { useLogs, DEFAULT_LOG_BUFFER_SIZE } from '@/hooks/use-logs'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { invokeCommand } from '@/hooks/use-command'
import type { LogFilter, LogSource, LogSettings } from '@/modules/logger/types'

const DEFAULT_SETTINGS: LogSettings = {
  bufferSize: 5000,
  logDir: '',
  maxRetentionDays: 30,
  enableFilePersistence: false,
  maxFileSizeMb: 10,
  maxFileCount: 7,
  fileLogLevel: 'INFO',
  enableRequestLog: true,
  enableResponseLog: true,
  forwardMaxBodyLength: 4096,
  enableGatewayRequestLog: true,
  enableGatewayResponseLog: true,
  gatewayMaxBodyLength: 4096,
  enableCommandLog: true,
  enableCommandRequestLog: false,
  enableCommandResponseLog: false,
  commandMaxBodyLength: 4096,
}

/**
 * 日志模块主页
 */
function LogsIndexPage() {
  const { t } = useTranslation('logger')
  const [activeTab, setActiveTab] = useState<'gateway' | 'system' | 'settings'>('gateway')
  const [keyword, setKeyword] = useState('')
  const [timeRange, setTimeRange] = useState<DateRange | undefined>()
  const [refreshInterval, setRefreshInterval] = useState<number | null>(2000)

  // 统一日志配置
  const [settings, setSettings] = useState<LogSettings>(DEFAULT_SETTINGS)

  const loadSettings = useCallback(async () => {
    try {
      const s = await invokeCommand<LogSettings>('log_get_settings')
      setSettings(s)
    } catch { /* 忽略 */ }
  }, [])

  useEffect(() => { void loadSettings() }, [loadSettings])

  const updateSettings = useCallback(async (patch: Partial<LogSettings>) => {
    const newSettings = { ...settings, ...patch }
    try {
      const updated = await invokeCommand<LogSettings>('log_set_settings', { settings: newSettings })
      setSettings(updated)
    } catch { /* 忽略 */ }
  }, [settings])

  /**
   * 根据当前 Tab 决定日志来源
   */
  const sources: LogSource[] = useMemo(
    () => (activeTab === 'gateway' ? ['gateway', 'provider-api'] : ['system']),
    [activeTab]
  )

  const filter: LogFilter = useMemo(() => {
    const next: LogFilter = { sources }
    const trimmed = keyword.trim()
    if (trimmed) next.keyword = trimmed
    const range: { from?: string; to?: string } = {}
    if (timeRange?.from) range.from = timeRange.from.toISOString()
    if (timeRange?.to) range.to = timeRange.to.toISOString()
    if (range.from || range.to) next.timeRange = range
    return next
  }, [sources, keyword, timeRange])

  const { logs, error, clear } = useLogs({
    filter,
    bufferSize: DEFAULT_LOG_BUFFER_SIZE,
    autoRefresh: refreshInterval != null,
    refreshInterval: refreshInterval ?? 3000,
  })

  // 始终可见的外层容器总高度
  const [pageHeight, pageRef] = useAvailableHeight()

  // 始终可见的表头区域实际高度（TabsList + 筛选栏）
  const [headerHeight, headerRef] = useAvailableHeight()

  // 设置 Tab 中 ScrollPage 的可用高度
  // p-6 = 24px 上 + 24px 下 = 48px 内边距
  const settingsHeight = useMemo(
    () => Math.max(0, pageHeight - headerHeight - 48),
    [pageHeight, headerHeight]
  )

  return (
    <div ref={pageRef} className="flex h-full flex-col p-6">
      <Tabs
        value={activeTab}
        onValueChange={(value) => setActiveTab(value as 'gateway' | 'system' | 'settings')}
        className="flex flex-1 flex-col"
      >
        {/* Tab 与筛选控件 */}
        <div ref={headerRef} className="mb-4 space-y-3">
          <div className="flex items-center justify-between">
            <TabsList>
              <TabsTrigger value="gateway" className="text-xs">{t('tabs.gateway')}</TabsTrigger>
              <TabsTrigger value="system" className="text-xs">{t('tabs.system')}</TabsTrigger>
              <TabsTrigger value="settings" className="text-xs">
                <i className="fa-solid fa-gear mr-1.5 size-3" />
                {t('tabs.settings')}
              </TabsTrigger>
            </TabsList>
            {activeTab !== 'settings' && (
              <Badge variant="outline" className="text-xs">
                {t('buffer', { count: logs.length, size: settings.bufferSize })}
              </Badge>
            )}
          </div>

          {activeTab !== 'settings' && (
            <div className="flex flex-wrap items-center gap-3">
              {/* 时间范围筛选 */}
              <DateTimeRangePicker
                value={timeRange}
                onChange={setTimeRange}
                placeholder={t('timeRange.placeholder')}
              />

              {/* 关键字筛选 */}
              <Input
                placeholder={t('keywordPlaceholder')}
                value={keyword}
                onChange={(e) => setKeyword(e.target.value)}
                className="h-8 w-52 text-xs"
                aria-label={t('keywordPlaceholder')}
              />

              {/* 自动刷新下拉 */}
              <AutoRefreshSelect value={refreshInterval} onValueChange={setRefreshInterval} />
              <Button variant="outline" size="sm" className="h-8 text-xs" onClick={clear}>
                <i className="fa-solid fa-trash-can mr-1.5" />
                {t('clear')}
              </Button>
            </div>
          )}

          {error && <p className="text-destructive text-xs">{error}</p>}
        </div>

        {/* 网关日志 */}
        <TabsContent value="gateway" className="flex-1">
          <LogViewer logs={logs} style={{ height: settingsHeight || undefined }} emptyText={t('empty.gateway')} />
        </TabsContent>

        {/* 系统日志 */}
        <TabsContent value="system" className="flex-1">
          <LogViewer logs={logs} style={{ height: settingsHeight || undefined }} emptyText={t('empty.system')} />
        </TabsContent>
           {/*<Card className="flex h-full flex-col">
            <CardHeader className="pb-2">
              <CardTitle className="text-sm">系统日志</CardTitle>
              <CardDescription className="text-xs">应用启动、停止与系统级事件</CardDescription>
            </CardHeader>
            <CardContent className="flex-1 overflow-hidden">
               
            </CardContent>
           
          </Card> */}

        {/* 日志设置 */}
        <TabsContent value="settings" className="flex-1 min-h-0 overflow-hidden ">
          <ScrollPage style={{ height: settingsHeight || undefined }} variant="borderless" scrollbarVisible="auto">
          <div className="space-y-4 pr-4 h-full pb-100">
            {/* 基础设置 */}
            <Card>
              <CardHeader className="pb-3">
                <CardTitle className="text-sm">{t('settings.basic')}</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="grid grid-cols-[auto_1fr] items-center gap-x-4 gap-y-2">
                  <Label htmlFor="buffer-size" className="text-xs whitespace-nowrap">{t('settings.bufferSize')}</Label>
                  <Input
                    id="buffer-size"
                    type="number"
                    min={100}
                    max={50000}
                    value={settings.bufferSize}
                    onChange={(e) => updateSettings({ bufferSize: parseInt(e.target.value) || 5000 })}
                    className="h-8 w-40 text-xs"
                  />

                  <Label htmlFor="log-dir" className="text-xs whitespace-nowrap">{t('settings.logDir')}</Label>
                  <Input
                    id="log-dir"
                    placeholder={t('settings.logDirPlaceholder')}
                    value={settings.logDir}
                    onChange={(e) => updateSettings({ logDir: e.target.value })}
                    className="h-8 w-80 text-xs"
                  />

                  <Label htmlFor="retention-days" className="text-xs whitespace-nowrap">{t('settings.retentionDays')}</Label>
                  <Input
                    id="retention-days"
                    type="number"
                    min={1}
                    max={365}
                    value={settings.maxRetentionDays}
                    onChange={(e) => updateSettings({ maxRetentionDays: parseInt(e.target.value) || 30 })}
                    className="h-8 w-40 text-xs"
                  />

                  <Label htmlFor="file-persistence" className="text-xs whitespace-nowrap">{t('settings.filePersistence')}</Label>
                  <div className="flex items-center gap-2">
                    <Switch
                      id="file-persistence"
                      checked={settings.enableFilePersistence}
                      onCheckedChange={(v) => updateSettings({ enableFilePersistence: v })}
                    />
                    <span className="text-xs text-muted-foreground">{t('settings.filePersistenceHint')}</span>
                  </div>
                </div>
              </CardContent>
            </Card>

            {/* 转发详细日志 */}
            <Card>
              <CardHeader className="pb-3">
                <CardTitle className="text-sm">{t('settings.forward')}</CardTitle>
                <CardDescription className="text-xs">{t('settings.forwardDesc')}</CardDescription>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <Switch
                      id="fwd-req"
                      checked={settings.enableRequestLog}
                      onCheckedChange={(v) => updateSettings({ enableRequestLog: v })}
                    />
                    <Label htmlFor="fwd-req" className="text-xs">{t('settings.recordRequestBody')}</Label>
                    <span className="text-xs text-muted-foreground">{t('settings.recordRequestBodyHint')}</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <Switch
                      id="fwd-res"
                      checked={settings.enableResponseLog}
                      onCheckedChange={(v) => updateSettings({ enableResponseLog: v })}
                    />
                    <Label htmlFor="fwd-res" className="text-xs">{t('settings.recordResponseBody')}</Label>
                    <span className="text-xs text-muted-foreground">{t('settings.recordResponseBodyHint')}</span>
                  </div>
                  <Separator className="my-2" />
                  <div className="flex items-center gap-4">
                    <Label htmlFor="fwd-max-len" className="text-xs whitespace-nowrap">{t('settings.maxLength')}</Label>
                    <Input
                      id="fwd-max-len"
                      type="number"
                      min={256}
                      max={65536}
                      value={settings.forwardMaxBodyLength}
                      onChange={(e) => updateSettings({ forwardMaxBodyLength: parseInt(e.target.value) || 4096 })}
                      className="h-8 w-32 text-xs"
                    />
                    <span className="text-xs text-muted-foreground">{t('settings.chars')}</span>
                  </div>
                </div>
              </CardContent>
            </Card>

            {/* 直连网关请求日志 */}
            <Card>
              <CardHeader className="pb-3">
                <CardTitle className="text-sm">{t('settings.direct')}</CardTitle>
                <CardDescription className="text-xs">{t('settings.directDesc')}</CardDescription>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <Switch
                      id="gw-req"
                      checked={settings.enableGatewayRequestLog}
                      onCheckedChange={(v) => updateSettings({ enableGatewayRequestLog: v })}
                    />
                    <Label htmlFor="gw-req" className="text-xs">{t('settings.recordRequestBody')}</Label>
                    <span className="text-xs text-muted-foreground">{t('settings.recordRequestBodyHint')}</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <Switch
                      id="gw-res"
                      checked={settings.enableGatewayResponseLog}
                      onCheckedChange={(v) => updateSettings({ enableGatewayResponseLog: v })}
                    />
                    <Label htmlFor="gw-res" className="text-xs">{t('settings.recordResponseBody')}</Label>
                    <span className="text-xs text-muted-foreground">{t('settings.directResponseBodyHint')}</span>
                  </div>
                  <Separator className="my-2" />
                  <div className="flex items-center gap-4">
                    <Label htmlFor="gw-max-len" className="text-xs whitespace-nowrap">{t('settings.maxLength')}</Label>
                    <Input
                      id="gw-max-len"
                      type="number"
                      min={256}
                      max={65536}
                      value={settings.gatewayMaxBodyLength}
                      onChange={(e) => updateSettings({ gatewayMaxBodyLength: parseInt(e.target.value) || 4096 })}
                      className="h-8 w-32 text-xs"
                    />
                    <span className="text-xs text-muted-foreground">{t('settings.chars')}</span>
                  </div>
                </div>
              </CardContent>
            </Card>

            {/* Command 交互日志 */}
            <Card>
              <CardHeader className="pb-3">
                <CardTitle className="text-sm">{t('settings.command')}</CardTitle>
                <CardDescription className="text-xs">{t('settings.commandDesc')}</CardDescription>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <Switch
                      id="cmd-log"
                      checked={settings.enableCommandLog}
                      onCheckedChange={(v) => updateSettings({ enableCommandLog: v })}
                    />
                    <Label htmlFor="cmd-log" className="text-xs">{t('settings.commandLog')}</Label>
                  </div>
                  <div className="flex items-center gap-2 pl-6">
                    <Switch
                      id="cmd-req"
                      checked={settings.enableCommandRequestLog}
                      onCheckedChange={(v) => updateSettings({ enableCommandRequestLog: v })}
                      disabled={!settings.enableCommandLog}
                    />
                    <Label htmlFor="cmd-req" className="text-xs">{t('settings.recordCommandRequest')}</Label>
                  </div>
                  <div className="flex items-center gap-2 pl-6">
                    <Switch
                      id="cmd-res"
                      checked={settings.enableCommandResponseLog}
                      onCheckedChange={(v) => updateSettings({ enableCommandResponseLog: v })}
                      disabled={!settings.enableCommandLog}
                    />
                    <Label htmlFor="cmd-res" className="text-xs">{t('settings.recordCommandResponse')}</Label>
                  </div>
                  <Separator className="my-2" />
                  <div className="flex items-center gap-4">
                    <Label htmlFor="cmd-max-len" className="text-xs whitespace-nowrap">{t('settings.maxLength')}</Label>
                    <Input
                      id="cmd-max-len"
                      type="number"
                      min={256}
                      max={65536}
                      value={settings.commandMaxBodyLength}
                      onChange={(e) => updateSettings({ commandMaxBodyLength: parseInt(e.target.value) || 4096 })}
                      className="h-8 w-32 text-xs"
                    />
                    <span className="text-xs text-muted-foreground">{t('settings.chars')}</span>
                  </div>
                </div>
              </CardContent>
            </Card>
          </div>
          </ScrollPage>
        </TabsContent>
      </Tabs>
    </div>
  )
}

export const Route = createFileRoute('/logs/')({
  component: LogsIndexPage,
})
