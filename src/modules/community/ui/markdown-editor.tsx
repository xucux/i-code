/**
 * 社区 Markdown 编辑器（编辑 / 预览双 Tab + 语法工具栏）
 *
 * 用于发帖正文与一级回复，替代原先两处重复的 Tabs + Textarea 片段：
 * - 编辑 Tab：顶部语法工具栏（粗体/斜体/标题/引用/代码/链接/图片/列表/表格/分隔线），
 *   点击在光标处插入 Markdown 语法；下方为等高的 Textarea
 * - 预览 Tab：实时渲染社区隔离版 Markdown（含代码超长折叠）
 *
 * 语法插入基于光标选中区间处理，未选中时插入占位文本；行级语法（标题/列表/引用/任务）
 * 作用于当前行或选中的多行。
 */

import { useRef } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Textarea } from '@/components/ui/textarea'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { cn } from '@/lib/utils'
import { CommunityMarkdownContent } from './community-markdown-content'

export interface MarkdownEditorProps {
  /** 透传给 textarea（用于 Label htmlFor 关联） */
  id?: string
  value: string
  onChange: (value: string) => void
  maxLength?: number
  placeholder?: string
  /** textarea 与预览区高度（如 `h-[50vh]` / `h-[45vh]`） */
  heightClass?: string
  /** 置位时以 flex 铺满父容器（父级弹窗全屏时使用），忽略 heightClass */
  fill?: boolean
  autoFocus?: boolean
  /** 透传给 textarea（如一级评论 Ctrl/Cmd+Enter 快捷发送） */
  onKeyDown?: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void
}

/** 变换结果：新文本 + 光标选区 */
type Transform = { text: string; sel: [number, number] }
/** 语法动作：返回依据原始文本(value)与选区(s,e)生成的新文本与选区 */
type Action = {
  key: string
  icon: string
  transform: (value: string, s: number, e: number) => Transform
}

/** 行首前缀是否已存在（避免重复插入） */
function lineHasPrefix(line: string, prefix: string): boolean {
  return line.startsWith(prefix)
}

/**
 * 行级前缀插入：作用于选中区域的首行到末行（无选择时作用于光标所在行）。
 * 对已带前缀的行做「去前缀」切换，对空行直接加前缀。
 */
function prefixLines(value: string, s: number, e: number, allPrefix: string): Transform {
  const lineStart = value.lastIndexOf('\n', s) + 1
  let lineEnd = value.indexOf('\n', e)
  if (lineEnd === -1) lineEnd = value.length
  const block = value.slice(lineStart, lineEnd)
  const replaced = block
    .split('\n')
    .map((line) => line.trim())
    .map((line) =>
      line
        ? lineHasPrefix(line, allPrefix)
          ? line.slice(allPrefix.length)
          : allPrefix + line
        : line
    )
    .join('\n')
  return {
    text: value.slice(0, lineStart) + replaced + value.slice(lineEnd),
    sel: [lineStart, lineStart + replaced.length],
  }
}

/**
 * 包裹式插入：给选中文本（或占位符）加上 prefix / suffix 前缀后缀。
 */
function wrapSelection(value: string, s: number, e: number, prefix: string, suffix: string, placeholder: string): Transform {
  const selected = value.slice(s, e)
  const inner = selected || placeholder
  // 选中内容已带前缀时切换为去除（避免重复包裹）
  const target = selected.startsWith(prefix) && selected.endsWith(suffix) ? selected.slice(prefix.length, -suffix.length) : inner
  const text = value.slice(0, s) + prefix + target + suffix + value.slice(e)
  return { text, sel: [s + prefix.length, s + prefix.length + target.length] }
}

/**
 * 社区 Markdown 编辑器
 */
