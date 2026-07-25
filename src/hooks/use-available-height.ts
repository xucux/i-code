/**
 * useAvailableHeight — 测量容器可视高度
 *
 * 实时监听容器尺寸变化，减去偏移量后返回可用像素高度。
 * 解决 flex 布局下高度链断裂导致 ScrollArea/ScrollPage 无法确定高度的问题。
 *
 * ## 用法
 *
 * ```tsx
 * const [height, ref] = useAvailableHeight({ offset: 80 })
 *
 * return (
 *   <div ref={ref} className="flex-1">
 *     <ScrollPage style={{ height: height || undefined }}>
 *       {content}
 *     </ScrollPage>
 *   </div>
 * )
 * ```
 *
 * 当容器从 `display: none` 变为可见时，ResizeObserver 会自动触发测量，
 * 因此适用于 Tab 切换场景。
 */

import { useCallback, useEffect, useRef, useState } from 'react'

export interface UseAvailableHeightOptions {
  /** 从测量高度中减去的偏移量（px），用于 header/padding 等固定空间，默认 0 */
  offset?: number
}

/**
 * @returns [height, ref]
 * - height: 可用像素高度（容器不可见时返回 0）
 * - ref: 绑定到待测量容器的回调 ref
 */
export function useAvailableHeight(
  options: UseAvailableHeightOptions = {}
): [number, React.RefCallback<HTMLElement>] {
  const { offset = 0 } = options
  const [height, setHeight] = useState(0)
  const nodeRef = useRef<HTMLElement | null>(null)

  const measure = useCallback(() => {
    const node = nodeRef.current
    if (!node) {
      setHeight(0)
      return
    }
    const rect = node.getBoundingClientRect()
    setHeight(Math.max(0, rect.height - offset))
  }, [offset])

  // 挂载 ResizeObserver — 当容器尺寸变化（含从 display:none 变为可见）时自动测量
  useEffect(() => {
    const node = nodeRef.current
    if (!node) return

    measure()
    const ro = new ResizeObserver(() => measure())
    ro.observe(node)
    return () => ro.disconnect()
  }, [measure])

  // 回调 ref — 当 React 挂载或更换 DOM 节点时触发
  const ref = useCallback(
    (node: HTMLElement | null) => {
      nodeRef.current = node
      if (node) {
        // 用 rAF 确保布局完成后再测量
        requestAnimationFrame(() => measure())
      }
    },
    [measure]
  )

  return [height, ref]
}
