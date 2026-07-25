import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow, PhysicalSize } from '@tauri-apps/api/window'
import { cn } from '@/lib/utils'
import { formatCompactCount } from '@/core/utils'
import { isTauri } from '@/core/utils'
import { useGatewayStatus } from '@/hooks/use-gateway-status'
import { useModelCallStats } from '@/hooks/use-model-call-stats'
import { useAggregatedStats } from '@/hooks/use-aggregated-stats'
import { getTodayTokens } from '@/hooks/use-call-records-mutation'

/** 迷你面板数据窗口：最近 1 小时 */
const WINDOW_HOURS = 1
/** 今日 token 刷新间隔（毫秒） */
const REFRESH_INTERVAL = 5_000

const STORAGE_KEY = 'i-code:mini-panel-settings'

/** 可独立控制显隐的信息项 */
interface VisibleFields {
  provider?: boolean
  currentModel?: boolean
  quota?: boolean
  modelConsumption?: boolean
  gateway?: boolean
  chart?: boolean
}

interface MiniPanelSettings {
  width: number
  height: number
  visibleFields: Required<VisibleFields>
}

/** 最小化模式尺寸 */
const MINIMIZED_WIDTH = 52
const MINIMIZED_HEIGHT = 180

const DEFAULT_SETTINGS: MiniPanelSettings = {
  width: 320,
  height: 220,
  visibleFields: {
    provider: true,
    currentModel: true,
    quota: true,
    modelConsumption: true,
    gateway: true,
    chart: true,
  },
}

function loadSettings(): MiniPanelSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return DEFAULT_SETTINGS
    const parsed = JSON.parse(raw) as MiniPanelSettings
    return { ...DEFAULT_SETTINGS, ...parsed, visibleFields: { ...DEFAULT_SETTINGS.visibleFields, ...parsed.visibleFields } }
  } catch {
    return DEFAULT_SETTINGS
  }
}

function saveSettings(settings: MiniPanelSettings) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
}

/** 根据网关状态返回对应颜色类名 */
function statusDotColor(status: string): string {
  switch (status) {
    case 'running': return 'bg-emerald-500'
    case 'error': return 'bg-red-500'
    default: return 'bg-muted-foreground'
  }
}

/**
 * 格式化时间桶为图表横坐标标签
 *
 * 取本地小时与分钟，保持标签简短。
 */
function formatChartLabel(timeBucket: string): string {
  const d = new Date(timeBucket)
  const hours = d.getHours().toString().padStart(2, '0')
  const minutes = d.getMinutes().toString().padStart(2, '0')
  return `${hours}:${minutes}`
}

/**
 * 迷你面板独立窗口页面
 *
 * 不使用组件库的 Card/Panel 容器，直接以原生元素紧凑排列。
 * 无标题栏、无滚动条，操作按钮使用原生 <button> 元素。
 * 支持拖拽（data-tauri-drag-region）、最小化至竖长条、展开回主窗口。
 */
