import { useMemo, useState } from 'react'
import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Slider } from '@/components/ui/slider'
import { useAggregatedStats } from '@/hooks/use-aggregated-stats'
import { AutoRefreshSelect } from '@/components/ui/auto-refresh'
import type { RefreshInterval } from '@/components/ui/auto-refresh'
import { useTranslation } from '@/modules/i18n/use-translation'
import type { AggregatedStatsRow } from '@/modules/call-records/types'
import { getWindowConfig, generateBuckets, type WindowConfig } from './chart-utils'

/** 最小时间窗口：1 小时 */
const MIN_WINDOW_HOURS = 1
/** 最大时间窗口：24 小时 */
const MAX_WINDOW_HOURS = 24

interface TrafficDataPoint {
  time: string
  requests: number
}

/**
 * 将聚合统计行按指定桶宽汇总为总请求数
 *
 * 不区分模型与供应商，仅展示整体流量；空桶补 0 保持时间轴连续。
 */
function bucketRows(
  rows: AggregatedStatsRow[],
  windowHours: number,
  config: WindowConfig
): TrafficDataPoint[] {
  const now = new Date()
  const buckets = generateBuckets(now, windowHours, config.bucketSeconds)
  const counts = new Map<number, number>()

  for (const row of rows) {
    const t = new Date(row.timeBucket).getTime()
    const windowStart = buckets[0].getTime()
    const windowEnd = buckets[buckets.length - 1].getTime() + config.bucketSeconds * 1000
    if (t < windowStart || t >= windowEnd) continue

    const bucketMs = Math.floor(t / (config.bucketSeconds * 1000)) * (config.bucketSeconds * 1000)
    counts.set(bucketMs, (counts.get(bucketMs) ?? 0) + row.requestCount)
  }

  return buckets.map((bucket) => ({
    time: config.formatLabel(bucket),
    requests: counts.get(bucket.getTime()) ?? 0,
  }))
}

/**
 * 网关请求流量图
 *
 * 读取最近 1-24 小时调用记录，按当前窗口对应的粒度聚合展示总请求流量：
 * - 1-2 小时：30 秒
 * - 2-12 小时：2 分钟
 * - 12-24 小时：5 分钟
 *
 * 底部提供滑动条调整时间窗口：最左 1 小时，最右 24 小时。
 * 无数据时显示 0，保持时间轴连续。
 */
export function GatewayTrafficChart() {
  const { t } = useTranslation('aiGateway')

  const [windowHours, setWindowHours] = useState<number>(MIN_WINDOW_HOURS)
  const config = useMemo(() => getWindowConfig(windowHours), [windowHours])

  const input = useMemo(() => {
    const endAt = new Date().toISOString()
    const startAt = new Date(Date.now() - windowHours * 60 * 60 * 1000).toISOString()
    return { granularity: config.granularity, startAt, endAt }
  }, [windowHours, config.granularity])

  const { rows, intervalMs, setIntervalMs } = useAggregatedStats(input)
  const data = useMemo(
    () => bucketRows(rows, windowHours, config),
    [rows, windowHours, config]
  )

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="text-base">{t('charts.trafficTitle')}</CardTitle>
            <CardDescription className="text-xs">{t('charts.trafficDescription')}</CardDescription>
          </div>
          <AutoRefreshSelect
            value={intervalMs as RefreshInterval}
            onValueChange={(ms) => setIntervalMs(ms as typeof intervalMs)}
          />
        </div>
      </CardHeader>
      <CardContent>
        <ResponsiveContainer width="100%" height={150}>
          <AreaChart data={data} margin={{ top: 8, right: 8, left: -16, bottom: 0 }}>
            <defs>
              <linearGradient id="gatewayTrafficGradient" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor="hsl(var(--primary))" stopOpacity={0.35} />
                <stop offset="95%" stopColor="hsl(var(--primary))" stopOpacity={0.02} />
              </linearGradient>
            </defs>
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" vertical={false} />
            <XAxis
              dataKey="time"
              tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 11 }}
              axisLine={false}
              tickLine={false}
              interval="preserveStartEnd"
              minTickGap={24}
            />
            <YAxis
              tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 11 }}
              axisLine={false}
              tickLine={false}
              allowDecimals={false}
            />
            <Tooltip
              contentStyle={{
                backgroundColor: 'hsl(var(--card))',
                borderColor: 'hsl(var(--border))',
                borderRadius: 'var(--radius)',
                color: 'hsl(var(--card-foreground))',
              }}
            />
            <Area
              type="monotone"
              dataKey="requests"
              stroke="hsl(var(--primary))"
              fill="url(#gatewayTrafficGradient)"
              strokeWidth={2}
              isAnimationActive={false}
            />
          </AreaChart>
        </ResponsiveContainer>

        {/* 时间窗口滑动条：1-24 小时 */}
        <div className="mt-3 flex items-center justify-center gap-3">
          <span className="text-muted-foreground text-xs tabular-nums">
            {t('charts.windowHours', { hours: MIN_WINDOW_HOURS })}
          </span>
          <Slider
            value={[windowHours]}
            min={MIN_WINDOW_HOURS}
            max={MAX_WINDOW_HOURS}
            step={1}
            size="sm"
            onValueChange={([value]) => setWindowHours(value ?? MAX_WINDOW_HOURS)}
            className="w-48"
          />
          <span className="text-muted-foreground text-xs tabular-nums">
            {t('charts.windowHours', { hours: windowHours })}
          </span>
        </div>
      </CardContent>
    </Card>
  )
}
