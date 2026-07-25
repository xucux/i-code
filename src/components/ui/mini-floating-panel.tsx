"use client"

import * as React from "react"
import { Area, AreaChart, ResponsiveContainer, Tooltip, YAxis } from "recharts"

import { cn } from "@/lib/utils"
import { Card } from "@/components/ui/card"
import { Progress } from "@/components/ui/progress"
import { formatCompactCount } from "@/core/utils"

/**
 * 迷你面积图单点数据
 */
export interface MiniChartPoint {
  /** 数据点标签（tooltip 使用） */
  label?: string
  /** 数值 */
  value: number
}

/**
 * 网关状态
 */
export type GatewayStatus = "idle" | "running" | "error"

/**
 * 迷你悬浮面板中可独立控制显示/隐藏的信息项
 */
export interface MiniFloatingPanelVisibleFields {
  /** 是否显示供应商名称 */
  provider?: boolean
  /** 是否显示当前模型 */
  currentModel?: boolean
  /** 是否显示额度进度 */
  quota?: boolean
  /** 是否显示模型消耗 */
  modelConsumption?: boolean
  /** 是否显示网关地址与状态 */
  gateway?: boolean
  /** 是否显示面积图（仅在 chartData 存在时） */
  chart?: boolean
}

export interface MiniFloatingPanelProps {
  /** 当前供应商名称 */
  provider: string
  /** 当前模型名称 */
  currentModel: string
  /** 总额度（可选） */
  totalQuota?: number | null
  /** 已用额度（可选） */
  usedQuota?: number | null
  /** 当前模型累计消耗量 */
  modelConsumption?: number | null
  /** 网关地址 */
  gatewayUrl?: string
  /** 网关运行状态 */
  gatewayStatus?: GatewayStatus
  /** 迷你面积图数据；为空时自动切换为数字模式 */
  chartData?: MiniChartPoint[]
  /** 面板宽度（px），默认 256 */
  width?: number
  /** 面板最小高度（px），默认自适应 */
  height?: number
  /** 各信息项的显示开关 */
  visibleFields?: MiniFloatingPanelVisibleFields
  /** 自定义类名 */
  className?: string
  /** 数字模式下是否显示「今日消耗」文案 */
  showConsumptionLabel?: boolean
}

/**
 * 根据网关状态返回对应颜色
 */
function statusColor(status: GatewayStatus): string {
  switch (status) {
    case "running":
      return "bg-emerald-500"
    case "error":
      return "bg-destructive"
    default:
      return "bg-muted-foreground"
  }
}

/**
 * 默认所有字段均显示
 */
function resolveVisibleFields(
  fields?: MiniFloatingPanelVisibleFields
): Required<MiniFloatingPanelVisibleFields> {
  return {
    provider: fields?.provider !== false,
    currentModel: fields?.currentModel !== false,
    quota: fields?.quota !== false,
    modelConsumption: fields?.modelConsumption !== false,
    gateway: fields?.gateway !== false,
    chart: fields?.chart !== false,
  }
}

/**
 * 迷你悬浮面板
 *
 * 以小型卡片形式简明展示供应商额度、模型消耗、当前模型与网关信息。
 * 提供「面积图模式」与「数字模式」两种展示，调用方可根据场景切换。
 * 支持通过 visibleFields 控制各信息项的显示与隐藏。
 */
export function MiniFloatingPanel({
  provider,
  currentModel,
  totalQuota,
  usedQuota,
  modelConsumption,
  gatewayUrl,
  gatewayStatus = "idle",
  chartData,
  width = 256,
  height,
  visibleFields,
  className,
  showConsumptionLabel = true,
}: MiniFloatingPanelProps) {
  const show = resolveVisibleFields(visibleFields)
  const quotaPercent = React.useMemo(() => {
    if (totalQuota && totalQuota > 0) {
      const used = usedQuota ?? 0
      return Math.min(100, Math.max(0, (Number(used) / Number(totalQuota)) * 100))
    }
    return 0
  }, [totalQuota, usedQuota])

  const showChart = show.chart && chartData && chartData.length > 0

  return (
    <Card
      className={cn(
        "overflow-hidden border bg-background/95 p-3 shadow-lg backdrop-blur-sm",
        className
      )}
      style={{ width, minHeight: height }}
    >
      {/* 头部：供应商与网关状态 */}
      {(show.provider || show.gateway) && (
        <div className="mb-2 flex items-center justify-between">
          {show.provider && (
            <div className="flex items-center gap-1.5 text-xs font-medium">
              <i className="fa-solid fa-building size-3 text-primary" />
              <span className="max-w-[7rem] truncate">{provider}</span>
            </div>
          )}
          {show.gateway && (
            <div className="flex items-center gap-1.5">
              <span
                className={cn("size-1.5 rounded-full", statusColor(gatewayStatus))}
              />
              {gatewayUrl && (
                <span className="max-w-[5rem] truncate text-[10px] text-muted-foreground">
                  {gatewayUrl}
                </span>
              )}
            </div>
          )}
        </div>
      )}

      {/* 当前模型 */}
      {show.currentModel && (
        <div className="mb-2 flex items-center gap-1.5 text-[10px] text-muted-foreground">
          <i className="fa-solid fa-cube size-3" />
          <span className="truncate">{currentModel}</span>
        </div>
      )}

      {/* 额度进度 */}
      {show.quota && totalQuota !== undefined && totalQuota !== null && (
        <div className="mb-2 space-y-1">
          <div className="flex justify-between text-[10px] text-muted-foreground">
            <span>额度</span>
            <span className="tabular-nums">
              {formatCompactCount(usedQuota ?? 0)} / {formatCompactCount(totalQuota)}
            </span>
          </div>
          <Progress value={quotaPercent} className="h-1" />
        </div>
      )}

      {/* 模型消耗：图表或数字 */}
      {show.modelConsumption && modelConsumption !== undefined && modelConsumption !== null && (
        <div className="mt-2">
          {showChart ? (
            <div className="h-14 w-full">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={chartData} margin={{ top: 2, right: 0, left: -24, bottom: 0 }}>
                  <defs>
                    <linearGradient id="miniPanelGradient" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="5%" stopColor="hsl(var(--primary))" stopOpacity={0.45} />
                      <stop offset="95%" stopColor="hsl(var(--primary))" stopOpacity={0.02} />
                    </linearGradient>
                  </defs>
                  <YAxis hide domain={[0, "auto"]} />
                  <Tooltip
                    contentStyle={{
                      backgroundColor: "hsl(var(--card))",
                      borderColor: "hsl(var(--border))",
                      borderRadius: "var(--radius)",
                      color: "hsl(var(--card-foreground))",
                      fontSize: 12,
                    }}
                    formatter={(value) =>
                      [formatCompactCount(typeof value === "number" ? value : Number(value)), "消耗"]}
                    labelFormatter={(label) => label}
                  />
                  <Area
                    type="monotone"
                    dataKey="value"
                    stroke="hsl(var(--primary))"
                    fill="url(#miniPanelGradient)"
                    strokeWidth={1.5}
                  />
                </AreaChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <div className="flex items-center justify-between rounded-md bg-muted/50 px-2 py-1.5">
              <span className="text-[10px] text-muted-foreground">
                {showConsumptionLabel ? "累计消耗" : "消耗"}
              </span>
              <span className="text-sm font-semibold tabular-nums text-primary">
                {formatCompactCount(modelConsumption)}
              </span>
            </div>
          )}
        </div>
      )}
    </Card>
  )
}
