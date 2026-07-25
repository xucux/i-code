import { useEffect, useMemo, useState } from 'react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { TitleBarInfo, type TitleBarInfoItem } from '@/components/ui/title-bar-info'
import { useGatewayStatus } from '@/hooks/use-gateway-status'
import { useMemoryUsage } from '@/modules/system/use-memory-usage'
import { useModelCallStats } from '@/hooks/use-model-call-stats'
import { getTodayTokens } from '@/hooks/use-call-records-mutation'
import { getSettings } from '@/hooks/use-settings'
import { useTranslation } from '@/modules/i18n/use-translation'
import { BACKEND_EVENTS } from '@/core/events'
import { formatMemory } from '@/core/utils'
import type { TitleBarInfoConfig } from '@/modules/settings/types'
import { DEFAULT_TITLEBAR_INFO_CONFIG } from '@/modules/settings/types'

/** 标题栏信息刷新间隔（毫秒） */
const METRICS_REFRESH_INTERVAL = 5_000

/** 标题栏中间信息展示容器
 *
 * 根据应用设置中的 `titlebarInfo` 配置，动态组合 Tokens / RPM / Latency /
 * 网关状态 / 内存占用等信息胶囊。最多在大胶囊中同时展示 3 项，超出时
 * 按固定优先级取舍：网关状态 > 内存 > Tokens > RPM > Latency。
 */
export function TitleBarInfoContainer() {
  const [config, setConfig] = useState<TitleBarInfoConfig>(DEFAULT_TITLEBAR_INFO_CONFIG)
  const { t } = useTranslation()

  // 加载用户设置的标题栏配置，并监听后端变更事件实时刷新
  useEffect(() => {
    let cancelled = false
    let unlisten: UnlistenFn | undefined

    const load = () => {
      getSettings()
        .then((s) => {
          if (cancelled) return
          if (s.titlebarInfo) {
            setConfig(s.titlebarInfo)
          }
        })
        .catch(() => {
          // 忽略错误，使用默认配置
        })
    }

    load()

    // 后端 settings_update 成功后会广播 settings:changed 事件，
    // payload 为最新的 TitleBarInfoConfig，直接更新即可避免再次 invoke。
    listen<TitleBarInfoConfig>(BACKEND_EVENTS.SETTINGS_CHANGED, (event) => {
      if (cancelled) return
      setConfig(event.payload)
    }).then((fn) => {
      unlisten = fn
    })

    return () => {
      cancelled = true
      if (unlisten) unlisten()
    }
  }, [])

  // 定时刷新时间锚点，确保统计窗口始终相对当前时间
  const [now, setNow] = useState(Date.now())
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), METRICS_REFRESH_INTERVAL)
    return () => clearInterval(timer)
  }, [])

  // 网关状态
  const { status } = useGatewayStatus()
  // 内存占用
  const { memoryKb } = useMemoryUsage({ enabled: config.showMemory })

  // 今日 Token 总数
  const [totalTokens, setTotalTokens] = useState(0)
  useEffect(() => {
    let cancelled = false
    const refresh = async () => {
      try {
        const count = await getTodayTokens()
        if (!cancelled) setTotalTokens(count)
      } catch {
        // 忽略错误，保持上次有效值
      }
    }
    refresh()
    const timer = setInterval(refresh, METRICS_REFRESH_INTERVAL)
    return () => {
      cancelled = true
      clearInterval(timer)
    }
  }, [])

  // 近 1 分钟请求数与延迟（用于 RPM / Latency）
  const oneMinuteAgo = useMemo(
    () => new Date(now - 60 * 1000).toISOString(),
    [now]
  )
  const { rows: recentRows } = useModelCallStats({ startAt: oneMinuteAgo })
  const { rpm, avgLatency } = useMemo(() => {
    const requestCount = recentRows.reduce((sum, r) => sum + (r.requestCount ?? 0), 0)
    // 以 60 秒为窗口计算 RPM
    const rpm = requestCount
    const totalDuration = recentRows.reduce(
      (sum, r) => sum + (r.avgDurationMs ?? 0) * (r.requestCount ?? 0),
      0
    )
    const avgLatency = requestCount > 0 ? totalDuration / requestCount : 0
    return { rpm, avgLatency }
  }, [recentRows])

  // 根据配置和优先级构建信息项列表
  const infoItems = useMemo<TitleBarInfoItem[]>(() => {
    const candidates: Array<{ enabled: boolean; item: TitleBarInfoItem; priority: number }> = [
      {
        enabled: config.showGatewayStatus,
        priority: 1,
        item: {
          icon: status.isRunning ? 'circle-check' : 'circle-pause',
          label: t('settings.titlebarInfo.gatewayStatus'),
          value: status.isRunning
            ? t('settings.titlebarInfo.gatewayRunning')
            : t('settings.titlebarInfo.gatewayStopped'),
          active: status.isRunning,
        },
      },
      {
        enabled: config.showMemory,
        priority: 2,
        item: {
          icon: 'memory',
          label: t('settings.titlebarInfo.memory'),
          value: memoryKb != null ? formatMemory(memoryKb) : '-',
          active: true,
        },
      },
      {
        enabled: config.showTokens,
        priority: 3,
        item: {
          icon: 'coins',
          label: t('settings.titlebarInfo.tokens'),
          value: totalTokens,
          active: true,
        },
      },
      {
        enabled: config.showRpm,
        priority: 4,
        item: {
          icon: 'bolt',
          label: t('settings.titlebarInfo.rpm'),
          value: rpm,
          active: rpm > 0,
        },
      },
      {
        enabled: config.showLatency,
        priority: 5,
        item: {
          icon: 'clock',
          label: t('settings.titlebarInfo.latency'),
          value: avgLatency > 0 ? `${Math.round(avgLatency)}ms` : '-',
          active: avgLatency > 0,
        },
      },
    ]

    return candidates
      .filter((c) => c.enabled)
      .sort((a, b) => a.priority - b.priority)
      .slice(0, 3)
      .map((c) => c.item)
  }, [config, status, memoryKb, totalTokens, rpm, avgLatency, t])

  return (
    <div className="flex items-center gap-3">
      {infoItems.length > 0 && <TitleBarInfo items={infoItems} />}
    </div>
  )
}
