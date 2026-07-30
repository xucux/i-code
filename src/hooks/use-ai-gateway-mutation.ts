import { invokeCommand } from '@/hooks/use-command'
import type {
  Provider,
  ModelConfig,
  GatewayModel,
  GatewaySettings,
  GatewayAuthKey,
  CreateProviderInput,
  UpdateProviderInput,
  CreateModelConfigInput,
  UpdateModelConfigInput,
  CreateGatewayModelInput,
  UpdateGatewayModelInput,
  UpdateGatewaySettingsInput,
  CreateGatewayAuthKeyInput,
  UpdateGatewayAuthKeyInput,
  ExportProviderInput,
  ImportProviderInput,
  PingMode,
  CallbackServerInfo,
} from '@/modules/ai-gateway/types'

/**
 * AI Gateway 写入操作集合
 *
 * 直接调用后端 Tauri Commands 完成供应商/模型配置/网关模型的增删改。
 * 错误会向上抛出，由调用方（UI 组件）捕获并展示。
 */

// ===== 供应商 =====

export async function createProvider(input: CreateProviderInput): Promise<Provider> {
  return invokeCommand<Provider>('gateway_provider_create', { input })
}

export async function updateProvider(
  id: string,
  input: UpdateProviderInput
): Promise<Provider> {
  return invokeCommand<Provider>('gateway_provider_update', { id, input })
}

export async function deleteProvider(id: string): Promise<void> {
  await invokeCommand<void>('gateway_provider_delete', { id })
}

export async function exportProvider(input: ExportProviderInput): Promise<string> {
  return invokeCommand<string>('gateway_provider_export', { input })
}

export async function importProvider(input: ImportProviderInput): Promise<Provider> {
  return invokeCommand<Provider>('gateway_provider_import', { input })
}

/** 检测所有供应商 URL 的网络连通性（直连/代理），结果通过事件推送，返回供应商总数 */
export async function pingProviders(mode: PingMode): Promise<number> {
  return invokeCommand<number>('gateway_provider_ping', { mode })
}

// ===== 模型配置 =====

export async function createModelConfig(input: CreateModelConfigInput): Promise<ModelConfig> {
  return invokeCommand<ModelConfig>('gateway_model_config_create', { input })
}

export async function updateModelConfig(
  id: string,
  input: UpdateModelConfigInput
): Promise<ModelConfig> {
  return invokeCommand<ModelConfig>('gateway_model_config_update', { id, input })
}

export async function deleteModelConfig(id: string): Promise<void> {
  await invokeCommand<void>('gateway_model_config_delete', { id })
}

// ===== 网关模型 =====

export async function createGatewayModel(input: CreateGatewayModelInput): Promise<GatewayModel> {
  return invokeCommand<GatewayModel>('gateway_model_create', { input })
}

export async function deleteGatewayModel(id: string): Promise<void> {
  await invokeCommand<void>('gateway_model_delete', { id })
}

export async function updateGatewayModel(
  id: string,
  input: UpdateGatewayModelInput
): Promise<GatewayModel> {
  return invokeCommand<GatewayModel>('gateway_model_update', { id, input })
}

// ===== 网关设置 =====

export async function getGatewaySettings(): Promise<GatewaySettings> {
  return invokeCommand<GatewaySettings>('gateway_settings_get', {})
}

export async function updateGatewaySettings(input: UpdateGatewaySettingsInput): Promise<GatewaySettings> {
  return invokeCommand<GatewaySettings>('gateway_settings_update', { input })
}

// ===== 网关认证 API Key =====

export async function createGatewayAuthKey(input: CreateGatewayAuthKeyInput): Promise<GatewayAuthKey> {
  return invokeCommand<GatewayAuthKey>('gateway_auth_key_create', { input })
}

export async function updateGatewayAuthKey(id: string, input: UpdateGatewayAuthKeyInput): Promise<GatewayAuthKey> {
  return invokeCommand<GatewayAuthKey>('gateway_auth_key_update', { id, input })
}

export async function deleteGatewayAuthKey(id: string): Promise<void> {
  return invokeCommand<void>('gateway_auth_key_delete', { id })
}

export async function listGatewayAuthKeys(): Promise<GatewayAuthKey[]> {
  return invokeCommand<GatewayAuthKey[]>('gateway_auth_key_list', {})
}

// ===== OAuth 回调服务器 =====

/** 列出当前活跃的 OAuth 回调服务器（仅内存数据） */
export async function listOauthCallbacks(): Promise<CallbackServerInfo[]> {
  return invokeCommand<CallbackServerInfo[]>('gateway_oauth_callback_list', {})
}

/** 强制关闭指定的 OAuth 回调服务器 */
export async function closeOauthCallback(id: string): Promise<boolean> {
  return invokeCommand<boolean>('gateway_oauth_callback_close', { id })
}

/**
 * 清空供应商的 OAuth token（保留端点配置等非敏感字段）
 *
 * 用于「重新授权」场景：用户勾选「删除历史认证信息」后，先调用此命令清空旧 token，
 * 再发起授权流程。仅清空 token/expires_at，保留 method、OAuth 端点配置等。
 */
export async function clearOauthToken(providerId: string): Promise<Provider> {
  return invokeCommand<Provider>('gateway_provider_clear_oauth_token', { providerId })
}
