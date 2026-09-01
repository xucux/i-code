/**
 * 社区专用 Markdown 渲染组件（与全局渲染器完全隔离）
 *
 * 与 `components/ui/markdown-content.tsx` 解耦，社区正文 / 一级评论 / 发帖预览
 * 均改用本组件渲染，社区特有的代码超长折叠、图片紧凑等逻辑不会影响软件内
 * 其他场景（更新日志、更新检查等仍使用全局渲染器）。
 *
 * 能力：
 * - 独立 `Marked` 实例（GFM + 任务列表/表格/删除线），不复用全局 singleton
 * - GitHub 风格 Alert 引用块（> [!NOTE] 等），标题走全局 i18n `settings.about.alert.*`
 * - 代码块超长折叠：单块行数超过 `FOLD_THRESHOLD` 时默认折叠，底部渐变遮罩 +
 *   「展开全部 N 行 / 收起」按钮；点击切换通过事件委托完成（无需 React state）
 * - 图片点击放大查看（全屏遮罩，同全局渲染器）
 * - `compactImages` 置位时图片宽度缩为容器 1/3（评论区紧凑布局）
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import { createPortal } from 'react-dom'
import { Marked } from 'marked'
import hljs from 'highlight.js/lib/core'
import { useTranslation } from '@/modules/i18n/use-translation'
import { cn } from '@/lib/utils'

// 按需注册代码语言（覆盖社区常用：java / html / css / shell / powershell / xml / yaml / javascript 等）
import hljsBash from 'highlight.js/lib/languages/bash'
import hljsC from 'highlight.js/lib/languages/c'
import hljsCpp from 'highlight.js/lib/languages/cpp'
import hljsCsharp from 'highlight.js/lib/languages/csharp'
import hljsCss from 'highlight.js/lib/languages/css'
import hljsDiff from 'highlight.js/lib/languages/diff'
import hljsDockerfile from 'highlight.js/lib/languages/dockerfile'
import hljsGo from 'highlight.js/lib/languages/go'
import hljsIni from 'highlight.js/lib/languages/ini'
import hljsJava from 'highlight.js/lib/languages/java'
import hljsJavascript from 'highlight.js/lib/languages/javascript'
import hljsJson from 'highlight.js/lib/languages/json'
import hljsMarkdown from 'highlight.js/lib/languages/markdown'
import hljsNginx from 'highlight.js/lib/languages/nginx'
import hljsPlaintext from 'highlight.js/lib/languages/plaintext'
import hljsPowershell from 'highlight.js/lib/languages/powershell'
import hljsProperties from 'highlight.js/lib/languages/properties'
import hljsPython from 'highlight.js/lib/languages/python'
import hljsRust from 'highlight.js/lib/languages/rust'
import hljsSql from 'highlight.js/lib/languages/sql'
import hljsTypescript from 'highlight.js/lib/languages/typescript'
import hljsXml from 'highlight.js/lib/languages/xml'
import hljsYaml from 'highlight.js/lib/languages/yaml'

hljs.registerLanguage('bash', hljsBash)
hljs.registerLanguage('shell', hljsBash) // shell / sh / bash 共用 Bash 语法
hljs.registerLanguage('c', hljsC)
hljs.registerLanguage('cpp', hljsCpp)
hljs.registerLanguage('csharp', hljsCsharp)
hljs.registerLanguage('css', hljsCss)
hljs.registerLanguage('diff', hljsDiff)
hljs.registerLanguage('dockerfile', hljsDockerfile)
hljs.registerLanguage('go', hljsGo)
hljs.registerLanguage('ini', hljsIni)
hljs.registerLanguage('java', hljsJava)
hljs.registerLanguage('javascript', hljsJavascript)
hljs.registerLanguage('json', hljsJson)
hljs.registerLanguage('markdown', hljsMarkdown)
hljs.registerLanguage('nginx', hljsNginx)
hljs.registerLanguage('plaintext', hljsPlaintext)
hljs.registerLanguage('powershell', hljsPowershell)
hljs.registerLanguage('properties', hljsProperties)
hljs.registerLanguage('python', hljsPython)
hljs.registerLanguage('rust', hljsRust)
hljs.registerLanguage('sql', hljsSql)
hljs.registerLanguage('typescript', hljsTypescript)
hljs.registerLanguage('markup', hljsXml) // html 等 Markup 语法
hljs.registerLanguage('xml', hljsXml) // xml 精确高亮（Markup 别名不含 xml）
hljs.registerLanguage('yaml', hljsYaml)

/** 独立 Marked 实例：与全局 `marked` singleton 隔离；breaks 开启单换行渲染为 <br>（支持换行） */
const communityMarked = new Marked({ gfm: true, async: false, breaks: true })

