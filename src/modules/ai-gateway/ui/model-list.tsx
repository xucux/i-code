import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { AutoRefreshSelect } from '@/components/ui/auto-refresh'
import { cn } from '@/lib/utils'
import { useModelCallStats } from '@/hooks/use-model-call-stats'
import { useAggregatedStats } from '@/hooks/use-aggregated-stats'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { listGatewayAuthKeys } from '@/hooks/use-ai-gateway-mutation'
import { ModelStatisticsTable } from './model-statistics-table'
import { AggregatedStatsTable } from './aggregated-stats-table'
import type { CallSource, StatsGranularity } from '@/modules/call-records/types'
import type { RefreshInterval } from '@/components/ui/auto-refresh'
import type { GatewayAuthKey } from '@/modules/ai-gateway/types'

/** 时间范围快捷选项
 *
 * - `hours` 存在：按「当前时间 - hours」计算起点
 * - `hours` 缺省（today）：按「今天 00:00:00」计算起点，跨日时自动回滚到当天 0 点
 */
function useTimeRangeOptions() {
  const { t } = useTranslation('aiGateway')
  return [
    { id: '1', label: t('timeRange.1hour'), hours: 1 },
    { id: '6', label: t('timeRange.6hours'), hours: 6 },
    { id: '12', label: t('timeRange.12hours'), hours: 12 },
    { id: 'today', label: t('timeRange.today') },
    { id: '24', label: t('timeRange.24hours'), hours: 24 },
    { id: '168', label: t('timeRange.7days'), hours: 168 },
    { id: '720', label: t('timeRange.30days'), hours: 720 },
  ]
}

/**
 * 计算时间范围起点的 ISO 8601 字符串
 * - hours 提供：now - hours*3600_000
 * - hours 缺省：今天本地 00:00:00
 */
function getTimeRangeStart(hours?: number): string {
  const d = new Date()
  if (hours == null) {
    d.setHours(0, 0, 0, 0)
  } else {
    d.setTime(d.getTime() - hours * 3600_000)
  }
  return d.toISOString()
}

/**
 * 截断展示 API Key 前缀
 *
 * 按用户要求：下拉中仅显示前缀，无需全部展示也无需隐藏，直接截断。
 * 默认保留前 12 个字符并追加 "..."，完整 key 通过 title 提示展示。
 */
function truncateApiKey(key: string | undefined | null, maxLen = 12): string {
  if (!key) return ''
  if (key.length <= maxLen) return key
  return `${key.slice(0, maxLen)}...`
}

/**
 * 将 token 数值格式化为紧凑计数（K / M / B 西式单位）
 * - K = 千（1,000）
 * - M = 百万（1,000,000）
 * - B = 十亿（1,000,000,000）
 *
 * 用于统计描述中的总 token 展示。
 */
function formatTokenKMB(value: number): string {
  if (!Number.isFinite(value) || value < 0) return '0'
  if (value < 1_000) return String(Math.round(value))
  if (value < 1_000_000) return `${(value / 1_000).toFixed(1)}K`
  if (value < 1_000_000_000) return `${(value / 1_000_000).toFixed(2)}M`
  return `${(value / 1_000_000_000).toFixed(2)}B`
}

/**
 * AI Gateway 模型统计页面
 *
 * 双模式：
 * - 「明细」：从 model_call_logs 实时 GROUP BY 聚合
 * - 「聚合」：从 model_call_stats_hourly / _daily 预聚合表查询（高性能）
 *
 * 支持过滤条件：入口、路由模式、时间范围。
 * 聚合模式额外支持时间粒度选择（小时/天）。
 */
