/**
 * 代码编辑器
 *
 * 基于 @uiw/react-codemirror，主题使用 CSS 变量与项目一致。
 * - yaml：CLI 配置等
 * - javascript：脚本模板（Rhai 语法近似高亮）
 * - text：纯文本
 * - 可选 extensions：脚本联想补全等
 */

import { useMemo } from 'react'
import CodeMirror from '@uiw/react-codemirror'
import { yaml } from '@codemirror/lang-yaml'
import { javascript } from '@codemirror/lang-javascript'
import type { Extension } from '@codemirror/state'
import { createTheme } from '@uiw/codemirror-themes'
import { tags } from '@lezer/highlight'
import { cn } from '@/lib/utils'

const icodeTheme = createTheme({
  theme: 'light',
  settings: {
    background: 'hsl(var(--card))',
    foreground: 'hsl(var(--card-foreground))',
    caret: 'hsl(var(--primary))',
    selection: 'hsl(var(--primary) / 0.15)',
    selectionMatch: 'hsl(var(--primary) / 0.1)',
    gutterBackground: 'hsl(var(--muted))',
    gutterForeground: 'hsl(var(--muted-foreground))',
    gutterBorder: 'hsl(var(--border))',
    lineHighlight: 'hsl(var(--muted) / 0.5)',
  },
  styles: [
    { tag: tags.comment, color: 'hsl(var(--muted-foreground))', fontStyle: 'italic' },
    { tag: tags.keyword, color: 'hsl(var(--primary))' },
    { tag: tags.string, color: 'hsl(var(--accent-foreground))' },
    { tag: tags.number, color: 'hsl(var(--secondary-foreground))' },
    { tag: tags.operator, color: 'hsl(var(--foreground))' },
    { tag: tags.punctuation, color: 'hsl(var(--muted-foreground))' },
    { tag: tags.propertyName, color: 'hsl(var(--foreground))' },
    { tag: tags.className, color: 'hsl(var(--primary))' },
    { tag: tags.name, color: 'hsl(var(--foreground))' },
    { tag: tags.heading, color: 'hsl(var(--primary))', fontWeight: 'bold' },
  ],
})

export interface CodeEditorProps {
  /** 代码内容 */
  value: string
  /** 内容变更回调 */
  onChange?: (value: string) => void
  /** 语言：yaml / javascript（Rhai 近似）/ text */
  language?: 'yaml' | 'javascript' | 'text' | string
  /** 是否只读 */
  readOnly?: boolean
  /** 自定义类名 */
  className?: string
  /** 自定义行内样式 */
  style?: React.CSSProperties
  /** 占位符 */
  placeholder?: string
  /** 最小高度 */
  minHeight?: string
  /** 额外 CodeMirror 扩展（如脚本补全） */
  extensions?: Extension[]
}

/**
 * 代码编辑器（CodeMirror）
 *
 * 用于 CLI 配置预览与脚本模板编辑；高度由 minHeight / style 控制。
 */
export function CodeEditor({
  value,
  onChange,
  language = 'text',
  readOnly = false,
  className,
  style,
  placeholder,
  minHeight = '200px',
  extensions: extraExtensions,
}: CodeEditorProps) {
  const extensions = useMemo(() => {
    const base: Extension[] = []
    if (language === 'yaml') base.push(yaml())
    if (language === 'javascript') base.push(javascript())
    if (extraExtensions?.length) base.push(...extraExtensions)
    return base
  }, [language, extraExtensions])

  return (
    <div
      className={cn('overflow-hidden rounded-md border', className)}
      style={{
        fontFamily: "'JetBrains Mono', Consolas, 'Courier New', monospace",
        ...style,
      }}
    >
      <CodeMirror
        value={value}
        height={minHeight}
        extensions={extensions}
        theme={icodeTheme}
        placeholder={placeholder}
        readOnly={readOnly}
        onChange={onChange}
        className="text-sm"
        basicSetup={{
          lineNumbers: true,
          highlightActiveLineGutter: true,
          highlightActiveLine: true,
          foldGutter: false,
          autocompletion: true,
        }}
      />
    </div>
  )
}
