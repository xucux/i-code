import { useCallback, useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'

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
 * 顺序：192.168.0.0/16 → 172.16.0.0/12 → 10.0.0.0/8 → 其他
 * 组内保持系统枚举顺序，避免地址频繁跳动。
 */
export function sortLocalIps(ips: string[]): string[] {
  const groupOf = (ip: string): number => {
    const parts = ip.split('.')
    if (parts.length !== 4) return Number.MAX_SAFE_INTEGER
    const a = Number(parts[0])
    const b = Number(parts[1])
    if (a === 192 && b === 168) return 0
    if (a === 172 && b >= 16 && b <= 31) return 1
    if (a === 10) return 2
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

  if (!boundHost) {
    return { displayHost: '', hosts: [], isWildcard: wildcard, loading: false }
  }

  if (!wildcard) {
    return {
      displayHost: boundHost,
      hosts: [boundHost],
      isWildcard: false,
      loading: false,
    }
  }

  return {
    displayHost: ips[0] ?? boundHost,
    hosts: ips.length > 0 ? ips : [boundHost],
    isWildcard: true,
    loading,
  }
}