/** 代码块折叠阈值：行数超过该值时折叠 */
const FOLD_THRESHOLD = 12

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

/** 代码折叠按钮文案（渲染时由 i18n 注入，见 `foldT`） */
let foldT: (key: string, options?: Record<string, unknown>) => string = (key) => key

/** 简易 HTML 转义（用于语言类名等） */
function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

/**
 * 代码语法高亮
 *
 * 依据代码围栏标注的语言名精确高亮；语言未注册 / 未标注时回退 highlightAuto
 * 自动探测，仍失败则仅做 HTML 转义（纯文本）。返回可直接注入的 HTML。
 */
function highlightCodeBlock(code: string, lang?: string): string {
  const want = (lang || '').trim().toLowerCase()
  if (want) {
    try {
      if (hljs.getLanguage(want)) {
        return hljs.highlight(code, { language: want }).value
      }
    } catch {
      /* 标注语言高亮失败，落入自动探测 */
    }
  }
  try {
    return hljs.highlightAuto(code).value
  } catch {
    return escapeHtml(code)
  }
}

/**
 * 判断当前是否深色主题（应用主题类名形如 `theme-dark` / `theme-claude-dark`）
 */
function isDarkDocument(): boolean {
  return Array.from(document.documentElement.classList).some(
    (c) => c.startsWith('theme-') && c.includes('dark')
  )
}

/** 响应式深色主题探测：监听 document root 主题类名变化 */
function useDarkTheme(): boolean {
  const [dark, setDark] = useState(isDarkDocument)
  useEffect(() => {
    const observer = new MutationObserver(() => setDark(isDarkDocument()))
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
    return () => observer.disconnect()
  }, [])
  return dark
}

