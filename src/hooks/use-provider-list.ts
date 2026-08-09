import { useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import type { Provider, BuiltinProvider } from '@/modules/ai-gateway/types'

/**
 * 获取 AI Gateway 供应商列表
 *
 * 调用后端 `gateway_provider_list` 命令。
 */
export function useProviderList(): { providers: Provider[]; loading: boolean; refetch: () => void } {
  const [providers, setProviders] = useState<Provider[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    setLoading(true)
    try {
      const result = await invokeCommand<Provider[]>('gateway_provider_list')
      setProviders(result)
    } catch {
      setProviders([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [])

  return { providers, loading, refetch: load }
}

/**
 * 获取内置供应商预设列表
 *
 * 调用后端 `gateway_builtin_providers_list` 命令。
 */
export function useBuiltinProviders(): {
  builtinProviders: BuiltinProvider[]
  loading: boolean
  refetch: () => void
} {
  const [builtinProviders, setBuiltinProviders] = useState<BuiltinProvider[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    setLoading(true)
    try {
      const result = await invokeCommand<BuiltinProvider[]>('gateway_builtin_providers_list')
      // 按 sortOrder 倒序排列（值越大越靠前）
      const sorted = [...result].sort((a, b) => b.sortOrder - a.sortOrder)
      setBuiltinProviders(sorted)
    } catch {
      setBuiltinProviders([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [])

  return { builtinProviders, loading, refetch: load }
}
