import { useTranslation as useI18nTranslation } from 'react-i18next'
import type { Namespace } from './types'

export function useTranslation(ns?: Namespace) {
  const { t: baseT, ...rest } = useI18nTranslation()

  const t = (key: string, options?: Record<string, unknown> | string) => {
    const optionsObj = typeof options === 'string' ? { defaultValue: options } : options
    const nsOverride = optionsObj?.ns as string | undefined
    const fullKey = nsOverride ? `${nsOverride}.${key}` : ns ? `${ns}.${key}` : key
    // ns 仅用于拼接 key 前缀，不能透传给 i18next，否则会被当作命名空间查找
    //（项目只有 translation 命名空间，common / virtualProvider 等均为其下的顶层 key）
    const { ns: _ns, ...restOptions } = optionsObj ?? {}
    return baseT(fullKey, restOptions)
  }

  return { ...rest, t }
}
