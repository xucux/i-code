/**
 * 内置浏览器专属窗口（「在应用内打开」）
 *
 * 结构：顶部工具栏（返回 / 刷新 / 地址 / 外部浏览器打开）+ 下方 iframe 承载目标页面。
 * - 返回：调用后端关闭浏览器窗口并聚焦主窗口（回到应用）
 * - 刷新：通过变更 iframe 的 key 强制重新装载目标页面
 * - 若目标站点不允许被内嵌（X-Frame-Options 等），页面可能空白，此时可使用「外部浏览器打开」
 */
import { useCallback, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { invokeCommand } from '@/hooks/use-command'

export interface BrowserWindowProps {
  /** 需要在应用内打开的外部 URL */
  url: string
}

export function BrowserWindow({ url }: BrowserWindowProps) {
  const { t } = useTranslation('browser')
  /** 刷新计数：作为 iframe 的 key，key 变化即整体卸载并重新挂载以刷新内容 */
  const [reloadKey, setReloadKey] = useState(0)

  /** 返回：关闭浏览器窗口并聚焦主窗口 */
  const handleBack = useCallback(async () => {
    await invokeCommand<void>('close_browser_window')
  }, [])

  /** 刷新：重新装载 iframe */
  const handleRefresh = useCallback(() => {
    setReloadKey((k) => k + 1)
  }, [])

  /** 在系统默认浏览器中打开当前地址（iframe 受限时的兜底） */
  const handleOpenOutside = useCallback(async () => {
    await invokeCommand<void>('open_url', { url })
  }, [url])

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-background">
      {/* 顶部工具栏 */}
      <div className="flex h-10 shrink-0 items-center gap-1.5 border-b bg-card px-2">
        <Button
          variant="ghost"
          size="icon"
          className="text-muted-foreground size-8"
          title={t('back')}
          onClick={() => void handleBack()}
        >
          <i className="fa-solid fa-arrow-left size-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="text-muted-foreground size-8"
          title={t('refresh')}
          onClick={handleRefresh}
        >
          <i className="fa-solid fa-rotate size-3.5" />
        </Button>

        {/* 地址栏 */}
        <div className="min-w-0 flex-1 truncate rounded-md border bg-muted px-2 py-1 text-xs text-muted-foreground select-none">
          {url}
        </div>

        <Button
          variant="ghost"
          size="sm"
          className="text-muted-foreground h-8 gap-1.5"
          title={t('openOutside')}
          onClick={() => void handleOpenOutside()}
        >
          <i className="fa-solid fa-arrow-up-right-from-square size-3" />
          <span className="text-xs">{t('openOutside')}</span>
        </Button>
      </div>

      {/* 目标页面（iframe），跨源站点的内嵌受目标网站安全策略限制 */}
      <div className="min-h-0 flex-1">
        <iframe
          key={reloadKey}
          src={url}
          title={url}
          className="h-full w-full border-0 bg-white"
        />
      </div>
    </div>
  )
}