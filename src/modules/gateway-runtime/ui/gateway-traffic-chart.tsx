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
import type { AggregatedStatsRow, StatsGranularity } from '@/modules/call-records/types'

/** 最小时间窗口：1 小时 */
const MIN_WINDOW_HOURS = 1
/** 最大时间窗口：24 小时 */
const MAX_WINDOW_HOURS = 24

interface TrafficDataPoint {
  time: string
  requests: number
}

/** 窗口配置：根据小时数选择聚合粒度、桶宽与标签格式 */
interface WindowConfig {
  granularity: StatsGranularity
  bucketSeconds: number
  formatLabel: (d: Date) => string
}

/**
 * 根据时间窗口长度选择最合适的聚合粒度
 *
 * - 1 小时：30 秒桶
 * - 2-6 小时：1 分钟桶
 * - 6-12 小时：10 分钟桶
 * - 12-24 小时：30 分钟桶
 */
function getWindowConfig(windowHours: number): WindowConfig {
  const pad = (n: number) => n.toString().padStart(2, '0')

  if (windowHours <= 1) {
    return {
      granularity: 'thirtySeconds',
      bucketSeconds: 30,
      formatLabel: (d) => `${pad(d.getHours())}:${pad(d.getMinutes())}`,
    }
  }
  if (windowHours <= 6) {
    return {
      granularity: 'oneMinute',
      bucketSeconds: 60,
      formatLabel: (d) => `${pad(d.getHours())}:${pad(d.getMinutes())}`,
    }
  }
  if (windowHours <= 12) {
    return {
      granularity: 'tenMinutes',
      bucketSeconds: 600,
      formatLabel: (d) => `${pad(d.getHours())}:${pad(d.getMinutes())}`,
    }
  }
  return {
    granularity: 'thirtyMinutes',
    bucketSeconds: 1800,
    formatLabel: (d) =>
      `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`,
  }
}

/**
 * 生成时间桶序列
 *
 * 结束时间对齐到桶宽边界，向前生成覆盖整个窗口的桶。
 */
function generateBuckets(now: Date, windowHours: number, bucketSeconds: number): Date[] {
  const buckets: Date[] = []
  const ms = now.getTime()
  const alignedMs = Math.floor(ms / (bucketSeconds * 1000)) * (bucketSeconds * 1000)
  const aligned = new Date(alignedMs)

  const totalBuckets = (windowHours * 60 * 60) / bucketSeconds
  for (let i = totalBuckets - 1; i >= 0; i--) {
    buckets.push(new Date(aligned.getTime() - i * bucketSeconds * 1000))
  }
  return buckets
}

/**
 * 将聚合统计行按指定桶宽汇总为总请求数
 *
 * 不区分模型与供应商，仅展示整体流量。
 */
function bucketRows(
  rows: AggregatedStatsRow[],
  windowHours: number,
  bucketSeconds: number,
  formatLabel: (d: Date) => string
): TrafficDataPoint[] {
  const now = new Date()
  const buckets = generateBuckets(now, windowHours, bucketSeconds)
  const counts = new Map<number, number>()

  for (const row of rows) {
    const t = new Date(row.timeBucket).getTime()
    const windowStart = buckets[0].getTime()
    const windowEnd = buckets[buckets.length - 1].getTime() + bucketSeconds * 1000
    if (t < windowStart || t >= windowEnd) continue

    const bucketMs = Math.floor(t / (bucketSeconds * 1000)) * (bucketSeconds * 1000)
    counts.set(bucketMs, (counts.get(bucketMs) ?? 0) + row.requestCount)
  }

  return buckets.map((bucket) => ({
    time: formatLabel(bucket),
    requests: counts.get(bucket.getTime()) ?? 0,
  }))
}

/**
 * 网关请求流量图
 *
 * 读取最近 1-24 小时调用记录，按当前窗口对应的粒度聚合展示总请求流量：
 * - 1 小时：30 秒
 * - 2-6 小时：1 分钟
 * - 6-12 小时：10 分钟
 * - 12-24 小时：30 分钟
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
    () => bucketRows(rows, windowHours, config.bucketSeconds, config.formatLabel),
    [rows, windowHours, config.bucketSeconds, config.formatLabel]
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
