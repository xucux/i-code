import { useMemo } from 'react'
import CodeMirror from '@uiw/react-codemirror'
import { yaml } from '@codemirror/lang-yaml'
import { createTheme } from '@uiw/codemirror-themes'
import { tags } from '@lezer/highlight'

// 使用 CSS 变量构建与项目主题融合的 CodeMirror 主题
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

interface CodeEditorProps {
  value: string
  onChange?: (value: string) => void
  language?: 'yaml' | 'text'
  placeholder?: string
  readOnly?: boolean
  minHeight?: string
}

/**
 * 轻量级代码/文本编辑器组件
 * 支持 YAML 等语言，自动根据项目主题高亮
 */
export function CodeEditor({
  value,
  onChange,
  language = 'yaml',
  placeholder,
  readOnly,
  minHeight = '200px',
}: CodeEditorProps) {
  // 根据语言动态加载 CodeMirror 扩展
  const extensions = useMemo(
    () => (language === 'yaml' ? [yaml()] : []),
    [language]
  )

  return (
    // 外层容器提供圆角与边框，保证与项目卡片风格一致
    // 日志/代码区域使用等宽字体 'JetBrains Mono', Consolas, 'Courier New', monospace
    <div
      className="overflow-hidden rounded-md border"
      style={{ fontFamily: "'JetBrains Mono', Consolas, 'Courier New', monospace" }}
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
        }}
      />
    </div>
  )
}
