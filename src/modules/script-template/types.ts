/**
 * 脚本模板模块类型定义
 *
 * 与后端 `script_template/types.rs` 及提案 balance-script-templates 对齐。
 */

/** 脚本模板类型（本期仅额度监控） */
export type ScriptTemplateKind = 'balance'

/** 生命周期状态 */
export type ScriptTemplateStatus = 'draft' | 'active' | 'disabled'

/** 状态迁移动作 */
export type ScriptTemplateStatusAction = 'publish' | 'disable' | 'revert_to_draft'

/** 脚本模板 */
export interface ScriptTemplate {
  id: string
  name: string
  slug: string
  kind: string
  status: string
  description?: string
  scriptBody: string
  engine: string
  defaultTimeoutMs: number
  allowedHostsJson?: string
  snippetId?: string
  lastTestAt?: string
  lastTestOk?: boolean
  lastTestMessage?: string
  sortOrder: number
  createdAt: string
  updatedAt: string
}

/** 列表筛选 */
export interface ScriptTemplateListFilter {
  kind?: string
  status?: string
  keyword?: string
}

/** 创建输入 */
export interface CreateScriptTemplateInput {
  name: string
  slug: string
  kind?: string
  description?: string
  scriptBody?: string
  defaultTimeoutMs?: number
  allowedHostsJson?: string
  snippetId?: string
  sortOrder?: number
}

/** 更新输入 */
export interface UpdateScriptTemplateInput {
  name?: string
  slug?: string
  description?: string
  scriptBody?: string
  defaultTimeoutMs?: number
  allowedHostsJson?: string
  sortOrder?: number
}

/** 试运行输入 */
export interface ScriptTemplateTestInput {
  templateId: string
  providerId: string
  scriptBodyOverride?: string
  timeoutMs?: number
}

/** 试运行结果 */
export interface ScriptTemplateTestResult {
  ok: boolean
  snapshot?: import('@/modules/balance/types').BalanceSnapshot
  error?: string
  durationMs: number
  logs?: string[]
}

/** 引用该模板的供应商 */
export interface ScriptTemplateRef {
  providerId: string
  slug: string
  displayName: string
}

/** 内置 snippet */
export interface ScriptSnippet {
  id: string
  name: string
  description: string
  body: string
}

/** 下拉选择项 */
export interface ScriptTemplateSelectItem {
  id: string
  name: string
  slug: string
}
