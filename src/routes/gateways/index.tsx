import { useState } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { useGatewayStatus } from '@/hooks/use-gateway-status'
import { useLocalIps } from '@/hooks/use-local-ips'
import { cn } from '@/lib/utils'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { ProviderList } from '@/modules/ai-gateway/ui/provider-list'
import { ModelList } from '@/modules/ai-gateway/ui/model-list'
import { PortInUseDialog } from '@/modules/ai-gateway/ui/port-in-use-dialog'
import { toIcodeError } from '@/core/errors'
import { VirtualProviderList } from '@/modules/virtual-provider/ui/virtual-provider-list'
import { ScriptTemplateList } from '@/modules/script-template/ui/script-template-list'
import { GatewayBasicSettings, GatewayAuthKeyManager, CallbackServerManager } from '@/modules/ai-gateway/ui/gateway-settings'
import { GatewayTrafficChart } from '@/modules/gateway-runtime/ui/gateway-traffic-chart'
import { GatewayTrendChart } from '@/modules/gateway-runtime/ui/gateway-trend-chart'
import { GatewayTokenChart } from '@/modules/gateway-runtime/ui/gateway-token-chart'
import { GatewayTokenCumulativeChart } from '@/modules/gateway-runtime/ui/gateway-token-cumulative-chart'
import { GatewayApiDocsDialog } from '@/modules/gateway-runtime/ui/gateway-api-docs-dialog'
import { ScrollPage } from '@/components/ui/scroll-page'
import { toast } from 'sonner'

/**
 * AI Gateway 总览页
 *
 * 通过顶部 Tab 统一组织网关相关功能：
 * - 网关：运行状态总览、流量/趋势图
 * - 认证：网关 API Key 管理
 * - 配置：网关监听地址、端口、日志级别等
 * - 对外模型：暴露给客户端的 Gateway 模型
 * - 供应商：真实上游供应商
 * - 虚拟供应商：聚合真实供应商的故障转移入口
 * - 脚本模板：自定义额度监控 Rhai 脚本
 */
