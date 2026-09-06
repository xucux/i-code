/**
 * imagebed 模块状态（图床列表 + 外链回传 → 社区编辑器）
 *
 * 数据流：
 * ```
 * 注入脚本写 document.title → Rust 轮询解析 → emit("imagebed:link-ready")
 *   → 本模块单例 listen → store.push() → 编辑器组件消费（consume）并插入光标处
 * ```
 */

import { create } from 'zustand'
import { listen } from '@tauri-apps/api/event'
import { BACKEND_EVENTS } from '@/core/events'
import { invokeCommand } from '@/hooks/use-command'
import type { ImagebedLinkReady, ImagebedProvider } from './types'

interface ImagebedState {
  /** 内置图床 provider 列表（imagebed_list 懒加载缓存） */
  providers: ImagebedProvider[]
  /** 已加载标记（避免重复请求） */
  providersLoaded: boolean
  /** 未消费的最新外链（图床窗口上传完成 → 后端事件推送） */
  pending: ImagebedLinkReady | null
  /** 自增序号，用于判断新外链到达 */
  seq: number
  /** 惰性加载图床列表（幂等）；失败静默保持空列表 */
  loadProviders: () => Promise<void>
  push: (link: ImagebedLinkReady) => void
  /** 消费当前外链（被某个编辑器插入后标记已处理）；无则返回 null */
  consume: () => ImagebedLinkReady | null
}

let loadPromise: Promise<void> | null = null

export const useImagebedStore = create<ImagebedState>((set, get) => ({
  providers: [],
  providersLoaded: false,
  pending: null,
  seq: 0,
  loadProviders: () => {
    if (get().providersLoaded) return Promise.resolve()
    if (loadPromise) return loadPromise
    loadPromise = invokeCommand<ImagebedProvider[]>('imagebed_list')
      .then((list) => {
        set({ providers: list, providersLoaded: true })
      })
      .catch(() => {
        // 后端模块不可用时静默容错，下拉展示为空
      })
      .finally(() => {
        loadPromise = null
      })
    return loadPromise
  },
  push: (link) => set((s) => ({ pending: link, seq: s.seq + 1 })),
  consume: () => {
    const { pending } = get()
    if (!pending) return null
    set({ pending: null })
    return pending
  },
}))

/**
 * 注册图床外链事件监听（模块级单例，幂等）。
 *
 * 在应用入口或 MarkdownEditor 首帧调用一次即可；同一图床外链只落地一次，
 * 由首个持有编辑器焦点的组件消费，其余组件通过 toast 提示剪贴板粘贴。
 */
let registered = false
export function registerImagebedEvents(): void {
  if (registered) return
  registered = true
  void listen<ImagebedLinkReady>(BACKEND_EVENTS.IMAGEBED_LINK_READY, (event) => {
    useImagebedStore.getState().push(event.payload)
  })
}