import { useCallback, useEffect, useState } from 'react'
import { getVersion } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Progress } from '@/components/ui/progress'
import { Separator } from '@/components/ui/separator'
import { useTranslation } from '@/modules/i18n/use-translation'
import { BACKEND_EVENTS } from '@/core/events'
import { cn } from '@/lib/utils'
import { toast } from 'sonner'
import { MarkdownContent } from './markdown-content'

const GITHUB_REPO = 'xucux/i-code'
const RELEASES_URL = `https://github.com/${GITHUB_REPO}/releases/latest`

interface CheckUpdateResult {
  has_update: boolean
  is_beta: boolean
  current_version: string
  latest_version: string
  notes: string
  pub_date: string
  platforms: Record<string, { signature: string; url: string }>
}

interface DownloadProgress {
  downloaded: number
  total: number
}

/**
 * 检测当前运行平台，在 platforms 中找到最佳匹配的下载条目
 *
 * 优先匹配格式后缀键（如 `windows-x86_64-nsis`、`linux-x86_64-appimage`），
 * 未命中时回退到基础平台键（如 `windows-x86_64`）。
 */
function resolvePlatformDownload(
  platforms: Record<string, { signature: string; url: string }>
): { key: string; url: string } | null {
  const ua = navigator.userAgent.toLowerCase()
  let base = ''
  const preferredSuffixes: string[] = []

  if (ua.includes('win')) {
    base = 'windows-x86_64'
    preferredSuffixes.push('nsis', 'msi')
  } else if (ua.includes('mac')) {
    // macOS：优先 .app.tar.gz（更新器标准格式），其次 .dmg
    const isArm = ua.includes('arm64') || ua.includes('aarch64')
    base = isArm ? 'darwin-aarch64' : 'darwin-x86_64'
    preferredSuffixes.push('app', 'dmg')
  } else if (ua.includes('linux')) {
    base = 'linux-x86_64'
    preferredSuffixes.push('appimage', 'deb', 'rpm')
  } else {
    return null
  }

  // 优先匹配格式后缀键
  for (const suffix of preferredSuffixes) {
    const key = `${base}-${suffix}`
    if (platforms[key]) {
      return { key, url: platforms[key].url }
    }
  }

  // 回退到基础平台键
  if (platforms[base]) {
    return { key: base, url: platforms[base].url }
  }

  return null
}

/**
 * 从 URL 中提取文件名（用于后端保存下载文件）
 */
function extractFileName(url: string): string {
  const parts = url.split('/')
  return decodeURIComponent(parts[parts.length - 1] || 'update-installer')
}

function stripV(v: string): string {
  return v.replace(/^v/i, '')
}

/**
 * 更新弹窗下载状态
 */
type DownloadState =
  | { status: 'idle' }
  | { status: 'downloading'; downloaded: number; total: number }
  | { status: 'done' }
  | { status: 'error'; message: string }

/**
 * 更新检查弹窗（受控组件）
 *
 * 供设置页面「检查更新」入口与标题栏更新指示器复用，
 * 由父级控制 open 状态并传入检查结果。
 *
 * 底部三个功能按钮：
 *   - 取消：关闭弹窗
 *   - 打开下载页：跳转 GitHub Release 页面
 *   - 下载更新：自动下载当前平台安装包，完成后触发安装
 */
interface UpdateCheckDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  result: CheckUpdateResult | null
  currentVersion: string
}

