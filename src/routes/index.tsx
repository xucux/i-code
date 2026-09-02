import { useState } from 'react'
import { createFileRoute, Link } from '@tanstack/react-router'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { useGatewayStatus } from '@/hooks/use-gateway-status'
import { useProviderList } from '@/hooks/use-provider-list'
import { useModelList } from '@/hooks/use-model-list'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import { DashboardTokenChart, DashboardRequestChart } from '@/modules/call-records/ui/dashboard-charts'
import { GatewayApiDocsDialog } from '@/modules/gateway-runtime/ui/gateway-api-docs-dialog'
import { PortInUseDialog } from '@/modules/ai-gateway/ui/port-in-use-dialog'
import { toIcodeError } from '@/core/errors'
import { useLocalIps } from '@/hooks/use-local-ips'

/** 图表显隐状态持久化键 */
const CHARTS_STORAGE_KEY = 'i-code:dashboard-charts-visible'

/**
 * 仪表盘首页
 *
 * 展示应用核心状态概览：网关状态、供应商数量、模型数量，并提供 AI Gateway、CLI、工作区、备份与组件预览等主要入口。
 * 网关状态卡片支持一键启动/停止；底部提供 Token 消耗与请求次数两张概览图（右上角可隐藏）。
 */
function IndexPage() {
  const { t } = useTranslation()
  const { t: tg } = useTranslation('aiGateway')
  const { status, loading, start, stop } = useGatewayStatus()
  const { providers } = useProviderList()
  const { models } = useModelList()
  // 当网关监听 0.0.0.0 / :: 时，解析本机 LAN 地址用于接口文档弹窗
  const resolved = useLocalIps(status.boundHost, status.boundPort)
  const [apiDocsOpen, setApiDocsOpen] = useState(false)
  // 网关启动失败（端口被占用/权限不足等）时弹出帮助弹窗
  const [portInUsePort, setPortInUsePort] = useState<number | null>(null)

  // 图表显隐状态（持久化到 localStorage）
  const [chartsVisible, setChartsVisible] = useState(() => {
    try {
      return localStorage.getItem(CHARTS_STORAGE_KEY) !== '0'
    } catch {
      return true
    }
  })
  const toggleCharts = () => {
    setChartsVisible((visible) => {
      const next = !visible
      try {
        localStorage.setItem(CHARTS_STORAGE_KEY, next ? '1' : '0')
      } catch {
        // 忽略持久化失败，仅影响本次会话
      }
      return next
    })
  }

  /** 切换网关启动/停止 */
  const handleToggleGateway = async () => {
    if (status.isRunning) {
      try {
        await stop()
        toast.success(t('dashboard.gatewayStopped'))
      } catch (err) {
        toast.error(String(err))
      }
    } else {
      try {
        const result = await start()
        if (result.success) {
          toast.success(t('dashboard.gatewayStarted'))
        } else {
          toast.error(result.error ?? t('dashboard.gatewayStartFailed'))
        }
      } catch (err) {
        const error = toIcodeError(err)
        // 绑定失败（端口被占用 / 权限不足 / 地址不可用）时复用端口帮助弹窗，
        // 与网关页启动行为保持一致
        if (error.details?.reason && typeof error.details.port === 'number') {
          setPortInUsePort(error.details.port)
        } else {
          toast.error(error.message)
        }
      }
    }
  }

  const stats = [
    {
      label: t('dashboard.gatewayStatus'),
      value: status.isRunning ? t('dashboard.running') : t('dashboard.stopped'),
      icon: 'fa-solid fa-network-wired',
      status: status.isRunning ? 'success' : 'inactive',
    },
    {
      label: t('dashboard.providers'),
      value: String(providers.length),
      icon: 'fa-solid fa-server',
      to: '/gateways/providers',
    },
    {
      label: t('dashboard.modelStats'),
      value: String(models.length),
      icon: 'fa-solid fa-cube',
      to: '/gateways/models',
    },
  ] as const

  return (
    <div className="h-full overflow-auto p-6">
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-lg font-semibold">{t('nav.dashboard')}</h1>
        {/* 图表显示/隐藏：最右侧 icon 按钮 */}
        <Button
          variant="outline"
          size="icon"
          className="size-7 rounded-md"
          title={chartsVisible ? t('dashboard.hideCharts') : t('dashboard.showCharts')}
          onClick={toggleCharts}
        >
          <i className={cn('fa-solid text-xs text-muted-foreground', chartsVisible ? 'fa-eye' : 'fa-eye-slash')} />
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        {stats.map((stat) => (
          <Card key={stat.label}>
            <CardHeader className="pb-2">
              <CardDescription className="text-xs">{stat.label}</CardDescription>
              <div className="flex items-center gap-2">
                <i className={cn(stat.icon, 'text-muted-foreground')} />
                <CardTitle className="text-2xl font-semibold">{stat.value}</CardTitle>
              </div>
            </CardHeader>
            <CardContent>
              {'status' in stat && (
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-1">
                    <Badge
                      variant="outline"
                      className={cn(
                        'flex h-7 items-center whitespace-nowrap text-[10px]',
                        stat.status === 'success' && 'border-emerald-500 text-emerald-500',
                        stat.status === 'inactive' && 'text-muted-foreground'
                      )}
                    >
                      {stat.status === 'success' ? t('dashboard.healthy') : t('dashboard.inactive')}
                    </Badge>
                    {/* 接口文档：仅网关运行时显示，紧贴 Badge，圆角方框按钮 */}
                    {status.isRunning && (
                      <Button
                        variant="outline"
                        size="icon"
                        className="size-7 rounded-md"
                        title={tg('gatewayApiDocs.title')}
                        onClick={() => setApiDocsOpen(true)}
                      >
                        <i className="fa-solid fa-book-open text-xs text-muted-foreground" />
                      </Button>
                    )}
                  </div>
                  {/* 网关启停：圆角方框按钮，仅 icon */}
                  <Button
                    variant="outline"
                    size="icon"
                    className="size-7 rounded-md"
                    title={status.isRunning ? t('dashboard.stopGateway') : t('dashboard.startGateway')}
                    disabled={loading}
                    onClick={handleToggleGateway}
                  >
                    <i
                      className={cn('fa-solid text-xs text-muted-foreground', status.isRunning ? 'fa-stop' : 'fa-play')}
                    />
                  </Button>
                </div>
              )}
              {'to' in stat && (
                <Button asChild variant="ghost" size="sm" className="h-7 text-xs">
                  <Link to={stat.to}>{t('dashboard.view')}</Link>
                </Button>
              )}
            </CardContent>
          </Card>
        ))}
      </div>

      {/* 接口文档弹窗：地址与端口跟随网关状态解析结果 */}
      <GatewayApiDocsDialog
        open={apiDocsOpen}
        onOpenChange={setApiDocsOpen}
        hosts={resolved.hosts}
        defaultHost={resolved.displayHost}
        port={status.boundPort}
      />

      {/* 网关启动失败帮助弹窗：端口被占用/权限不足时复用端口排查指引 */}
      <PortInUseDialog
        open={portInUsePort !== null}
        port={portInUsePort}
        onOpenChange={(v) => !v && setPortInUsePort(null)}
        title={tg('gatewayOverview.gatewayPortInUseTitle')}
        description={tg('gatewayOverview.gatewayPortInUseDesc', { port: portInUsePort ?? '' })}
      />

      <div className="mt-6 grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        {[
          { to: '/gateways', label: t('dashboard.aiGateway'), icon: 'fa-solid fa-network-wired', desc: t('dashboard.aiGatewayDesc') },
          { to: '/cli', label: t('nav.cli'), icon: 'fa-solid fa-terminal', desc: t('dashboard.cliDesc') },
          { to: '/backups', label: t('nav.backup'), icon: 'fa-solid fa-cloud-arrow-up', desc: t('dashboard.backupDesc') },
          // { to: '/preview', label: t('nav.preview'), icon: 'fa-solid fa-palette', desc: t('dashboard.previewDesc') },
          { to: '/chat', label: t('nav.chat'), icon: 'fa-solid fa-comments', desc: t('dashboard.chatDesc') },
        ].map((item) => (
          <Button
            key={item.to}
            asChild
            variant="outline"
            className="h-auto flex-col items-start gap-1 p-4 text-left"
          >
            <Link to={item.to}>
              <span className="flex items-center gap-2 text-sm font-medium">
                <i className={cn(item.icon, 'text-muted-foreground')} />
                {item.label}
              </span>
              <span className="text-muted-foreground text-xs">{item.desc}</span>
            </Link>
          </Button>
        ))}
      </div>

      {/* 概览图：Token 消耗 + 请求次数（可隐藏） */}
      {chartsVisible && (
        <div className="mt-6 grid gap-4 md:grid-cols-2">
          <DashboardTokenChart />
          <DashboardRequestChart />
        </div>
      )}
    </div>
  )
}

export const Route = createFileRoute('/')({
  component: IndexPage,
})