// 代码语法高亮配色：亮色/暗色两套（GitHub 风格），由容器根 `data-code-theme` 切换。
// 以 `data-code-theme="light"` / `"dark"` 为作用域，自动适配应用亮/暗主题。
const HIGHLIGHT_CSS = `
[data-code-theme="light"] .hljs-comment,
[data-code-theme="light"] .hljs-quote { color: #7d8790; font-style: italic; }
[data-code-theme="light"] .hljs-keyword,
[data-code-theme="light"] .hljs-selector-tag,
[data-code-theme="light"] .hljs-literal,
[data-code-theme="light"] .hljs-doctag,
[data-code-theme="light"] .hljs-selector-attr,
[data-code-theme="light"] .hljs-selector-pseudo { color: #cf222e; }
[data-code-theme="light"] .hljs-string,
[data-code-theme="light"] .hljs-regexp,
[data-code-theme="light"] .hljs-addition,
[data-code-theme="light"] .hljs-symbol { color: #0a3069; }
[data-code-theme="light"] .hljs-number,
[data-code-theme="light"] .hljs-bullet,
[data-code-theme="light"] .hljs-link { color: #0550ae; }
[data-code-theme="light"] .hljs-title,
[data-code-theme="light"] .hljs-section,
[data-code-theme="light"] .hljs-title.class_,
[data-code-theme="light"] .hljs-title.function_ { color: #8250df; }
[data-code-theme="light"] .hljs-attribute,
[data-code-theme="light"] .hljs-selector-class,
[data-code-theme="light"] .hljs-selector-id,
[data-code-theme="light"] .hljs-name { color: #116329; }
[data-code-theme="light"] .hljs-type,
[data-code-theme="light"] .hljs-built_in,
[data-code-theme="light"] .hljs-variable,
[data-code-theme="light"] .hljs-template-variable,
[data-code-theme="light"] .hljs-params { color: #0550ae; }
[data-code-theme="light"] .hljs-meta { color: #8250df; }
[data-code-theme="light"] .hljs-deletion { text-decoration: line-through; text-decoration-thickness: 1px; }
[data-code-theme="light"] .hljs-emphasis { font-style: italic; }
[data-code-theme="light"] .hljs-strong { font-weight: 600; }

[data-code-theme="dark"] .hljs-comment,
[data-code-theme="dark"] .hljs-quote { color: #8b949e; font-style: italic; }
[data-code-theme="dark"] .hljs-keyword,
[data-code-theme="dark"] .hljs-selector-tag,
[data-code-theme="dark"] .hljs-literal,
[data-code-theme="dark"] .hljs-doctag,
[data-code-theme="dark"] .hljs-selector-attr,
[data-code-theme="dark"] .hljs-selector-pseudo { color: #ff7b72; }
[data-code-theme="dark"] .hljs-string,
[data-code-theme="dark"] .hljs-regexp,
[data-code-theme="dark"] .hljs-addition,
[data-code-theme="dark"] .hljs-symbol { color: #a5d6ff; }
[data-code-theme="dark"] .hljs-number,
[data-code-theme="dark"] .hljs-bullet,
[data-code-theme="dark"] .hljs-link { color: #79c0ff; }
[data-code-theme="dark"] .hljs-title,
[data-code-theme="dark"] .hljs-section,
[data-code-theme="dark"] .hljs-title.class_,
[data-code-theme="dark"] .hljs-title.function_ { color: #d2a8ff; }
[data-code-theme="dark"] .hljs-attribute,
[data-code-theme="dark"] .hljs-selector-class,
[data-code-theme="dark"] .hljs-selector-id,
[data-code-theme="dark"] .hljs-name { color: #7ee787; }
[data-code-theme="dark"] .hljs-type,
[data-code-theme="dark"] .hljs-built_in,
[data-code-theme="dark"] .hljs-variable,
[data-code-theme="dark"] .hljs-template-variable,
[data-code-theme="dark"] .hljs-params { color: #79c0ff; }
[data-code-theme="dark"] .hljs-meta { color: #ffa657; }
[data-code-theme="dark"] .hljs-deletion { text-decoration: line-through; text-decoration-thickness: 1px; }
[data-code-theme="dark"] .hljs-emphasis { font-style: italic; }
[data-code-theme="dark"] .hljs-strong { font-weight: 600; }
`

/**
 * 预处理 GitHub 风格 Markdown Alert（如 > [!NOTE]），并解析为提示块 HTML。
 * 社区渲染器自行维护，不复用全局渲染器的预处理器，保持隔离。
 */
export function preprocessCommunityAlerts(markdown: string): string {
  return markdown.replace(
    /^> \[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]\n((?:>.*(?:\n|$))+)/gim,
    (_match, type: string, body: string) => {
      const alertType = type.toLowerCase() as AlertType
      const title = foldT(`settings.about.alert.${alertType}`)
      const icon = ALERT_ICON[alertType] ?? 'fa-circle-info'
      const color = ALERT_STYLE[alertType] ?? ALERT_STYLE.note
      // 去掉每行引用前缀（允许 `>` 或 `> ` / `>\t`），保留正文 Markdown 供后续解析
      const content = body.replace(/^>[ \t]?/gim, '').trim()
      const contentHtml = communityMarked.parse(content) as string
      // 前后保留空行，避免紧跟的 Markdown 被 marked 误判为 HTML 块内容
      return `\n<div class="markdown-alert ${color.wrapper}">\n<p class="markdown-alert-title ${color.title}"><i class="fa-solid ${icon} ${color.icon}"></i> ${title}</p>\n${contentHtml}\n</div>\n`
    }
  )
}