export function ModelList() {
  const { t } = useTranslation('aiGateway')

  // ===== 通用过滤条件 =====
  const [source, setSource] = useState<CallSource | 'all'>('all')
  const [routeMode, setRouteMode] = useState<string>('all')
  const [timeRangeId, setTimeRangeId] = useState<string>('1')
  const [apiKeySecretId, setApiKeySecretId] = useState<string>('all')
  const [authKeys, setAuthKeys] = useState<GatewayAuthKey[]>([])

  // 加载网关 API Key 列表，用于下拉筛选
  useEffect(() => {
    let cancelled = false
    listGatewayAuthKeys()
      .then((keys) => {
        if (!cancelled) setAuthKeys(keys)
      })
      .catch(() => {
        // 加载失败时保留空列表，下拉仅展示「全部 API Key」
      })
    return () => { cancelled = true }
  }, [])

  // ===== 聚合模式特有 =====
  const [granularity, setGranularity] = useState<StatsGranularity>('hourly')

  // ===== 表格视图模式：紧凑（自适应换行） / 滚动（固定列宽横向滚动） =====
  const [viewMode, setViewMode] = useState<'compact' | 'scroll'>('scroll')

  // ===== 当前激活的 Tab：用于计算对应数据的总 token =====
  const [activeTab, setActiveTab] = useState<'detail' | 'aggregated'>('detail')

  const timeRangeOptions = useTimeRangeOptions()

  // 计算时间范围：直接依赖基本类型 timeRangeId（字符串/数字），避免依赖 timeRangeOptions
  // 数组引用（每次 render 重建）导致 startAt 连锁重建，进而破坏 AutoRefresh 的 setInterval 节奏。
  // today 跨日：startAt 缓存为切换当天的 0 点，跨日后不自动推进；切换时间范围或刷新页面会修正。
  const startAt = useMemo(() => {
    const hours = timeRangeId === 'today' ? undefined : Number(timeRangeId)
    return getTimeRangeStart(hours)
  }, [timeRangeId])

  // 仅供 statsDescription 文案使用，引用稳定性不影响 fetch / timer
  const timeRangeOption = useMemo(
    () => timeRangeOptions.find((o) => o.id === timeRangeId) ?? timeRangeOptions[0],
    [timeRangeOptions, timeRangeId]
  )

  // ===== 明细模式 =====
  const detailInput = useMemo(() => ({
    startAt,
    source: source !== 'all' ? source as CallSource : undefined,
    routeMode: routeMode !== 'all' ? Number(routeMode) : undefined,
    apiKeySecretId: apiKeySecretId !== 'all' ? apiKeySecretId : undefined,
  }), [startAt, source, routeMode, apiKeySecretId])

  const {
    rows: detailRows,
    loading: detailLoading,
    refetch: detailRefetch,
    intervalMs: detailInterval,
    setIntervalMs: setDetailInterval,
  } = useModelCallStats(detailInput)

  // ===== 聚合模式 =====
  const aggInput = useMemo(() => ({
    granularity,
    startAt,
    source: source !== 'all' ? source as CallSource : undefined,
    routeMode: routeMode !== 'all' ? Number(routeMode) : undefined,
    apiKeySecretId: apiKeySecretId !== 'all' ? apiKeySecretId : undefined,
  }), [granularity, startAt, source, routeMode, apiKeySecretId])

  const {
    rows: aggRows,
    loading: aggLoading,
    refetch: aggRefetch,
    intervalMs: _aggInterval,
    setIntervalMs: setAggInterval,
  } = useAggregatedStats(aggInput)

  // 当前 Tab 对应数据的总 token（K/M/B 紧凑格式）
  const totalTokensText = useMemo(() => {
    const rows = activeTab === 'detail' ? detailRows : aggRows
    const sum = rows.reduce((acc, r) => acc + (r.totalTokens ?? 0), 0)
    return formatTokenKMB(sum)
  }, [activeTab, detailRows, aggRows])

  // 统计描述文本
  const statsDescription = useMemo(() => {
    const timeText = timeRangeOption?.hours == null
      ? t('modelManagement.today')
      : timeRangeOption.hours >= 24
        ? t('modelManagement.days', { count: timeRangeOption.hours / 24 })
        : t('modelManagement.hours', { count: timeRangeOption.hours })
    const sourceText = source !== 'all'
      ? ` · ${source === 'cli' ? 'CLI' : source === 'gateway' ? t('modelStatsTable.sourceGateway') : t('modelStatsTable.sourceInternal')}`
      : ''
    const routeText = routeMode !== 'all'
      ? ` · ${routeMode === '1' ? t('modelStatsTable.routeDirect') : t('modelStatsTable.routeFailover')}`
      : ''
    return `${t('modelManagement.statsDescription', { time: timeText, totalTokens: totalTokensText })}${sourceText}${routeText}`
  }, [timeRangeOption, source, routeMode, t, totalTokensText])

  // ===== 高度计算（§5.5） =====
  // 直接测量表格宿主区域，避免 header 估算与 magic number 偏差
  const [tableHeight, tableHostRef] = useAvailableHeight()

  return (
    <Card className="h-full min-w-0 flex flex-col">
      <CardHeader className="pb-2 shrink-0">
          <div className="flex items-center justify-between">
            <CardTitle className="text-base">{t('models')}</CardTitle>
            <div className="flex items-center gap-2">
              {/* 时间范围快捷选择 */}
              <Select
                value={timeRangeId}
                onValueChange={setTimeRangeId}
              >
                <SelectTrigger className="h-7 w-24 text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {timeRangeOptions.map((opt) => (
                    <SelectItem key={opt.id} value={opt.id} className="text-xs">
                      {opt.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>

              {/* 入口过滤 */}
              <Select value={source} onValueChange={(v) => setSource(v as CallSource | 'all')}>
                <SelectTrigger className="h-7 w-20 text-xs">
                  <SelectValue placeholder={t('modelStatsTable.source')} />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all" className="text-xs">{t('modelManagement.allSources')}</SelectItem>
                  <SelectItem value="cli" className="text-xs">CLI</SelectItem>
                  <SelectItem value="gateway" className="text-xs">{t('modelStatsTable.sourceGateway')}</SelectItem>
                  <SelectItem value="internal" className="text-xs">{t('modelStatsTable.sourceInternal')}</SelectItem>
                </SelectContent>
              </Select>

              {/* 路由模式过滤 */}
              <Select value={routeMode} onValueChange={setRouteMode}>
                <SelectTrigger className="h-7 w-20 text-xs">
                  <SelectValue placeholder={t('modelStatsTable.route')} />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all" className="text-xs">{t('modelManagement.allRoutes')}</SelectItem>
                  <SelectItem value="1" className="text-xs">{t('modelStatsTable.routeDirect')}</SelectItem>
                  <SelectItem value="2" className="text-xs">{t('modelStatsTable.routeFailover')}</SelectItem>
                </SelectContent>
              </Select>

              {/* API Key 过滤：仅展示前缀，直接截断 */}
              <Select value={apiKeySecretId} onValueChange={setApiKeySecretId}>
                <SelectTrigger className="h-7 w-28 text-xs" title={apiKeySecretId !== 'all' ? apiKeySecretId : t('modelManagement.allApiKeys')}>
                  <SelectValue placeholder={t('modelStatsTable.apiKey')} />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all" className="text-xs">{t('modelManagement.allApiKeys')}</SelectItem>
                  {authKeys.map((key) => (
                    <SelectItem
                      key={key.id}
                      value={key.apiKeySecretId ?? key.id}
                      className="text-xs"
                      title={key.apiKeySecretId ?? ''}
                    >
                      <span className="block truncate max-w-[160px]">
                        {truncateApiKey(key.apiKeySecretId)}
                      </span>
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>

              {/* 视图模式切换：紧凑 / 左右滚动 */}
              <ToggleGroup
                type="single"
                value={viewMode}
                onValueChange={(v) => v && setViewMode(v as 'compact' | 'scroll')}
                variant="outline"
                size="sm"
              >
                <ToggleGroupItem
                  value="compact"
                  className="h-7 px-2 text-xs"
                  title={t('modelStats.viewModeCompact')}
                >
                  <i className="fa-solid fa-table-cells" />
                </ToggleGroupItem>
                <ToggleGroupItem
                  value="scroll"
                  className="h-7 px-2 text-xs"
                  title={t('modelStats.viewModeScroll')}
                >
                  <i className="fa-solid fa-table-columns" />
                </ToggleGroupItem>
              </ToggleGroup>

              {/* 刷新按钮 */}
              <Button
                variant="outline"
                size="sm"
                className="h-7 text-xs"
                onClick={() => { detailRefetch(); aggRefetch() }}
                disabled={detailLoading || aggLoading}
              >
                <i className={cn('fa-solid fa-rotate', (detailLoading || aggLoading) && 'animate-spin', 'mr-1.5')} />
                {t('refresh')}
              </Button>
            </div>
          </div>
          <CardDescription className="text-xs">
            {statsDescription}
          </CardDescription>
        </CardHeader>

        <CardContent className="flex-1 min-h-0 min-w-0 p-0 px-4 pb-4">
          <Tabs value={activeTab} onValueChange={(v) => setActiveTab(v as 'detail' | 'aggregated')} className="flex h-full min-w-0 flex-col">
            <div className="flex items-center justify-between pb-2">
              <TabsList className="h-7">
                <TabsTrigger value="detail" className="text-xs px-3 py-1">
                  {t('tabs.detail')}
                </TabsTrigger>
                <TabsTrigger value="aggregated" className="text-xs px-3 py-1">
                  {t('tabs.aggregated')}
                </TabsTrigger>
              </TabsList>

              {/* 聚合模式的粒度选择 + 刷新间隔 */}
              <div className="flex items-center gap-2">
                <TabsContent value="aggregated" className="mt-0">
                  <div className="flex items-center gap-2">
                    <Select value={granularity} onValueChange={(v) => setGranularity(v as StatsGranularity)}>
                      <SelectTrigger className="h-7 w-20 text-xs">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="hourly" className="text-xs">{t('granularity.hourly')}</SelectItem>
                        <SelectItem value="daily" className="text-xs">{t('granularity.daily')}</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </TabsContent>

                {/* 自动刷新间隔 */}
                <AutoRefreshSelect
                  value={detailInterval as RefreshInterval}
                  onValueChange={(ms) => {
                    setDetailInterval(ms as typeof detailInterval)
                    setAggInterval(ms as typeof detailInterval)
                  }}
                />
              </div>
            </div>

            <TabsContent value="detail" className="flex-1 min-h-0 min-w-0 mt-0">
              <div ref={tableHostRef} className="h-full min-h-0 min-w-0">
                <ModelStatisticsTable
                  rows={detailRows}
                  loading={detailLoading}
                  viewMode={viewMode}
                  style={{ height: tableHeight || undefined }}
                />
              </div>
            </TabsContent>

            <TabsContent value="aggregated" className="flex-1 min-h-0 min-w-0 mt-0">
              <div className="h-full min-h-0 min-w-0">
                <AggregatedStatsTable
                  rows={aggRows}
                  loading={aggLoading}
                  viewMode={viewMode}
                  style={{ height: tableHeight || undefined }}
                />
              </div>
            </TabsContent>
          </Tabs>
        </CardContent>
    </Card>
  )
}
