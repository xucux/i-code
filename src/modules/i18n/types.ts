import type { Locale } from '@/core/types'

export type AppLocale = Locale

export type Namespace =
  | 'common'
  | 'settings'
  | 'aiGateway'
  | 'cli'
  | 'virtualProvider'
  | 'scriptTemplate'
  | 'community'
  | 'theme'
  | 'preview'
  | 'dashboard'
  | 'date'
  | 'autoRefresh'
  | 'logger'
  | 'chat'
  | 'browser'
  | 'vision'

export interface I18nState {
  locale: AppLocale
}
