import type { Locale } from '@/core/types'

export type AppLocale = Locale

export type Namespace =
  | 'common'
  | 'settings'
  | 'aiGateway'
  | 'cli'
  | 'workspace'
  | 'virtualProvider'
  | 'scriptTemplate'
  | 'theme'
  | 'preview'
  | 'dashboard'
  | 'date'
  | 'autoRefresh'
  | 'logger'
  | 'chat'

export interface I18nState {
  locale: AppLocale
}
