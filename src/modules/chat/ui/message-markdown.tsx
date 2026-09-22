/**
 * 聊天气泡 Markdown 渲染
 *
 * ## 界面描述
 *
 * - 助手气泡正文按 GFM 渲染：标题、列表、任务列表、表格、引用、代码块、
 *   行内代码、链接、图片，复用通用组件的 GitHub Alert（`> [!NOTE]`）与
 *   KaTeX 数学公式（`$...$` / `\(...\)` / `$$...$$` / `\[...\]`）。
 * - 流式中正文末尾保留脉冲光标 `▍`。
 * - 不含任何 Markdown 语法的短句（「好的」「收到」）按纯文本原样输出。
 *
 * ## 逻辑描述
 *
 * 1. **纯文本快路径**（`hasMarkdownSyntax`）：正文无 Markdown 语法时直接输出字符串，
 *    由气泡自身的 `whitespace-pre-wrap` 负责换行。好处有二：
 *    短句不会被 `prose` 包裹后多出块级间距；省掉一次解析与一次 innerHTML 替换。
 * 2. **流式节流**（`useThrottledStreamContent`）：流式期间每 `STREAM_THROTTLE_MS`
 *    提交一次渲染内容，避免每个 chunk 全量重解析；流式结束后由调用方直接使用原始
 *    正文（见 `message-list.tsx`），保证终态不丢字。
 * 3. **解析**：复用 `components/ui/markdown-content` 的 `preprocessMarkdownAlerts`
 *    与全局 `marked` 单例（GFM 同步解析由该模块初始化，导入即生效）。
 *
 * ## 已知限制
 *
 * `marked` 默认放行原始 HTML，本组件与 `MarkdownContent` 行为一致（未做净化）。
 * 详见 `docs/chat-module.md` §2.4「原始 HTML 与安全边界」。
 */

import { useEffect, useMemo, useRef, useState } from 'react'
import { marked } from 'marked'
import { cn } from '@/lib/utils'
import { useTranslation } from '@/modules/i18n/use-translation'
import { preprocessMarkdownAlerts } from '@/components/ui/markdown-content'
import { hasMathSyntax } from '@/components/ui/markdown-math'

/**
 * 流式渲染节流间隔（毫秒）
 *
 * 约 8 帧：既能让长回答的字流保持连续观感，又把全量重解析次数压到 1/8 左右。
 */
export const STREAM_THROTTLE_MS = 120

/**
 * 正文是否含需要 Markdown 解析的语法
 *
 * 保守匹配：只认明确的 Markdown 标记，宁可漏判（按纯文本显示）也不误判——
 * 误判会让「3 * 4 = 12」这类短句被 `prose` 包裹而多出块级间距。
 *
 * | 类别 | 匹配内容 |
 * |------|----------|
 * | 块级 | 标题、引用、无序/有序列表、代码围栏、表格、分隔线 |
 * | 行内 | 行内代码、加粗、链接、图片、删除线 |
 * | 公式 | `$...$`、`\(...\)`、`$$...$$`、`\[...\]`（口径见 `markdown-math.ts`） |
 */
