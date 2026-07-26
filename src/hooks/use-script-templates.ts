/**
 * 脚本模板读取 hooks
 */

import { useCallback, useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import type {
  ScriptSnippet,
  ScriptTemplate,
  ScriptTemplateListFilter,
  ScriptTemplateSelectItem,
} from '@/modules/script-template/types'

/** 脚本模板列表 */
export function useScriptTemplateList(filter: ScriptTemplateListFilter = {}): {
  templates: ScriptTemplate[]
  loading: boolean
  refetch: () => void
} {
  const [templates, setTemplates] = useState<ScriptTemplate[]>([])
  const [loading, setLoading] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const result = await invokeCommand<ScriptTemplate[]>('script_template_list', {
        kind: filter.kind,
        status: filter.status,
        keyword: filter.keyword,
      })
      setTemplates(result)
    } catch {
      setTemplates([])
    } finally {
      setLoading(false)
    }
  }, [filter.kind, filter.status, filter.keyword])

  useEffect(() => {
    void load()
  }, [load])

  return { templates, loading, refetch: load }
}

/** 启用中的额度脚本模板（供应商表单下拉） */
export function useActiveScriptTemplates(): {
  items: ScriptTemplateSelectItem[]
  loading: boolean
  refetch: () => void
} {
  const [items, setItems] = useState<ScriptTemplateSelectItem[]>([])
  const [loading, setLoading] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const result = await invokeCommand<ScriptTemplateSelectItem[]>(
        'script_template_list_active_for_select'
      )
      setItems(result)
    } catch {
      setItems([])
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  return { items, loading, refetch: load }
}

/** 内置 snippet */
export function useScriptSnippets(): {
  snippets: ScriptSnippet[]
  loading: boolean
} {
  const [snippets, setSnippets] = useState<ScriptSnippet[]>([])
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      setLoading(true)
      try {
        const result = await invokeCommand<ScriptSnippet[]>('script_template_list_snippets')
        if (!cancelled) setSnippets(result)
      } catch {
        if (!cancelled) setSnippets([])
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])

  return { snippets, loading }
}
