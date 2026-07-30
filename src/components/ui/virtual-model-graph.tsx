"use client"

import * as React from "react"

import { cn } from "@/lib/utils"

/**
 * 模型特性标记
 */
export interface VirtualModelFeatures {
  /** 是否支持工具调用 */
  toolCalling?: boolean
  /** 是否支持图像输入 */
  imageInput?: boolean
  /** 是否支持函数调用（旧版术语） */
  functionCalling?: boolean
  /** 是否支持流式输出 */
  streaming?: boolean
}

/**
 * 虚拟模型路由图中的目标模型节点
 */
export interface VirtualModelTargetNode {
  /** 节点唯一标识 */
  id: string
  /** 目标供应商名称 */
  provider: string
  /** 目标模型 ID */
  model: string
  /** 优先级，数值越小越优先 */
  priority: number
  /** 健康状态，undefined 表示未启用健康检查 */
  healthy?: boolean
  /** 是否启用该路由 */
  enabled?: boolean
  /** 可选额度百分比（0-100），用于展示额度进度 */
  quotaPercent?: number
  /** 父级节点 ID，未设置时表示该节点为父级 */
  parentId?: string
  /** 上下文配置 JSON 字符串或对象，如 temperature、max_tokens 等 */
  contextConfig?: string | Record<string, unknown>
  /** 模型特性 */
  features?: VirtualModelFeatures
}

export interface VirtualModelGraphProps {
  /** 中心虚拟模型展示名称 */
  virtualModel: string
  /** 目标模型节点列表 */
  targets: VirtualModelTargetNode[]
  /** 当前选中的目标节点 ID */
  selectedId?: string
  /** 节点点击回调（兼容旧用法） */
  onSelect?: (id: string) => void
  /** 父级节点点击回调 */
  onSelectParent?: (id: string) => void
  /** 子级节点点击回调 */
  onSelectChild?: (id: string) => void
  /** 是否处于编辑模式：显示父级操作按钮 */
  editMode?: boolean
  /** 自定义父级节点操作区（仅在编辑模式下渲染） */
  renderParentActions?: (target: VirtualModelTargetNode) => React.ReactNode
  /** 自定义类名 */
  className?: string
}

/**
 * 根据健康状态返回节点边框颜色
 */
function healthBorderClass(healthy?: boolean, enabled = true): string {
  if (!enabled) return "border-muted-foreground/30"
  if (healthy === undefined) return "border-primary/60"
  return healthy ? "border-emerald-500/60" : "border-destructive/60"
}

/**
 * 根据健康状态返回状态指示灯颜色
 */
function statusDotClass(healthy?: boolean, enabled = true): string {
  if (!enabled) return "bg-muted-foreground/40"
  if (healthy === undefined) return "bg-primary"
  return healthy ? "bg-emerald-500" : "bg-destructive"
}

/**
 * 将上下文配置格式化为紧凑键值对数组
 */
function parseContextConfig(config?: string | Record<string, unknown>): [string, string][] {
  if (!config) return []
  try {
    const obj = typeof config === "string" ? (JSON.parse(config) as Record<string, unknown>) : config
    return Object.entries(obj).map(([key, value]) => [key, String(value)])
  } catch {
    return []
  }
}

/**
 * 虚拟模型关系图组件（左侧父级 / 右侧子级）
 *
 * 左侧展示父级虚拟模型节点，右侧展示关联的真实供应商模型子节点，
 * 使用 SVG 三次贝塞尔曲线绘制父子连线，颜色取自当前主题 CSS 变量。
 * 卡片采用水平紧凑布局，充分利用两侧宽度。
 */
