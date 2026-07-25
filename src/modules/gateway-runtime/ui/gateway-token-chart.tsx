import { useMemo } from 'react'
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
import { useAggregatedStats } from '@/hooks/use-aggregated-stats'
import { AutoRefreshSelect } from '@/components/ui/auto-refresh'
import type { RefreshInterval } from '@/components/ui/auto-refresh'
import { useTranslation } from '@/modules/i18n/use-translation'

/** Token 消耗图默认展示近 1 小时 */
const WINDOW_HOURS = 1

interface TokenDataPoint {
  time: string
  [modelId: string]: string | number
}

/**
 * 生成 HSL 色相环颜色，保证不同模型曲线视觉可区分
 */
function generateColor(index: number, total: number): string {
  const hue = Math.round((index * 360) / Math.max(total, 1))
  return `hsl(${hue} 70% 55%)`
}

/**
 * 将时间桶格式化为 "MM-DD HH:mm"
 */
function formatBucketLabel(timeBucket: string): string {
  const t = new Date(timeBucket)
  const month = (t.getMonth() + 1).toString().padStart(2, '0')
  const day = t.getDate().toString().padStart(2, '0')
  const hours = t.getHours().toString().padStart(2, '0')
  const minutes = t.getMinutes().toString().padStart(2, '0')
  return `${month}-${day} ${hours}:${minutes}`
}

/**
 * 将聚合统计行按 10 分钟桶 + 模型展开为 Token 消耗堆叠面积图数据
 */
function buildTokenData(
  rows: Array<{ modelId: string; timeBucket: string; totalTokens: number }>
): { data: TokenDataPoint[]; models: string[] } {
  const modelSet = new Set<string>()
  const bucketMap = new Map<string, Map<string, number>>()

  for (const row of rows) {
    modelSet.add(row.modelId)
    const timeLabel = formatBucketLabel(row.timeBucket)

    if (!bucketMap.has(timeLabel)) {
      bucketMap.set(timeLabel, new Map())
    }
    const modelMap = bucketMap.get(timeLabel)!
    modelMap.set(row.modelId, (modelMap.get(row.modelId) ?? 0) + row.totalTokens)
  }

  const sortedTimes = Array.from(bucketMap.keys()).sort()
  const models = Array.from(modelSet).sort()

  const data = sortedTimes.map((time) => {
    const point: TokenDataPoint = { time }
    const modelMap = bucketMap.get(time)!
    for (const model of models) {
      point[model] = modelMap.get(model) ?? 0
    }
    return point
  })

  return { data, models }
}

/**
 * 网关 Token 消耗图
 *
 * 从 `model_call_logs` 明细表实时聚合最近 1 小时数据（默认），按 10 分钟桶统计，
 * 每个模型展示一条堆叠面积曲线，反映各模型每 10 分钟的 Token 消耗总量。
 */
export function GatewayTokenChart() {
  const { t } = useTranslation('aiGateway')

  const input = useMemo(() => {
    const endAt = new Date().toISOString()
    const startAt = new Date(Date.now() - WINDOW_HOURS * 60 * 60 * 1000).toISOString()
    return { granularity: 'tenMinutes' as const, startAt, endAt }
  }, [])

  const { rows, intervalMs, setIntervalMs } = useAggregatedStats(input)
  const { data, models } = useMemo(() => buildTokenData(rows), [rows])

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="text-base">{t('charts.tokenTitle')}</CardTitle>
            <CardDescription className="text-xs">{t('charts.tokenDescription')}</CardDescription>
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
              {models.map((model, index) => {
                const color = generateColor(index, models.length)
                return (
                  <linearGradient
                    key={model}
                    id={`tokenGradient-${model}`}
                    x1="0"
                    y1="0"
                    x2="0"
                    y2="1"
                  >
                    <stop offset="5%" stopColor={color} stopOpacity={0.35} />
                    <stop offset="95%" stopColor={color} stopOpacity={0.02} />
                  </linearGradient>
                )
              })}
            </defs>
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
              <Area
                key={model}
                type="monotone"
                dataKey={model}
                stroke={generateColor(index, models.length)}
                fill={`url(#tokenGradient-${model})`}
                strokeWidth={2}
                stackId="tokens"
                isAnimationActive={false}
              />
            ))}
          </AreaChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  )
}
