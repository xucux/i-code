import { useCallback, useEffect, useState } from 'react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invokeCommand } from '@/hooks/use-command'
import { useAutoRefresh } from '@/components/ui/auto-refresh'
import { BACKEND_EVENTS } from '@/core/events'
import type { GatewayRuntimeState, StartGatewayInput, StartGatewayResult } from '@/modules/gateway-runtime/types'

export interface GatewayStatus extends GatewayRuntimeState {}

/**
 * 获取并操作本地网关运行状态
 *
 * 调用后端 `gateway_status` / `gateway_start` / `gateway_stop` 命令。
 * 同时监听后端 `gateway:status-changed` 事件，在网关启停后立即更新状态；
 * 并保留 5 秒一次的轮询作为兜底刷新。
 */
export function useGatewayStatus(): {
  status: GatewayStatus
  loading: boolean
  refresh: () => Promise<void>
  start: (input?: StartGatewayInput) => Promise<StartGatewayResult>
  stop: () => Promise<void>
} {
  const [status, setStatus] = useState<GatewayStatus>({ isRunning: false })
  const [loading, setLoading] = useState(false)

  const refresh = useCallback(async () => {
    try {
      const result = await invokeCommand<GatewayRuntimeState>('gateway_status')
      setStatus(result)
    } catch {
      setStatus({ isRunning: false })
    }
  }, [])

  const start = useCallback(async (input?: StartGatewayInput) => {
    setLoading(true)
    try {
      const result = await invokeCommand<StartGatewayResult>('gateway_start', { input })
      await refresh()
      return result
    } finally {
      setLoading(false)
    }
  }, [refresh])

  const stop = useCallback(async () => {
    setLoading(true)
    try {
      await invokeCommand<void>('gateway_stop')
      await refresh()
    } finally {
      setLoading(false)
    }
  }, [refresh])

  // 首次加载 + 监听后端状态变更事件 + 5 秒兜底轮询
  useEffect(() => {
    let unlisten: UnlistenFn | undefined

    void refresh()
    listen<GatewayRuntimeState>(BACKEND_EVENTS.GATEWAY_STATUS_CHANGED, (event) => {
      setStatus(event.payload)
    }).then((fn) => {
      unlisten = fn
    })

    return () => {
      if (unlisten) unlisten()
    }
  }, [refresh])
  useAutoRefresh({ onRefresh: refresh, intervalMs: 5_000 })

  return { status, loading, refresh, start, stop }
}