export function VirtualModelGraph({
  virtualModel,
  targets,
  selectedId,
  onSelect,
  onSelectParent,
  onSelectChild,
  editMode = false,
  renderParentActions,
  className,
}: VirtualModelGraphProps) {
  const containerRef = React.useRef<HTMLDivElement>(null)
  const parentRefs = React.useRef<Record<string, HTMLElement | null>>({})
  const childRefs = React.useRef<Record<string, HTMLElement | null>>({})
  const [paths, setPaths] = React.useState<{ id: string; d: string }[]>([])
  const [dimensions, setDimensions] = React.useState({ width: 0, height: 0 })

  // 按 parentId 拆分父级与子级
  const parentTargets = React.useMemo(
    () => targets.filter((t) => !t.parentId),
    [targets]
  )
  const childTargets = React.useMemo(
    () => targets.filter((t) => t.parentId),
    [targets]
  )

  // 测量容器与节点位置，计算父子连线路径
  const recalculatePaths = React.useCallback(() => {
    const container = containerRef.current
    if (!container || parentTargets.length === 0 || childTargets.length === 0) {
      setPaths([])
      return
    }

    const containerRect = container.getBoundingClientRect()
    const nextPaths: { id: string; d: string }[] = []

    childTargets.forEach((child) => {
      const parentEl = parentRefs.current[child.parentId!]
      const childEl = childRefs.current[child.id]
      if (!parentEl || !childEl) return

      const parentRect = parentEl.getBoundingClientRect()
      const childRect = childEl.getBoundingClientRect()

      const sourceX = parentRect.right - containerRect.left
      const sourceY = parentRect.top - containerRect.top + parentRect.height / 2
      const targetX = childRect.left - containerRect.left
      const targetY = childRect.top - containerRect.top + childRect.height / 2

      // 三次贝塞尔曲线
      const midX = (sourceX + targetX) / 2
      const d = `M ${sourceX} ${sourceY} C ${midX} ${sourceY}, ${midX} ${targetY}, ${targetX} ${targetY}`

      nextPaths.push({ id: child.id, d })
    })

    setPaths(nextPaths)
    setDimensions({ width: containerRect.width, height: containerRect.height })
  }, [parentTargets, childTargets])

  React.useEffect(() => {
    recalculatePaths()
    window.addEventListener("resize", recalculatePaths)
    return () => window.removeEventListener("resize", recalculatePaths)
  }, [recalculatePaths])

  React.useEffect(() => {
    const timer = setTimeout(recalculatePaths, 100)
    return () => clearTimeout(timer)
  }, [recalculatePaths, virtualModel, targets.length])

  // 编辑模式切换会改变父级节点尺寸，需重算连线
  React.useEffect(() => {
    const timer = setTimeout(recalculatePaths, 50)
    return () => clearTimeout(timer)
  }, [recalculatePaths, editMode])

  const enabledCount = targets.filter((t) => t.enabled !== false).length

  // 动态分配左右比例：父级少则收窄左侧
  const parentFlex = parentTargets.length <= 1 ? "w-[28%]" : "w-[35%]"
  const childFlex = parentTargets.length <= 1 ? "w-[72%]" : "w-[65%]"

  return (
    <div
      ref={containerRef}
      className={cn("relative w-full overflow-hidden rounded-md border bg-card p-2.5", className)}
    >
      {/* SVG 连线层 */}
      <svg
        className="pointer-events-none absolute inset-0 z-0"
        width={dimensions.width || "100%"}
        height={dimensions.height || "100%"}
      >
        {paths.map((path) => (
          <path
            key={path.id}
            d={path.d}
            fill="none"
            stroke="hsl(var(--primary))"
            strokeWidth={1.5}
            strokeOpacity={0.4}
          />
        ))}
      </svg>

      {/* 顶部标题栏 */}
      <div className="relative z-10 mb-2 flex items-center gap-1.5 text-xs font-semibold text-foreground">
        <i className="fa-solid fa-circle-nodes size-3 text-primary" />
        <span>{virtualModel}</span>
        <span className="ml-auto text-[10px] font-normal text-muted-foreground">
          {targets.length} 条路由 · 已启用 {enabledCount}
        </span>
      </div>

      {/* 主体两栏 */}
      <div className="relative z-10 flex gap-3">
        {/* 左侧：父级节点 */}
        <div className={cn("flex flex-col gap-1.5", parentFlex)}>
          <div className="text-[9px] font-medium uppercase tracking-wider text-muted-foreground/70">父级</div>
          {parentTargets.length === 0 && (
            <div className="text-[10px] text-muted-foreground">暂无</div>
          )}
          {parentTargets.map((target) => (
            <ParentNode
              key={target.id}
              target={target}
              selected={selectedId === target.id}
              editMode={editMode}
              actions={editMode ? renderParentActions?.(target) : undefined}
              ref={(el) => { parentRefs.current[target.id] = el }}
              onClick={() => (onSelectParent ?? onSelect)?.(target.id)}
            />
          ))}
        </div>

        {/* 右侧：子级节点 */}
        <div className={cn("flex flex-col gap-1.5", childFlex)}>
          <div className="text-[9px] font-medium uppercase tracking-wider text-muted-foreground/70">供应商模型</div>
          {childTargets.length === 0 && (
            <div className="text-[10px] text-muted-foreground">暂无</div>
          )}
          {childTargets.map((target) => (
            <ChildNode
              key={target.id}
              target={target}
              selected={selectedId === target.id}
              ref={(el) => { childRefs.current[target.id] = el }}
              onClick={() => (onSelectChild ?? onSelect)?.(target.id)}
            />
          ))}
        </div>
      </div>
    </div>
  )
}

/* ─── 父级节点：水平紧凑行 ─── */

interface ParentNodeProps {
  target: VirtualModelTargetNode
  selected?: boolean
  editMode?: boolean
  actions?: React.ReactNode
  onClick?: () => void
}

