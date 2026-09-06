import React, { useEffect } from 'react'
import ReactDOM from 'react-dom/client'
import { RouterProvider, createRouter } from '@tanstack/react-router'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { ThemeProvider } from '@/modules/theme/theme-provider'
import { registerConsoleLogForwarder } from '@/core/events'
import { registerImagebedEvents } from '@/modules/imagebed/store'
import '@/modules/i18n/i18n'
import '@/index.css'

import { routeTree } from './routeTree.gen'

const router = createRouter({ routeTree })

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

/**
 * 应用根组件
 *
 * 在此注册全局副作用（如后端日志转发到 DevTools 控制台），
 * 而非散落在各路由组件中。
 */
function App() {
  // 注册后端 tracing 日志转发监听器，将 Rust 侧 log::info! 等日志输出到 DevTools 控制台。
  // 替代 tauri-plugin-log 的 Webview 目标。
  useEffect(() => {
    let unlisten: UnlistenFn | undefined
    registerConsoleLogForwarder().then((fn) => {
      unlisten = fn
    })
    // 注册图床外链事件监听（幂等），社区编辑器自动插入上传外链
    registerImagebedEvents()
    return () => {
      unlisten?.()
    }
  }, [])

  return (
    <ThemeProvider>
      <RouterProvider router={router} />
    </ThemeProvider>
  )
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)
