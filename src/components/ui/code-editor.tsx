"use client"

import { cn } from "@/lib/utils"
import { Textarea } from "@/components/ui/textarea"

export interface CodeEditorProps {
  /** 代码内容 */
  value: string
  /** 内容变更回调 */
  onChange?: (value: string) => void
  /** 语言/格式，用于样式提示（预留） */
  language?: string
  /** 是否只读 */
  readOnly?: boolean
  /** 自定义类名 */
  className?: string
  /** 自定义行内样式 */
  style?: React.CSSProperties
  /** 占位符 */
  placeholder?: string
}

/**
 * 简易代码编辑器
 *
 * 基于 Textarea 的轻量代码编辑组件，使用等宽字体与主题适配。
 * 用于 CLI 配置文件的预览与编辑。
 */
export function CodeEditor({
  value,
  onChange,
  readOnly = false,
  className,
  style,
  placeholder,
}: CodeEditorProps) {
  return (
    <Textarea
      value={value}
      onChange={(e) => onChange?.(e.target.value)}
      readOnly={readOnly}
      spellCheck={false}
      placeholder={placeholder}
      className={cn(
        "min-h-[200px] resize-y font-mono text-xs leading-relaxed",
        "scrollbar-thin scrollbar-track-transparent",
        className
      )}
      style={style}
    />
  )
}
