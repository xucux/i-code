/**
 * useAutoHideScrollbar — 滚动条自动隐藏 Hook
 *
 * 长时间不滚动时隐藏滚动条 thumb，滚动或悬停容器时立即显示。
 * 用于配合 `.custom-scrollbar-auto-hide` + `.scrollbar-visible` CSS utility。
 *
 * ## 用法
 *
 * ```tsx
 * const [ref, visible] = useAutoHideScrollbar()
 *
 * return (
 *   <div
 *     ref={ref}
 *     className={cn(
 *       'overflow-y-auto custom-scrollbar custom-scrollbar-auto-hide',
 *       visible && 'scrollbar-visible'
 *     )}
 *   >
 *     {content}
 *   </div>
 * )
 * ```
 */

import { useCallback, useEffect, useRef, useState } from 'react'

export interface UseAutoHideScrollbarOptions {
  /** 停止滚动后多久隐藏滚动条（ms），默认 1000 */
  hideDelay?: number
  /** 是否启用自动隐藏，默认 true */
  enabled?: boolean
}

/**
 * @returns [ref, visible]
 * - ref: 绑定到滚动容器的回调 ref
 * - visible: 当前是否应显示滚动条（滚动中 / 鼠标悬停由 CSS :hover 处理）
 */
export function useAutoHideScrollbar(
  options: UseAutoHideScrollbarOptions = {}
): [React.RefCallback<HTMLElement>, boolean] {
  const { hideDelay = 1000, enabled = true } = options
  const [visible, setVisible] = useState(false)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const nodeRef = useRef<HTMLElement | null>(null)

  const clearTimer = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current)
      timerRef.current = null
    }
  }, [])

  const handleScroll = useCallback(() => {
    if (!enabled) return
    clearTimer()
    setVisible(true)
    timerRef.current = setTimeout(() => {
      setVisible(false)
    }, hideDelay)
  }, [enabled, hideDelay, clearTimer])

  const ref = useCallback(
    (node: HTMLElement | null) => {
      nodeRef.current = node
    },
    []
  )

  useEffect(() => {
    const node = nodeRef.current
    if (!node || !enabled) return

    node.addEventListener('scroll', handleScroll, { passive: true })
    return () => {
      node.removeEventListener('scroll', handleScroll)
      clearTimer()
    }
  }, [enabled, handleScroll, clearTimer])

  return [ref, visible]
}
