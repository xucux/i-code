/**
 * 内置浏览器专属窗口路由（/browser）
 *
 * 由后端 open_browser_window 命令打开独立 webview 窗口时加载，
 * 通过查询参数 `url` 传入目标地址，渲染 BrowserWindow（顶部返回/刷新 + iframe）。
 * __root 中对该路由跳过标题栏与侧栏，保证全屏展示（类似 mini-panel）。
 */
import { createFileRoute } from '@tanstack/react-router'
import { BrowserWindow } from '@/modules/browser/ui/browser-window'

interface BrowserSearch {
  /** 需要内嵌打开的外部 URL（百分比编码） */
  url?: string
}

export const Route = createFileRoute('/browser')({
  validateSearch: (search: Record<string, unknown>): BrowserSearch => ({
    url: typeof search.url === 'string' ? search.url : '',
  }),
  component: BrowserPage,
})

function BrowserPage() {
  const { url } = Route.useSearch()
  return <BrowserWindow url={url ?? ''} />
}