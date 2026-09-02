/**
 * 图片放大查看遮罩（通用组件）
 *
 * 半透明黑底 + 等比居中原图 + 自由缩放：
 * - 缩放：底部半透明按钮组（缩小 / 当前倍数 / 重置 / 放大），0.25 步进，范围 [0.5, 4]；
 *   transform scale 实现，150ms 过渡；切图（src 变化）时自动重置为 1
 * - 关闭：点击遮罩 / 右上角关闭按钮 / ESC
 * - 通过 createPortal 渲染到 document.body，不受宿主页面 overflow / 层叠上下文影响
 * - 多图场景：传入 onPrev / onNext 即显示左右切换按钮（边界自动隐藏）
 * - onImageContextMenu：转发图片右键事件，供宿主页面展示自定义右键菜单
 */
import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'

export interface ImageLightboxProps {
  /** 图片地址（data URL 或普通 URL） */
  src: string
  alt?: string
  /** 关闭回调（遮罩点击 / 关闭按钮 / ESC） */
  onClose: () => void
  /** 图片下方的说明文字区域（如提示词全文） */
  caption?: React.ReactNode
  /** 底部附加操作区（如「复用提示词」按钮），与缩放按钮组并列 */
  actions?: React.ReactNode
  /** 上一张回调（提供即显示切换按钮；首张时不传即隐藏） */
  onPrev?: () => void
  /** 下一张回调（提供即显示切换按钮；末张时不传即隐藏） */
  onNext?: () => void
  /** 图片右键事件转发（宿主页面自定义右键菜单） */
  onImageContextMenu?: (e: React.MouseEvent) => void
}

const MIN_SCALE = 0.5
const MAX_SCALE = 4
const SCALE_STEP = 0.25

export function ImageLightbox({
  src,
  alt = '',
  onClose,
  caption,
  actions,
  onPrev,
  onNext,
  onImageContextMenu,
}: ImageLightboxProps) {
  // 缩放倍数：初始 1；0.25 步进，范围 [0.5, 4]
  const [scale, setScale] = useState(1)

  // 切图时重置缩放为原始大小
  useEffect(() => {
    setScale(1)
  }, [src])

  const handleZoom = (delta: number) => {
    setScale((s) => Math.min(MAX_SCALE, Math.max(MIN_SCALE, Math.round((s + delta) * 100) / 100)))
  }

  // ESC 关闭
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  return createPortal(
    // data-suppress-global-contextmenu：灯箱渲染在 body（createPortal），
    // 逃出了宿主页面的接管作用域，需自行声明——图片右键由宿主通过
    // onImageContextMenu 展示专属菜单，不与全局自定义右键（AppGlobalMenu）叠加
    <div
      data-suppress-global-contextmenu
      className="fixed inset-0 z-[100] flex cursor-zoom-out items-center justify-center overflow-hidden bg-black/70 p-6"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
    >
      {/* 右上角关闭按钮；z 高于图片，放大后仍可点 */}
      <button
        type="button"
        className="absolute right-3 top-3 z-10 flex size-8 items-center justify-center rounded-md text-white/80 transition-colors hover:bg-white/10 hover:text-white"
        aria-label="close"
        onClick={(e) => {
          e.stopPropagation()
          onClose()
        }}
      >
        <i className="fa-solid fa-xmark size-4" />
      </button>

      {/* 左右切换（多图时显示）；z 高于图片，放大后仍可点 */}
      {onPrev && (
        <button
          type="button"
          className="absolute left-3 top-1/2 z-10 flex size-9 -translate-y-1/2 items-center justify-center rounded-md text-white/80 transition-colors hover:bg-white/10 hover:text-white"
          aria-label="previous"
          onClick={(e) => {
            e.stopPropagation()
            onPrev()
          }}
        >
          <i className="fa-solid fa-chevron-left size-4" />
        </button>
      )}
      {onNext && (
        <button
          type="button"
          className="absolute right-3 top-1/2 z-10 flex size-9 -translate-y-1/2 items-center justify-center rounded-md text-white/80 transition-colors hover:bg-white/10 hover:text-white"
          aria-label="next"
          onClick={(e) => {
            e.stopPropagation()
            onNext()
          }}
        >
          <i className="fa-solid fa-chevron-right size-4" />
        </button>
      )}

      {/* 底部控制条：缩放按钮组 + 附加操作；半透明，z 高于图片 */}
      <div
        className="absolute bottom-3 left-1/2 z-10 flex -translate-x-1/2 items-center gap-1 rounded-md bg-white/10 p-1 backdrop-blur-sm"
        onClick={(e) => e.stopPropagation()}
      >
        <button
          type="button"
          className="flex size-7 items-center justify-center rounded text-white/80 transition-colors hover:bg-white/10 hover:text-white disabled:opacity-30"
          aria-label="zoom out"
          disabled={scale <= MIN_SCALE}
          onClick={() => handleZoom(-SCALE_STEP)}
        >
          <i className="fa-solid fa-magnifying-glass-minus size-3.5" />
        </button>
        <span className="w-11 text-center text-xs text-white/80 tabular-nums">
          {Math.round(scale * 100)}%
        </span>
        <button
          type="button"
          className="flex size-7 items-center justify-center rounded text-white/80 transition-colors hover:bg-white/10 hover:text-white"
          aria-label="reset zoom"
          onClick={() => setScale(1)}
        >
          <i className="fa-solid fa-arrows-to-circle size-3" />
        </button>
        <button
          type="button"
          className="flex size-7 items-center justify-center rounded text-white/80 transition-colors hover:bg-white/10 hover:text-white disabled:opacity-30"
          aria-label="zoom in"
          disabled={scale >= MAX_SCALE}
          onClick={() => handleZoom(SCALE_STEP)}
        >
          <i className="fa-solid fa-magnifying-glass-plus size-3.5" />
        </button>
        {actions && <div className="ml-1 border-l border-white/20 pl-1">{actions}</div>}
      </div>

      {/* 内容区：原图 + 说明文字，纵向排列；点击不冒泡避免关闭遮罩 */}
      <div
        className="flex max-h-full max-w-full flex-col items-center gap-3"
        onClick={(e) => e.stopPropagation()}
      >
        <img
          src={src}
          alt={alt}
          className="max-h-[75vh] max-w-full cursor-default rounded-md object-contain shadow-2xl transition-transform duration-150"
          style={{ transform: `scale(${scale})` }}
          onContextMenu={onImageContextMenu}
        />
        {caption && (
          <div className="max-h-20 w-full overflow-y-auto text-center text-xs text-white/80">
            {caption}
          </div>
        )}
      </div>
    </div>,
    document.body,
  )
}