export function MarkdownEditor({
  id,
  value,
  onChange,
  maxLength,
  placeholder,
  heightClass = 'h-[40vh]',
  fill = false,
  autoFocus = false,
  onKeyDown,
}: MarkdownEditorProps) {
  const { t } = useTranslation('community')
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const previewRef = useRef<HTMLDivElement>(null)

  /** 根据编辑区滚动比例同步预览区滚动位置（全屏分屏模式） */
  const syncPreviewScroll = () => {
    const ta = textareaRef.current
    const preview = previewRef.current
    if (!ta || !preview) return
    const taMax = ta.scrollHeight - ta.clientHeight
    const previewMax = preview.scrollHeight - preview.clientHeight
    if (taMax <= 0 || previewMax <= 0) return
    const ratio = ta.scrollTop / taMax
    preview.scrollTop = ratio * previewMax
  }

  /** 执行一次文本变换并回写，随后恢复光标到目标选区 */
  const apply = (transform: Transform) => {
    onChange(transform.text)
    const ta = textareaRef.current
    if (!ta) return
    // 等 React 提交新值后再恢复光标
    requestAnimationFrame(() => {
      ta.focus()
      ta.setSelectionRange(transform.sel[0], transform.sel[1])
    })
  }

  const tEd = (key: string) => t(`editor.${key}`)

  /** 语法动作表（图标/工具提示/变换逻辑） */
  const actions: Action[] = [
    { key: 'bold', icon: 'fa-bold', transform: (v, s, e) => wrapSelection(v, s, e, '**', '**', tEd('bold')) },
    { key: 'italic', icon: 'fa-italic', transform: (v, s, e) => wrapSelection(v, s, e, '*', '*', tEd('italic')) },
    { key: 'strikethrough', icon: 'fa-strikethrough', transform: (v, s, e) => wrapSelection(v, s, e, '~~', '~~', tEd('strikethrough')) },
    { key: 'heading', icon: 'fa-heading', transform: (v, s, e) => prefixLines(v, s, e, '## ') },
    { key: 'quote', icon: 'fa-quote-right', transform: (v, s, e) => prefixLines(v, s, e, '> ') },
    { key: 'inlineCode', icon: 'fa-code', transform: (v, s, e) => wrapSelection(v, s, e, '`', '`', tEd('inlineCode')) },
    { key: 'codeBlock', icon: 'fa-file-code', transform: (v, s, e) => wrapSelection(v, s, e, '```\n', '\n```', tEd('codeBlock')) },
    {
      key: 'link',
      icon: 'fa-link',
      transform: (v, s, e) => {
        const selected = v.slice(s, e)
        // 选中文本作为链接文字；否则使用占位
        const text = selected || tEd('linkText')
        const before = v.slice(0, s) + '[' + text + ']('
        const after = ')' + v.slice(e)
        return { text: before + tEd('linkUrl') + after, sel: [s + text.length + 3, s + text.length + 3 + tEd('linkUrl').length] }
      },
    },
    {
      key: 'image',
      icon: 'fa-image',
      transform: (v, s, e) => {
        // 插入图片语法，占位符预选中：![alt](url)
        const part = '![' + tEd('imageAlt') + '](' + tEd('imageUrl') + ')'
        const text = v.slice(0, s) + part + v.slice(e)
        return { text, sel: [s, s + part.length] }
      },
    },
    { key: 'taskList', icon: 'fa-list-check', transform: (v, s, e) => prefixLines(v, s, e, '- [ ] ') },
    { key: 'ulList', icon: 'fa-list-ul', transform: (v, s, e) => prefixLines(v, s, e, '- ') },
    { key: 'olList', icon: 'fa-list-ol', transform: (v, s, e) => prefixLines(v, s, e, '1. ') },
    {
      key: 'newline',
      icon: 'fa-arrow-turn-down',
      // 换行：新行插入 <br>（HTML 硬换行标记），<br> 前后均保留换行，避免与相邻文本黏连
      transform: (v, s, e) => {
        const ins = '\n<br>\n'
        const text = v.slice(0, s) + ins + v.slice(e)
        return { text, sel: [s + ins.length, s + ins.length] }
      },
    },
    {
      key: 'table',
      icon: 'fa-table',
      transform: (v, s, e) => {
        const tpl = '\n| ' + tEd('tableCol1') + ' | ' + tEd('tableCol2') + ' |\n| --- | --- |\n| ' + tEd('tableCell') + ' | ' + tEd('tableCell') + ' |\n'
        const text = v.slice(0, s) + tpl + v.slice(e)
        return { text, sel: [s + 2, s + 2] }
      },
    },
    {
      key: 'divider',
      icon: 'fa-ellipsis',
      transform: (v, s, e) => {
        const text = v.slice(0, s) + (s > 0 ? '\n' : '') + '---\n' + v.slice(e)
        return { text, sel: [s, s] }
      },
    },
  ]

  // 语法工具栏（普通 Tab / 全屏分屏共用；置于输入区上方）
  const toolbar = (
    <div className="flex flex-wrap items-center gap-0.5 border-b bg-muted/30 px-1 py-0.5">
      {actions.map((action) => (
        <button
          key={action.key}
          type="button"
          title={tEd(action.key)}
          aria-label={tEd(action.key)}
          className="text-muted-foreground hover:bg-muted hover:text-foreground flex size-6 items-center justify-center rounded transition-colors"
          onMouseDown={(ev) => ev.preventDefault() /* 防止失焦丢失选中 */}
          onClick={() => {
            const ta = textareaRef.current
            if (!ta) return
            apply(action.transform(ta.value, ta.selectionStart, ta.selectionEnd))
          }}
        >
          <i className={`fa-solid ${action.icon} size-2.5`} />
        </button>
      ))}
    </div>
  )

  const textareaClasses =
    'w-full resize-none rounded-none border-0 bg-transparent font-mono text-xs leading-relaxed shadow-none focus-visible:ring-0 focus-visible:ring-offset-0'

  // 全屏分屏：左侧编辑 + 右侧预览（工具栏横跨顶部），用于弹窗系统全屏放大态
  if (fill) {
    return (
      <div className="flex min-h-0 flex-1 flex-col gap-1.5">
        <div className="overflow-hidden rounded-md border">{toolbar}</div>
        <div className="flex min-h-0 flex-1 gap-3">
          {/* 左：编辑 */}
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-md border">
            <Textarea
              ref={textareaRef}
              id={id}
              value={value}
              maxLength={maxLength}
              autoFocus={autoFocus}
              placeholder={placeholder}
              onChange={(e) => onChange(e.target.value)}
              onKeyDown={onKeyDown}
              onScroll={syncPreviewScroll}
              className={cn(textareaClasses, 'min-h-0 flex-1 px-3 py-2')}
            />
          </div>
          {/* 右：预览（随输入实时渲染） */}
          <div ref={previewRef} className="min-h-0 flex-1 overflow-y-auto rounded-md border p-3">
            {value.trim() ? (
              <CommunityMarkdownContent content={value} />
            ) : (
              <p className="text-muted-foreground text-xs">{t('post.previewEmpty')}</p>
            )}
          </div>
        </div>
      </div>
    )
  }

  return (
    <Tabs defaultValue="edit">
      <TabsList className="h-6">
        <TabsTrigger value="edit" className="text-muted-foreground h-4 px-2 text-[11px] data-[state=active]:text-foreground">
          <i className="fa-solid fa-pen mr-1 size-2.5" />
          {t('post.editTab')}
        </TabsTrigger>
        <TabsTrigger value="preview" className="text-muted-foreground h-4 px-2 text-[11px] data-[state=active]:text-foreground">
          <i className="fa-solid fa-eye mr-1 size-2.5" />
          {t('post.previewTab')}
        </TabsTrigger>
      </TabsList>

      <TabsContent value="edit" className="mt-1.5">
        <div className="overflow-hidden rounded-md border">{toolbar}
          <Textarea
            ref={textareaRef}
            id={id}
            value={value}
            maxLength={maxLength}
            autoFocus={autoFocus}
            placeholder={placeholder}
            onChange={(e) => onChange(e.target.value)}
            onKeyDown={onKeyDown}
            className={cn(textareaClasses, heightClass)}
          />
        </div>
      </TabsContent>

      <TabsContent value="preview" className="mt-1.5">
        {/* 预览区与编辑区等高，内容超高时内部滚动 */}
        <div className={cn('overflow-y-auto rounded-md border p-3', heightClass)}>
          {value.trim() ? (
            <CommunityMarkdownContent content={value} />
          ) : (
            <p className="text-muted-foreground text-xs">{t('post.previewEmpty')}</p>
          )}
        </div>
      </TabsContent>
    </Tabs>
  )
}