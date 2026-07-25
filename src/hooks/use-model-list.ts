import { useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import type { GatewayModel } from '@/modules/ai-gateway/types'

/**
 * 获取 AI Gateway 暴露模型列表
 *
 * 调用后端 `gateway_model_list` 命令。
 */
export function useModelList(): { models: GatewayModel[]; loading: boolean; refetch: () => void } {
  const [models, setModels] = useState<GatewayModel[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    setLoading(true)
    try {
      const result = await invokeCommand<GatewayModel[]>('gateway_model_list')
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
