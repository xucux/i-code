/**
 * 发帖弹层（§8.3 顶部「发帖」按钮）
 *
 * 标题 ≤ 80 字、正文 ≤ 10000 字，前后端双重校验（§6 字数限制）。
 * 正文支持 Markdown：编辑 / 预览双 Tab，预览实时渲染（GFM + GitHub Alert）。
 */

import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { HelpIcon } from '@/components/ui/help-icon'
import { cn } from '@/lib/utils'
import { MarkdownEditor } from './markdown-editor'
import { useDialogFullscreen } from './use-dialog-fullscreen'
import {
  COMMUNITY_SECTIONS,
  type CommunitySection,
} from '@/modules/community/types'

/** 字数上限（与 Worker / Rust 侧一致） */
const TITLE_MAX = 80
const CONTENT_MAX = 10000

/** 板块图标（与 SectionBadge 一致） */
const SECTION_ICON: Record<CommunitySection, string> = {
  chat: 'fa-comments',
  eggs: 'fa-egg',
  tech: 'fa-laptop-code',
}

export interface CreatePostDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 默认板块（通常取自当前板块 Tab） */
  defaultSection: CommunitySection
  /** 提交发帖（调用方执行 create_post），成功后由调用方关闭并刷新列表 */
  onSubmit: (title: string, content: string, section: CommunitySection) => Promise<unknown>
}

export function CreatePostDialog({ open, onOpenChange, defaultSection, onSubmit }: CreatePostDialogProps) {
  const { t } = useTranslation('community')
  const [title, setTitle] = useState('')
  const [content, setContent] = useState('')
  const [section, setSection] = useState<CommunitySection>(defaultSection)
  const [submitting, setSubmitting] = useState(false)
  // 系统全屏（Fullscreen API + CSS 回退），放大整个弹窗 / 关闭时自动退出
  const { expanded, toggleFullscreen } = useDialogFullscreen(open)

  // 每次打开重置表单（回到编辑 Tab，板块回退到入口时的默认板块）
  useEffect(() => {
    if (open) {
      setTitle('')
      setContent('')
      setSection(defaultSection)
    }
  }, [open, defaultSection])

  const titleValid = title.trim().length > 0 && title.trim().length <= TITLE_MAX
  const contentValid = content.trim().length > 0 && content.length <= CONTENT_MAX

  const handleSubmit = async () => {
    if (!titleValid || !contentValid || submitting) return
    setSubmitting(true)
    try {
      await onSubmit(title.trim(), content.trim(), section)
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className={cn(
          'flex max-w-lg flex-col h-[98vh]',
          expanded &&
            '!fixed !inset-0 !left-0 !top-0 !h-screen !w-screen !max-h-none !max-w-none !translate-x-0 !translate-y-0 !rounded-none border-0 !overflow-hidden'
        )}
      >
        {/* 整窗全屏放大 / 还原（置于 close 按钮左侧，参考脚本编辑器） */}
        <button
          type="button"
          title={expanded ? t('editor.restore') : t('editor.expand')}
          aria-label={expanded ? t('editor.restore') : t('editor.expand')}
          onClick={() => void toggleFullscreen()}
          className="absolute right-11 top-4 rounded-sm opacity-70 ring-offset-background transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
        >
          <i className={cn('fa-solid h-4 w-4', expanded ? 'fa-compress' : 'fa-expand')} />
        </button>

        <DialogHeader className="shrink-0 pr-14 flex flex-row items-center">
          <DialogTitle className="text-sm">{t('post.createTitle')}</DialogTitle>
          {/* <DialogDescription className="text-xs">{t('post.createDesc')}</DialogDescription> */}
          {/* 字数限制提示：helpicon 形式置于标题右侧 */}
          <HelpIcon size="sm" type="popover" trigger="click" side="bottom" align="start" contentClassName="max-w-xs text-xs leading-relaxed">
            <p>{t('post.createDesc')}</p>
          </HelpIcon>
        </DialogHeader>

        <div className={cn('min-h-0', expanded ? 'flex flex-1 flex-col gap-1.5 overflow-hidden' : 'space-y-1.5 py-1')}>
          {/* 板块选择（固定三板块，默认取当前 Tab） */}
          <div className={cn('space-y-1', expanded && 'shrink-0')}>
            <Label className="text-xs">{t('post.sectionLabel')}</Label>
            <div className="flex gap-1">
              {COMMUNITY_SECTIONS.map((s) => (
                <button
                  key={s}
                  type="button"
                  title={t(`section.${s}`)}
                  className={cn(
                    'flex h-7 flex-1 items-center justify-center gap-1 rounded-md border px-2 text-xs transition-colors',
                    section === s
                      ? 'border-primary bg-primary text-primary-foreground'
                      : 'bg-background text-muted-foreground hover:bg-muted hover:text-foreground'
                  )}
                  onClick={() => setSection(s)}
                >
                  <i className={cn('fa-solid size-3', SECTION_ICON[s])} />
                  {t(`section.${s}`)}
                </button>
              ))}
            </div>
          </div>

          <div className={cn('space-y-1', expanded && 'shrink-0')}>
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-0.5">
                <Label htmlFor="post-title" className="text-xs">
                  {t('post.titleLabel')}
                </Label>

              </div>
              <span className="text-muted-foreground text-[10px] tabular-nums">
                {title.trim().length}/{TITLE_MAX}
              </span>
            </div>
            <Input
              id="post-title"
              value={title}
              maxLength={TITLE_MAX}
              placeholder={t('post.titlePlaceholder')}
              onChange={(e) => setTitle(e.target.value)}
              className="h-8 text-xs"
            />
          </div>

          {/* 正文：Markdown 编辑 / 预览（含语法工具栏与代码折叠） */}
          <div className={cn('space-y-1', expanded ? 'flex min-h-0 flex-1 flex-col' : '')}>
            <div className="flex items-center justify-between">
              <Label htmlFor="post-content" className="text-xs">
                {t('post.contentLabel')}
              </Label>
              <span className="text-muted-foreground text-[10px] tabular-nums">
                {content.length}/{CONTENT_MAX}
              </span>
            </div>
            <MarkdownEditor
              id="post-content"
              value={content}
              onChange={setContent}
              maxLength={CONTENT_MAX}
              placeholder={t('post.contentPlaceholder')}
              heightClass="h-[50vh]"
              fill={expanded}
            />
          </div>
        </div>

        <DialogFooter className="shrink-0">
          <Button variant="outline" size="sm" className="h-8 text-xs" onClick={() => onOpenChange(false)}>
            {t('post.cancel')}
          </Button>
          <Button size="sm" className="h-8 text-xs" disabled={!titleValid || !contentValid || submitting} onClick={handleSubmit}>
            {submitting && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
            {t('post.submit')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
