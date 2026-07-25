import { createFileRoute, Link } from '@tanstack/react-router'
import { useTranslation } from '@/modules/i18n/use-translation'
import { useGatewayStatus } from '@/hooks/use-gateway-status'
import { useProviderList } from '@/hooks/use-provider-list'
import { useModelList } from '@/hooks/use-model-list'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'

/**
 * 仪表盘首页
 *
 * 展示应用核心状态概览：网关状态、供应商数量、模型数量，并提供 AI Gateway、CLI、工作区、备份与组件预览等主要入口。
 */
function IndexPage() {
  const { t } = useTranslation()
  const { status } = useGatewayStatus()
  const { providers } = useProviderList()
  const { models } = useModelList()

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
      <div className="mb-6">
        <h1 className="text-lg font-semibold">{t('nav.dashboard')}</h1>
        {/* <p className="text-muted-foreground text-sm">{t('app.tagline', { ns: 'app' })}</p> */}
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
                <Badge
                  variant="outline"
                  className={cn(
                    'text-[10px]',
                    stat.status === 'success' && 'border-emerald-500 text-emerald-500',
                    stat.status === 'inactive' && 'text-muted-foreground'
                  )}
                >
                  {stat.status === 'success' ? t('dashboard.healthy') : t('dashboard.inactive')}
                </Badge>
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

      <div className="mt-6 grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        {[
          { to: '/gateways', label: t('dashboard.aiGateway'), icon: 'fa-solid fa-network-wired', desc: t('dashboard.aiGatewayDesc') },
          { to: '/cli', label: t('nav.cli'), icon: 'fa-solid fa-terminal', desc: t('dashboard.cliDesc') },
          // { to: '/workspaces', label: t('nav.workspace'), icon: 'fa-solid fa-briefcase', desc: t('dashboard.workspaceDesc') },
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
                <i className={cn(item.icon)} />
                {item.label}
              </span>
              <span className="text-muted-foreground text-xs">{item.desc}</span>
            </Link>
          </Button>
        ))}
      </div>
    </div>
  )
}

export const Route = createFileRoute('/')({
  component: IndexPage,
})
