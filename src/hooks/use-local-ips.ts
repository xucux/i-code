import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react'
import { invokeCommand } from '@/hooks/use-command'

/** 网关首选展示地址持久化键（localStorage） */
const PREFERRED_HOST_STORAGE_KEY = 'i-code:gateway-preferred-host'

/** 订阅回调集合：当地址偏好变化时通知所有 useLocalIps 消费方 */
const preferredHostListeners = new Set<() => void>()

/** 读取用户最后选中的网关展示地址；读取失败（如隐私模式）返回 null */
export function getPreferredGatewayHost(): string | null {
  try {
    return localStorage.getItem(PREFERRED_HOST_STORAGE_KEY)
  } catch {
    return null
  }
}

/** 持久化用户最后选中的网关展示地址，并通知所有消费方刷新 */
export function setPreferredGatewayHost(host: string): void {
  try {
    localStorage.setItem(PREFERRED_HOST_STORAGE_KEY, host)
  } catch {
    // 忽略持久化失败，仅影响本次会话
  }
  preferredHostListeners.forEach((listener) => listener())
}

/**
 * 网关监听地址解析结果
 */
export interface ResolvedGatewayHosts {
  /** 用于在 UI 上优先展示的地址；当绑定 0.0.0.0 时为首选 LAN 地址，否则为原绑定地址 */
  displayHost: string
  /** 所有可用于访问网关的地址列表（已按优先级排序，绑定具体地址时仅含该地址） */
  hosts: string[]
  /** 当前绑定地址是否为通配地址（0.0.0.0 / ::） */
  isWildcard: boolean
  /** 是否正在加载本机网卡地址 */
  loading: boolean
}

/** 判断是否为通配绑定地址（IPv4 0.0.0.0 或 IPv6 ::） */
export function isWildcardHost(host: string | undefined | null): boolean {
  if (!host) return false
  const lower = host.trim().toLowerCase()
  return lower === '0.0.0.0' || lower === '::' || lower === '[::]'
}

/**
 * 按网关展示优先级对 IPv4 地址排序
 *
 * 顺序：192.168.0.0/24 → 192.168.0.0/16 → 172.16.0.0/12 → 10.0.0.0/8 → 其他
 * 组内保持系统枚举顺序，避免地址频繁跳动。
 */
export function sortLocalIps(ips: string[]): string[] {
  const groupOf = (ip: string): number => {
    const parts = ip.split('.')
    if (parts.length !== 4) return Number.MAX_SAFE_INTEGER
    const a = Number(parts[0])
    const b = Number(parts[1])
    const c = Number(parts[2])
    // 192.168.0.x（/24）优先，常见路由器默认网关网段
    if (a === 192 && b === 168 && c === 0) return 0
    if (a === 192 && b === 168) return 1
    if (a === 172 && b >= 16 && b <= 31) return 2
    if (a === 10) return 3
    return Number.MAX_SAFE_INTEGER
  }

  return [...ips].sort((x, y) => {
    const gx = groupOf(x)
    const gy = groupOf(y)
    if (gx !== gy) return gx - gy
    return 0
  })
}

/**
 * 解析网关监听地址
 *
 * 当网关绑定通配地址（0.0.0.0 / ::）时，调用后端 `gateway_list_local_ips`
 * 获取本机所有可用 IPv4 地址并按 LAN 优先级排序；否则直接返回绑定地址。
 *
 * @param boundHost 网关运行时返回的 `boundHost`
 * @param boundPort 网关运行时返回的 `boundPort`（仅用于触发刷新依赖）
 */
export function useLocalIps(
  boundHost: string | undefined,
  boundPort: number | undefined
): ResolvedGatewayHosts {
  const wildcard = isWildcardHost(boundHost)
  const [ips, setIps] = useState<string[]>([])
  const [loading, setLoading] = useState(false)

  // 订阅「接口文档中最后选中的地址」，变化后立即重算展示地址
  const preferredHost = useSyncExternalStore(
    (onStoreChange) => {
      preferredHostListeners.add(onStoreChange)
      return () => preferredHostListeners.delete(onStoreChange)
    },
    getPreferredGatewayHost
  )

  const refresh = useCallback(async () => {
    if (!wildcard) {
      setIps([])
      return
    }
    setLoading(true)
    try {
      const result = await invokeCommand<string[]>('gateway_list_local_ips')
      setIps(sortLocalIps(result ?? []))
    } catch {
      setIps([])
    } finally {
      setLoading(false)
    }
  }, [wildcard])

  useEffect(() => {
    void refresh()
  }, [refresh, boundPort])

  // 所有可用于访问网关的地址：
  // - 未绑定时为空
  // - 绑定具体地址时仅含该地址
  // - 通配监听时：LAN 地址列表末尾追加 loopback（127.0.0.1），方便本机访问；
  //   追加在排序之后，确保始终位于列表最末尾。
  const hosts = useMemo(() => {
    if (!boundHost) return []
    if (!wildcard) return [boundHost]
    const base = ips.length > 0 ? ips : [boundHost]
    return base.includes('127.0.0.1') ? base : [...base, '127.0.0.1']
  }, [boundHost, wildcard, ips])

  // 展示地址优先级：接口文档中最后选中的地址（若仍在当前可用列表内）> 排序首个 LAN 地址
  const displayHost = useMemo(() => {
    if (!boundHost) return ''
    if (!wildcard) return boundHost
    if (preferredHost && hosts.includes(preferredHost)) return preferredHost
    return ips[0] ?? boundHost
  }, [boundHost, wildcard, preferredHost, hosts, ips])

  if (!boundHost) {
    return { displayHost: '', hosts: [], isWildcard: wildcard, loading: false }
  }

  if (!wildcard) {
    return { displayHost: boundHost, hosts: [boundHost], isWildcard: false, loading: false }
  }

  return {
    displayHost,
    hosts,
    isWildcard: true,
    loading,
  }
}