// 自定义代码块渲染器：右上角提供「⋯」菜单（复制 / 自动换行）；行数超阈值时再包裹折叠容器
communityMarked.use({
  renderer: {
    code({ text, lang }: { text: string; lang?: string }) {
      const langString = (lang || '').match(/^\S*/)?.[0] ?? ''
      const codeContent = text.replace(/\n$/, '') || ' '
      // 语法高亮（按语言精确高亮，未知则自动探测），输出已转义 + span 的 HTML
      const highlighted = highlightCodeBlock(codeContent, langString)
      const codeAttr = ` class="hljs${langString ? ` language-${escapeHtml(langString)}` : ''}"`
      // 代码块右上角操作菜单：⋮ 触发，下拉「复制 / 自动换行」；visible 前由事件委托控制显隐
      const codeMenu =
        `<div class="code-actions">` +
        `<button type="button" class="code-menu-btn" title="${foldT('community.editor.more')}" aria-label="${foldT('community.editor.more')}"><i class="fa-solid fa-ellipsis-vertical size-3"></i></button>` +
        `<div class="code-menu">` +
        `<button type="button" class="code-menu-item" data-menu-action="copy"><i class="fa-regular fa-copy size-2.5 mr-1.5"></i><span>${foldT('community.editor.copy')}</span></button>` +
        `<button type="button" class="code-menu-item" data-menu-action="wrap"><i class="fa-regular fa-circle-check size-2.5 mr-1.5 code-menu-wrap-hint hidden"></i><span>${foldT('community.editor.autoWrap')}</span></button>` +
        `</div>` +
        `</div>`
      const codeWrap =
        `<div class="code-block-wrap">` + `<pre class="code-block"><code${codeAttr}>${highlighted}</code></pre>` + codeMenu + `</div>`

      const lineCount = codeContent.split('\n').length
      if (lineCount <= FOLD_THRESHOLD) return codeWrap

      // 长代码：默认折叠（内层 body 裁剪 + 渐变遮罩），折叠按钮在容器尾部始终可见
      return (
        `<div class="code-fold" data-folded="true" data-lines="${lineCount}">` +
        `<div class="code-fold-body">` +
        codeWrap +
        `<div class="code-fold-fade"></div>` +
        `</div>` +
        `<button type="button" class="code-fold-toggle"><i class="fa-solid fa-angles-down size-2.5 mr-1"></i><span>${foldT('community.editor.expandAll', { count: lineCount })}</span></button>` +
        `</div>`
      )
    },
  },
})

/** 当前打开的代码块菜单（模块级，用于外部点击关闭） */
let openCodeMenu: HTMLElement | null = null

/** 关闭所有已打开的代码块菜单 */
function closeCodeMenus() {
  openCodeMenu?.classList.remove('code-menu-open')
  openCodeMenu = null
}

/** 打开指定代码块菜单（同时关闭其它） */
function openOneCodeMenu(menu: HTMLElement) {
  closeCodeMenus()
  menu.classList.add('code-menu-open')
  openCodeMenu = menu
}

/**
 * 复制代码块到剪贴板（事件委托处理器）
 *
 * 将 `.code-block-wrap` 内 `pre code` 的原文写入剪贴板；成功后临时把菜单项的复制
 * 图标切换为绿色对勾，1.5s 后还原。写入失败静默忽略（保持无感知）。
 */
