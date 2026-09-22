/**
 * 聊天气泡 Markdown 渲染的单测
 *
 * 覆盖两处纯函数逻辑（不涉及 DOM，vitest 默认 node 环境即可）：
 *
 * 1. `hasMarkdownSyntax` —— 纯文本快路径的判定精度。
 *    误判为 true 会让「3 * 4 = 12」这类短句被 `prose` 包裹而多出块级间距；
 *    漏判则 Markdown 语法会以原始符号显示。
 * 2. `isInsideOpenFence` —— 流式光标落点判定。
 *    判断失误会把光标 `<span>` 拼进代码围栏，被转义成字面量显示给用户。
 * 3. 解析管线 —— 公式（KaTeX）与代码内容的边界，以及非法公式的降级行为。
 */

import { describe, expect, it } from 'vitest'
import { marked } from 'marked'
import { hasMarkdownSyntax, isInsideOpenFence } from './message-markdown'
import { preprocessMarkdownAlerts } from '@/components/ui/markdown-content'

/** Alert 标题在单测里不需要真实翻译，取 key 末段即可 */
const t = (key: string) => key.split('.').pop() ?? key

/** 与 `ChatMarkdownBody` 内构造光标源码的方式保持一致 */
const CURSOR = ' <span class="inline-block animate-pulse text-muted-foreground">▍</span>'

const parse = (content: string, withCursor = false) =>
  marked.parse(preprocessMarkdownAlerts(withCursor ? `${content}${CURSOR}` : content, t)) as string

describe('hasMarkdownSyntax', () => {
  it('不含语法的纯文本应走快路径', () => {
    const plain = [
      '好的',
      '收到，我这就看',
      '3 * 4 = 12',
      '版本号 1.2.3 已发布',
      '范围 1) 到 5) 都覆盖了',
      '这是一个比较长的段落，里面用了中文标点，没有换行，也没有任何标记语法。',
      '第一行\n第二行\n第三行',
      // 单个 `$` 或金额区间不构成公式（公式定界符要求成对且不贴空白）
      '价格 $5 到 $10 之间',
      '总价 $ 100 元',
    ]
    for (const s of plain) {
      expect(hasMarkdownSyntax(s), `应判为纯文本：${JSON.stringify(s)}`).toBe(false)
    }
  })

  it('含块级或行内语法应走 Markdown', () => {
    const md = [
      '## 标题\n正文',
      '- 第一项\n- 第二项',
      '1. 第一步\n2. 第二步',
      '```js\nconst a = 1\n```',
      '> 引用内容',
      '| a | b |\n| - | - |\n| 1 | 2 |',
      '这里有 `code` 行内代码',
      '这是 **加粗** 文本',
      '这是 __加粗__ 文本',
      '看 [文档](https://example.com/a)',
      '![图](https://example.com/a.png)',
      '这是 ~~删除~~ 文本',
      '---',
      // 公式：行内与块级定界符（LaTeX 原生 `\(...\)` / `\[...\]` 与 `$` 系）
      '铁块体积 \\(30^3 = 27000\\) 立方厘米',
      '质能方程 $E = mc^2$ 的说明',
      '\\[a^2 + b^2 = c^2\\]',
      '$$\n\\frac{a}{b}\n$$',
    ]
    for (const s of md) {
      expect(hasMarkdownSyntax(s), `应判为 Markdown：${JSON.stringify(s)}`).toBe(true)
    }
  })

  it('空内容不触发解析', () => {
    expect(hasMarkdownSyntax('')).toBe(false)
  })
})

describe('isInsideOpenFence', () => {
  it('应按围栏奇偶数判断是否停在代码块内', () => {
    expect(isInsideOpenFence('```ts\nconst a = 1')).toBe(true)
    expect(isInsideOpenFence('```ts\nconst a = 1\n```')).toBe(false)
    expect(isInsideOpenFence('说明文字')).toBe(false)
    expect(isInsideOpenFence('aaa\n```\nbbb\n```\nccc\n```\nddd')).toBe(true)
    expect(isInsideOpenFence('~~~\nfoo')).toBe(true)
    expect(isInsideOpenFence('~~~\nfoo\n~~~')).toBe(false)
  })
})

describe('解析管线', () => {
  it('光标应并入最后一个块，不独立成行', () => {
    const html = parse('## 结论\n\n这是 **正文**', true)
    expect(html).toContain('<strong>正文</strong>')
    expect(html).toMatch(/<span class="inline-block animate-pulse[^>]*>▍<\/span><\/p>/)
  })

  it('代码围栏未闭合时光标必须挂在块外，不能被转义成字面量', () => {
    // 反例：把光标拼进源码会被 marked 当作代码内容转义
    expect(parse('看代码：\n\n```ts\nconst a = 1', true)).toContain('&lt;span class=&quot;')
  })

  it('GitHub Alert 应生成告警块并保留正文行内语法', () => {
    const html = parse('> [!NOTE]\n> 注意这里 **加粗**')
    expect(html).toContain('markdown-alert')
    expect(html).toContain('<strong>加粗</strong>')
  })

  it('表格与代码块应正常渲染', () => {
    const html = parse('| 名称 | 值 |\n| --- | --- |\n| a | 1 |\n\n```json\n{"a":1}\n```')
    expect(html).toContain('<table>')
    expect(html).toContain('language-json')
    // 代码内容只转义一次，不应出现双重转义
    expect(html).toContain('{&quot;a&quot;:1}')
  })

  it('LaTeX 公式应渲染为 KaTeX 结构', () => {
    // 行内 `\(...\)`：模型输出 LaTeX 原生定界符的默认写法
    expect(parse('铁块体积 \\(30^3 = 27000\\) 立方厘米')).toContain('class="katex"')
    // 行内 `$...$`
    expect(parse('质能方程 $E = mc^2$')).toContain('class="katex"')
    // 块级 `$$...$$` 与 `\[...\]`
    expect(parse('$$\n\\frac{a}{b}\n$$')).toContain('katex-display')
    expect(parse('\\[a^2 + b^2 = c^2\\]')).toContain('katex-display')
  })

  it('公式不得误伤行内代码与代码围栏', () => {
    expect(parse('用 `$a$` 取值')).toContain('<code>$a$</code>')
    expect(parse('```sh\n$x$\n```')).not.toContain('katex')
  })

  it('非法公式降级为错误文本，不中断整篇渲染', () => {
    const html = parse('前缀 $\\frac{1}{$ 后缀')
    expect(html).toContain('katex-error')
    expect(html).toContain('后缀')
  })
})
