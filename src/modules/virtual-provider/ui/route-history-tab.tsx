import { useMemo, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { useVirtualRouteAttemptStats, useVirtualRouteAttempts } from '@/hooks/use-virtual-provider'
import { useVirtualRoutesByProvider } from '@/hooks/use-virtual-provider'
import {
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { ScrollableTable } from '@/components/ui/scrollable-table'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { formatDateTime } from '@/core/utils'
import type {
  RouteAttemptStats,
  VirtualModelRoute,
} from '@/modules/virtual-provider/types'
import type { Provider } from '@/modules/ai-gateway/types'

interface RouteHistoryTabProps {
  /** 虚拟供应商 ID */
  virtualProviderId: string | null
  /** 真实供应商映射，用于展示路由对应的目标供应商名称 */
  providerMap: Map<string, Provider>
  /** 容器高度，由 useAvailableHeight 计算后传入 */
  style?: React.CSSProperties
}

/**
 * 路由历史 Tab
 *
 * 上半部分：路由维度尝试统计表（总数 / 成功 / 失败 / 成功率 / 平均耗时 / 最近失败原因）
 * 下半部分：选中路由后展示最近 N 次尝试明细（序号 / 结果 / 状态码 / 耗时 / 错误 / 时间）
 *
 * 数据来源：后端 `virtual_route_attempts` 表，由网关 VirtualForwarder 异步写入。
 */
export function RouteHistoryTab({
  virtualProviderId,
  providerMap,
  style,
}: RouteHistoryTabProps) {
  const { t } = useTranslation('virtualProvider')
  const { stats, loading: statsLoading, refetch: refetchStats } =
    useVirtualRouteAttemptStats(virtualProviderId)
  // 拉取全部路由，用于把 routeId 映射到目标供应商/模型展示
  const { routes } = useVirtualRoutesByProvider(virtualProviderId)

  const [selectedRouteId, setSelectedRouteId] = useState<string | null>(null)
  const { attempts, loading: attemptsLoading } = useVirtualRouteAttempts(
    selectedRouteId,
    50,
  )

  // routeId -> 路由信息映射，用于展示路由对应的目标供应商/模型
  const routeMap = useMemo(() => {
    const map = new Map<string, VirtualModelRoute>()
    for (const route of routes) {
      map.set(route.id, route)
    }
    return map
  }, [routes])

  // routeId -> 展示标签（目标供应商名/模型）
  const routeLabel = (routeId: string): string => {
    const route = routeMap.get(routeId)
    if (!route) return routeId
    const provider = providerMap.get(route.targetProviderId)
    return provider
      ? `${provider.displayName ?? provider.slug}/${route.targetModelId}`
      : route.targetModelId
  }

  const hasStats = stats.length > 0

  return (
    <div className="flex h-full flex-col gap-3" style={style}>
      {/* 路由维度统计表 */}
      <div className="flex min-h-0 flex-1 flex-col">
        <div className="mb-1.5 flex items-center justify-between">
          <span className="text-xs text-muted-foreground">{t('routeHistoryHint')}</span>
          <Button
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs"
            onClick={() => refetchStats()}
          >
            <i className="fa-solid fa-rotate-right mr-1" />
            {/* 刷新 */}
          </Button>
        </div>

        <ScrollableTable
          viewMode="compact"
          density="compact"
          loading={statsLoading}
          loadingText={t('loading')}
          className="flex-1"
        >
          <TableHeader className="sticky top-0 z-10 bg-muted">
            <TableRow>
              <TableHead className="w-[40px]"></TableHead>
              <TableHead>{t('statsRoute')}</TableHead>
              <TableHead className="text-right tabular-nums">{t('statsTotal')}</TableHead>
              <TableHead className="text-right tabular-nums">{t('statsSuccess')}</TableHead>
              <TableHead className="text-right tabular-nums">{t('statsFailure')}</TableHead>
              <TableHead className="text-right tabular-nums">{t('statsSuccessRate')}</TableHead>
              <TableHead className="text-right tabular-nums">{t('statsAvgDuration')}</TableHead>
              <TableHead className="min-w-[160px]">{t('statsLastError')}</TableHead>
              <TableHead className="whitespace-nowrap">{t('statsLastAttempt')}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {!hasStats && !statsLoading && (
              <TableRow>
                <TableCell colSpan={9} className="py-8 text-center text-xs text-muted-foreground">
                  {t('routeHistoryEmpty')}
                </TableCell>
              </TableRow>
            )}
            {stats.map((stat) => (
              <StatsRow
                key={stat.virtualRouteId}
                stat={stat}
                label={routeLabel(stat.virtualRouteId)}
                selected={selectedRouteId === stat.virtualRouteId}
                onSelect={() =>
                  setSelectedRouteId(
                    selectedRouteId === stat.virtualRouteId ? null : stat.virtualRouteId,
                  )
                }
              />
            ))}
          </TableBody>
        </ScrollableTable>
      </div>

      {/* 选中路由的最近尝试明细 */}
      {selectedRouteId && (
        <div className="flex min-h-0 flex-1 flex-col">
          <div className="mb-1.5 flex items-center gap-2">
            <span className="text-xs font-medium">{t('recentAttemptsTitle')}</span>
            <Badge variant="outline" className="text-[10px]">
              {routeLabel(selectedRouteId)}
            </Badge>
          </div>

          <ScrollableTable
            viewMode="compact"
            density="compact"
            loading={attemptsLoading}
            loadingText={t('loading')}
            className="flex-1"
          >
            <TableHeader className="sticky top-0 z-10 bg-muted">
              <TableRow>
                <TableHead className="w-[48px] text-right tabular-nums">{t('attemptIndex')}</TableHead>
                <TableHead className="w-[72px]">{t('attemptResult')}</TableHead>
                <TableHead className="w-[72px] text-right tabular-nums">{t('attemptStatusCode')}</TableHead>
                <TableHead className="w-[88px] text-right tabular-nums">{t('attemptDuration')}</TableHead>
                <TableHead className="min-w-[180px]">{t('attemptError')}</TableHead>
                <TableHead className="whitespace-nowrap">{t('attemptTime')}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {!attemptsLoading && attempts.length === 0 && (
                <TableRow>
                  <TableCell colSpan={6} className="py-6 text-center text-xs text-muted-foreground">
                    {t('routeHistoryEmpty')}
                  </TableCell>
                </TableRow>
              )}
              {attempts.map((attempt) => (
                <TableRow key={attempt.id}>
                  <TableCell className="text-right tabular-nums text-xs text-muted-foreground">
                    {attempt.attemptIndex + 1}
                  </TableCell>
                  <TableCell>
                    {attempt.success ? (
                      <Badge variant="default" className="bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 text-[10px]">
                        <i className="fa-solid fa-check mr-1 text-[8px]" />
                        {t('attemptSuccess')}
                      </Badge>
                    ) : (
                      <Badge variant="destructive" className="text-[10px]">
                        <i className="fa-solid fa-xmark mr-1 text-[8px]" />
                        {t('attemptFailed')}
                      </Badge>
                    )}
                  </TableCell>
                  <TableCell className="text-right tabular-nums text-xs text-muted-foreground">
                    {attempt.statusCode ?? '—'}
                  </TableCell>
                  <TableCell className="text-right tabular-nums text-xs text-muted-foreground">
                    {attempt.durationMs}ms
                  </TableCell>
                  <TableCell className="text-xs text-muted-foreground">
                    {attempt.errorMessage ?? '—'}
                  </TableCell>
                  <TableCell className="whitespace-nowrap text-xs text-muted-foreground">
                    {formatDateTime(attempt.attemptedAt)}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </ScrollableTable>
        </div>
      )}

      {!selectedRouteId && hasStats && (
        <div className="flex flex-1 items-center justify-center text-xs text-muted-foreground">
          {t('recentAttemptsSelectRoute')}
        </div>
      )}
    </div>
  )
}

interface StatsRowProps {
  stat: RouteAttemptStats
  label: string
  selected: boolean
  onSelect: () => void
}

/**
 * 路由统计行
 *
 * 点击行切换选中状态，选中后下方展示该路由最近尝试明细。
 */
function StatsRow({ stat, label, selected, onSelect }: StatsRowProps) {
  // 成功率颜色编码：>=90 绿 / >=70 黄 / <70 红 / 无数据灰
  const rateVariant: 'success' | 'warning' | 'danger' | 'muted' =
    stat.total === 0
      ? 'muted'
      : stat.successRate >= 90
        ? 'success'
        : stat.successRate >= 70
          ? 'warning'
          : 'danger'

  const rateColor = {
    success: 'text-emerald-600 dark:text-emerald-400',
    warning: 'text-amber-600 dark:text-amber-400',
    danger: 'text-red-600 dark:text-red-400',
    muted: 'text-muted-foreground',
  }[rateVariant]

  return (
    <TableRow
      className={cn(
        'cursor-pointer transition-colors',
        selected ? 'bg-accent' : 'hover:bg-muted/50',
      )}
      onClick={onSelect}
    >
      <TableCell className="text-center">
        <i
          className={cn(
            'fa-solid fa-chevron-right text-[10px] text-muted-foreground transition-transform',
            selected && 'rotate-90',
          )}
        />
      </TableCell>
      <TableCell className="text-xs">
        <span className="font-medium">{label}</span>
      </TableCell>
      <TableCell className="text-right tabular-nums text-xs">{stat.total}</TableCell>
      <TableCell className="text-right tabular-nums text-xs text-emerald-600 dark:text-emerald-400">
        {stat.successCount}
      </TableCell>
      <TableCell className="text-right tabular-nums text-xs text-red-600 dark:text-red-400">
        {stat.failureCount}
      </TableCell>
      <TableCell className={cn('text-right tabular-nums text-xs font-medium', rateColor)}>
        {stat.total > 0 ? `${stat.successRate}%` : '—'}
      </TableCell>
      <TableCell className="text-right tabular-nums text-xs text-muted-foreground">
        {stat.avgDurationMs}ms
      </TableCell>
      <TableCell className="max-w-[280px] truncate text-xs text-muted-foreground" title={stat.lastError ?? ''}>
        {stat.lastError ?? '—'}
      </TableCell>
      <TableCell className="whitespace-nowrap text-xs text-muted-foreground">
        {stat.lastAttemptedAt ? formatDateTime(stat.lastAttemptedAt) : '—'}
      </TableCell>
    </TableRow>
  )
}
