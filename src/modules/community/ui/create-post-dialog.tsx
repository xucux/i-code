/**
 * 发帖弹层（§8.3 顶部「发帖」按钮）
 *
 * 标题 ≤ 80 字、正文 ≤ 5000 字，前后端双重校验（§6 字数限制）。
 * 正文支持 Markdown：编辑 / 预览双 Tab，预览实时渲染（GFM + GitHub Alert）。
 */

import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { MarkdownContent } from '@/components/ui/markdown-content'
import { cn } from '@/lib/utils'
import {
  COMMUNITY_SECTIONS,
  type CommunitySection,
} from '@/modules/community/types'

/** 字数上限（与 Worker / Rust 侧一致） */
const TITLE_MAX = 80
const CONTENT_MAX = 5000

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
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="text-sm">{t('post.createTitle')}</DialogTitle>
          <DialogDescription className="text-xs">{t('post.createDesc')}</DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2">
          {/* 板块选择（固定三板块，默认取当前 Tab） */}
          <div className="space-y-1">
            <Label className="text-xs">{t('post.sectionLabel')}</Label>
            <div className="flex gap-1.5">
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

          <div className="space-y-1">
            <div className="flex items-center justify-between">
              <Label htmlFor="post-title" className="text-xs">
                {t('post.titleLabel')}
              </Label>
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

          {/* 正文：Markdown 编辑 / 预览 */}
          <div className="space-y-1">
            <div className="flex items-center justify-between">
              <Label htmlFor="post-content" className="text-xs">
                {t('post.contentLabel')}
              </Label>
              <span className="text-muted-foreground text-[10px] tabular-nums">
                {content.length}/{CONTENT_MAX}
              </span>
            </div>
            <Tabs defaultValue="edit">
              <TabsList className="h-6">
                <TabsTrigger value="edit" className="h-4 px-2 text-[11px]">
                  <i className="fa-solid fa-pen mr-1 size-2.5" />
                  {t('post.editTab')}
                </TabsTrigger>
                <TabsTrigger value="preview" className="h-4 px-2 text-[11px]">
                  <i className="fa-solid fa-eye mr-1 size-2.5" />
                  {t('post.previewTab')}
                </TabsTrigger>
              </TabsList>
              <TabsContent value="edit" className="mt-1.5">
                <Textarea
                  id="post-content"
                  value={content}
                  maxLength={CONTENT_MAX}
                  placeholder={t('post.contentPlaceholder')}
                  onChange={(e) => setContent(e.target.value)}
                  className="min-h-40 font-mono text-xs leading-relaxed"
                />
              </TabsContent>
              <TabsContent value="preview" className="mt-1.5">
                {/* 预览区与编辑区等高，内容超高时内部滚动 */}
                <div className="min-h-40 overflow-y-auto rounded-md border p-3">
                  {content.trim() ? (
                    <MarkdownContent content={content} />
                  ) : (
                    <p className="text-muted-foreground text-xs">{t('post.previewEmpty')}</p>
                  )}
                </div>
              </TabsContent>
            </Tabs>
          </div>
        </div>

        <DialogFooter>
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
