import { createContext, useCallback, useEffect, useMemo, useState, type ReactNode } from 'react'
import type { AppTheme, ThemeContextValue, ThemeState } from './types'

// 主题 CSS 类名前缀，用于切换不同主题样式
const THEME_CLASS_PREFIX = 'theme-'
// 本地存储键名，持久化用户选择的主题
const STORAGE_KEY = 'icode-theme'

export const ThemeContext = createContext<ThemeContextValue | null>(null)

// 应用支持的主题列表：基础明暗、Claude 风格、DeepSeek 风格
const themes: AppTheme[] = [
  'light',
  'dark',
  'claude-light',
  'claude-dark',
  'deepseek-light',
  'deepseek-dark',
]

// 获取初始主题：优先使用本地缓存，否则根据系统暗色模式偏好决定
function getInitialTheme(): AppTheme {
  if (typeof window === 'undefined') return 'dark'
  const stored = localStorage.getItem(STORAGE_KEY) as AppTheme | null
  if (stored && themes.includes(stored)) return stored
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
  return prefersDark ? 'dark' : 'light'
}

export function ThemeProvider({ children, defaultTheme }: { children: ReactNode; defaultTheme?: AppTheme }) {
  const [state, setState] = useState<ThemeState>(() => ({
    theme: defaultTheme ?? getInitialTheme(),
    systemTheme: null,
  }))

  // 应用主题：清除旧主题类名并添加新主题类名，同时设置 color-scheme 以同步系统 UI
  const applyTheme = useCallback((theme: AppTheme) => {
    const root = document.documentElement
    for (const t of themes) {
      root.classList.remove(`${THEME_CLASS_PREFIX}${t}`)
    }
    root.classList.add(`${THEME_CLASS_PREFIX}${theme}`)
    root.style.colorScheme = theme.includes('dark') ? 'dark' : 'light'
  }, [])

  // 主题变化时：应用样式并持久化到本地存储
  useEffect(() => {
    applyTheme(state.theme)
    localStorage.setItem(STORAGE_KEY, state.theme)
  }, [state.theme, applyTheme])

  // 监听系统暗色模式变化，用于显示或后续自动切换参考
  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const handleChange = (e: MediaQueryListEvent) => {
      setState((prev) => ({ ...prev, systemTheme: e.matches ? 'dark' : 'light' }))
    }
    setState((prev) => ({ ...prev, systemTheme: media.matches ? 'dark' : 'light' }))
    media.addEventListener('change', handleChange)
    return () => media.removeEventListener('change', handleChange)
  }, [])

  // 手动设置当前主题
  const setTheme = useCallback((theme: AppTheme) => {
    setState((prev) => ({ ...prev, theme }))
  }, [])

  // 循环切换到下一个可用主题
  const toggleTheme = useCallback(() => {
    setState((prev) => {
      const idx = themes.indexOf(prev.theme)
      const next = themes[(idx + 1) % themes.length]
      return { ...prev, theme: next }
    })
  }, [])

  // 组合 Context 值，避免每次渲染都产生新对象
  const value = useMemo<ThemeContextValue>(
    () => ({
      ...state,
      setTheme,
      toggleTheme,
      themes,
    }),
    [state, setTheme, toggleTheme]
  )

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
}
