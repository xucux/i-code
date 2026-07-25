import { useTranslation as useI18nTranslation } from 'react-i18next'
import type { Namespace } from './types'

export function useTranslation(ns?: Namespace) {
  const { t: baseT, ...rest } = useI18nTranslation()

  const t = (key: string, options?: Record<string, unknown> | string) => {
    const optionsObj = typeof options === 'string' ? { defaultValue: options } : options
    const nsOverride = optionsObj?.ns as string | undefined
    const fullKey = nsOverride ? `${nsOverride}.${key}` : ns ? `${ns}.${key}` : key
    return baseT(fullKey, optionsObj ?? {})
  }

  return { ...rest, t }
}
