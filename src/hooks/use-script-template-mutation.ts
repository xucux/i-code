/**
 * 脚本模板写入操作
 */

import { invokeCommand } from '@/hooks/use-command'
import type {
  CreateScriptTemplateInput,
  MarketplaceApplyInput,
  ScriptTemplate,
  ScriptTemplateRef,
  ScriptTemplateStatusAction,
  ScriptTemplateTestInput,
  ScriptTemplateTestResult,
  UpdateScriptTemplateInput,
} from '@/modules/script-template/types'

export async function createScriptTemplate(
  input: CreateScriptTemplateInput
): Promise<ScriptTemplate> {
  return invokeCommand<ScriptTemplate>('script_template_create', { input })
}

export async function updateScriptTemplate(
  id: string,
  input: UpdateScriptTemplateInput
): Promise<ScriptTemplate> {
  return invokeCommand<ScriptTemplate>('script_template_update', { id, input })
}

export async function deleteScriptTemplate(id: string): Promise<void> {
  await invokeCommand<void>('script_template_delete', { id })
}

export async function setScriptTemplateStatus(
  id: string,
  action: ScriptTemplateStatusAction
): Promise<ScriptTemplate> {
  return invokeCommand<ScriptTemplate>('script_template_set_status', { id, action })
}

export async function testScriptTemplate(
  input: ScriptTemplateTestInput
): Promise<ScriptTemplateTestResult> {
  return invokeCommand<ScriptTemplateTestResult>('script_template_test', { input })
}

export async function listScriptTemplateRefs(id: string): Promise<ScriptTemplateRef[]> {
  return invokeCommand<ScriptTemplateRef[]>('script_template_list_refs', { id })
}

export async function getScriptTemplate(id: string): Promise<ScriptTemplate> {
  return invokeCommand<ScriptTemplate>('script_template_get', { id })
}

/** 从公共仓市场应用为本地模板（默认 draft） */
export async function applyMarketplaceTemplate(
  input: MarketplaceApplyInput
): Promise<ScriptTemplate> {
  return invokeCommand<ScriptTemplate>('script_template_marketplace_apply', { input })
}