async function copyCode(menuItem: HTMLElement) {
  const wrap = menuItem.closest<HTMLElement>('.code-block-wrap')
  const codeEl = wrap?.querySelector<HTMLElement>('pre code')
  if (!codeEl) return
  const raw = codeEl.textContent ?? ''
  if (!raw) return
  try {
    await navigator.clipboard.writeText(raw)
  } catch {
    return
  }
  const icon = menuItem.querySelector('i')
  const original = icon?.className ?? ''
  if (icon) icon.className = 'fa-solid fa-check size-2.5 mr-1.5 text-green-500'
  setTimeout(() => {
    if (icon) icon.className = original
  }, 1500)
}

/**
 * 切换代码块「自动换行」：给 `pre.code-block` 切换 `code-wrap` 类并同步菜单项对勾
 */
function toggleCodeWrap(menuItem: HTMLElement) {
  const wrap = menuItem.closest<HTMLElement>('.code-block-wrap')
  const pre = wrap?.querySelector<HTMLElement>('pre.code-block')
  if (!pre) return
  pre.classList.toggle('code-wrap')
  const hint = menuItem.querySelector<HTMLElement>('.code-menu-wrap-hint')
  hint?.classList.toggle('hidden', !pre.classList.contains('code-wrap'))
}

/** 处理菜单项点击（事件委托） */
function handleCodeMenuAction(menuItem: HTMLElement) {
  const action = menuItem.getAttribute('data-menu-action')
  if (action === 'copy') void copyCode(menuItem)
  else if (action === 'wrap') toggleCodeWrap(menuItem)
  closeCodeMenus()
}

/**
 * 代码折叠 / 展开切换（事件委托处理器）
 *
 * 通过切换容器上的 `data-folded` 与内联样式展开/收起；同时更新按钮图标与文案。
 */
function toggleCodeFold(container: HTMLElement) {
  const folded = container.dataset.folded !== 'false'
  const body = container.querySelector<HTMLElement>('.code-fold-body')
  const button = container.querySelector<HTMLElement>('.code-fold-toggle')
  const fade = body?.querySelector<HTMLElement>('.code-fold-fade')
  const iconEl = button?.querySelector<HTMLElement>('.fa-solid')
  const labelEl = button?.querySelector<HTMLElement>('span')

  if (folded) {
    // 折叠 → 展开
    body?.style.setProperty('max-height', 'none')
    body?.style.setProperty('overflow', 'visible')
    if (fade) fade.style.display = 'none'
    iconEl?.setAttribute('class', 'fa-solid fa-angles-up size-2.5 mr-1')
    if (labelEl) labelEl.textContent = foldT('community.editor.collapse')
  } else {
    // 展开 → 折叠（移除内联样式，回退到类名的 max-h + overflow-hidden）
    body?.style.removeProperty('max-height')
    body?.style.removeProperty('overflow')
    if (fade) fade.style.display = ''
    iconEl?.setAttribute('class', 'fa-solid fa-angles-down size-2.5 mr-1')
    if (labelEl) {
      labelEl.textContent = foldT('community.editor.expandAll', { count: Number(container.dataset.lines ?? 0) })
    }
  }
  container.dataset.folded = folded ? 'false' : 'true'
}

/**
 * 社区 Markdown 渲染组件（隔离版，含代码块折叠）
 *
 * @param content     原始 Markdown 文本
 * @param compactImages 置位时图片宽度缩为容器 1/3（评论区紧凑布局）
 */
