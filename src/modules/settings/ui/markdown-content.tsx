import { useMemo } from 'react'
import { marked } from 'marked'
import { useTranslation } from '@/modules/i18n/use-translation'
import { cn } from '@/lib/utils'

// 配置 marked：同步渲染 + GFM（任务列表、表格、删除线等）
marked.use({ async: false, gfm: true })

type AlertType = 'note' | 'tip' | 'important' | 'warning' | 'caution'

const ALERT_ICON: Record<AlertType, string> = {
  note: 'fa-circle-info',
  tip: 'fa-lightbulb',
  important: 'fa-circle-exclamation',
  warning: 'fa-triangle-exclamation',
  caution: 'fa-octagon-exclamation',
}

// GitHub 官方 Alert 配色：NOTE=蓝 / TIP=绿 / IMPORTANT=紫 / WARNING=琥珀 / CAUTION=红
// 类名以字面量形式保留在源码中，供 Tailwind 扫描生成
const ALERT_STYLE: Record<AlertType, { wrapper: string; title: string; icon: string }> = {
  note: {
    wrapper: 'border-blue-200 bg-blue-50 dark:border-blue-800 dark:bg-blue-950/50',
    title: 'text-blue-700 dark:text-blue-300',
    icon: 'text-blue-500 dark:text-blue-400',
  },
  tip: {
    wrapper: 'border-green-200 bg-green-50 dark:border-green-800 dark:bg-green-950/50',
    title: 'text-green-700 dark:text-green-300',
    icon: 'text-green-500 dark:text-green-400',
  },
  important: {
    wrapper: 'border-purple-200 bg-purple-50 dark:border-purple-800 dark:bg-purple-950/50',
    title: 'text-purple-700 dark:text-purple-300',
    icon: 'text-purple-500 dark:text-purple-400',
  },
  warning: {
    wrapper: 'border-amber-200 bg-amber-50 dark:border-amber-800 dark:bg-amber-950/50',
    title: 'text-amber-700 dark:text-amber-300',
    icon: 'text-amber-500 dark:text-amber-400',
  },
  caution: {
    wrapper: 'border-red-200 bg-red-50 dark:border-red-800 dark:bg-red-950/50',
    title: 'text-red-700 dark:text-red-300',
    icon: 'text-red-500 dark:text-red-400',
  },
}

/**
 * 预处理 GitHub 风格 Markdown Alert（如 > [!NOTE]）
 *
 * 将引用块转换为带图标与标题的提示块 HTML，标题走 i18n key，
 * 后续再由 marked 解析正文中的行内 Markdown。
 */
export function preprocessMarkdownAlerts(markdown: string, t: (key: string) => string): string {
  return markdown.replace(
    /^> \[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]\n((?:>.*(?:\n|$))+)/gim,
    (_match, type: string, body: string) => {
      const alertType = type.toLowerCase() as AlertType
      const title = t(`settings.about.alert.${alertType}`)
      const icon = ALERT_ICON[alertType] ?? 'fa-circle-info'
      const color = ALERT_STYLE[alertType] ?? ALERT_STYLE.note
      // 去掉每行引用前缀（允许 `>` 或 `> ` / `>\t`），保留正文 Markdown 供后续解析
      const content = body.replace(/^>[ \t]?/gim, '').trim()
      const contentHtml = marked.parse(content) as string
      // 前后保留空行，避免紧跟的 Markdown 被 marked 误判为 HTML 块内容
      return `\n<div class="markdown-alert ${color.wrapper}">\n<p class="markdown-alert-title ${color.title}"><i class="fa-solid ${icon} ${color.icon}"></i> ${title}</p>\n${contentHtml}\n</div>\n`
    }
  )
}

/**
 * Markdown 渲染组件（GFM + GitHub Alert）
 *
 * 供更新检查弹窗（Release Notes）与「查看历史更新」弹窗（CHANGELOG）复用，
 * 展示前对 GitHub 风格 Alert 引用块做预处理，样式统一走 Tailwind Typography。
 */
export function MarkdownContent({ content }: { content: string }) {
  const { t } = useTranslation()
  const html = useMemo(() => {
    if (!content) return ''
    const processed = preprocessMarkdownAlerts(content, t)
    return marked.parse(processed) as string
  }, [content, t])

  return (
    <div
      className={cn(
        'prose prose-xs max-w-none text-xs leading-relaxed',
        'prose-headings:mt-3 prose-headings:mb-1.5 prose-headings:text-foreground',
        'prose-h1:mt-3.5 prose-h1:mb-2 prose-h1:text-lg prose-h1:font-bold',
        'prose-h2:mt-3 prose-h2:mb-1.5 prose-h2:text-base prose-h2:font-semibold',
        'prose-h3:mt-2.5 prose-h3:mb-1 prose-h3:text-sm prose-h3:font-semibold',
        'prose-h4:mt-2 prose-h4:mb-0.5 prose-h4:text-xs prose-h4:font-medium',
        'prose-p:my-1 prose-p:text-foreground',
        'prose-li:my-0.5 prose-li:text-foreground',
        'prose-ul:my-1 prose-ul:pl-4 prose-ul:list-disc',
        'prose-ol:my-1 prose-ol:pl-4 prose-ol:list-decimal',
        'prose-code:rounded prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:text-xs prose-code:font-mono',
        'prose-a:text-primary prose-a:underline prose-a:underline-offset-2',
        'prose-strong:text-foreground prose-strong:font-semibold',
        'prose-del:text-muted-foreground',
        'prose-table:my-2 prose-table:text-xs',
        'prose-th:border prose-th:px-2 prose-th:py-1 prose-th:text-left prose-th:text-xs prose-th:text-muted-foreground prose-th:bg-muted/50',
        'prose-td:border prose-td:px-2 prose-td:py-1 prose-td:text-xs',
        // GFM 任务列表：input checkbox 美化
        'prose-li:input:mr-1.5 prose-li:input:size-3 prose-li:input:align-middle',
        '[&_li>input[type="checkbox"]]:mr-1.5',
        '[&_li>input[type="checkbox"]]:size-3',
        '[&_li>input[type="checkbox"]]:align-middle',
        '[&_li>input[type="checkbox"]]:accent-primary',
        // 去除任务列表 li 的圆点
        '[&_li:has(>input[type="checkbox"])]:list-none',
        '[&_li:has(>input[type="checkbox"])]:-ml-4',
        '[&h2:first-child]:mt-0',
        // GitHub 风格 Alert 提示块：布局统一，颜色按类型注入（见 ALERT_STYLE 注释）
        '[&_.markdown-alert]:my-3 [&_.markdown-alert]:rounded-md [&_.markdown-alert]:border [&_.markdown-alert]:border-l-4 [&_.markdown-alert]:p-3',
        '[&_.markdown-alert-title]:mb-1.5 [&_.markdown-alert-title]:flex [&_.markdown-alert-title]:items-center [&_.markdown-alert-title]:gap-1.5 [&_.markdown-alert-title]:text-xs [&_.markdown-alert-title]:font-semibold',
        '[&_.markdown-alert_p]:my-1 [&_.markdown-alert_p]:text-foreground'
      )}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  )
}