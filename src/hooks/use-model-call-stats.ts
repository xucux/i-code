import { useCallback, useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import type { ModelCallStatsInput, ModelCallStatsRow } from '@/modules/call-records/types'
import type { RefreshInterval } from '@/components/ui/auto-refresh'

const DEFAULT_REFRESH_INTERVAL: RefreshInterval = 5_000 // 默认 5 秒刷新

/**
 * 获取模型调用统计数据（明细表实时 GROUP BY）
 *
 * 支持手动刷新、按指定间隔自动刷新、过滤条件。
 */
export function useModelCallStats(input: ModelCallStatsInput = {}) {
  const [rows, setRows] = useState<ModelCallStatsRow[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [intervalMs, setIntervalMs] = useState<RefreshInterval>(DEFAULT_REFRESH_INTERVAL)

  const fetch = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const data = await invokeCommand<ModelCallStatsRow[]>('gateway_model_call_stats', { input })
      setRows(data)
    } catch (err) {
      setError(String(err))
    } finally {
      setLoading(false)
    }
  }, [input.endAt, input.providerId, input.modelId, input.source, input.startAt, input.routeMode, input.apiKeySecretId])

  useEffect(() => {
    void fetch()
    if (!intervalMs) return
    const timer = setInterval(() => void fetch(), intervalMs)
    return () => clearInterval(timer)
  }, [fetch, intervalMs])

  return { rows, loading, error, refetch: fetch, intervalMs, setIntervalMs }
}
