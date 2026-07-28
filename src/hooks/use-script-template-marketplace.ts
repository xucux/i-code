/**
 * 脚本模板市场：列表 / 详情 / 预览
 */

import { useCallback, useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import type {
  MarketplaceItemDetail,
  MarketplaceListFilter,
  MarketplaceListResult,
  MarketplaceScriptPreview,
} from '@/modules/script-template/types'

export async function listMarketplace(
  filter: MarketplaceListFilter = {}
): Promise<MarketplaceListResult> {
  return invokeCommand<MarketplaceListResult>('script_template_marketplace_list', {
    kind: filter.kind,
    keyword: filter.keyword,
    forceRefresh: filter.forceRefresh ?? false,
  })
}

export async function getMarketplaceItem(
  id: string,
  includeScript = false
): Promise<MarketplaceItemDetail> {
  return invokeCommand<MarketplaceItemDetail>('script_template_marketplace_get', {
    id,
    includeScript,
  })
}

export async function previewMarketplaceScript(
  id: string
): Promise<MarketplaceScriptPreview> {
  return invokeCommand<MarketplaceScriptPreview>(
    'script_template_marketplace_preview_script',
    { id }
  )
}

/** 市场列表 hook */
export function useScriptTemplateMarketplace(filter: MarketplaceListFilter = {}): {
  result: MarketplaceListResult | null
  items: MarketplaceListResult['items']
  loading: boolean
  error: string | null
  refetch: (force?: boolean) => void
} {
  const [result, setResult] = useState<MarketplaceListResult | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(
    async (force = false) => {
      setLoading(true)
      setError(null)
      try {
        const data = await listMarketplace({
          kind: filter.kind,
          keyword: filter.keyword,
          forceRefresh: force,
        })
        setResult(data)
      } catch (err) {
        setResult(null)
        const message =
          err && typeof err === 'object' && 'message' in err
            ? String((err as { message: unknown }).message)
            : String(err)
        setError(message)
      } finally {
        setLoading(false)
      }
    },
    [filter.kind, filter.keyword]
  )

  useEffect(() => {
    void load(false)
  }, [load])

  return {
    result,
    items: result?.items ?? [],
    loading,
    error,
    refetch: (force = true) => {
      void load(force)
    },
  }
}