export function hasMarkdownSyntax(content: string): boolean {
  if (!content) return false
  // 块级：行首标记（缩进 ≤3 空格，与 Markdown 规范一致）
  if (/^[ \t]{0,3}(#{1,6} |>\s?|[-*+] |\d+[.)] |```|~~~|\|)/m.test(content)) return true
  // 分隔线：--- / *** / ___（同一字符重复 ≥3 次且独占一行）
  if (/^[ \t]{0,3}([-*_])([ \t]*\1){2,}[ \t]*$/m.test(content)) return true
  // 行内代码：需成对反引号且不跨行
  if (/`[^`\n]+`/.test(content)) return true
  // 加粗：** 或 __ 紧跟非空白字符
  if (/\*\*[^\s*][^*\n]*\*\*/.test(content)) return true
  if (/__[^\s_][^_\n]*__/.test(content)) return true
  // 链接 / 图片：[](...)
  if (/!?\[[^\]\n]*\]\([^)\s][^)\n]*\)/.test(content)) return true
  // 删除线
  if (/~~[^\s~][^~\n]*~~/.test(content)) return true
  // 数学公式：定界符判定与 marked 扩展同源（见 markdown-math.ts）
  if (hasMathSyntax(content)) return true
  return false
}

/**
 * 流式内容节流
 *
 * - `active` 为 true 时，最多每 `delayMs` 提交一次新内容；不足间隔的更新合并为
 *   一次尾触发，保证最后一个 chunk 不会因为"恰好落在冷却窗口内"而永久丢失。
 * - `active` 为 false（流式结束）时不经过节流，直接返回原始内容。
 *
 * 注意：返回值仅在 `active` 时用于渲染，终态由调用方直接取原文，因此重置逻辑
 * 不需要额外处理。
 */
export function useThrottledStreamContent(
  content: string,
  active: boolean,
  delayMs: number = STREAM_THROTTLE_MS
): string {
  const [rendered, setRendered] = useState(content)
  const lastCommitRef = useRef(0)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  // 定时器回调里需要拿到最新内容，用 ref 穿透闭包
  const latestRef = useRef(content)
  latestRef.current = content

  useEffect(() => {
    if (!active) return
    const elapsed = Date.now() - lastCommitRef.current
    if (elapsed >= delayMs) {
      lastCommitRef.current = Date.now()
      setRendered(content)
      return
    }
    // 冷却窗口内：复用已挂起的尾触发，不重复排期
    if (timerRef.current !== null) return
    timerRef.current = setTimeout(() => {
      timerRef.current = null
      lastCommitRef.current = Date.now()
      setRendered(latestRef.current)
    }, delayMs - elapsed)
  }, [content, active, delayMs])

  // 卸载时清理挂起的定时器；同时重置标记，避免 StrictMode 重挂载后误判"已有定时器"
  useEffect(
    () => () => {
      if (timerRef.current !== null) {
        clearTimeout(timerRef.current)
        timerRef.current = null
      }
    },
    []
  )

  return active ? rendered : content
}

/**
 * 聊天 Markdown 正文样式
 *
 * 与气泡统一为 `text-xs`；`prose` 的默认上下间距在气泡里偏大，此处逐项收紧。
 * 代码块单独覆盖 `pre code`，避免行内代码样式叠加成双层底色。
 */
const MARKDOWN_CLASS = cn(
  'prose prose-xs max-w-none text-xs leading-relaxed break-words',
  'prose-headings:text-foreground',
  'prose-h1:mt-3 prose-h1:mb-1.5 prose-h1:text-base prose-h1:font-semibold',
  'prose-h2:mt-3 prose-h2:mb-1.5 prose-h2:text-sm prose-h2:font-semibold',
  'prose-h3:mt-2.5 prose-h3:mb-1 prose-h3:text-xs prose-h3:font-semibold',
  'prose-h4:mt-2 prose-h4:mb-1 prose-h4:text-xs prose-h4:font-medium',
  'prose-p:my-1 prose-p:text-foreground',
  'prose-li:my-0.5 prose-li:text-foreground',
  'prose-ul:my-1 prose-ul:pl-4 prose-ul:list-disc',
  'prose-ol:my-1 prose-ol:pl-4 prose-ol:list-decimal',
  'prose-strong:font-semibold prose-strong:text-foreground',
  'prose-a:text-primary prose-a:underline prose-a:underline-offset-2',
  'prose-del:text-muted-foreground',
  'prose-hr:my-3 prose-hr:border-border',
  'prose-blockquote:my-2 prose-blockquote:border-l-2 prose-blockquote:border-border',
  'prose-blockquote:pl-3 prose-blockquote:font-normal prose-blockquote:text-muted-foreground',
  'prose-code:rounded prose-code:bg-background/70 prose-code:px-1 prose-code:py-0.5',
  'prose-code:text-[11px] prose-code:font-mono prose-code:before:content-none prose-code:after:content-none',
  'prose-pre:my-2 prose-pre:rounded-md prose-pre:border prose-pre:border-border/60',
  'prose-pre:bg-background/70 prose-pre:p-2 prose-pre:text-[11px] prose-pre:leading-relaxed',
  // 代码块内的 code：清掉行内样式叠加（prose 的 `pre code` 修饰不含子级组合，用任意变体兜住）
  '[&_pre_code]:bg-transparent [&_pre_code]:p-0 [&_pre_code]:text-[11px] [&_pre_code]:font-normal',
  'prose-table:my-2 prose-table:text-xs',
  'prose-th:border prose-th:border-border prose-th:px-2 prose-th:py-1',
  'prose-th:bg-muted/50 prose-th:text-left prose-th:text-xs prose-th:text-muted-foreground',
  'prose-td:border prose-td:border-border prose-td:px-2 prose-td:py-1 prose-td:text-xs',
  // 图片：等比缩放不溢出气泡（聊天正文不挂灯箱，与 MarkdownContent 的放大查看区分开）
  'prose-img:my-1 prose-img:max-w-full prose-img:h-auto prose-img:rounded-md',
  // GFM 任务列表：复选框与列表项对齐
  '[&_li>input[type="checkbox"]]:mr-1.5',
  '[&_li>input[type="checkbox"]]:size-3',
  '[&_li>input[type="checkbox"]]:align-middle',
  '[&_li>input[type="checkbox"]]:accent-primary',
  '[&_li:has(>input[type="checkbox"])]:list-none',
  '[&_li:has(>input[type="checkbox"])]:-ml-4',
  '[&>:first-child]:mt-0 [&>:last-child]:mb-0',
  // GitHub 风格 Alert：布局统一（配色由 preprocessMarkdownAlerts 注入）
  '[&_.markdown-alert]:my-2 [&_.markdown-alert]:rounded-md [&_.markdown-alert]:border',
  '[&_.markdown-alert]:border-l-4 [&_.markdown-alert]:p-2.5',
  '[&_.markdown-alert-title]:mb-1 [&_.markdown-alert-title]:flex',
  '[&_.markdown-alert-title]:items-center [&_.markdown-alert-title]:gap-1.5',
  '[&_.markdown-alert-title]:text-xs [&_.markdown-alert-title]:font-semibold',
  '[&_.markdown-alert_p]:my-1 [&_.markdown-alert_p]:text-foreground',
  // KaTeX 块级公式：长公式在气泡内横向滚动，避免撑破气泡宽度
  '[&_.katex-display]:my-2 [&_.katex-display]:py-0.5',
  '[&_.katex-display]:overflow-x-auto [&_.katex-display]:overflow-y-hidden'
)

/** 流式光标：与纯文本分支的 `▍` 观感保持一致 */
const STREAM_CURSOR_CLASS = 'inline-block animate-pulse text-muted-foreground'
const STREAM_CURSOR_HTML = ` <span class="${STREAM_CURSOR_CLASS}">▍</span>`

/**
 * 正文是否停在未闭合的代码围栏内
 *
 * 流式输出到一半时经常正好停在 ``` 开栏之后，此时把光标拼进源码会被 marked
 * 当作代码内容转义成字面量（用户会看到 `<span class=...>` 这样的乱码），
 * 因此这种情形改用块外光标。
 */
export function isInsideOpenFence(content: string): boolean {
  const fences = content.match(/^[ \t]{0,3}(?:```|~~~)/gm)
  return fences !== null && fences.length % 2 === 1
}

/** 流式光标元素（块外形态） */
function StreamCursor() {
  return <span className={STREAM_CURSOR_CLASS}>▍</span>
}

/**
 * 聊天 Markdown 正文
 *
 * - `content` 应为已节流的正文（流式期间由 `useThrottledStreamContent` 提供）。
 * - `streaming` 为 true 时把光标并入 Markdown 源码，随最后一个块一起排版，
 *   避免独立成行；仅当正文停在未闭合围栏内时退回块外光标。
 */
export function ChatMarkdownBody({
  content,
  streaming = false,
}: {
  content: string
  streaming?: boolean
}) {
  // Alert 标题走 `settings.about.alert.*`，需默认命名空间（`chat` 命名空间会拼错前缀）
  const { t } = useTranslation()

  const inlineCursor = streaming && !isInsideOpenFence(content)

  const html = useMemo(() => {
    if (!content) return ''
    return marked.parse(
      preprocessMarkdownAlerts(inlineCursor ? `${content}${STREAM_CURSOR_HTML}` : content, t)
    ) as string
  }, [content, inlineCursor, t])

  return (
    <>
      <div className={MARKDOWN_CLASS} dangerouslySetInnerHTML={{ __html: html }} />
      {streaming && !inlineCursor && <StreamCursor />}
    </>
  )
}
