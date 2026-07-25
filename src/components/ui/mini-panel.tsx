"use client"

import * as React from "react"

import { cn } from "@/lib/utils"
import { Card } from "@/components/ui/card"
import { Button } from "@/components/ui/button"
import { Label } from "@/components/ui/label"
import { Slider } from "@/components/ui/slider"
import { Switch } from "@/components/ui/switch"
import {
  MiniFloatingPanel,
  type MiniFloatingPanelVisibleFields,
} from "@/components/ui/mini-floating-panel"

/**
 * 迷你面板展示形式
 */
export type MiniPanelVariant = "compact" | "normal" | "expanded"

/**
 * 迷你面板信息展示量
 */
export type MiniPanelInfoLevel = "minimal" | "normal" | "full"

export interface MiniPanelData {
  provider: string
  currentModel: string
  totalQuota?: number | null
  usedQuota?: number | null
  modelConsumption?: number | null
  gatewayUrl?: string
  gatewayStatus?: "idle" | "running" | "error"
  chartData?: { label?: string; value: number }[]
}

/**
 * 迷你面板完整设置项
 */
export interface MiniPanelSettings {
  /** 展示形式 */
  variant: MiniPanelVariant
  /** 信息展示量（兼容旧逻辑） */
  infoLevel: MiniPanelInfoLevel
  /** 面板宽度（px） */
  width: number
  /** 面板高度（px） */
  height: number
  /** 各信息项显示开关 */
  visibleFields: MiniFloatingPanelVisibleFields
}

export interface MiniPanelProps {
  /** 面板数据 */
  data: MiniPanelData
  /** 当前展示形式 */
  variant?: MiniPanelVariant
  /** 当前信息展示量 */
  infoLevel?: MiniPanelInfoLevel
  /** 面板宽度（px） */
  width?: number
  /** 面板高度（px） */
  height?: number
  /** 各信息项显示开关 */
  visibleFields?: MiniFloatingPanelVisibleFields
  /** 用户修改设置时的回调 */
  onSettingsChange?: (settings: MiniPanelSettings) => void
  /** 切换回主界面的回调 */
  onBackToMain?: () => void
  /** 自定义类名 */
  className?: string
}

const DEFAULT_WIDTH = 256
const DEFAULT_HEIGHT = 160

const DEFAULT_VISIBLE_FIELDS: Required<MiniFloatingPanelVisibleFields> = {
  provider: true,
  currentModel: true,
  quota: true,
  modelConsumption: true,
  gateway: true,
  chart: true,
}

/**
 * 迷你悬浮面板容器
 *
 * 通过顶部栏闪电图标触发后，将主界面切换为紧凑的迷你卡片模式。
 * 面板顶部提供设置与返回主界面入口；点击设置后展开设置区，
 * 可调整面板宽度、高度以及各信息项的显示/隐藏。
 */