export function UpdateCheckDialog({ open, onOpenChange, result, currentVersion }: UpdateCheckDialogProps) {
  const { t } = useTranslation()
  const hasUpdate = result?.has_update ?? false
  const [dlState, setDlState] = useState<DownloadState>({ status: 'idle' })

  // 打开下载页
  const openDownloadPage = useCallback(() => {
    invoke('open_url', { url: RELEASES_URL })
    onOpenChange(false)
  }, [onOpenChange])

  // 下载并安装
  const startDownload = useCallback(async () => {
    if (!result?.platforms) return

    const match = resolvePlatformDownload(result.platforms)
    if (!match) {
      setDlState({ status: 'error', message: t('settings.about.downloadUnsupported') })
      return
    }

    setDlState({ status: 'downloading', downloaded: 0, total: 0 })

    // 监听下载进度
    let unlisten: UnlistenFn | undefined
    try {
      unlisten = await listen<DownloadProgress>(BACKEND_EVENTS.UPDATE_DOWNLOAD_PROGRESS, (event) => {
        const { downloaded, total } = event.payload
        setDlState({ status: 'downloading', downloaded, total })
      })

      const fileName = extractFileName(match.url)
      await invoke('download_and_install_update', { url: match.url, fileName })

      // 安装程序已启动，显示完成状态，让用户手动关闭弹窗
      setDlState({ status: 'done' })
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      setDlState({ status: 'error', message: msg })
    } finally {
      if (unlisten) unlisten()
    }
  }, [result, onOpenChange, t])

  // 弹窗关闭时重置下载状态
  const handleOpenChange = useCallback((nextOpen: boolean) => {
    if (!nextOpen && dlState.status !== 'downloading') {
      setDlState({ status: 'idle' })
    }
    // 下载中不允许关闭弹窗
    if (!nextOpen && dlState.status === 'downloading') {
      return
    }
    onOpenChange(nextOpen)
  }, [dlState.status, onOpenChange])

  const isBusy = dlState.status === 'downloading'
  const isDone = dlState.status === 'done'
  const downloadPercent = dlState.status === 'downloading' && dlState.total > 0
    ? Math.round((dlState.downloaded / dlState.total) * 100)
    : 0

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className=" gap-0 p-0 h-15">
        {/* 标题区 */}
        <DialogHeader className="px-4 py-1">
          <DialogTitle className="flex items-center gap-2 text-sm">
            <i className={cn(
              'fa-regular',
              hasUpdate ? 'fa-circle-up text-primary' : 'fa-circle-check text-primary'
            )} />
            {hasUpdate ? t('settings.about.updateAvailable') : t('settings.about.upToDate')}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {hasUpdate
              ? t('settings.about.updateDescription', {
                  current: stripV(currentVersion),
                  latest: stripV(result?.latest_version ?? ''),
                })
              : t('settings.about.upToDateDescription', {
                  current: stripV(currentVersion),
                })}
          </DialogDescription>
        </DialogHeader>

        <Separator />

        {/* 版本对比 */}
        {result && (
          <div className="px-4 pt-3">
            <div className="flex items-center justify-center gap-3 rounded-md border bg-muted/30 p-2.5">
              <div className="text-center">
                <div className="text-[10px] text-muted-foreground">{t('settings.about.currentVersion')}</div>
                <div className="mt-0.5 font-mono text-xs tabular-nums">{stripV(result.current_version)}</div>
              </div>
              <i className="fa-solid fa-arrow-right text-[10px] text-muted-foreground" />
              <div className="text-center">
                <div className="text-[10px] text-muted-foreground">{t('settings.about.latestVersion')}</div>
                <div className={cn(
                  'mt-0.5 font-mono text-xs tabular-nums',
                  hasUpdate ? 'font-semibold text-primary' : ''
                )}>
                  {stripV(result.latest_version)}
                  {result.is_beta && (
                    <span className="ml-1 rounded bg-amber-100 px-1 py-0.5 text-[10px] font-medium text-amber-700 dark:bg-amber-900 dark:text-amber-300">
                      Beta
                    </span>
                  )}
                </div>
              </div>
            </div>
          </div>
        )}

        {/* 预览版提示 */}
        {result?.is_beta && hasUpdate && (
          <div className="mx-4 mt-3 flex items-start gap-2 rounded-md border border-amber-200 bg-amber-50 p-2.5 text-xs text-amber-800 dark:border-amber-800 dark:bg-amber-950 dark:text-amber-200">
            <i className="fa-solid fa-triangle-exclamation mt-0.5 shrink-0 text-[10px]" />
            <span>{t('settings.about.betaWarning')}</span>
          </div>
        )}

        {/* 更新日志 */}
        {result?.notes && (
          <div className="px-4 py-3">
            <div className="max-h-52 overflow-y-auto rounded-md border p-3 custom-scrollbar">
              <MarkdownContent content={result.notes} />
            </div>
          </div>
        )}

        {result && !result.notes && (
          <p className="px-4 pt-3 text-xs text-muted-foreground">
            {t('settings.about.noReleaseNotes')}
          </p>
        )}

        {/* 下载进度 */}
        {isBusy && (
          <div className="px-4 pt-2">
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <i className="fa-solid fa-spinner fa-spin text-[10px]" />
              <span>
                {dlState.total > 0
                  ? t('settings.about.downloadingPercent', { percent: downloadPercent })
                  : t('settings.about.downloading')}
              </span>
            </div>
            <Progress value={downloadPercent} className="mt-1.5 h-1.5" />
          </div>
        )}

        {/* 安装程序已启动 */}
        {isDone && (
          <div className="px-4 pt-2">
            <div className="flex items-center gap-2 text-xs text-green-600 dark:text-green-400">
              <i className="fa-solid fa-circle-check text-[10px]" />
              <span>{t('settings.about.downloadDone')}</span>
            </div>
            <Progress value={100} className="mt-1.5 h-1.5" />
          </div>
        )}

        {/* 下载错误提示 */}
        {dlState.status === 'error' && (
          <div className="mx-4 mt-2 flex items-start gap-2 rounded-md border border-red-200 bg-red-50 p-2 text-xs text-red-700 dark:border-red-800 dark:bg-red-950 dark:text-red-300">
            <i className="fa-solid fa-circle-exclamation mt-0.5 shrink-0 text-[10px]" />
            <span>{dlState.message}</span>
          </div>
        )}

        {/* 底部按钮 */}
        <div className="flex items-center justify-between gap-2 border-t px-4 py-2">
          {/* 完成状态：单个关闭按钮 */}
          {isDone && (
            <div className="flex w-full justify-end">
              <Button size="sm" onClick={() => onOpenChange(false)}>
                {t('common.close')}
              </Button>
            </div>
          )}

          {/* 非完成状态：三个功能按钮 */}
          {!isDone && (
            <>
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => onOpenChange(false)}
                  disabled={isBusy}
                >
                  {t('common.cancel')}
                </Button>
                {hasUpdate && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={openDownloadPage}
                    disabled={isBusy}
                  >
                    <i className="fa-solid fa-arrow-up-right-from-square mr-1.5 text-xs" />
                    {t('settings.about.openDownloadPage')}
                  </Button>
                )}
              </div>
              {hasUpdate && (
                <Button
                  size="sm"
                  onClick={startDownload}
                  disabled={isBusy}
                >
                  <i className={cn(
                    'mr-1.5 text-xs',
                    isBusy ? 'fa-solid fa-spinner fa-spin' : 'fa-solid fa-download'
                  )} />
                  {isBusy
                    ? t('settings.about.downloading')
                    : t('settings.about.downloadUpdate')}
                </Button>
              )}
            </>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}

