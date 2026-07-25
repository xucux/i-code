import { useCallback, useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import type {
  AggregatedStatsInput,
  AggregatedStatsRow,
} from '@/modules/call-records/types'

import type { RefreshInterval } from '@/components/ui/auto-refresh'

const DEFAULT_REFRESH_INTERVAL = 5_000 // 聚合数据默认 5 秒刷新

/**
 * 获取聚合统计数据（预聚合表，高性能）
 *
 * 从 `model_call_stats_hourly` / `model_call_stats_daily` 表读取，
 * 适合趋势图和长时间跨度查询。
 * 支持手动刷新、按指定间隔自动刷新。
 */
export function useAggregatedStats(input: AggregatedStatsInput) {
  const [rows, setRows] = useState<AggregatedStatsRow[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [intervalMs, setIntervalMs] = useState<RefreshInterval>(DEFAULT_REFRESH_INTERVAL)

  const fetch = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const data = await invokeCommand<AggregatedStatsRow[]>('call_stats_aggregated', { input })
      setRows(data)
    } catch (err) {
      setError(String(err))
    } finally {
      setLoading(false)
    }
  }, [input.granularity, input.startAt, input.endAt, input.source, input.providerId, input.modelId, input.routeMode, input.apiKeySecretId])

  useEffect(() => {
    void fetch()
    if (!intervalMs) return
    const timer = setInterval(() => void fetch(), intervalMs)
    return () => clearInterval(timer)
  }, [fetch, intervalMs])

  return { rows, loading, error, refetch: fetch, intervalMs, setIntervalMs }
}
