import { useTranslation } from '@/modules/i18n/use-translation'
import { getLocale } from '@/modules/i18n/i18n'
import {
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { ScrollableTable } from '@/components/ui/scrollable-table'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import type { CallSource, ModelCallStatsRow } from '@/modules/call-records/types'

interface ModelStatisticsTableProps {
  rows: ModelCallStatsRow[]
  loading?: boolean
  /** 视图模式：compact 自适应换行 / scroll 固定列宽横向滚动 */
  viewMode?: 'compact' | 'scroll'
  /** 容器高度，由 useAvailableHeight 计算 */
  style?: React.CSSProperties
}

/**
 * 请求入口Badge映射
 */
function SourceBadge({ source }: { source: CallSource }) {
  const { t } = useTranslation('aiGateway')
  const map: Record<CallSource, { label: string; variant: 'default' | 'secondary' | 'outline' }> = {
    cli: { label: 'CLI', variant: 'outline' },
    gateway: { label: t('modelStatsTable.sourceGateway'), variant: 'default' },
    internal: { label: t('modelStatsTable.sourceInternal'), variant: 'secondary' },
  }
  const item = map[source] ?? { label: source, variant: 'outline' }
  return <Badge variant={item.variant} className="text-[10px]">{item.label}</Badge>
}

/**
 * 格式化 API Key 展示：仅保留前 16 位，超出部分截断并追加 "..."
 */
function formatApiKeyDisplay(key: string | undefined | null): string {
  if (!key) return '-'
  if (key.length <= 16) return key
  return `${key.slice(0, 16)}...`
}

/**
 * 格式化数字为千分位
 */
function formatNumber(value: number, fractionDigits = 0): string {
  return value.toLocaleString('zh-CN', {
    minimumFractionDigits: fractionDigits,
    maximumFractionDigits: fractionDigits,
  })
}

/**
 * 格式化金额：中文环境使用 ¥，其他环境使用 $
 */
function formatCny(value: number): string {
  const symbol = getLocale() === 'zh-CN' ? '¥' : '$'
  return `${symbol}${value.toLocaleString('zh-CN', { minimumFractionDigits: 4, maximumFractionDigits: 4 })}`
}

/**
 * 根据视图模式合并 className
 *
 * scroll 模式下所有列禁止换行，确保表头与单元格都在同一行展示。
 */
function cellClass(viewMode: 'compact' | 'scroll', base?: string): string {
  return cn(base, viewMode === 'scroll' && 'whitespace-nowrap')
}

/**
 * 模型调用统计表格
 *
 * 展示供应商、模型ID、入口、路由、API Key、请求数、成功率、总Token、缓存/命中率、
 * 花费金额、费用占比、¥/1M Token、平均耗时、平均首字、平均速率。
 * 使用 ScrollableTable 实现可靠的横纵向滚动，支持 compact / scroll 两种视图。
 */
export function ModelStatisticsTable({ rows, loading, viewMode = 'compact', style }: ModelStatisticsTableProps) {
  const { t } = useTranslation('aiGateway')
  const scroll = viewMode === 'scroll'

  return (
    <ScrollableTable
      viewMode={viewMode}
      density="compact"
      loading={loading}
      loadingText={t('loading', { ns: 'common' })}
      style={style}
    >
      <TableHeader className="sticky top-0 z-10 bg-muted">
        <TableRow>
            <TableHead className="w-[80px]">{t('modelStatsTable.provider')}</TableHead>
            <TableHead>{t('modelStatsTable.modelId')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.source')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.route')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.apiKey')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.requests')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.successRate')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.4xx')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.5xx')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.totalTokens')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.cacheHit')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.cost')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.costRatio')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.costPer1m')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.avgDuration')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.avgTtfb')}</TableHead>
            <TableHead className="text-right">{t('modelStatsTable.avgRate')}</TableHead> 
          {/* <TableHead className={cellClass(viewMode, 'w-[120px]')}>供应商</TableHead>
          <TableHead className={cellClass(viewMode, 'min-w-[180px]')}>模型 ID</TableHead>
          <TableHead className={cellClass(viewMode, 'w-[80px]')}>入口</TableHead>
          <TableHead className={cellClass(viewMode, 'w-[60px]')}>路由</TableHead>
          <TableHead className={cellClass(viewMode, 'w-[100px]')}>API Key</TableHead>
          <TableHead className={cellClass(viewMode, 'text-right w-[80px]')}>请求数</TableHead>
          <TableHead className={cellClass(viewMode, 'text-right w-[110px]')}>成功率</TableHead>
          <TableHead className={cellClass(viewMode, 'text-right w-[60px]')}>4xx</TableHead>
          <TableHead className={cellClass(viewMode, 'text-right w-[60px]')}>5xx</TableHead>
          <TableHead className={cellClass(viewMode, 'text-right w-[90px]')}>总 Token</TableHead>
          <TableHead className={cellClass(viewMode, 'text-right w-[90px]')}>缓存 / 命中</TableHead>
          <TableHead className={cellClass(viewMode, 'text-right w-[110px]')}>花费金额</TableHead>
          <TableHead className={cellClass(viewMode, 'text-right w-[80px]')}>费用占比</TableHead>
          <TableHead className={cellClass(viewMode, 'text-right w-[100px]')}>¥/1M Token</TableHead>
          <TableHead className={cellClass(viewMode, 'text-right w-[100px]')}>平均耗时</TableHead>
          <TableHead className={cellClass(viewMode, 'text-right w-[100px]')}>平均首字</TableHead>
          <TableHead className={cellClass(viewMode, 'text-right w-[90px]')}>平均速率</TableHead> */}
        </TableRow>
      </TableHeader>
      <TableBody>
        {rows.length === 0 && !loading && (
          <TableRow>
            <TableCell colSpan={17} className="h-24 text-center text-muted-foreground">
              {t('empty', { ns: 'common' })}
            </TableCell>
          </TableRow>
        )}
        {rows.map((row) => (
          <TableRow key={`${row.providerId}-${row.modelId}-${row.source}-${row.routeMode}-${row.apiKeySecretId}`}>
            <TableCell className={cellClass(viewMode, 'font-medium')}>{row.providerName}</TableCell>
            <TableCell className={cellClass(viewMode, 'font-mono text-[10px]')}>{row.modelId}</TableCell>
            <TableCell>
              <SourceBadge source={row.source} />
            </TableCell>
            <TableCell className={cellClass(viewMode, 'text-[10px] text-muted-foreground')}>
              {row.routeMode === 1 ? t('modelStatsTable.routeDirect') : t('modelStatsTable.routeFailover')}
            </TableCell>
            <TableCell
              className={cn(
                'text-[10px] text-muted-foreground',
                scroll ? 'whitespace-nowrap' : 'truncate max-w-[100px]'
              )}
              title={row.apiKeySecretId || undefined}
            >
              {formatApiKeyDisplay(row.apiKeySecretId)}
            </TableCell>
            <TableCell className={cellClass(viewMode, 'text-right tabular-nums')}>{formatNumber(row.requestCount)}</TableCell>
            <TableCell className={cellClass(viewMode, 'text-right')}>
              <div className="flex items-center justify-end gap-2">
                <span className="tabular-nums">{row.successRate.toFixed(1)}%</span>
                <div className="h-1.5 w-8 overflow-hidden rounded-full bg-muted">
                  <div
                    className={cn(
                      'h-full rounded-full',
                      row.successRate >= 95 ? 'bg-emerald-500' : row.successRate >= 80 ? 'bg-amber-500' : 'bg-destructive'
                    )}
                    style={{ width: `${Math.min(100, Math.max(0, row.successRate))}%` }}
                  />
                </div>
              </div>
            </TableCell>
            <TableCell className={cellClass(viewMode, 'text-right')}>
              <span className={cn('tabular-nums', row.errorCount4xx > 0 && 'text-amber-600')}>{formatNumber(row.errorCount4xx)}</span>
            </TableCell>
            <TableCell className={cellClass(viewMode, 'text-right')}>
              <span className={cn('tabular-nums', row.errorCount5xx > 0 && 'text-destructive')}>{formatNumber(row.errorCount5xx)}</span>
            </TableCell>
            <TableCell className={cellClass(viewMode, 'text-right tabular-nums')}>{formatNumber(row.totalTokens)}</TableCell>
            <TableCell className={cellClass(viewMode, 'text-right')}>
              {scroll ? (
                <span className="tabular-nums">
                  {formatNumber(row.cachedTokens)} / {row.cacheHitRate.toFixed(1)}%
                </span>
              ) : (
                <div className="flex flex-col items-end">
                  <span className="tabular-nums">{formatNumber(row.cachedTokens)}</span>
                  <span className="text-[10px] text-muted-foreground tabular-nums">{row.cacheHitRate.toFixed(1)}%</span>
                </div>
              )}
            </TableCell>
            <TableCell className={cellClass(viewMode, 'text-right tabular-nums')}>{formatCny(row.costCny)}</TableCell>
            <TableCell className={cellClass(viewMode, 'text-right tabular-nums')}>{(row.costRatio * 100).toFixed(1)}%</TableCell>
            <TableCell className={cellClass(viewMode, 'text-right tabular-nums')}>{formatCny(row.costPer1mTokens)}</TableCell>
            <TableCell className={cellClass(viewMode, 'text-right tabular-nums')}>{formatNumber(row.avgDurationMs)} ms</TableCell>
            <TableCell className={cellClass(viewMode, 'text-right tabular-nums')}>{formatNumber(row.avgTimeToFirstTokenMs)} ms</TableCell>
            <TableCell className={cellClass(viewMode, 'text-right tabular-nums')}>{formatNumber(row.avgTokensPerSecond, 1)} t/s</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </ScrollableTable>
  )
}
