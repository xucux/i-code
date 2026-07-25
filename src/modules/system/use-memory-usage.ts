import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

interface UseMemoryUsageOptions {
  /** 是否启用内存监控 */
  enabled?: boolean
  /** 轮询间隔（毫秒），默认 5000 */
  interval?: number
}

/**
 * 应用内存占用 Hook
 * 通过调用 Rust 命令 `get_memory_usage` 获取当前进程物理内存（KB），
 * 同时监听后端广播的 `memory-usage` 事件以保持数值实时更新。
 */
export function useMemoryUsage({ enabled = true, interval = 5000 }: UseMemoryUsageOptions = {}) {
  const [memoryKb, setMemoryKb] = useState<number | null>(null)
  const [error, setError] = useState<Error | null>(null)

  // 调用 Rust 命令获取当前内存占用
  const fetchMemory = useCallback(async () => {
    try {
      const usage = await invoke<number>('get_memory_usage')
      setMemoryKb(usage)
      setError(null)
    } catch (e) {
      setError(e instanceof Error ? e : new Error(String(e)))
    }
  }, [])

  useEffect(() => {
    if (!enabled) {
      setMemoryKb(null)
      return
    }

    let unlisten: UnlistenFn | undefined
    let timer: ReturnType<typeof setInterval> | undefined

    // 立即获取一次，并建立事件监听与轮询
    fetchMemory()
    listen<number>('memory-usage', (event) => {
      setMemoryKb(event.payload)
      setError(null)
    }).then((fn) => {
      unlisten = fn
    })
    timer = setInterval(fetchMemory, interval)

    return () => {
      if (unlisten) unlisten()
      if (timer) clearInterval(timer)
    }
  }, [enabled, interval, fetchMemory])

  return { memoryKb, error }
}


