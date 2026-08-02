import { useMemo } from 'react'
import {
  Area,
  AreaChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useAggregatedStats } from '@/hooks/use-aggregated-stats'
import { useTranslation } from '@/modules/i18n/use-translation'
import type { AggregatedStatsRow, StatsGranularity } from '@/modules/call-records/types'
import { generateBuckets } from '@/modules/gateway-runtime/ui/chart-utils'

/** 仪表盘图表固定时间窗口：最近 12 小时 */
const WINDOW_HOURS = 12
/** 仪表盘图表粒度：30 分钟桶（12h → 24 点，小卡片足够） */
const GRANULARITY: StatsGranularity = 'thirtyMinutes'
const BUCKET_SECONDS = 1800

const pad = (n: number) => n.toString().padStart(2, '0')
const formatLabel = (d: Date) => `${pad(d.getHours())}:${pad(d.getMinutes())}`

interface SeriesPoint {
  time: string
  value: number
}

/**
 * 将聚合统计行按桶汇总为单一序列
 *
 * 不区分模型与供应商（汇总全部维度），空桶补 0 保持时间轴连续。
 */
function buildSeries(
  rows: AggregatedStatsRow[],
  pick: (row: AggregatedStatsRow) => number
): SeriesPoint[] {
  const buckets = generateBuckets(new Date(), WINDOW_HOURS, BUCKET_SECONDS)
  const counts = new Map<number, number>()
  const windowStart = buckets[0].getTime()
  const windowEnd = buckets[buckets.length - 1].getTime() + BUCKET_SECONDS * 1000

  for (const row of rows) {
    const t = new Date(row.timeBucket).getTime()
    if (t < windowStart || t >= windowEnd) continue
    const bucketMs = Math.floor(t / (BUCKET_SECONDS * 1000)) * (BUCKET_SECONDS * 1000)
    counts.set(bucketMs, (counts.get(bucketMs) ?? 0) + pick(row))
  }

  return buckets.map((bucket) => ({
    time: formatLabel(bucket),
    value: counts.get(bucket.getTime()) ?? 0,
  }))
}

/** 仪表盘小面积图：紧凑渲染，隐藏坐标轴，仅保留面积与 tooltip */
function DashboardAreaChart({ data, gradientId }: { data: SeriesPoint[]; gradientId: string }) {
  return (
    <ResponsiveContainer width="100%" height={40}>
      <AreaChart data={data} margin={{ top: 4, right: 4, left: 0, bottom: 0 }}>
        <defs>
          <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%" stopColor="hsl(var(--primary))" stopOpacity={0.35} />
            <stop offset="95%" stopColor="hsl(var(--primary))" stopOpacity={0.02} />
          </linearGradient>
        </defs>
        <XAxis dataKey="time" hide />
        <YAxis hide domain={[0, 'auto']} allowDecimals={false} />
        <Tooltip
          contentStyle={{
            backgroundColor: 'hsl(var(--card))',
            borderColor: 'hsl(var(--border))',
            borderRadius: 'var(--radius)',
            color: 'hsl(var(--card-foreground))',
            fontSize: 11,
          }}
        />
        <Area
          type="monotone"
          dataKey="value"
          stroke="hsl(var(--primary))"
          strokeWidth={1.5}
          fill={`url(#${gradientId})`}
          isAnimationActive={false}
        />
      </AreaChart>
    </ResponsiveContainer>
  )
}

/**
 * 仪表盘 Token 消耗概览图
 *
 * 最近 12 小时、30 分钟桶，面积图，不分模型（汇总全部模型 Token 用量）。
 */
export function DashboardTokenChart() {
  const { t } = useTranslation()

  const input = useMemo(
    () => ({
      granularity: GRANULARITY,
      startAt: new Date(Date.now() - WINDOW_HOURS * 60 * 60 * 1000).toISOString(),
      endAt: new Date().toISOString(),
    }),
    []
  )
  const { rows } = useAggregatedStats(input)
  const data = useMemo(() => buildSeries(rows, (r) => r.totalTokens), [rows])

  return (
    <Card>
      <CardHeader className="px-4 pt-3 pb-1">
        <div className="flex items-center justify-between">
          <CardTitle className="text-xs font-medium">{t('dashboard.tokenConsumption')}</CardTitle>
          <span className="text-[10px] text-muted-foreground">{t('dashboard.tokenConsumptionDesc')}</span>
        </div>
      </CardHeader>
      <CardContent className="px-2 pb-2 pt-0">
        <DashboardAreaChart data={data} gradientId="dashTokenGrad" />
      </CardContent>
    </Card>
  )
}

/**
 * 仪表盘请求次数概览图
 *
 * 最近 12 小时、30 分钟桶，面积图，不分模型（汇总全部请求数）。
 */
export function DashboardRequestChart() {
  const { t } = useTranslation()

  const input = useMemo(
    () => ({
      granularity: GRANULARITY,
      startAt: new Date(Date.now() - WINDOW_HOURS * 60 * 60 * 1000).toISOString(),
      endAt: new Date().toISOString(),
    }),
    []
  )
  const { rows } = useAggregatedStats(input)
  const data = useMemo(() => buildSeries(rows, (r) => r.requestCount), [rows])

  return (
    <Card>
      <CardHeader className="px-4 pt-3 pb-1">
        <div className="flex items-center justify-between">
          <CardTitle className="text-xs font-medium">{t('dashboard.requestCount')}</CardTitle>
          <span className="text-[10px] text-muted-foreground">{t('dashboard.requestCountDesc')}</span>
        </div>
      </CardHeader>
      <CardContent className="px-2 pb-2 pt-0">
        <DashboardAreaChart data={data} gradientId="dashRequestGrad" />
      </CardContent>
    </Card>
  )
}
