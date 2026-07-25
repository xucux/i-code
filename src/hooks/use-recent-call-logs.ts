import { useCallback, useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import { useAutoRefresh } from '@/components/ui/auto-refresh'
import type { ListModelCallLogsInput, ModelCallLog } from '@/modules/call-records/types'
import type { RefreshInterval } from '@/components/ui/auto-refresh'

const DEFAULT_REFRESH_INTERVAL: RefreshInterval = 5_000 // 默认 5 秒刷新

/**
 * 获取最近一段时间的调用记录
 *
 * 主要用于网关首页「请求流量」实时图等需要按时间窗口聚合的场景。
 * 支持按指定间隔自动刷新。
 */
export function useRecentCallLogs(input: ListModelCallLogsInput = {}) {
  const [logs, setLogs] = useState<ModelCallLog[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [intervalMs, setIntervalMs] = useState<RefreshInterval>(DEFAULT_REFRESH_INTERVAL)

  const fetch = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const data = await invokeCommand<ModelCallLog[]>('call_records_list', { input })
      setLogs(data)
    } catch (err) {
      setError(String(err))
    } finally {
      setLoading(false)
    }
  }, [input.endAt, input.limit, input.modelId, input.offset, input.providerId, input.startAt])

  useEffect(() => {
    void fetch()
  }, [fetch])

  useAutoRefresh({ onRefresh: fetch, intervalMs })

  return { logs, loading, error, refetch: fetch, intervalMs, setIntervalMs }
}
