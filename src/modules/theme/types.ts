import type { Theme } from '@/core/types'

export type AppTheme = Theme

export interface ThemeState {
  theme: AppTheme
  systemTheme: 'light' | 'dark' | null
}

export interface ThemeContextValue extends ThemeState {
  setTheme: (theme: AppTheme) => void
  toggleTheme: () => void
  themes: AppTheme[]
}
