import { useMemo, useState } from 'react'
import {
  CartesianGrid,
  Line,
  LineChart,
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
import { getChartColor, getWindowConfig, generateBuckets, type WindowConfig } from './chart-utils'

/** 最小时间窗口：1 小时 */
const MIN_WINDOW_HOURS = 1
/** 最大时间窗口：24 小时 */
const MAX_WINDOW_HOURS = 24
/** 默认时间窗口：12 小时 */
const DEFAULT_WINDOW_HOURS = 12

interface TrendDataPoint {
  time: string
  [modelId: string]: string | number
}

/**
 * 将聚合统计行按桶宽 + 模型展开为折线图数据
 *
 * 生成连续时间序列（空桶补 0），每个模型一条折线。
 */
function buildTrendData(
  rows: Array<{ modelId: string; timeBucket: string; requestCount: number }>,
  windowHours: number,
  config: WindowConfig
): { data: TrendDataPoint[]; models: string[] } {
  const buckets = generateBuckets(new Date(), windowHours, config.bucketSeconds)
  const data: TrendDataPoint[] = buckets.map((bucket) => ({
    time: config.formatLabel(bucket),
  }))
  const bucketIndex = new Map(buckets.map((bucket, i) => [bucket.getTime(), i]))
  const modelSet = new Set<string>()

  for (const row of rows) {
    const idx = bucketIndex.get(new Date(row.timeBucket).getTime())
    if (idx === undefined) continue
    modelSet.add(row.modelId)
    const point = data[idx]
    point[row.modelId] = (Number(point[row.modelId]) || 0) + row.requestCount
  }

  const models = Array.from(modelSet).sort()
  // 所有模型在所有桶补 0，保证折线连续
  for (const point of data) {
    for (const model of models) {
      point[model] = Number(point[model]) || 0
    }
  }

  return { data, models }
}

/**
 * 网关请求趋势图
 *
 * 读取最近 1-24 小时调用记录，按当前窗口对应的粒度聚合，
 * 每个模型展示一条独立折线，反映各模型在不同时段的请求量分布：
 * - 1-2 小时：30 秒
 * - 2-12 小时：2 分钟
 * - 12-24 小时：5 分钟
 *
 * 底部提供滑动条调整时间窗口：最左 1 小时，最右 24 小时（默认 12 小时）。
 */
export function GatewayTrendChart() {
  const { t } = useTranslation('aiGateway')

  const [windowHours, setWindowHours] = useState<number>(DEFAULT_WINDOW_HOURS)
  const config = useMemo(() => getWindowConfig(windowHours), [windowHours])

  const input = useMemo(() => {
    const endAt = new Date().toISOString()
    const startAt = new Date(Date.now() - windowHours * 60 * 60 * 1000).toISOString()
    return { granularity: config.granularity, startAt, endAt }
  }, [windowHours, config.granularity])

  const { rows, intervalMs, setIntervalMs } = useAggregatedStats(input)
  const { data, models } = useMemo(
    () => buildTrendData(rows, windowHours, config),
    [rows, windowHours, config]
  )

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="text-base">{t('charts.trendTitle')}</CardTitle>
            <CardDescription className="text-xs">{t('charts.trendDescription')}</CardDescription>
          </div>
          <AutoRefreshSelect
            value={intervalMs as RefreshInterval}
            onValueChange={(ms) => setIntervalMs(ms as typeof intervalMs)}
          />
        </div>
      </CardHeader>
      <CardContent>
        <ResponsiveContainer width="100%" height={150}>
          <LineChart data={data} margin={{ top: 8, right: 8, left: -16, bottom: 0 }}>
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" vertical={false} />
            <XAxis
              dataKey="time"
              tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 11 }}
              axisLine={false}
              tickLine={false}
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
            {models.map((model, index) => (
              <Line
                key={model}
                type="monotone"
                dataKey={model}
                stroke={getChartColor(index, models.length)}
                strokeWidth={2}
                dot={false}
                isAnimationActive={false}
              />
            ))}
          </LineChart>
        </ResponsiveContainer>

        {/* 时间窗口滑动条：1-24 小时，默认 12 小时 */}
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
