import { useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import type { CatalogModel, CatalogProvider } from '@/modules/gateway-runtime/types'

/**
 * 获取目录模型列表（真实 + 虚拟供应商合并）
 *
 * 调用后端 `gateway_catalog_models` 命令，供聊天、CLI 配置管理等
 * 拉取「内部供应商/模型列表」时使用。
 */
export function useCatalogModels(): {
  models: CatalogModel[]
  loading: boolean
  refetch: () => void
} {
  const [models, setModels] = useState<CatalogModel[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    setLoading(true)
    try {
      const result = await invokeCommand<CatalogModel[]>('gateway_catalog_models')
      setModels(result)
    } catch {
      setModels([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [])

  return { models, loading, refetch: load }
}

/**
 * 获取目录供应商列表（真实 + 虚拟供应商合并）
 *
 * 调用后端 `gateway_catalog_providers` 命令，供 CLI 配置管理
 * 「添加供应商」绑定下拉使用。虚拟供应商条目 `id` 为 `virtual:{id}`。
 */
export function useCatalogProviders(): {
  providers: CatalogProvider[]
  loading: boolean
  refetch: () => void
} {
  const [providers, setProviders] = useState<CatalogProvider[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    setLoading(true)
    try {
      const result = await invokeCommand<CatalogProvider[]>('gateway_catalog_providers')
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
 * 解析网关默认授权 Key 明文
 *
 * 供虚拟供应商在 CLI 配置中预填 apiKey 使用（虚拟供应商无独立凭证，
 * 统一使用网关默认授权 Key）。未配置默认 Key 时返回 `null`。
 */
export async function resolveDefaultGatewayKey(): Promise<string | null> {
  return invokeCommand<string | null>('gateway_resolve_default_key')
}