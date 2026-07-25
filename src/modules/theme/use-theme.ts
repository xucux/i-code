import { useContext } from 'react'
import { ThemeContext } from './theme-provider'

// 自定义 Hook：在 ThemeProvider 范围内获取当前主题与切换方法
export function useTheme() {
  const context = useContext(ThemeContext)
  if (!context) {
    throw new Error('useTheme must be used within a ThemeProvider')
  }
  return context
}
