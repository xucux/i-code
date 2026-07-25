"use client"

import * as React from "react"
import * as ScrollAreaPrimitive from "@radix-ui/react-scroll-area"

import { cn } from "@/lib/utils"

export interface ScrollPageProps
  extends React.ComponentPropsWithoutRef<typeof ScrollAreaPrimitive.Root> {
  /** Viewport 额外类名 */
  viewportClassName?: string
  /** 滚动条额外类名 */
  scrollbarClassName?: string
  /** 滚动方向，默认垂直 */
  orientation?: "vertical" | "horizontal" | "both"
  /** 外观变体：default 带边框圆角；borderless 无边框无边距 */
  variant?: "default" | "borderless"
  /** 滚动条显隐策略：auto 表示滚动时显示，静止 hideDelay 毫秒后隐藏；always 表示始终显示 */
  scrollbarVisible?: "auto" | "always"
  /** auto 模式下滚动条隐藏延迟（毫秒），默认 1000 */
  hideDelay?: number
  /** 滚动条粗细：thin / default / thick，或传入任意 Tailwind class */
  scrollbarThickness?: "thin" | "default" | "thick" | string
  /** 滚动条滑块长度：传入 Tailwind class 控制 min/max height/width */
  scrollbarLength?: string
}

/**
 * 滚动页面组件
 *
 * 为内部页面提供垂直/水平滚动能力，并定制滚动条样式以贴合当前主题。
 * 基于 Radix ScrollArea，支持：
 * - default / borderless 两种外观
 * - 滚动条常显或静止后自动隐藏
 * - 滚动条粗细、长度定制
 */
export const ScrollPage = React.forwardRef<
  React.ElementRef<typeof ScrollAreaPrimitive.Root>,
  ScrollPageProps
>(
  (
    {
      className,
      viewportClassName,
      scrollbarClassName,
      orientation = "vertical",
      variant = "default",
      scrollbarVisible = "always",
      hideDelay = 1000,
      scrollbarThickness = "default",
      scrollbarLength,
      children,
      ...props
    },
    ref
  ) => {
    const showVertical = orientation === "vertical" || orientation === "both"
    const showHorizontal = orientation === "horizontal" || orientation === "both"

    const [scrolling, setScrolling] = React.useState(false)
    const scrollTimerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null)

    const handleScroll = React.useCallback(() => {
      if (scrollbarVisible !== "auto") return
      setScrolling(true)
      if (scrollTimerRef.current) clearTimeout(scrollTimerRef.current)
      scrollTimerRef.current = setTimeout(() => {
        setScrolling(false)
      }, hideDelay)
    }, [scrollbarVisible, hideDelay])

    React.useEffect(() => {
      return () => {
        if (scrollTimerRef.current) clearTimeout(scrollTimerRef.current)
      }
    }, [])

    const thicknessClass = React.useMemo(() => {
      switch (scrollbarThickness) {
        case "thin":
          return "w-1 h-1"
        case "thick":
          return "w-3 h-3"
        case "default":
          return "w-2 h-2"
        default:
          return scrollbarThickness
      }
    }, [scrollbarThickness])

    const verticalThickness = React.useMemo(() => {
      const cls = thicknessClass.split(" ")
      return cls.find((c) => c.startsWith("w-")) ?? "w-2"
    }, [thicknessClass])

    const horizontalThickness = React.useMemo(() => {
      const cls = thicknessClass.split(" ")
      return cls.find((c) => c.startsWith("h-")) ?? "h-2"
    }, [thicknessClass])

    const autoHideClass =
      scrollbarVisible === "auto"
        ? scrolling
          ? "opacity-100"
          : "opacity-0"
        : "opacity-100"

    return (
      <ScrollAreaPrimitive.Root
        ref={ref}
        className={cn(
          "relative overflow-hidden",
          variant === "default" && "rounded-md border",
          className
        )}
        {...props}
      >
        <ScrollAreaPrimitive.Viewport
          className={cn("!h-full !w-full rounded-[inherit]", viewportClassName)}
          onScroll={handleScroll}
        >
          {/* 底部余量：补偿 Radix ScrollArea Viewport 内部 display:table 包裹层
              对底部 padding/margin 的吞没，确保内容末尾完整可见 */}
          <div className="pb-10">
            {children}
          </div>
        </ScrollAreaPrimitive.Viewport>
        {showVertical && (
          <ScrollAreaPrimitive.ScrollAreaScrollbar
            orientation="vertical"
            className={cn(
              "flex touch-none select-none transition-opacity duration-300",
              verticalThickness,
              variant === "default" && "h-full border-l border-l-transparent p-[1px]",
              variant === "borderless" && "h-full",
              autoHideClass,
              scrollbarClassName
            )}
          >
            <ScrollAreaPrimitive.ScrollAreaThumb
              className={cn(
                "relative flex-1 rounded-full bg-muted-foreground/30 transition-colors hover:bg-muted-foreground/50",
                scrollbarLength
              )}
            />
          </ScrollAreaPrimitive.ScrollAreaScrollbar>
        )}
        {showHorizontal && (
          <ScrollAreaPrimitive.ScrollAreaScrollbar
            orientation="horizontal"
            className={cn(
              "flex touch-none select-none flex-col transition-opacity duration-300",
              horizontalThickness,
              variant === "default" && "w-full border-t border-t-transparent p-[1px]",
              variant === "borderless" && "w-full",
              autoHideClass,
              scrollbarClassName
            )}
          >
            <ScrollAreaPrimitive.ScrollAreaThumb
              className={cn(
                "relative flex-1 rounded-full bg-muted-foreground/30 transition-colors hover:bg-muted-foreground/50",
                scrollbarLength
              )}
            />
          </ScrollAreaPrimitive.ScrollAreaScrollbar>
        )}
        <ScrollAreaPrimitive.Corner />
      </ScrollAreaPrimitive.Root>
    )
  }
)
ScrollPage.displayName = "ScrollPage"
