import { createFileRoute } from '@tanstack/react-router'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { useGatewayStatus } from '@/hooks/use-gateway-status'
import { cn } from '@/lib/utils'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { ProviderList } from '@/modules/ai-gateway/ui/provider-list'
import { ModelList } from '@/modules/ai-gateway/ui/model-list'
import { VirtualProviderList } from '@/modules/virtual-provider/ui/virtual-provider-list'
import { ScriptTemplateList } from '@/modules/script-template/ui/script-template-list'
import { GatewayBasicSettings, GatewayAuthKeyManager } from '@/modules/ai-gateway/ui/gateway-settings'
import { GatewayTrafficChart } from '@/modules/gateway-runtime/ui/gateway-traffic-chart'
import { GatewayTrendChart } from '@/modules/gateway-runtime/ui/gateway-trend-chart'
import { GatewayTokenChart } from '@/modules/gateway-runtime/ui/gateway-token-chart'
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
        toast.error(String(err))
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
            <div className="flex flex-col gap-4 p-4">
              {/* 状态栏：圆点 + 状态 + IP:端口 */}
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
                        {status.boundHost}:{status.boundPort}
                      </span>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="size-6"
                        title={tc('common.copy')}
                        onClick={async () => {
                          try {
                            await navigator.clipboard.writeText(
                              `http://${status.boundHost}:${status.boundPort}`
                            )
                            toast.success(tc('common.copied'))
                          } catch {
                            toast.error(tc('common.copyFailed'))
                          }
                        }}
                      >
                        <i className="fa-solid fa-copy text-xs" />
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

              {/* 上方：实时请求流量；下方：请求趋势 + Token 消耗
                  趋势与 Token 消耗使用预聚合表数据，按模型分曲线 */}
              <GatewayTrafficChart />
              <div className="grid grid-cols-2 gap-4">
                <GatewayTrendChart />
                <GatewayTokenChart />
              </div>
            </div>
          </ScrollPage>
        </TabsContent>

        {/* 网关认证：API Key 管理 */}
        <TabsContent value="gateway-auth" className="h-screen overflow-auto">
          <GatewayAuthKeyManager />
        </TabsContent>

        {/* 网关配置：监听地址、端口、日志级别 */}
        <TabsContent value="gateway-settings" className="h-screen overflow-auto">
          <GatewayBasicSettings />
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
    </div>
  )
}

export const Route = createFileRoute('/gateways/')({
  component: GatewaysIndexPage,
})
