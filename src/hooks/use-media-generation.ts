import { useCallback, useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import type { MediaGeneration } from '@/modules/media-generation/types'

/**
 * 图像生成历史缓存（模块级单例）
 *
 * 生成成功 / 删除后调用 `refresh()` 拉取最新列表；
 * TTL 内的首次挂载直接复用缓存，避免画廊页反复全量拉取。
 */
let historyCache: MediaGeneration[] | null = null
let historyCacheAt = 0
const HISTORY_CACHE_TTL_MS = 5 * 60 * 1000
const historyListeners = new Set<() => void>()

function notifyHistoryListeners() {
  historyListeners.forEach((fn) => fn())
}

/** 拉取生成历史并更新缓存（强制刷新） */
async function fetchHistory(limit?: number): Promise<MediaGeneration[]> {
  const result = await invokeCommand<MediaGeneration[]>('media_history_list', {
    limit: limit ?? 200,
  })
  historyCache = result
  historyCacheAt = Date.now()
  return result
}

/**
 * 获取图像生成历史
 *
 * 数据来自后端 `media_history_list` 命令，带 5 分钟 TTL 缓存；
 * `refresh()` 强制重新拉取（生成成功 / 删除后调用）。
 */
export function useMediaHistory(): {
  history: MediaGeneration[]
  loading: boolean
  refresh: () => Promise<void>
} {
  const [history, setHistory] = useState<MediaGeneration[]>(historyCache ?? [])
  const [loading, setLoading] = useState(false)

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const result = await fetchHistory()
      setHistory(result)
    } catch {
      // 拉取失败时保留缓存内容，由调用方 toast 呈现错误
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    historyListeners.add(onCacheUpdate)
    function onCacheUpdate() {
      setHistory(historyCache ?? [])
    }
    // TTL 内的首次挂载直接复用缓存，过期则刷新
    if (historyCache === null || Date.now() - historyCacheAt > HISTORY_CACHE_TTL_MS) {
      void refresh()
    }
    return () => {
      historyListeners.delete(onCacheUpdate)
    }
  }, [refresh])

  return { history, loading, refresh }
}

/**
 * 通知历史缓存已变化（生成成功 / 删除后由调用方触发）
 *
 * 内部强制刷新一次以更新缓存，同时通知所有挂载中的页面重渲染。
 */
export async function notifyMediaHistoryChanged(): Promise<void> {
  try {
    await fetchHistory()
  } catch {
    // 忽略刷新失败
  }
  notifyHistoryListeners()
}

/**
 * 读取媒体产物为 data URL
 *
 * 调用后端 `media_asset_read`（Base64 字节），拼装为 `data:image/{ext};base64,...`。
 * 组件内使用 useMemo 缓存，避免重复读取。
 */
export async function readMediaAssetDataUrl(relativePath: string): Promise<string> {
  const base64 = await invokeCommand<string>('media_asset_read', {
    relativePath,
  })
  const ext = relativePath.split('.').pop()?.toLowerCase() ?? 'png'
  const mime = ext === 'jpg' ? 'image/jpeg' : `image/${ext}`
  return `data:${mime};base64,${base64}`
}