function MiniPanelPage() {
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [minimized, setMinimized] = useState(false)
  const [settings, setSettings] = useState<MiniPanelSettings>(loadSettings)
  const [now, setNow] = useState(Date.now())

  // 保存最小化前的尺寸，用于恢复
  const preMinimizeSize = useRef({ width: settings.width, height: settings.height })

  const show = settings.visibleFields
  /** 仅在 Tauri 环境下获取窗口实例 */
  const win = isTauri() ? getCurrentWindow() : null

  // 时间 tick，用于周期性刷新相对时间窗口
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), REFRESH_INTERVAL)
    return () => clearInterval(timer)
  }, [])

  // 宽度/高度变化时同步窗口物理尺寸
  useEffect(() => {
    if (!win) return
    const w = minimized ? MINIMIZED_WIDTH : settings.width
    const h = minimized ? MINIMIZED_HEIGHT : settings.height
    void win.setSize(new PhysicalSize(w, h))
    if (!minimized) saveSettings(settings)
  }, [settings.width, settings.height, minimized])

  useEffect(() => {
    if (!minimized) saveSettings(settings)
  }, [settings.visibleFields, minimized])

  const toggleField = (key: keyof VisibleFields) => {
    setSettings((prev) => ({
      ...prev,
      visibleFields: { ...prev.visibleFields, [key]: !prev.visibleFields[key] },
    }))
  }

  /** 关闭迷你窗口，展示主窗口 */
  const backToMain = useCallback(async () => {
    await invoke('close_mini_panel')
  }, [])

  /** 切换最小化模式 */
  const toggleMinimized = useCallback(() => {
    setMinimized((prev) => {
      if (!prev) {
        // 即将最小化：保存当前尺寸
        preMinimizeSize.current = { width: settings.width, height: settings.height }
      } else {
        // 即将恢复：无需额外操作，useEffect 会恢复尺寸
      }
      return !prev
    })
    // 最小化时关闭设置面板
    setSettingsOpen(false)
  }, [settings.width, settings.height])

  // 网关真实状态
  const { status } = useGatewayStatus()

  // 今日 Token 总数
  const [todayTokens, setTodayTokens] = useState(0)
  useEffect(() => {
    let cancelled = false
    const refresh = async () => {
      try {
        const count = await getTodayTokens()
        if (!cancelled) setTodayTokens(count)
      } catch {
        // 忽略错误，保持上次有效值
      }
    }
    refresh()
    const timer = setInterval(refresh, REFRESH_INTERVAL)
    return () => {
      cancelled = true
      clearInterval(timer)
    }
  }, [])

  // 最近 1 小时模型调用统计，用于推导最活跃供应商/模型
  const statsInput = useMemo(() => {
    const startAt = new Date(Date.now() - WINDOW_HOURS * 60 * 60 * 1000).toISOString()
    return { startAt }
  }, [now])
  const { rows: modelStats } = useModelCallStats(statsInput)

  const { provider, currentModel } = useMemo(() => {
    if (modelStats.length === 0) {
      return { provider: '-', currentModel: '-' }
    }
    const top = modelStats.reduce(
      (max, row) => ((row.requestCount ?? 0) > (max.requestCount ?? 0) ? row : max),
      modelStats[0]
    )
    return {
      provider: top.providerName || top.providerId || '-',
      currentModel: top.modelId || '-',
    }
  }, [modelStats])

  // 最近 1 小时流量图数据（10 分钟桶）
  const aggInput = useMemo(() => {
    const endAt = new Date().toISOString()
    const startAt = new Date(Date.now() - WINDOW_HOURS * 60 * 60 * 1000).toISOString()
    return { granularity: 'tenMinutes' as const, startAt, endAt }
  }, [now])
  const { rows: aggRows } = useAggregatedStats(aggInput)

  const chartData = useMemo(() => {
    const bucketMap = new Map<string, number>()
    for (const row of aggRows) {
      const label = formatChartLabel(row.timeBucket)
      bucketMap.set(label, (bucketMap.get(label) ?? 0) + row.requestCount)
    }
    return Array.from(bucketMap.entries())
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([label, value]) => ({ label, value }))
  }, [aggRows])

  // 网关地址与状态
  const gatewayUrl = status.boundHost && status.boundPort
    ? `${status.boundHost}:${status.boundPort}`
    : undefined
  const gatewayStatus = status.isRunning ? 'running' : 'idle'

  // 简易 SVG 面积图数据
  const chartPoints = show.chart && chartData.length > 0
    ? (() => {
        const data = chartData
        const maxVal = Math.max(...data.map((d) => d.value))
        const w = settings.width - 24
        const h = 36
        return data.map((d, i) => {
          const x = (i / (data.length - 1)) * w
          const y = h - (d.value / maxVal) * h
          return `${x},${y}`
        }).join(' ')
      })()
    : null

  // ========== 最小化模式 ==========
  if (minimized) {
    return (
      <div
        className="flex h-screen w-screen flex-col items-center overflow-hidden bg-background/95 py-1.5 backdrop-blur-sm"
        data-tauri-drag-region
      >
        {/* 最小化信息：纵向排列文本 */}
        <div className="flex flex-1 flex-col items-center gap-1 overflow-hidden px-1" data-tauri-drag-region>
          {show.provider && (
            <span className="max-w-full truncate text-center text-[8px] font-medium" title={provider}>
              {provider.slice(0, 4)}
            </span>
          )}
          {show.currentModel && (
            <span className="max-w-full truncate text-center text-[7px] text-muted-foreground" title={currentModel}>
              {currentModel.slice(0, 5)}
            </span>
          )}
          {show.gateway && (
            <span className={cn('inline-block size-1.5 rounded-full', statusDotColor(gatewayStatus))} />
          )}
          {show.modelConsumption && (
            <span className="text-[7px] font-semibold tabular-nums text-primary">
              {formatCompactCount(todayTokens)}
            </span>
          )}
        </div>

        {/* 还原按钮 */}
        <div className="pb-0.5" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
          <button
            type="button"
            onClick={toggleMinimized}
            className="flex h-5 w-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
            aria-label="还原"
            title="还原"
          >
            <i className="fa-solid fa-plus text-[8px]" />
          </button>
        </div>
      </div>
    )
  }

  // ========== 正常模式 ==========
  return (
    <div
      className="flex h-screen w-screen flex-col overflow-hidden bg-background/95 backdrop-blur-sm"
      data-tauri-drag-region
    >
      {/* 顶部操作行：拖拽区域 + 操作按钮 */}
      <div className="flex items-center justify-between px-2 pt-1.5 pb-0.5" data-tauri-drag-region>
        {/* 左侧可拖拽区域 */}
        <div className="flex-1" data-tauri-drag-region />
        {/* 右侧操作按钮：原生 button 元素 */}
        <div className="flex items-center gap-0.5" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
          <button
            type="button"
            onClick={() => setSettingsOpen((v) => !v)}
            className={cn(
              'flex h-5 w-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground',
              settingsOpen && 'bg-accent text-accent-foreground'
            )}
            aria-label="设置"
            title="设置"
          >
            <i className="fa-solid fa-gear text-[9px]" />
          </button>
          <button
            type="button"
            onClick={toggleMinimized}
            className="flex h-5 w-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
            aria-label="最小化"
            title="最小化"
          >
            <i className="fa-solid fa-minus text-[9px]" />
          </button>
          <button
            type="button"
            onClick={backToMain}
            className="flex h-5 w-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
            aria-label="展开主窗口"
            title="展开主窗口"
          >
            <i className="fa-regular fa-square text-[9px]" />
          </button>
          <button
            type="button"
            onClick={backToMain}
            className="flex h-5 w-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
            aria-label="关闭"
            title="关闭"
          >
            <i className="fa-solid fa-xmark text-[9px]" />
          </button>
        </div>
      </div>

      {/* 主体信息区：紧凑排列，禁止滚动，支持拖拽 */}
      <div className="flex flex-1 flex-col gap-1 overflow-hidden px-3 pb-2" data-tauri-drag-region>
        {/* 供应商 + 网关状态 */}
        {(show.provider || show.gateway) && (
          <div className="flex items-center justify-between" data-tauri-drag-region>
            {show.provider && (
              <div className="flex items-center gap-1">
                <i className="fa-solid fa-building text-[8px] text-primary" />
                <span className="max-w-[8rem] truncate text-[10px] font-medium">{provider}</span>
              </div>
            )}
            {show.gateway && (
              <div className="flex items-center gap-1">
                <span className={cn('inline-block size-1.5 rounded-full', statusDotColor(gatewayStatus))} />
                {gatewayUrl && (
                  <span className="max-w-[5rem] truncate text-[9px] text-muted-foreground">{gatewayUrl}</span>
                )}
              </div>
            )}
          </div>
        )}

        {/* 当前模型 */}
        {show.currentModel && (
          <div className="flex items-center gap-1" data-tauri-drag-region>
            <i className="fa-solid fa-cube text-[8px] text-muted-foreground" />
            <span className="truncate text-[10px] text-muted-foreground">{currentModel}</span>
          </div>
        )}

        {/* 模型消耗 */}
        {show.modelConsumption && (
          <div className="flex items-center justify-between rounded bg-muted/50 px-1.5 py-1" data-tauri-drag-region>
            <span className="text-[9px] text-muted-foreground">消耗</span>
            <span className="text-xs font-semibold tabular-nums text-primary">
              {formatCompactCount(todayTokens)}
            </span>
          </div>
        )}

        {/* 迷你面积图：原生 SVG */}
        {chartPoints && (
          <div className="mt-auto" data-tauri-drag-region>
            <svg
              viewBox={`0 0 ${settings.width - 24} 36`}
              className="h-9 w-full"
              preserveAspectRatio="none"
            >
              <defs>
                <linearGradient id="miniAreaGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="hsl(var(--primary))" stopOpacity={0.35} />
                  <stop offset="95%" stopColor="hsl(var(--primary))" stopOpacity={0.02} />
                </linearGradient>
              </defs>
              <polygon
                points={`0,36 ${chartPoints} ${settings.width - 24},36`}
                fill="url(#miniAreaGrad)"
              />
              <polyline
                points={chartPoints}
                fill="none"
                stroke="hsl(var(--primary))"
                strokeWidth={1.5}
                strokeLinejoin="round"
              />
            </svg>
          </div>
        )}
      </div>

      {/* 设置区：覆盖在主体上方 */}
      {settingsOpen && (
        <div className="absolute inset-x-0 bottom-0 z-20 border-t bg-background/95 p-2.5 backdrop-blur-sm">
          <div className="space-y-2">
            {/* 尺寸调节 */}
            <div className="flex items-center gap-2">
              <span className="shrink-0 text-[9px] text-muted-foreground">宽</span>
              <input
                type="range"
                min={200}
                max={480}
                step={8}
                value={settings.width}
                onChange={(e) => setSettings((prev) => ({ ...prev, width: Number(e.target.value) }))}
                className="mini-slider h-1 flex-1 cursor-pointer"
              />
              <span className="shrink-0 w-8 text-right text-[9px] tabular-nums text-muted-foreground">{settings.width}</span>
            </div>
            <div className="flex items-center gap-2">
              <span className="shrink-0 text-[9px] text-muted-foreground">高</span>
              <input
                type="range"
                min={140}
                max={400}
                step={8}
                value={settings.height}
                onChange={(e) => setSettings((prev) => ({ ...prev, height: Number(e.target.value) }))}
                className="mini-slider h-1 flex-1 cursor-pointer"
              />
              <span className="shrink-0 w-8 text-right text-[9px] tabular-nums text-muted-foreground">{settings.height}</span>
            </div>

            {/* 信息项开关 */}
            <div className="grid grid-cols-3 gap-1">
              {([
                ['provider', '供应商'],
                ['currentModel', '模型'],
                ['modelConsumption', '消耗'],
                ['gateway', '网关'],
                ['chart', '图表'],
              ] as const).map(([key, label]) => (
                <button
                  key={key}
                  type="button"
                  onClick={() => toggleField(key)}
                  className={cn(
                    'rounded px-1 py-0.5 text-[9px] transition-colors',
                    show[key]
                      ? 'bg-primary/15 text-primary'
                      : 'bg-muted text-muted-foreground'
                  )}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

export const Route = createFileRoute('/mini-panel')({
  component: MiniPanelPage,
})
