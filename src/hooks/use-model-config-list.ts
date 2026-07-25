import { useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import type { ModelConfig } from '@/modules/ai-gateway/types'

/**
 * 获取 AI Gateway 模型配置列表
 *
 * 调用后端 `gateway_model_config_list` 命令。
 */
export function useModelConfigList(): {
  configs: ModelConfig[]
  loading: boolean
  refetch: () => void
} {
  const [configs, setConfigs] = useState<ModelConfig[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    setLoading(true)
    try {
      const result = await invokeCommand<ModelConfig[]>('gateway_model_config_list')
      setConfigs(result)
    } catch {
      setConfigs([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [])

  return { configs, loading, refetch: load }
}
