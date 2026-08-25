/**
 * 弹窗系统全屏 Hook（社区编辑/发帖弹窗共用）
 *
 * 参考脚本编辑器 / 模型统计：优先 Fullscreen API（`document.documentElement.requestFullscreen()`）
 * 放大整个弹窗到系统全屏，监听 `fullscreenchange`（支持 ESC 退出）；API 不可用时回退 CSS 铺满。
 */

import { useCallback, useEffect, useState } from 'react'

export interface DialogFullscreen {
  /** 是否处于系统全屏（Fullscreen API 或 CSS 回退） */
  expanded: boolean
  /** 系统全屏切换入口 */
  toggleFullscreen: () => void
}

/** 弹窗系统全屏状态：`open` 为弹窗是否打开（关闭时自动退出全屏） */
export function useDialogFullscreen(open: boolean): DialogFullscreen {
  const [isFullscreen, setIsFullscreen] = useState(false)
  const [cssFullscreen, setCssFullscreen] = useState(false)

  // 监听系统 fullscreenchange：退出全屏（含 ESC）时同步状态并清 CSS 回退
  useEffect(() => {
    const onFullscreenChange = () => {
      const active = Boolean(document.fullscreenElement)
      setIsFullscreen(active)
      if (!active) setCssFullscreen(false)
    }
    document.addEventListener('fullscreenchange', onFullscreenChange)
    return () => document.removeEventListener('fullscreenchange', onFullscreenChange)
  }, [])

  // 关闭弹窗时若仍处于系统全屏，先退出，避免残留
  useEffect(() => {
    if (open) return
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => undefined)
    setIsFullscreen(false)
    setCssFullscreen(false)
  }, [open])

  const expanded = isFullscreen || cssFullscreen

  /** 系统全屏切换：放大整个弹窗至系统全屏；失败回退 CSS 铺满 */
  const toggleFullscreen = useCallback(async () => {
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
      // Fullscreen API 不可用（如非用户手势触发），回退到 CSS 铺满
      setCssFullscreen((v) => !v)
    }
  }, [cssFullscreen])

  return { expanded, toggleFullscreen }
}