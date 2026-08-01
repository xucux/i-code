/**
 * 脚本模板写入操作
 */

import { invokeCommand } from '@/hooks/use-command'
import type {
  CreateScriptTemplateInput,
  MarketplaceApplyInput,
  ScriptStorageEntry,
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

// ===== 脚本公共存储（script-storage.json）=====

export async function viewScriptStorage(): Promise<ScriptStorageEntry[]> {
  return invokeCommand<ScriptStorageEntry[]>('script_storage_view')
}

export async function setScriptStorage(
  key: string,
  value: unknown,
  ttlMs?: number | null
): Promise<void> {
  await invokeCommand<void>('script_storage_set', { key, value, ttlMs: ttlMs ?? null })
}

export async function deleteScriptStorage(key: string): Promise<void> {
  await invokeCommand<void>('script_storage_delete', { key })
}

export async function clearScriptStorage(): Promise<void> {
  await invokeCommand<void>('script_storage_clear')
}
