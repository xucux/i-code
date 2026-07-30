import { useCallback, useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import type { ExposedModel } from '@/modules/ai-gateway/types'
import type {
  VirtualProvider,
  VirtualModel,
  VirtualModelRoute,
} from '@/modules/virtual-provider/types'

/**
 * 获取虚拟供应商列表
 *
 * 调用后端 `virtual_provider_list` 命令。
 */
export function useVirtualProviderList(): {
  providers: VirtualProvider[]
  loading: boolean
  refetch: () => void
} {
  const [providers, setProviders] = useState<VirtualProvider[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    setLoading(true)
    try {
      const result = await invokeCommand<VirtualProvider[]>('virtual_provider_list')
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
 * 获取指定虚拟供应商下的虚拟模型列表
 *
 * 调用后端 `virtual_provider_model_list` 命令。
 */
export function useVirtualModels(virtualProviderId: string | null): {
  models: VirtualModel[]
  loading: boolean
  refetch: () => void
} {
  const [models, setModels] = useState<VirtualModel[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    if (!virtualProviderId) {
      setModels([])
      return
    }
    setLoading(true)
    try {
      const result = await invokeCommand<VirtualModel[]>('virtual_provider_model_list', {
        virtualProviderId,
      })
      setModels(result)
    } catch {
      setModels([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [virtualProviderId])

  return { models, loading, refetch: load }
}

/**
 * 获取指定虚拟模型下的路由列表
 *
 * 调用后端 `virtual_provider_route_list` 命令。
 */
export function useVirtualRoutes(virtualModelId: string | null): {
  routes: VirtualModelRoute[]
  loading: boolean
  refetch: () => void
} {
  const [routes, setRoutes] = useState<VirtualModelRoute[]>([])
  const [loading, setLoading] = useState(false)

  const load = useCallback(async () => {
    if (!virtualModelId) {
      setRoutes([])
      return
    }
    setLoading(true)
    try {
      const result = await invokeCommand<VirtualModelRoute[]>('virtual_provider_route_list', {
        virtualModelId,
      })
      setRoutes(result)
    } catch {
      setRoutes([])
    } finally {
      setLoading(false)
    }
  }, [virtualModelId])

  useEffect(() => {
    void load()
  }, [load])

  return { routes, loading, refetch: load }
}

/**
 * 获取指定虚拟供应商下所有已启用路由
 *
 * 用于「虚拟模型关系图」一次性渲染该供应商全部虚拟模型及其子级路由。
 */
export function useVirtualRoutesByProvider(virtualProviderId: string | null): {
  routes: VirtualModelRoute[]
  loading: boolean
  refetch: () => void
} {
  const [routes, setRoutes] = useState<VirtualModelRoute[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    if (!virtualProviderId) {
      setRoutes([])
      return
    }
    setLoading(true)
    try {
      const result = await invokeCommand<VirtualModelRoute[]>('virtual_provider_routes_by_provider', {
        virtualProviderId,
      })
      console.log('[useVirtualRoutesByProvider] virtualProviderId:', virtualProviderId, 'routes:', result.length, result)
      setRoutes(result)
    } catch (err) {
      console.error('[useVirtualRoutesByProvider] error:', err)
      setRoutes([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [virtualProviderId])

  return { routes, loading, refetch: load }
}

/**
 * 获取所有对外暴露的真实供应商模型
 *
 * 用于模型穿梭框候选池，仅返回已启用且已暴露的供应商模型。
 */
export function useExposedModels(): {
  models: ExposedModel[]
  loading: boolean
  refetch: () => void
} {
  const [models, setModels] = useState<ExposedModel[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    setLoading(true)
    try {
      const result = await invokeCommand<ExposedModel[]>('gateway_exposed_models')
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
 * 获取所有真实供应商模型（包含隐藏模型）
 *
 * 用于虚拟供应商模型穿梭框候选池，不限制 is_exposed，
 * 以便用户选择未对外暴露的模型作为故障转移目标。
 */
export function useAllModels(): {
  models: ExposedModel[]
  loading: boolean
  refetch: () => void
} {
  const [models, setModels] = useState<ExposedModel[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    setLoading(true)
    try {
      const result = await invokeCommand<ExposedModel[]>('gateway_all_models')
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
 * 获取指定真实供应商下的网关模型列表
 *
 * 用于在虚拟模型路由中选择目标模型。
 */
export function useProviderGatewayModels(providerId: string | null): {
  models: Array<{ id: string; modelId: string; displayName?: string }>
  loading: boolean
  refetch: () => void
} {
  const [models, setModels] = useState<Array<{ id: string; modelId: string; displayName?: string }>>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    if (!providerId) {
      setModels([])
      return
    }
    setLoading(true)
    try {
      const result = await invokeCommand<Array<{ id: string; modelId: string; displayName?: string }>>(
        'gateway_model_list_by_provider',
        { providerId }
      )
      setModels(result)
    } catch {
      setModels([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [providerId])

  return { models, loading, refetch: load }
}
