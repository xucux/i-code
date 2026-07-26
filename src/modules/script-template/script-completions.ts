/**
 * CodeMirror 联想补全：系统变量 / 系统函数
 */

import {
  autocompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from '@codemirror/autocomplete'
import type { Extension } from '@codemirror/state'
import { SCRIPT_COMPLETIONS } from './script-catalog'

function toCompletions(): Completion[] {
  return SCRIPT_COMPLETIONS.map((item) => {
    // snippet: ${name} → CodeMirror apply plain text without braces for simplicity
    const apply = item.insert.replace(/\$\{(\w+)\}/g, '$1')
    return {
      label: item.label,
      type: item.type === 'variable' ? 'variable' : 'function',
      detail: item.detail,
      apply,
      boost: item.type === 'variable' ? 2 : 1,
      info: item.detail,
    } satisfies Completion
  })
}

const ALL = toCompletions()

/**
 * 在标识符 / 模块路径（::）前提供补全
 */
function scriptCompletionSource(context: CompletionContext): CompletionResult | null {
  // 匹配：标识符，或 obj.prop 中的 prop 前缀
  const word = context.matchBefore(/[A-Za-z_][\w:.]*/)
  if (!word || (word.from === word.to && !context.explicit)) {
    return null
  }

  const typed = word.text
  const lower = typed.toLowerCase()

  const options = ALL.filter((c) => {
    const label = c.label.toLowerCase()
    return label.startsWith(lower) || label.includes(lower)
  })

  if (options.length === 0) return null

  return {
    from: word.from,
    options,
    validFor: /^[\w:.]*$/,
  }
}

/** 脚本编辑器补全扩展 */
export function createScriptCompletions(): Extension {
  return autocompletion({
    override: [scriptCompletionSource],
    activateOnTyping: true,
    maxRenderedOptions: 40,
    defaultKeymap: true,
  })
}