const ParentNode = React.forwardRef<HTMLElement, ParentNodeProps>(
  ({ target, selected, editMode, actions, onClick }, ref) => {
    return (
      <div
        ref={ref as React.Ref<HTMLDivElement>}
        role="button"
        tabIndex={0}
        onClick={onClick}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault()
            onClick?.()
          }
        }}
        className={cn(
          "group relative flex items-center gap-2 rounded-md border bg-background px-2 py-1.5 text-left shadow-sm transition-all",
          "hover:shadow-md focus:outline-none focus:ring-2 focus:ring-ring",
          healthBorderClass(target.healthy, target.enabled),
          selected && "ring-2 ring-primary",
          target.enabled === false && "opacity-60",
          editMode && "pr-16"
        )}
      >
        {/* 状态灯 */}
        <span className={cn("shrink-0 size-1.5 rounded-full", statusDotClass(target.healthy, target.enabled))} />

        {/* 供应商 + 模型 */}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1">
            <span className="truncate text-[10px] text-muted-foreground">{target.provider}</span>
          </div>
          <div className="truncate text-xs font-semibold">{target.model}</div>
        </div>

        {/* 右侧元信息 */}
        <div className="shrink-0 text-right text-[9px] text-muted-foreground">
          <div>P{target.priority}</div>
          {target.quotaPercent !== undefined && (
            <div className="tabular-nums">{target.quotaPercent}%</div>
          )}
        </div>

        {/* 编辑模式操作按钮 */}
        {actions && (
          <div
            className="absolute right-1 top-1/2 flex -translate-y-1/2 items-center gap-0.5"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.stopPropagation()}
          >
            {actions}
          </div>
        )}
      </div>
    )
  }
)
ParentNode.displayName = "ParentNode"

/* ─── 子级节点：多行紧凑布局 ─── */

interface ChildNodeProps {
  target: VirtualModelTargetNode
  selected?: boolean
  onClick?: () => void
}

const ChildNode = React.forwardRef<HTMLButtonElement, ChildNodeProps>(
  ({ target, selected, onClick }, ref) => {
    const contextPairs = parseContextConfig(target.contextConfig)
    const hasFeatures = target.features && (
      target.features.toolCalling || target.features.imageInput ||
      target.features.functionCalling || target.features.streaming
    )

    // 优先级底部着色：按健康状态分配颜色
    const priorityUnderlineClass = target.enabled === false
      ? "border-b-2 border-muted-foreground/30"
      : target.healthy === false
        ? "border-b-2 border-destructive/60"
        : target.healthy === true
          ? "border-b-2 border-emerald-500/60"
          : "border-b-2 border-primary/40"

    return (
      <button
        type="button"
        ref={ref}
        onClick={onClick}
        className={cn(
          "flex rounded-md border bg-background px-2 py-1.5 text-left shadow-sm transition-all",
          "hover:shadow-md focus:outline-none focus:ring-2 focus:ring-ring",
          healthBorderClass(target.healthy, target.enabled),
          selected && "ring-2 ring-primary",
          target.enabled === false && "opacity-60"
        )}
      >
        {/* 主体内容 */}
        <div className="min-w-0 flex-1">
          {/* 第一行：供应商 + 模型 + 优先级(底色) + 额度 */}
          <div className="flex items-baseline gap-1.5">
            <i className="fa-solid fa-building size-2.5 shrink-0 text-muted-foreground" />
            <span className="truncate text-[10px] text-muted-foreground">{target.provider}</span>
            <span className="shrink-0 text-muted-foreground/50">·</span>
            <span className="truncate text-xs font-semibold">{target.model}</span>
            <span className={cn("ml-auto shrink-0 text-[9px] tabular-nums text-muted-foreground", priorityUnderlineClass)}>
              P{target.priority}
            </span>
            {target.quotaPercent !== undefined && (
              <span className="shrink-0 text-[9px] tabular-nums text-muted-foreground">
                {target.quotaPercent}%
              </span>
            )}
          </div>

          {/* 第二行：上下文配置 + 特性标签 */}
          {(contextPairs.length > 0 || hasFeatures) && (
            <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5">
              {contextPairs.map(([key, val]) => (
                <span key={key} className="text-[9px] text-muted-foreground">
                  <span className="font-medium text-foreground/70">{key}</span>:{val}
                </span>
              ))}
              {target.features?.toolCalling && (
                <span className="inline-flex items-center gap-0.5 rounded bg-muted px-1 py-px text-[8px]">
                  <i className="fa-solid fa-wrench size-2" />工具
                </span>
              )}
              {target.features?.imageInput && (
                <span className="inline-flex items-center gap-0.5 rounded bg-muted px-1 py-px text-[8px]">
                  <i className="fa-solid fa-image size-2" />图像
                </span>
              )}
              {target.features?.functionCalling && (
                <span className="inline-flex items-center gap-0.5 rounded bg-muted px-1 py-px text-[8px]">
                  <i className="fa-solid fa-code size-2" />函数
                </span>
              )}
              {target.features?.streaming && (
                <span className="inline-flex items-center gap-0.5 rounded bg-muted px-1 py-px text-[8px]">
                  <i className="fa-solid fa-bolt size-2" />流式
                </span>
              )}
            </div>
          )}

          {/* 额度进度条 */}
          {target.quotaPercent !== undefined && (
            <div className="mt-1 h-0.5 w-full overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-primary transition-all"
                style={{ width: `${Math.min(100, Math.max(0, target.quotaPercent))}%` }}
              />
            </div>
          )}
        </div>
      </button>
    )
  }
)
ChildNode.displayName = "ChildNode"
