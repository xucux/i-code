import { useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import { format } from 'date-fns'
import type { DateRange } from 'react-day-picker'
import { invokeCommand } from '@/hooks/use-command'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { AutoRefreshSelect } from '@/components/ui/auto-refresh'
import { DateTimeRangePicker } from '@/components/ui/date-time-range-picker'
import { cn } from '@/lib/utils'
import { useModelCallStats } from '@/hooks/use-model-call-stats'
import { useAggregatedStats } from '@/hooks/use-aggregated-stats'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { listGatewayAuthKeys } from '@/hooks/use-ai-gateway-mutation'
import { ModelStatisticsTable } from './model-statistics-table'
import { AggregatedStatsTable } from './aggregated-stats-table'
import type { CallSource, AggregatedStatsRow, ModelCallStatsRow, StatsGranularity } from '@/modules/call-records/types'
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

/**
 * CSV 单元格转义：包含逗号、双引号或换行时用双引号包裹，内部双引号翻倍转义
 */
function csvEscape(value: string | number | undefined | null): string {
  if (value == null) return ''
  const s = String(value)
  if (s.includes(',') || s.includes('"') || s.includes('\n') || s.includes('\r')) {
    return `"${s.replace(/"/g, '""')}"`
  }
  return s
}

/**
 * 导出当前统计并为 CSV 并落盘到应用导出目录，随后用系统文件管理器打开所在目录
 *
 * Tauri WebView 下 blob URL + a.click() 不可靠（WebView2 默认不触发下载），
 * 改由后端 `call_records_export_stats_csv` 写入 `app_cache_dir/exports` 并返回路径，
 * 前端再调用 `settings_open_directory` 打开目录，与日志导出（log_export）保持一致。
 *
 * @param headers 表头数组
 * @param rows 每行原始字段值数组
 */
async function exportAndOpen(
  headers: string[],
  rows: (string | number | undefined | null)[][],
  t: (k: string) => string
): Promise<void> {
  const content = `\uFEFF${[headers.map(csvEscape).join(','), ...rows.map((r) => r.map(csvEscape).join(','))].join('\n')}`
  const filePath = await invokeCommand<string>('call_records_export_stats_csv', { content })
  // 打开导出目录，方便用户定位刚生成的文件（失败仅提示，不影响已落盘）
  try {
    await invokeCommand<void>('settings_open_directory', { path: filePath.replace(/[^/\\]+$/, '') })
  } catch {
    toast.info(t('modelStats.exportSaved'))
  }
}

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

  // ===== 全屏缩放：放大至浏览器全屏 / 还原 =====
  const [isFullscreen, setIsFullscreen] = useState(false)
  const cardRef = useRef<HTMLDivElement>(null)
  const [cssFullscreen, setCssFullscreen] = useState(false)
  // 全屏展开：Fullscreen API（isFullscreen）或 CSS 铺满回退（cssFullscreen）
  const expanded = isFullscreen || cssFullscreen

  useEffect(() => {
    const onFullscreenChange = () => {
      const active = Boolean(document.fullscreenElement)
      setIsFullscreen(active)
      if (!active) setCssFullscreen(false)
    }
    document.addEventListener('fullscreenchange', onFullscreenChange)
    return () => {
      document.removeEventListener('fullscreenchange', onFullscreenChange)
    }
  }, [])

  /** 放大至全屏 / 还原
   *
   * 优先使用浏览器 Fullscreen API，失败时（非用户手势等）回退为 CSS 铺满视口。
   */
  const toggleFullscreen = async () => {
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen()
        return
      }
      if (cssFullscreen) {
        setCssFullscreen(false)
        return
      }
      await document.documentElement.requestFullscreen()
      setCssFullscreen(true)
    } catch {
      setCssFullscreen((v) => !v)
    }
  }

  // ===== 当前激活的 Tab：用于计算对应数据的总 token =====
  const [activeTab, setActiveTab] = useState<'detail' | 'aggregated'>('detail')

  // ===== 全屏时的精确日期时间范围（非全屏走快捷下拉 timeRangeId） =====
  const [customRange, setCustomRange] = useState<DateRange | null>(null)

  const timeRangeOptions = useTimeRangeOptions()

  // 快捷下拉的起点：直接依赖基本类型 timeRangeId（字符串/数字），避免依赖 timeRangeOptions
  // 数组引用（每次 render 重建）导致 startAt 连锁重建，进而破坏 AutoRefresh 的 setInterval 节奏。
  // today 跨日：startAt 缓存为切换当天的 0 点，跨日后不自动推进；切换时间范围或刷新页面会修正。
  const quickStartAt = useMemo(() => {
    const hours = timeRangeId === 'today' ? undefined : Number(timeRangeId)
    return getTimeRangeStart(hours)
  }, [timeRangeId])

  // 生效的时间起止：全屏用精确日期时间范围，否则用快捷下拉起点（无结束时间）
  const { startAt, endAt } = useMemo(() => {
    if (expanded && customRange?.from) {
      return {
        startAt: customRange.from.toISOString(),
        endAt: customRange.to ? customRange.to.toISOString() : undefined,
      }
    }
    return { startAt: quickStartAt, endAt: undefined }
  }, [expanded, customRange, quickStartAt])

  // 进入全屏时若尚未选择精确范围，按当前快捷范围初始化起点（to=当前时间），便于直接微调
  useEffect(() => {
    if (expanded && !customRange) {
      setCustomRange({ from: new Date(quickStartAt), to: new Date() })
    }
    // 仅在全屏切换时初始化一次；customRange / quickStartAt 变化不触发
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [expanded])

  // 仅供 statsDescription 文案使用，引用稳定性不影响 fetch / timer
  const timeRangeOption = useMemo(
    () => timeRangeOptions.find((o) => o.id === timeRangeId) ?? timeRangeOptions[0],
    [timeRangeOptions, timeRangeId]
  )

  // ===== 明细模式 =====
  const detailInput = useMemo(() => ({
    startAt,
    endAt,
    source: source !== 'all' ? source as CallSource : undefined,
    routeMode: routeMode !== 'all' ? Number(routeMode) : undefined,
    apiKeySecretId: apiKeySecretId !== 'all' ? apiKeySecretId : undefined,
  }), [startAt, endAt, source, routeMode, apiKeySecretId])

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
    endAt,
    source: source !== 'all' ? source as CallSource : undefined,
    routeMode: routeMode !== 'all' ? Number(routeMode) : undefined,
    apiKeySecretId: apiKeySecretId !== 'all' ? apiKeySecretId : undefined,
  }), [granularity, startAt, endAt, source, routeMode, apiKeySecretId])

  const {
    rows: aggRows,
    loading: aggLoading,
    refetch: aggRefetch,
    intervalMs: _aggInterval,
    setIntervalMs: setAggInterval,
  } = useAggregatedStats(aggInput)

  // 导出当前 Tab 的统计行为 CSV，落盘后打开所在目录
  const handleExport = async () => {
    const isDetail = activeTab === 'detail'
    const rows = isDetail ? detailRows : aggRows
    const wireSource = (s: CallSource) => (s === 'cli' ? 'CLI' : s === 'gateway' ? t('modelStatsTable.sourceGateway') : t('modelStatsTable.sourceInternal'))
    const wireRoute = (r: number) => (r === 1 ? t('modelStatsTable.routeDirect') : t('modelStatsTable.routeFailover'))
    const num = (v: number) => (Number.isFinite(v) ? String(v) : '')
    // 金额/速率保留原始数值，便于 Excel 二次计算
    const money = (v: number) => (Number.isFinite(v) ? v.toFixed(4) : '')

    if (isDetail) {
      // 明细：供应商/模型/入口/路由/APIKey/请求数/成功率/4xx/5xx/总Token/缓存命中/花费/占比/¥1M/耗时/首字/速率
      await exportAndOpen(
        [
          t('modelStatsTable.provider'), t('modelStatsTable.modelId'), t('modelStatsTable.source'),
          t('modelStatsTable.route'), t('modelStatsTable.apiKey'), t('modelStatsTable.requests'),
          t('modelStatsTable.successRate'), t('modelStatsTable.4xx'), t('modelStatsTable.5xx'),
          t('modelStatsTable.totalTokens'), t('modelStatsTable.cacheHit'), t('modelStatsTable.cost'),
          t('modelStatsTable.costRatio'), t('modelStatsTable.costPer1m'),
          t('modelStatsTable.avgDuration'), t('modelStatsTable.avgTtfb'), t('modelStatsTable.avgRate'),
        ],
        (rows as ModelCallStatsRow[]).map((r) => [
          r.providerName, r.modelId, wireSource(r.source), wireRoute(r.routeMode), r.apiKeySecretId,
          num(r.requestCount), `${r.successRate.toFixed(1)}%`, num(r.errorCount4xx), num(r.errorCount5xx),
          num(r.totalTokens), `${num(r.cachedTokens)} / ${r.cacheHitRate.toFixed(1)}%`,
          money(r.costCny), `${(r.costRatio * 100).toFixed(1)}%`, money(r.costPer1mTokens),
          `${num(r.avgDurationMs)} ms`, `${num(r.avgTimeToFirstTokenMs)} ms`, `${num(r.avgTokensPerSecond)} t/s`,
        ]),
        t,
      )
    } else {
      // 聚合：时间桶 + 明细列（无占比/¥1M）
      await exportAndOpen(
        [
          t('modelStatsTable.timeBucket'), t('modelStatsTable.provider'), t('modelStatsTable.modelId'),
          t('modelStatsTable.source'), t('modelStatsTable.route'), t('modelStatsTable.apiKey'),
          t('modelStatsTable.requests'), t('modelStatsTable.successRate'), t('modelStatsTable.4xx'),
          t('modelStatsTable.5xx'), t('modelStatsTable.totalTokens'), t('modelStatsTable.cacheHit'),
          t('modelStatsTable.cost'), t('modelStatsTable.avgDuration'), t('modelStatsTable.avgTtfb'),
          t('modelStatsTable.avgRate'),
        ],
        (rows as AggregatedStatsRow[]).map((r) => [
          r.timeBucket, r.providerName || r.providerId, r.modelId, wireSource(r.source), wireRoute(r.routeMode),
          r.apiKeySecretId, num(r.requestCount), `${r.successRate.toFixed(1)}%`,
          num(r.errorCount4xx), num(r.errorCount5xx), num(r.totalTokens),
          `${num(r.cachedTokens)} / ${r.cacheHitRate.toFixed(1)}%`, money(r.costCny),
          `${num(r.avgDurationMs)} ms`, `${num(r.avgTimeToFirstTokenMs)} ms`, `${num(r.avgTokensPerSecond)} t/s`,
        ]),
        t,
      )
    }
  }

  // 当前 Tab 对应数据的总 token（K/M/B 紧凑格式）
  const totalTokensText = useMemo(() => {
    const rows = activeTab === 'detail' ? detailRows : aggRows
    const sum = rows.reduce((acc, r) => acc + (r.totalTokens ?? 0), 0)
    return formatTokenKMB(sum)
  }, [activeTab, detailRows, aggRows])

  // 统计描述文本
  const statsDescription = useMemo(() => {
    // 全屏精确范围：展示起止时间
    const timeText = expanded && customRange?.from
      ? `${format(customRange.from, 'yyyy-MM-dd HH:mm')} ~ ${customRange.to ? format(customRange.to, 'HH:mm') : '...'}`
      : timeRangeOption?.hours == null
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
  }, [expanded, customRange, timeRangeOption, source, routeMode, t, totalTokensText])

  // ===== 高度计算（§5.5） =====
  // 直接测量表格宿主区域，避免 header 估算与 magic number 偏差
  const [tableHeight, tableHostRef] = useAvailableHeight()

  return (
    <Card
      ref={cardRef}
      className={cn(
        'min-w-0 flex flex-col',
        expanded
          ? '!fixed !inset-0 !z-50 !h-screen !w-screen !max-w-none !rounded-none border-0 bg-background'
          : 'h-full'
      )}
    >
      <CardHeader className="pb-2 shrink-0">
          <div className="flex items-center justify-between">
            <CardTitle className="text-base">{t('models')}</CardTitle>
            <div className="flex items-center gap-2">
              {/* 时间范围：全屏用精确日期时间选择器，非全屏用快捷下拉 */}
              {expanded ? (
                <DateTimeRangePicker
                  value={customRange ?? undefined}
                  onChange={(range) => setCustomRange(range ?? null)}
                  className="h-7 min-w-[200px]"
                />
              ) : (
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
              )}

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

              {/* 刷新按钮（仅图标） */}
              <Button
                variant="outline"
                size="sm"
                className="h-7 w-7 px-0 text-xs"
                onClick={() => { detailRefetch(); aggRefetch() }}
                disabled={detailLoading || aggLoading}
                title={t('refresh')}
                aria-label={t('refresh')}
              >
                <i className={cn('fa-solid fa-rotate', (detailLoading || aggLoading) && 'animate-spin')} />
              </Button>

              {/* 导出按钮：导出当前 Tab 统计为 CSV */}
              <Button
                variant="outline"
                size="sm"
                className="h-7 w-7 px-0 text-xs"
                onClick={() => void handleExport()}
                disabled={detailLoading || aggLoading}
                title={t('modelStats.export')}
                aria-label={t('modelStats.export')}
              >
                <i className="fa-solid fa-file-csv" />
              </Button>

              {/* 全屏缩放按钮（仅图标） */}
              <Button
                variant="outline"
                size="sm"
                className="h-7 w-7 px-0 text-xs"
                onClick={() => void toggleFullscreen()}
                title={expanded ? t('modelStats.restore') : t('modelStats.expand')}
                aria-label={expanded ? t('modelStats.restore') : t('modelStats.expand')}
              >
                <i className={cn('fa-solid', expanded ? 'fa-compress' : 'fa-expand')} />
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