function GatewaysIndexPage() {
  const { t } = useTranslation('aiGateway')
  const { t: tc } = useTranslation()
  const { status, loading, start, stop } = useGatewayStatus()
  // 当网关监听 0.0.0.0 / :: 时，解析本机 LAN 地址用于展示、复制与接口文档
  const resolved = useLocalIps(status.boundHost, status.boundPort)
  const [apiDocsOpen, setApiDocsOpen] = useState(false)
  // 网关启动失败（端口被占用/权限不足等）时弹出帮助弹窗
  const [portInUsePort, setPortInUsePort] = useState<number | null>(null)

  /** 切换网关启动/停止 */
  const handleToggleGateway = async () => {
    if (status.isRunning) {
      try {
        await stop()
        toast.success(t('gatewayOverview.gatewayStopped'))
      } catch (err) {
        toast.error(String(err))
      }
    } else {
      try {
        const result = await start()
        if (result.success) {
          toast.success(t('gatewayOverview.gatewayStarted', { host: result.host, port: result.port }))
        } else {
          toast.error(result.error ?? t('gatewayOverview.gatewayStartFailed'))
        }
      } catch (err) {
        const error = toIcodeError(err)
        // 绑定失败（端口被占用 / 权限不足 / 地址不可用）时复用端口帮助弹窗，
        // 而不是只展示一条启动失败 toast
        if (error.details?.reason && typeof error.details.port === 'number') {
          setPortInUsePort(error.details.port)
        } else {
          toast.error(error.message)
        }
      }
    }
  }

  return (
    <div className="h-full  p-6">
      <Tabs defaultValue="real" className="h-[calc(100%-5rem)]">
        <div className="mb-4 flex items-center justify-between">
          <TabsList className="h-8">
            <TabsTrigger value="gateway" className="text-xs">{t('gatewayOverview.tabs.gateway')}</TabsTrigger>
            <TabsTrigger value="gateway-auth" className="text-xs">{t('gatewayOverview.tabs.auth')}</TabsTrigger>
            <TabsTrigger value="gateway-settings" className="text-xs">{t('gatewayOverview.tabs.settings')}</TabsTrigger>
            <TabsTrigger value="model-stats" className="text-xs">{t('gatewayOverview.tabs.modelStats')}</TabsTrigger>
            <TabsTrigger value="real" className="text-xs">{t('gatewayOverview.tabs.providers')}</TabsTrigger>
            <TabsTrigger value="virtual" className="text-xs">{t('gatewayOverview.tabs.virtualProviders')}</TabsTrigger>
            <TabsTrigger value="script-templates" className="text-xs">
              {/* <i className="fa-solid fa-scroll mr-1" /> */}
              {t('gatewayOverview.tabs.scriptTemplates')}
            </TabsTrigger>
          </TabsList>
          <div className="flex items-center gap-2">
            <Button
              variant={status.isRunning ? 'destructive' : 'outline'}
              size="sm"
              className="h-7 text-xs"
              onClick={handleToggleGateway}
              disabled={loading}
            >
              <i className={cn('fa-solid', status.isRunning ? 'fa-stop' : 'fa-play', 'mr-1.5')} />
              {status.isRunning ? t('gatewayOverview.stopGateway') : t('gatewayOverview.startGateway')}
            </Button>
            {/* 页面级帮助：点击问号 icon 展示本页所有注意事项 */}
            <Popover>
              <PopoverTrigger asChild>
                <button
                  type="button"
                  className="flex size-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                  aria-label={t('help', { ns: 'common' })}
                >
                  <i className="fa-regular fa-circle-question text-sm" />
                </button>
              </PopoverTrigger>
              <PopoverContent side="bottom" align="end" className="w-72 text-xs">
                <ul className="space-y-1.5">
                  <li>• {t('gatewayOverview.tabs.gateway')}：{t('gatewayOverview.help.gateway')}</li>
                  <li>• {t('gatewayOverview.tabs.auth')}：{t('gatewayOverview.help.auth')}</li>
                  <li>• {t('gatewayOverview.tabs.scriptTemplates')}：{t('gatewayOverview.help.scriptTemplates')}</li>
                  <li>• {t('gatewayOverview.tabs.settings')}：{t('gatewayOverview.help.settings')}</li>
                  <li>• {t('gatewayOverview.tabs.modelStats')}：{t('gatewayOverview.help.modelStats')}</li>
                  <li>• {t('gatewayOverview.tabs.providers')}：{t('gatewayOverview.help.providers')}</li>
                  <li>• {t('gatewayOverview.tabs.virtualProviders')}：{t('gatewayOverview.help.virtualProviders')}</li>
                </ul>
              </PopoverContent>
            </Popover>
          </div>
        </div>

        {/* 网关总览 */}
        <TabsContent value="gateway" className="h-screen overflow-auto">
          <ScrollPage variant="borderless" scrollbarVisible="auto" className="h-full">
            <div className="flex flex-col gap-4 p-4 pb-20">
              {/* 三幅全宽图：实时请求流量 → Token 消耗 → Token 累计消耗 → 请求趋势
                  三者均使用明细表实时聚合数据，按窗口自适应粒度 */}
              <GatewayTrafficChart />
              <GatewayTokenChart />
              <GatewayTokenCumulativeChart />
              <GatewayTrendChart />
            </div>
          </ScrollPage>
        </TabsContent>

        {/* 网关认证：API Key 管理 */}
        <TabsContent value="gateway-auth" className="h-screen overflow-auto">
          <GatewayAuthKeyManager />
        </TabsContent>

        {/* 网关配置：状态卡片、监听地址、端口、日志级别、回调服务器 */}
        <TabsContent value="gateway-settings" className="h-screen overflow-auto">
          <ScrollPage variant="borderless" scrollbarVisible="auto" className="h-full">
            <div className="flex flex-col gap-4 p-4 pb-20">
              {/* 状态栏：圆点 + 状态 + IP:端口 + 复制 + 接口文档 */}
              <Card>
                <CardContent className="flex items-center gap-4 py-3">
                  <div className="flex items-center gap-2">
                    <span
                      className={cn(
                        'inline-block size-2.5 rounded-full',
                        status.isRunning ? 'bg-emerald-500' : 'bg-muted-foreground'
                      )}
                    />
                    <span className="text-sm font-medium">
                      {status.isRunning ? t('gatewayOverview.statusRunning') : t('gatewayOverview.statusStopped')}
                    </span>
                  </div>
                  {status.boundHost && status.boundPort ? (
                    <div className="flex items-center gap-1.5">
                      <span className="text-muted-foreground font-mono text-xs">
                        {resolved.displayHost}:{status.boundPort}
                      </span>
                      {/* 监听通配地址提示：当前展示的为解析后的 LAN 地址 */}
                      {resolved.isWildcard && (
                        <span
                          className="rounded bg-blue-500/15 px-1.5 py-0.5 text-[10px] text-blue-600 dark:text-blue-400"
                          title={t('gatewayOverview.wildcardListeningHint')}
                        >
                          {t('gatewayOverview.wildcardListening')}
                        </span>
                      )}
                      <Button
                        variant="ghost"
                        size="icon"
                        className="size-6"
                        title={tc('common.copy')}
                        onClick={async () => {
                          try {
                            await navigator.clipboard.writeText(
                              `http://${resolved.displayHost}:${status.boundPort}/v1`
                            )
                            toast.success(tc('common.copied'))
                          } catch {
                            toast.error(tc('common.copyFailed'))
                          }
                        }}
                      >
                        <i className="fa-solid fa-copy text-xs" />
                      </Button>
                      {/* 接口文档：点击弹出网关支持的接口清单 */}
                      <Button
                        variant="ghost"
                        size="icon"
                        className="size-6"
                        title={t('gatewayApiDocs.title')}
                        onClick={() => setApiDocsOpen(true)}
                      >
                        <i className="fa-solid fa-book-open text-xs" />
                      </Button>
                    </div>
                  ) : (
                    <span className="text-muted-foreground text-xs">{t('gatewayOverview.unboundAddress')}</span>
                  )}
                  {status.lastError && (
                    <span className="text-destructive ml-auto text-xs">{status.lastError}</span>
                  )}
                </CardContent>
              </Card>

              {/* 网关接口文档弹窗：地址与端口跟随状态卡片解析结果 */}
              <GatewayApiDocsDialog
                open={apiDocsOpen}
                onOpenChange={setApiDocsOpen}
                hosts={resolved.hosts}
                defaultHost={resolved.displayHost}
                port={status.boundPort}
              />

              <div className="space-y-4">
                <GatewayBasicSettings />
                <CallbackServerManager />
              </div>
            </div>
          </ScrollPage>
        </TabsContent>

        {/* 对外模型：ModelList 自身已用 useAvailableHeight + ScrollableTable 做内部滚动，勿再套 ScrollPage */}
        <TabsContent value="model-stats" className="h-[calc(100%-1rem)]">
          <ModelList />
        </TabsContent>

        {/* 真实供应商 */}
        <TabsContent value="real" className="h-full">
          <ProviderList />
        </TabsContent>

        {/* 虚拟供应商 */}
        <TabsContent value="virtual" className="h-full">
          <VirtualProviderList />
        </TabsContent>

        {/* 脚本模板 */}
        <TabsContent value="script-templates" className="h-full">
          <ScriptTemplateList />
        </TabsContent>
      </Tabs>

      {/* 网关启动失败帮助弹窗：端口被占用/权限不足时复用端口排查指引。
          放在 Tabs 外层，确保在任意 Tab 下点击启动网关都能弹出。 */}
      <PortInUseDialog
        open={portInUsePort !== null}
        port={portInUsePort}
        onOpenChange={(v) => !v && setPortInUsePort(null)}
        title={t('gatewayOverview.gatewayPortInUseTitle')}
        description={t('gatewayOverview.gatewayPortInUseDesc', { port: portInUsePort ?? '' })}
      />
    </div>
  )
}

export const Route = createFileRoute('/gateways/')({
  component: GatewaysIndexPage,
})