export function MiniPanel({
  data,
  variant = "normal",
  infoLevel = "normal",
  width = DEFAULT_WIDTH,
  height = DEFAULT_HEIGHT,
  visibleFields,
  onSettingsChange,
  onBackToMain,
  className,
}: MiniPanelProps) {
  const [settingsOpen, setSettingsOpen] = React.useState(false)

  const currentVisible = React.useMemo(
    () => ({ ...DEFAULT_VISIBLE_FIELDS, ...visibleFields }),
    [visibleFields]
  )

  const emitChange = React.useCallback(
    (patch: Partial<MiniPanelSettings>) => {
      onSettingsChange?.({
        variant,
        infoLevel,
        width,
        height,
        visibleFields: currentVisible,
        ...patch,
      })
    },
    [onSettingsChange, variant, infoLevel, width, height, currentVisible]
  )

  const toggleField = (key: keyof MiniFloatingPanelVisibleFields) => {
    emitChange({
      visibleFields: { ...currentVisible, [key]: !currentVisible[key] },
    })
  }

  const scale = variant === "compact" ? 0.9 : variant === "expanded" ? 1.1 : 1

  return (
    <div
      className={cn(
        "fixed inset-0 z-40 flex flex-col items-center justify-center gap-4 bg-background/95 p-6 backdrop-blur-sm",
        className
      )}
    >
      {/* 顶部操作栏：设置与返回主界面 */}
      <div className="flex items-center gap-2">
        <Button
          type="button"
          variant="outline"
          size="icon"
          onClick={() => setSettingsOpen((v) => !v)}
          aria-label="设置"
          className={cn(settingsOpen && "bg-accent text-accent-foreground")}
        >
          <i className="fa-solid fa-gear size-4" />
        </Button>
        {onBackToMain && (
          <Button
            type="button"
            variant="outline"
            size="icon"
            onClick={onBackToMain}
            aria-label="切换回主界面"
          >
            <i className="fa-solid fa-expand size-4" />
          </Button>
        )}
      </div>

      {/* 迷你面板主体 */}
      <MiniFloatingPanel
        provider={data.provider}
        currentModel={data.currentModel}
        totalQuota={data.totalQuota}
        usedQuota={data.usedQuota}
        modelConsumption={data.modelConsumption}
        gatewayUrl={data.gatewayUrl}
        gatewayStatus={data.gatewayStatus ?? "idle"}
        chartData={data.chartData}
        width={Math.round(width * scale)}
        height={Math.round(height * scale)}
        visibleFields={currentVisible}
        className="shadow-xl"
      />

      {/* 设置区 */}
      {settingsOpen && (
        <Card className="w-full max-w-sm p-4">
          <h3 className="mb-3 text-sm font-medium">迷你面板设置</h3>

          <div className="space-y-5">
            {/* 面板尺寸 */}
            <div className="space-y-3">
              <Label className="text-xs text-muted-foreground">面板尺寸</Label>
              <div className="space-y-2">
                <div className="flex items-center justify-between text-xs">
                  <span>宽度</span>
                  <span className="tabular-nums text-muted-foreground">{width}px</span>
                </div>
                <Slider
                  value={[width]}
                  min={180}
                  max={480}
                  step={8}
                  onValueChange={([v]) => emitChange({ width: v })}
                />
              </div>
              <div className="space-y-2">
                <div className="flex items-center justify-between text-xs">
                  <span>高度</span>
                  <span className="tabular-nums text-muted-foreground">{height}px</span>
                </div>
                <Slider
                  value={[height]}
                  min={120}
                  max={400}
                  step={8}
                  onValueChange={([v]) => emitChange({ height: v })}
                />
              </div>
            </div>

            {/* 信息项开关 */}
            <div className="space-y-3">
              <Label className="text-xs text-muted-foreground">信息项显示</Label>
              <div className="grid grid-cols-2 gap-2">
                <FieldSwitch
                  label="供应商"
                  checked={currentVisible.provider}
                  onCheckedChange={() => toggleField("provider")}
                />
                <FieldSwitch
                  label="当前模型"
                  checked={currentVisible.currentModel}
                  onCheckedChange={() => toggleField("currentModel")}
                />
                <FieldSwitch
                  label="额度进度"
                  checked={currentVisible.quota}
                  onCheckedChange={() => toggleField("quota")}
                />
                <FieldSwitch
                  label="模型消耗"
                  checked={currentVisible.modelConsumption}
                  onCheckedChange={() => toggleField("modelConsumption")}
                />
                <FieldSwitch
                  label="网关状态"
                  checked={currentVisible.gateway}
                  onCheckedChange={() => toggleField("gateway")}
                />
                <FieldSwitch
                  label="面积图"
                  checked={currentVisible.chart}
                  onCheckedChange={() => toggleField("chart")}
                />
              </div>
            </div>
          </div>
        </Card>
      )}
    </div>
  )
}

interface FieldSwitchProps {
  label: string
  checked: boolean
  onCheckedChange: () => void
}

function FieldSwitch({ label, checked, onCheckedChange }: FieldSwitchProps) {
  return (
    <div className="flex items-center justify-between rounded-md border px-2.5 py-1.5">
      <Label className="text-xs">{label}</Label>
      <Switch checked={checked} onCheckedChange={onCheckedChange} className="scale-90" />
    </div>
  )
}
