import { useMemo, useState } from 'react'
import {
  Bar,
  BarChart,
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
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { useAggregatedStats } from '@/hooks/use-aggregated-stats'
import { useTranslation } from '@/modules/i18n/use-translation'
import { formatCompactCount } from '@/core/utils'
import type { AggregatedStatsRow } from '@/modules/call-records/types'
import { getTokenBarColors } from './chart-utils'

/** 时间范围下拉可选天数（默认 14 天） */
const DAY_RANGES = [7, 14, 31, 60, 90] as const
const DEFAULT_DAYS = 14
/** 模型筛选下拉中"全部模型"的选项值 */
const ALL_MODELS = 'all'

interface DailyTokenPoint {
  /** UTC 日期（YYYY-MM-DD），与聚合表天级时间桶对齐 */
  date: string
  /** 当天总 Token 消耗 */
  total: number
  /** 当天缓存命中 Token 消耗 */
  cached: number
}

/**
 * 生成最近 N 天的 UTC 日期序列（含今天，不补未来日期）
 *
 * 天级聚合表按 UTC 整天对齐，日期序列与时间桶保持同一时区口径。
 */
function generateUtcDates(days: number): string[] {
  const now = new Date()
  const list: string[] = []
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate() - i))
    list.push(d.toISOString().slice(0, 10))
  }
  return list
}

/**
 * 将聚合统计行映射为按天柱状图数据
 *
 * 依据 `timeBucket`（UTC 整天）与日期序列对齐，未产生调用的天补 0；
 * `modelId` 为 `ALL_MODELS` 时汇总全部模型，否则仅保留指定模型。
 */
function buildDailyData(
  rows: AggregatedStatsRow[],
  modelId: string,
  days: number
): DailyTokenPoint[] {
  const dates = generateUtcDates(days)
  const byDate = new Map(dates.map((date) => [date, { date, total: 0, cached: 0 }]))

  for (const row of rows) {
    if (modelId !== ALL_MODELS && row.modelId !== modelId) continue
    const point = byDate.get(row.timeBucket.slice(0, 10))
    if (!point) continue
    point.total += row.totalTokens
    point.cached += row.cachedTokens
  }

  return dates.map((date) => byDate.get(date)!)
}

/**
 * 网关 Token 累计消耗图
 *
 * 按「天级」聚合数据以柱状图展示每日 Token 消耗：
 * - 亮色柱：当天总 Token 消耗
 * - 深色柱：当天缓存命中 Token 消耗
 *
 * 右上角支持切换时间范围（7/14/31/60/90 天，默认 14 天）与按模型筛选
 * （默认全部模型）；数据来自预聚合表，随自动刷新更新。
 */
export function GatewayTokenCumulativeChart() {
  const { t } = useTranslation('aiGateway')
  const [selectedModel, setSelectedModel] = useState<string>(ALL_MODELS)
  const [days, setDays] = useState<number>(DEFAULT_DAYS)

  const input = useMemo(() => {
    const endAt = new Date().toISOString()
    const startAt = new Date(Date.now() - (days - 1) * 24 * 60 * 60 * 1000).toISOString()
    return { granularity: 'daily' as const, startAt, endAt }
  }, [days])

  // 全量拉取天数内聚合行（行数 = 天数 × 模型数，量级很小），模型筛选在前端完成
  const { rows } = useAggregatedStats(input)

  // 模型下拉候选：统计数据中实际出现过的模型
  const modelOptions = useMemo(() => {
    const set = new Set<string>()
    for (const row of rows) set.add(row.modelId)
    return Array.from(set).sort()
  }, [rows])

  const data = useMemo(
    () => buildDailyData(rows, selectedModel, days),
    [rows, selectedModel, days]
  )
  const colors = useMemo(() => getTokenBarColors(), [])

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="text-base">{t('charts.cumulativeTitle')}</CardTitle>
            <CardDescription className="text-xs">
              {t('charts.cumulativeDescription', { days })}
            </CardDescription>
          </div>
          <div className="flex items-center gap-2">
            <Select value={String(days)} onValueChange={(value) => setDays(Number(value))}>
              <SelectTrigger className="h-7 w-24 text-xs" title={t('charts.rangeDays')}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {DAY_RANGES.map((d) => (
                  <SelectItem key={d} value={String(d)} className="text-xs">
                    {t('charts.lastDays', { days: d })}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Select value={selectedModel} onValueChange={setSelectedModel}>
              <SelectTrigger className="h-7 w-44 text-xs" title={t('charts.filterModel')}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ALL_MODELS} className="text-xs">
                  {t('charts.allModels')}
                </SelectItem>
                {modelOptions.map((model) => (
                  <SelectItem key={model} value={model} className="text-xs">
                    {model}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <ResponsiveContainer width="100%" height={150}>
          <BarChart data={data} margin={{ top: 8, right: 8, left: -8, bottom: 0 }}>
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" vertical={false} />
            <XAxis
              dataKey="date"
              tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 11 }}
              axisLine={false}
              tickLine={false}
              minTickGap={24}
              tickFormatter={(value: string) => value.slice(5)}
            />
            <YAxis
              tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 11 }}
              axisLine={false}
              tickLine={false}
              allowDecimals={false}
              tickFormatter={(value: number) => formatCompactCount(value)}
            />
            <Tooltip
              cursor={{ fill: 'hsl(var(--muted))', fillOpacity: 0.5 }}
              contentStyle={{
                backgroundColor: 'hsl(var(--card))',
                borderColor: 'hsl(var(--border))',
                borderRadius: 'var(--radius)',
                color: 'hsl(var(--card-foreground))',
                fontSize: 12,
              }}
              formatter={(value, name) => [
                formatCompactCount(Number(value)),
                name === 'total' ? t('charts.legendTotal') : t('charts.legendCached'),
              ]}
            />
            <Bar dataKey="total" fill={colors.total} radius={[3, 3, 0, 0]} maxBarSize={18} isAnimationActive={false} />
            <Bar dataKey="cached" fill={colors.cached} radius={[3, 3, 0, 0]} maxBarSize={18} isAnimationActive={false} />
          </BarChart>
        </ResponsiveContainer>

        {/* 图例：总消耗（浅） / 缓存命中（深） */}
        <div className="mt-2 flex items-center justify-center gap-4 text-xs text-muted-foreground">
          <span className="flex items-center gap-1.5">
            <span className="inline-block size-2.5 rounded-[3px]" style={{ backgroundColor: colors.total }} />
            {t('charts.legendTotal')}
          </span>
          <span className="flex items-center gap-1.5">
            <span className="inline-block size-2.5 rounded-[3px]" style={{ backgroundColor: colors.cached }} />
            {t('charts.legendCached')}
          </span>
        </div>
      </CardContent>
    </Card>
  )
}