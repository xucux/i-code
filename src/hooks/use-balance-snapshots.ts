import { useCallback, useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invokeCommand } from '@/hooks/use-command'
import { BACKEND_EVENTS } from '@/core/events'
import type { ProviderBalanceSnapshotRow } from '@/modules/balance/types'

/**
 * 获取所有供应商的额度快照
 *
 * 调用后端 `balance_list_snapshots` 命令，并监听 `balance:snapshot-updated`
 * 事件自动刷新。返回 providerId → snapshotRow 的映射，便于供应商列表按 ID 查询。
 */
export function useBalanceSnapshots(): {
  snapshots: Map<string, ProviderBalanceSnapshotRow>
  loading: boolean
  refetch: () => void
} {
  const [snapshots, setSnapshots] = useState<Map<string, ProviderBalanceSnapshotRow>>(new Map())
  const [loading, setLoading] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const rows = await invokeCommand<ProviderBalanceSnapshotRow[]>('balance_list_snapshots')
      const map = new Map<string, ProviderBalanceSnapshotRow>()
      for (const row of rows) {
        map.set(row.providerId, row)
      }
      setSnapshots(map)
    } catch {
      setSnapshots(new Map())
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
    // 监听额度快照更新事件，自动刷新
    const unlisten = listen(BACKEND_EVENTS.BALANCE_SNAPSHOT_UPDATED, () => {
      void load()
    })
    return () => {
      void unlisten.then((fn) => fn())
    }
  }, [load])

  return { snapshots, loading, refetch: load }
}

/**
 * 刷新指定供应商的额度
 *
 * 调用后端 `balance_refresh_provider` 命令，触发额度查询并持久化快照。
 * 成功后由事件监听自动刷新 useBalanceSnapshots 的数据。
 */
export async function refreshProviderBalance(providerId: string): Promise<void> {
  await invokeCommand('balance_refresh_provider', { providerId })
}
