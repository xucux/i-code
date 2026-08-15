import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import * as DialogPrimitive from '@radix-ui/react-dialog'
import { Button } from '@/components/ui/button'
import { DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Separator } from '@/components/ui/separator'
import { useTranslation } from '@/modules/i18n/use-translation'
import { toIcodeError } from '@/core/errors'
import { cn } from '@/lib/utils'
import { MarkdownContent } from '@/components/ui/markdown-content'

const CHANGELOG_PAGE_URL = 'https://github.com/xucux/i-code/blob/main/CHANGELOG.md'

/** 弹窗加载状态 */
type LoadState =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'done'; content: string }
  | { status: 'error'; message: string }

/**
 * 清理 CHANGELOG 原始 Markdown：
 * - 移除开头 YAML frontmatter（editRules 配置块）
 * - 移除首个版本模板章节（`## [release-version-tempalte]` 及其空列表），
 *   保留其后第一个真实版本标题
 */
function cleanChangelog(md: string): string {
  let text = md
  // 移除 YAML frontmatter
  text = text.replace(/^---[\s\S]*?---\s*\n/, '')
  // 移除版本模板章节（截断到下一个 `## [` 标题或文件末尾）
  text = text.replace(/## \[release-version-tempalte\][\s\S]*?(?=## \[|$)/, '')
  return text.trim()
}

/**
 * 历史更新弹窗（受控组件）
 *
 * 打开时通过后端 `settings_fetch_changelog` 命令获取 GitHub 仓库
 * `CHANGELOG.md` 原始 Markdown（走全局代理配置），清理 frontmatter 与
 * 版本模板章节后渲染为可滚动内容。
 * 底部提供「在 GitHub 查看」入口，跳转仓库内 CHANGELOG 页面。
 * 刻意去掉右上角 X 关闭按钮，统一由底部「关闭」按钮关闭。
 */
export interface ChangelogDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function ChangelogDialog({ open, onOpenChange }: ChangelogDialogProps) {
  const { t } = useTranslation()
  const [state, setState] = useState<LoadState>({ status: 'idle' })

  // 打开时拉取一次；重新打开时刷新内容
  useEffect(() => {
    if (!open) return
    let cancelled = false

    setState((prev) => (prev.status === 'done' ? prev : { status: 'loading' }))
    invoke<string>('settings_fetch_changelog')
      .then((content) => {
        if (!cancelled) setState({ status: 'done', content: cleanChangelog(content) })
      })
      .catch((err) => {
        if (!cancelled) {
          setState({ status: 'error', message: toIcodeError(err).message })
        }
      })

    return () => {
      cancelled = true
    }
  }, [open])

  // 手动刷新
  const refresh = useCallback(() => {
    setState({ status: 'loading' })
    invoke<string>('settings_fetch_changelog')
      .then((content) => setState({ status: 'done', content: cleanChangelog(content) }))
      .catch((err) => setState({ status: 'error', message: toIcodeError(err).message }))
  }, [])

  // 在 GitHub 打开 CHANGELOG 页面
  const openOnGithub = useCallback(() => {
    invoke('open_url', { url: CHANGELOG_PAGE_URL })
    onOpenChange(false)
  }, [onOpenChange])

  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/80 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0" />
        <DialogPrimitive.Content
          className={cn(
            'fixed left-[50%] top-[50%] z-50 grid w-full max-w-lg translate-x-[-50%] translate-y-[-50%] border bg-background shadow-lg duration-200',
            'data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0',
            'data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95',
            'data-[state=closed]:slide-out-to-left-1/2 data-[state=closed]:slide-out-to-top-[48%]',
            'data-[state=open]:slide-in-from-left-1/2 data-[state=open]:slide-in-from-top-[48%] sm:rounded-lg',
            'flex max-h-[min(560px,78vh)] flex-col gap-0 p-0'
          )}
        >
          {/* 标题区（居中，无图标） */}
          <DialogHeader className="px-4 py-1">
            <DialogTitle className="text-center text-sm">
              {t('settings.about.changelogTitle')}
            </DialogTitle>
          </DialogHeader>

          <Separator />

          {/* 加载中 */}
          {state.status === 'loading' && (
            <div className="flex items-center justify-center gap-2 px-4 py-8 text-xs text-muted-foreground">
              <i className="fa-solid fa-spinner fa-spin text-[10px]" />
              {t('settings.about.changelogLoading')}
            </div>
          )}

          {/* 加载失败 */}
          {state.status === 'error' && (
            <div className="mx-4 mt-3 flex items-start gap-2 rounded-md border border-red-200 bg-red-50 p-2 text-xs text-red-700 dark:border-red-800 dark:bg-red-950 dark:text-red-300">
              <i className="fa-solid fa-circle-exclamation mt-0.5 shrink-0 text-[10px]" />
              <span>{state.message}</span>
            </div>
          )}

          {/* 更新历史内容 */}
          {state.status === 'done' && (
            <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3 custom-scrollbar">
              <MarkdownContent content={state.content} />
            </div>
          )}

          {/* 底部按钮 */}
          <div className="flex items-center justify-between gap-2 border-t px-4 py-2">
            <Button
              variant="outline"
              size="sm"
              onClick={refresh}
              disabled={state.status === 'loading'}
              title={t('settings.about.changelogRefresh')}
            >
              <i className={cn('fa-solid fa-rotate mr-1.5', state.status === 'loading' && 'animate-spin')} />
              {t('settings.about.changelogRefresh')}
            </Button>
            <div className="flex items-center gap-2">
              <Button variant="outline" size="sm" onClick={openOnGithub}>
                <i className="fa-brands fa-github mr-1.5" />
                {t('settings.about.changelogOpenGithub')}
              </Button>
              <Button size="sm" onClick={() => onOpenChange(false)}>
                {t('common.close')}
              </Button>
            </div>
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  )
}

/**
 * 「查看历史更新」入口（文字按钮 + 弹窗）
 *
 * 展示在设置页「关于和更新」版本行最右侧（检查更新按钮之后、
 * 以竖分隔线隔开），点击后拉取并渲染 GitHub 仓库 `CHANGELOG.md`。
 */
export function ChangelogButton() {
  const { t } = useTranslation()
  const [dialogOpen, setDialogOpen] = useState(false)

  return (
    <>
      <Button
        variant="ghost"
        size="sm"
        className="h-6 px-1.5 text-[11px] text-primary hover:text-primary/80"
        onClick={() => setDialogOpen(true)}
        title={t('settings.about.changelog')}
      >
        {t('settings.about.changelog')}
      </Button>
      <ChangelogDialog open={dialogOpen} onOpenChange={setDialogOpen} />
    </>
  )
}