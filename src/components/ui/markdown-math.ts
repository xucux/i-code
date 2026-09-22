/**
 * Markdown 数学公式渲染（KaTeX + marked 扩展）
 *
 * ## 支持的分隔符
 *
 * | 形式 | 定界符 | 排版 |
 * |------|--------|------|
 * | 行内 | `$...$`、`\(...\)` | 与正文同行的行内公式 |
 * | 块级 | `$$...$$`、`\[...\]` | 独占一行、居中 |
 *
 * `\(...\)` / `\[...\]` 是 LaTeX 原生定界符（也是各模型默认输出的写法），
 * 因此必须支持，不能只认 `$` 系。
 *
 * ## 为什么用 marked 扩展而不是预处理占位符
 *
 * 扩展的 tokenizer 只在「当前解析位置」被调用：行内代码 `` `$a$` `` 与代码围栏
 * 会在解析推进到 `$` 之前，就被 marked 自身的 codespan / fences 规则整段吃掉，
 * 因此公式永远不会误伤代码内容。预处理器（正则扫全文替换）无法感知这种上下文，
 * 会把代码块里的 `$` 也替换掉。
 *
 * ## 安全边界
 *
 * - `trust: false`（默认）：禁用 `\href` / `\htmlClass` / `\includegraphics` 等
 *   可注入 HTML 或外链的命令，公式源码来自模型输出，不可信。
 * - `throwOnError: false`：非法公式降级为红色错误文本，不抛错中断整篇渲染
 *   （流式输出期间尤其重要——半个公式随时可能出现）。
 * - 额外兜底 `try/catch`：KaTeX 仍可能抛出非 `ParseError` 的异常（如深层嵌套），
 *   此时降级为转义后的原文，保证单条公式的异常不会打挂整条消息。
 */

import katex from 'katex'
import type { TokenizerAndRendererExtension } from 'marked'

/** KaTeX 渲染选项：集中在此处便于审计（勿在调用点分散覆盖） */
const KATEX_OPTIONS = {
  throwOnError: false,
  strict: false,
  trust: false,
} as const

/**
 * 行内 `$...$`
 *
 * 两侧均不贴空白：`价格 $5 到 $10` 这类文本不会被误判为公式，
 * 与 GitHub / KaTeX auto-render 的判定规则一致。
 */
const INLINE_DOLLAR = /^\$(?!\s)((?:\\.|[^$\n\\])+?)(?<!\s)\$/
/** 行内 `\(...\)` */
const INLINE_PAREN = /^\\\(([\s\S]+?)\\\)/
/** 段落中间出现的 `\[...\]`：定界符本身表达块级语义，按 display 排版 */
const INLINE_BRACKET = /^\\\[([\s\S]+?)\\\]/
/** 块级 `$$...$$`：允许 ≤3 空格缩进（与 Markdown 规范一致，4 空格属代码块） */
const BLOCK_DOLLAR = /^[ \t]{0,3}\$\$[ \t]*\n?([\s\S]+?)\n?[ \t]*\$\$/
/** 块级 `\[...\]` */
const BLOCK_BRACKET = /^[ \t]{0,3}\\\[[ \t]*\n?([\s\S]+?)\n?[ \t]*\\\]/

/** 公式 token 载荷 */
interface MathToken {
  type: 'mathInline' | 'mathBlock'
  raw: string
  /** KaTeX 源码（已 trim） */
  text: string
  displayMode: boolean
}

/** 由正则结果构造行内公式 token */
function toInlineToken(match: RegExpExecArray, displayMode: boolean): MathToken {
  return { type: 'mathInline', raw: match[0], text: match[1].trim(), displayMode }
}

/** HTML 转义（仅兜底分支使用：公式源码不可信，不能直接当 HTML 注入） */
function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (ch) => `&#${ch.charCodeAt(0)};`)
}

/**
 * 渲染公式为 HTML
 *
 * 正常路径由 KaTeX 输出；异常时降级为转义后的原文（保留定界符便于用户看出原文）。
 */
function renderMath(tex: string, displayMode: boolean): string {
  try {
    return katex.renderToString(tex, { ...KATEX_OPTIONS, displayMode })
  } catch {
    const [open, close] = displayMode ? ['\\[', '\\]'] : ['\\(', '\\)']
    return `<code>${escapeHtml(`${open}${tex}${close}`)}</code>`
  }
}

/** 行内公式扩展：`$...$` / `\(...\)` / `\[...\]` */
const inlineMath: TokenizerAndRendererExtension = {
  name: 'mathInline',
  level: 'inline',
  // 提示 marked 下一个可能的行内扩展起点，避免整段文本被当作纯文本吞掉
  start(src) {
    const index = src.search(/\$|\\\(|\\\[/)
    return index < 0 ? undefined : index
  },
  tokenizer(src) {
    const paren = INLINE_PAREN.exec(src)
    if (paren) return toInlineToken(paren, false)
    const bracket = INLINE_BRACKET.exec(src)
    if (bracket) return toInlineToken(bracket, true)
    const dollar = INLINE_DOLLAR.exec(src)
    if (dollar) return toInlineToken(dollar, false)
    return undefined
  },
  renderer(token) {
    const { text, displayMode } = token as unknown as MathToken
    return renderMath(text, displayMode)
  },
}

/** 块级公式扩展：`$$...$$` / `\[...\]` */
const blockMath: TokenizerAndRendererExtension = {
  name: 'mathBlock',
  level: 'block',
  start(src) {
    const match = /^[ \t]{0,3}(?:\$\$|\\\[)/m.exec(src)
    return match?.index
  },
  tokenizer(src) {
    const match = BLOCK_DOLLAR.exec(src) ?? BLOCK_BRACKET.exec(src)
    if (!match) return undefined
    return { type: 'mathBlock', raw: match[0], text: match[1].trim(), displayMode: true }
  },
  renderer(token) {
    const { text } = token as unknown as MathToken
    return renderMath(text, true)
  },
}

/** 公式 marked 扩展（在 `markdown-content.tsx` 中随 marked 单例注册一次） */
export const MATH_EXTENSIONS: TokenizerAndRendererExtension[] = [blockMath, inlineMath]

/**
 * 正文是否含公式语法
 *
 * 供聊天气泡的纯文本快路径判定使用（见 `message-markdown.tsx`）。
 * 判定口径与扩展保持一致：只认**成对**的定界符，宁可漏判（按纯文本显示）
 * 也不误判——误判会让 `价格 $5` 这类短句被 `prose` 包裹而多出块级间距。
 */
export function hasMathSyntax(content: string): boolean {
  if (!content) return false
  // 块级：行首 `$$` 或 `\[`
  if (/^[ \t]{0,3}(?:\$\$|\\\[)/m.test(content)) return true
  // 行内 `\(...\)` / `\[...\]`
  if (/\\\([\s\S]+?\\\)/.test(content)) return true
  if (/\\\[[\s\S]+?\\\]/.test(content)) return true
  // 行内 `$...$`：两侧不贴空白，且前一个字符不能是 `$` 或 `\`
  if (/(^|[^\\$])\$(?!\s)(?:\\.|[^$\n\\])+?(?<!\s)\$/m.test(content)) return true
  return false
}
