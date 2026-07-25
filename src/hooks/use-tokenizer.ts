/**
 * Tokenizer 模块 React Hook
 *
 * 封装 tokenizer 相关的 Tauri Command 调用，
 * 提供分词器列表查询与 token 计数估算能力。
 */

import { useCommand } from './use-command'
import type {
  TokenCountInput,
  TokenCountResult,
  TokenizerInfo,
  MessageTokenCountInput,
} from '@/modules/tokenizer/types'

/**
 * 获取所有可用分词器信息列表
 *
 * 供模型配置页面的 tokenizer 选择器使用。
 */
export function useTokenizerList() {
  return useCommand<TokenizerInfo[], void>('tokenizer_list')
}

/**
 * 估算纯文本的 token 数
 *
 * @example
 * ```tsx
 * const { execute } = useTokenizerCount()
 * const result = await execute({ modelId: 'openai/gpt-4o', text: 'Hello world' })
 * ```
 */
export function useTokenizerCount() {
  return useCommand<TokenCountResult, { input: TokenCountInput }>('tokenizer_count')
}

/**
 * 估算消息列表的 token 数
 *
 * @example
 * ```tsx
 * const { execute } = useTokenizerMessageCount()
 * const result = await execute({
 *   input: {
 *     modelId: 'openai/gpt-4o',
 *     messages: [{ role: 'user', content: 'Hello' }],
 *   },
 * })
 * ```
 */
export function useTokenizerMessageCount() {
  return useCommand<TokenCountResult, { input: MessageTokenCountInput }>(
    'tokenizer_count_messages',
  )
}