/** 设置页面「检查更新」入口（按钮 + 弹窗） */
export function UpdateCheck() {
  const { t } = useTranslation()
  const [currentVersion, setCurrentVersion] = useState('')
  const [result, setResult] = useState<CheckUpdateResult | null>(null)
  const [hasUpdate, setHasUpdate] = useState(false)
  const [checking, setChecking] = useState(false)
  const [dialogOpen, setDialogOpen] = useState(false)

  useEffect(() => {
    getVersion()
      .then(setCurrentVersion)
      .catch(() => setCurrentVersion('0.0.1'))
  }, [])

  const checkUpdate = useCallback(async (showDialog: boolean) => {
    setChecking(true)
    try {
      const res = await invoke<CheckUpdateResult>('check_update')
      setResult(res)
      setHasUpdate(res.has_update)
      if (showDialog) {
        setDialogOpen(true)
      }
    } catch (err) {
      setHasUpdate(false)
      if (showDialog) {
        const msg = err instanceof Error ? err.message : String(err)
        toast.error(t('settings.about.checkUpdateFailed'), {
          description: msg,
        })
      }
    } finally {
      setChecking(false)
    }
  }, [])

  useEffect(() => {
    checkUpdate(false)
  }, [checkUpdate])

  return (
    <>
      {/* 更新检查按钮 */}
      <Button
        variant="ghost"
        size="icon"
        className="relative size-6 text-primary hover:text-primary/80"
        onClick={() => checkUpdate(true)}
        disabled={checking}
        title={t('settings.about.checkUpdate')}
      >
        <i className={`fa-regular fa-circle-up ${checking ? 'fa-spin' : ''}`} />
        {hasUpdate && (
          <span className="absolute -right-0.5 -top-0.5 size-2 rounded-full bg-red-500" />
        )}
      </Button>

      {/* 更新弹窗 */}
      <UpdateCheckDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        result={result}
        currentVersion={currentVersion}
      />
    </>
  )
}

/**
 * 标题栏更新指示器
 *
 * 监听后端启动期推送的 `update-check-result` 事件：
 * - 当 `has_update=true` 且 `is_beta=false` 时，在 i-code 标题右侧展示更新 icon；
 * - 点击 icon 打开复用的 `UpdateCheckDialog`；
 * - 当事件返回无更新或最新版本为 beta 时，icon 自动隐藏（实现自动关闭）。
 */
export function UpdateCheckIndicator() {
  const { t } = useTranslation()
  const [result, setResult] = useState<CheckUpdateResult | null>(null)
  const [dialogOpen, setDialogOpen] = useState(false)

  useEffect(() => {
    let unlisten: UnlistenFn | undefined
    listen<CheckUpdateResult>(BACKEND_EVENTS.UPDATE_CHECK_RESULT, (event) => {
      setResult(event.payload)
    }).then((fn) => {
      unlisten = fn
    })
    return () => {
      if (unlisten) unlisten()
    }
  }, [])

  const showIcon = (result?.has_update ?? false) && !(result?.is_beta ?? false)
  if (!showIcon) return null

  return (
    <>
      <button
        type="button"
        onClick={() => setDialogOpen(true)}
        className="flex items-center justify-center rounded p-0.5 text-primary transition-colors hover:text-primary/80"
        title={t('settings.about.updateAvailable')}
        aria-label={t('settings.about.updateAvailable')}
      >
        <i className="fa-regular fa-circle-up text-xs" />
      </button>
      <UpdateCheckDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        result={result}
        currentVersion={result?.current_version ?? ''}
      />
    </>
  )
}