export function CommunityMarkdownContent({
  content,
  compactImages = false,
}: {
  content: string
  compactImages?: boolean
}) {
  const { t } = useTranslation()
  // 深色主题探测（用于代码高亮亮/暗两套配色适配）
  const dark = useDarkTheme()
  // 正在预览放大的图片地址；null = 未放大
  const [zoomSrc, setZoomSrc] = useState<string | null>(null)

  const html = useMemo(() => {
    // 供渲染器读取的 i18n 函数（Alert 标题 / 代码折叠文案在解析时注入）
    foldT = t
    if (!content) return ''
    const processed = preprocessCommunityAlerts(content)
    return communityMarked.parse(processed) as string
  }, [content, t])

  // 点击容器内元素：代码块菜单(⋮/菜单项) → 代码折叠按钮 → 图片放大（事件委托）
  const handleContainerClick = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    const menuBtn = (e.target as HTMLElement).closest<HTMLElement>('.code-menu-btn')
    if (menuBtn) {
      e.preventDefault()
      const actions = menuBtn.closest<HTMLElement>('.code-actions')
      const menu = actions?.querySelector<HTMLElement>('.code-menu')
      if (!menu) return
      const isOpen = menu.classList.contains('code-menu-open')
      // 已打开则收起，否则打开（只允许同时打开一个）
      if (isOpen) closeCodeMenus()
      else openOneCodeMenu(menu)
      return
    }
    const menuItem = (e.target as HTMLElement).closest<HTMLElement>('.code-menu-item')
    if (menuItem) {
      e.preventDefault()
      handleCodeMenuAction(menuItem)
      return
    }
    // 点击容器内其它区域（代码文本 / 折叠按钮 / 图片）：同步收起已打开菜单
    closeCodeMenus()

    const toggle = (e.target as HTMLElement).closest<HTMLElement>('.code-fold-toggle')
    if (toggle) {
      const container = toggle.closest<HTMLElement>('.code-fold')
      if (container) toggleCodeFold(container)
      return
    }
    const img = (e.target as HTMLElement).closest('img')
    if (!img) return
    const src = img.getAttribute('src')
    if (src) {
      // 若图片嵌套在 Markdown 链接内，阻止默认跳转
      e.preventDefault()
      setZoomSrc(src)
    }
  }, [])

  // 文档级点击兜底：点击到代码容器之外时收起已打开的代码块菜单
  useEffect(() => {
    const onDocMousedown = (e: MouseEvent) => {
      if (!openCodeMenu) return
      const actions = (e.target as HTMLElement).closest<HTMLElement>('.code-actions')
      // 若点击仍属于某个代码块操作区，交由容器点击处理器决定开关
      if (actions) return
      closeCodeMenus()
    }
    document.addEventListener('mousedown', onDocMousedown)
    return () => document.removeEventListener('mousedown', onDocMousedown)
  }, [])

  // 放大查看时支持 ESC 关闭
  useEffect(() => {
    if (!zoomSrc) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setZoomSrc(null)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [zoomSrc])

  return (
    <>
      {/* 语法高亮配色（亮/暗两套），作用域由容器根 data-code-theme 决定 */}
      <style data-icode-code-theme dangerouslySetInnerHTML={{ __html: HIGHLIGHT_CSS }} />
      <div
        data-code-theme={dark ? 'dark' : 'light'}
        onClick={handleContainerClick}
        className={cn(
          'prose prose-xs max-w-none text-xs leading-relaxed',
          'prose-headings:mt-3 prose-headings:mb-1.5 prose-headings:text-foreground',
          'prose-h1:mt-3.5 prose-h1:mb-2 prose-h1:text-lg prose-h1:font-bold',
          'prose-h2:mt-3 prose-h2:mb-1.5 prose-h2:text-base prose-h2:font-semibold',
          'prose-h3:mt-2.5 prose-h3:mb-1 prose-h3:text-sm prose-h3:font-semibold',
          'prose-h4:mt-2 prose-h4:mb-0.5 prose-h4:text-xs prose-h4:font-medium',
          'prose-p:my-1 prose-p:text-foreground [&_p]:whitespace-pre-wrap',
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
          // 代码块：水平滚动 + 主题底色；基础文本用前景色（避免亮色下偏灰），token 由 HIGHLIGHT_CSS 上色
          '[&_pre.code-block]:my-2 [&_pre.code-block]:overflow-x-auto [&_pre.code-block]:rounded-md [&_pre.code-block]:border [&_pre.code-block]:border-border/50 [&_pre.code-block]:bg-muted/50 [&_pre.code-block]:p-3 [&_pre.code-block]:text-[11px] [&_pre.code-block]:leading-relaxed [&_pre.code-block]:text-foreground',
          '[&_pre.code-block code]:bg-transparent [&_pre.code-block code]:p-0 [&_pre.code-block code]:text-[11px] [&_pre.code-block code]:font-mono',
          // 代码块操作菜单（⋮ 触发）：右上角悬浮，下拉「复制 / 自动换行」
          '[&_.code-block-wrap]:relative',
          '[&_.code-actions]:absolute [&_.code-actions]:right-1.5 [&_.code-actions]:top-1.5 [&_.code-actions]:z-[2] [&_.code-actions]:flex [&_.code-actions]:flex-col [&_.code-actions]:items-end',
          '[&_.code-menu-btn]:flex [&_.code-menu-btn]:size-6 [&_.code-menu-btn]:items-center [&_.code-menu-btn]:justify-center [&_.code-menu-btn]:rounded [&_.code-menu-btn]:bg-background/60 [&_.code-menu-btn]:text-muted-foreground [&_.code-menu-btn]:transition-colors [&_.code-menu-btn]:hover:bg-muted [&_.code-menu-btn]:hover:text-foreground',
          '[&_.code-menu]:hidden [&_.code-menu]:absolute [&_.code-menu]:right-0 [&_.code-menu]:top-7 [&_.code-menu]:z-[3] [&_.code-menu]:min-w-32 [&_.code-menu]:flex-col [&_.code-menu]:rounded-md [&_.code-menu]:border [&_.code-menu]:bg-background [&_.code-menu]:py-1 [&_.code-menu]:shadow-md',
          '[&_.code-menu.code-menu-open]:flex',
          '[&_.code-menu-item]:flex [&_.code-menu-item]:w-full [&_.code-menu-item]:items-center [&_.code-menu-item]:px-2.5 [&_.code-menu-item]:py-1 [&_.code-menu-item]:text-left [&_.code-menu-item]:text-xs [&_.code-menu-item]:text-foreground [&_.code-menu-item]:transition-colors [&_.code-menu-item]:hover:bg-muted',
          // 自动换行：`pre.code-wrap` 开启软换行（不横向滚动）
          '[&_pre.code-block.code-wrap]:whitespace-pre-wrap [&_pre.code-block.code-wrap]:[overflow-wrap:break-word] [&_pre.code-block.code-wrap]:overflow-x-visible',
          // 代码折叠：外层相对定位；内层 body 默认 max-h 裁剪，展开由 JS 移除内联样式覆盖
          '[&_.code-fold]:relative',
          '[&_.code-fold-body]:relative [&_.code-fold-body]:max-h-[220px] [&_.code-fold-body]:overflow-hidden',
          // 折叠态底部渐变遮罩（pointer-events-none，仅视觉提示被截断）
          '[&_.code-fold-fade]:pointer-events-none [&_.code-fold-fade]:absolute [&_.code-fold-fade]:inset-x-0 [&_.code-fold-fade]:bottom-0 [&_.code-fold-fade]:h-14 [&_.code-fold-fade]:bg-gradient-to-t [&_.code-fold-fade]:from-background [&_.code-fold-fade]:to-transparent',
          // 展开/收起按钮（始终可见）
          '[&_.code-fold-toggle]:mt-1 [&_.code-fold-toggle]:flex [&_.code-fold-toggle]:w-full [&_.code-fold-toggle]:items-center [&_.code-fold-toggle]:justify-center [&_.code-fold-toggle]:gap-1 [&_.code-fold-toggle]:rounded-md [&_.code-fold-toggle]:py-1 [&_.code-fold-toggle]:text-[11px] [&_.code-fold-toggle]:text-muted-foreground [&_.code-fold-toggle]:transition-colors [&_.code-fold-toggle]:hover:bg-muted [&_.code-fold-toggle]:hover:text-foreground',
          // 图片：等比缩放不溢出容器，指针放大态；评论区用 compactImages 时缩为 1/3 宽
          compactImages
            ? 'prose-img:my-1 prose-img:h-auto prose-img:max-w-[33.333%] prose-img:rounded-md prose-img:cursor-zoom-in'
            : 'prose-img:my-1 prose-img:max-w-full prose-img:h-auto prose-img:rounded-md prose-img:cursor-zoom-in',
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
      {/* 图片放大查看遮罩（Portal 到 body，避免被父级 transform/overflow 限制） */}
      {zoomSrc && <CommunityImageLightbox src={zoomSrc} onClose={() => setZoomSrc(null)} />}
    </>
  )
}

/**
 * 图片放大查看遮罩：半透明黑底 + 等比居中显示原图，支持自由缩放
 *
 * 触发关闭：点击遮罩 / 点右上角关闭 / ESC。
 * 缩放：底部透明按钮组 缩小 / 重置 / 放大（0.25 步进，范围 0.5 ~ 4 倍）。
 */
function CommunityImageLightbox({ src, onClose }: { src: string; onClose: () => void }) {
  // 缩放倍数：初始 1；0.25 步进，范围 [0.5, 4]
  const [scale, setScale] = useState(1)
  const handleZoom = (delta: number) => {
    setScale((s) => {
      const next = Math.round((s + delta) * 100) / 100
      return Math.min(4, Math.max(0.5, next))
    })
  }
  return createPortal(
    <div
      className="fixed inset-0 z-[100] flex cursor-zoom-out items-center justify-center overflow-hidden bg-black/70 p-6"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
    >
      {/* 右上角关闭按钮（事件不冒泡，避免触发遮罩关闭外的重复逻辑）；z 高于图片，放大后仍可点 */}
      <button
        type="button"
        className="absolute right-3 top-3 z-10 flex size-8 items-center justify-center rounded-md text-white/80 transition-colors hover:bg-white/10 hover:text-white"
        title="关闭"
        onClick={(e) => {
          e.stopPropagation()
          onClose()
        }}
      >
        <i className="fa-solid fa-xmark size-4" />
      </button>
      {/* 底部缩放控制：半透明按钮组（点击不冒泡）；z 高于图片，放大后仍可点 */}
      <div
        className="absolute bottom-3 left-1/2 z-10 flex -translate-x-1/2 items-center gap-0.5 rounded-md bg-white/10 p-1 backdrop-blur-sm"
        onClick={(e) => e.stopPropagation()}
      >
        <button
          type="button"
          className="flex size-7 items-center justify-center rounded text-white/80 transition-colors hover:bg-white/10 hover:text-white disabled:opacity-30"
          title="缩小"
          disabled={scale <= 0.5}
          onClick={() => handleZoom(-0.25)}
        >
          <i className="fa-solid fa-magnifying-glass-minus size-3.5" />
        </button>
        <span className="w-11 text-center text-xs text-white/80 tabular-nums">
          {Math.round(scale * 100)}%
        </span>
        <button
          type="button"
          className="flex size-7 items-center justify-center rounded text-white/80 transition-colors hover:bg-white/10 hover:text-white"
          title="重置为原始大小"
          onClick={() => setScale(1)}
        >
          <i className="fa-solid fa-arrows-to-circle size-3" />
        </button>
        <button
          type="button"
          className="flex size-7 items-center justify-center rounded text-white/80 transition-colors hover:bg-white/10 hover:text-white disabled:opacity-30"
          title="放大"
          disabled={scale >= 4}
          onClick={() => handleZoom(0.25)}
        >
          <i className="fa-solid fa-magnifying-glass-plus size-3.5" />
        </button>
      </div>
      {/* 原图：等比显示不超过可视区域；transform scale 实现缩放；点击图片自身不关闭 */}
      <img
        src={src}
        alt=""
        className="max-h-full max-w-full cursor-zoom-out rounded-md object-contain shadow-2xl transition-transform duration-150"
        style={{ transform: `scale(${scale})` }}
        onClick={(e) => e.stopPropagation()}
      />
    </div>,
    document.body
  )
}