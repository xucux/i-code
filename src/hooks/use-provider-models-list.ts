import { useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import type { GatewayModel } from '@/modules/ai-gateway/types'

export function useProviderModelsList(providerId: string | undefined): {
  models: GatewayModel[]
  loading: boolean
  refetch: () => void
} {
  const [models, setModels] = useState<GatewayModel[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    if (!providerId) {
      setModels([])
      return
    }
    setLoading(true)
    try {
      const result = await invokeCommand<GatewayModel[]>('gateway_model_list_by_provider', {
        providerId,
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
  }, [providerId])

  return { models, loading, refetch: load }
}
