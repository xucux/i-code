import { invokeCommand } from '@/hooks/use-command'
import type {
  CliConfigFileContent,
  CliModelMapping,
  CliProfile,
  CliProvider,
  CliType,
  CreateCliProfileInput,
  UpdateCliProfileInput,
  CreateCliProviderInput,
  UpdateCliProviderInput,
  CreateCliModelMappingInput,
  UpdateCliModelMappingInput,
} from '@/modules/cli-management/types'

/**
 * CLI 管理模块写入操作集合
 */

// ===== CLI 档案 =====

export async function createCliProfile(input: CreateCliProfileInput): Promise<CliProfile | null> {
  try {
    return await invokeCommand<CliProfile>('cli_profile_create', { input })
  } catch {
    return null
  }
}

export async function updateCliProfile(
  id: string,
  input: UpdateCliProfileInput
): Promise<CliProfile | null> {
  try {
    return await invokeCommand<CliProfile>('cli_profile_update', { id, input })
  } catch {
    return null
  }
}

export async function deleteCliProfile(id: string): Promise<boolean> {
  try {
    await invokeCommand<void>('cli_profile_delete', { id })
    return true
  } catch {
    return false
  }
}

// ===== CLI 供应商绑定 =====

export async function createCliProvider(input: CreateCliProviderInput): Promise<CliProvider | null> {
  try {
    return await invokeCommand<CliProvider>('cli_provider_create', { input })
  } catch {
    return null
  }
}

export async function updateCliProvider(
  id: string,
  input: UpdateCliProviderInput
): Promise<CliProvider | null> {
  try {
    return await invokeCommand<CliProvider>('cli_provider_update', { id, input })
  } catch {
    return null
  }
}

export async function deleteCliProvider(id: string): Promise<boolean> {
  try {
    await invokeCommand<void>('cli_provider_delete', { id })
    return true
  } catch {
    return false
  }
}

// ===== CLI 模型映射 =====

export async function createCliModelMapping(
  input: CreateCliModelMappingInput
): Promise<CliModelMapping | null> {
  try {
    return await invokeCommand<CliModelMapping>('cli_model_mapping_create', { input })
  } catch {
    return null
  }
}

export async function updateCliModelMapping(
  id: string,
  input: UpdateCliModelMappingInput
): Promise<CliModelMapping | null> {
  try {
    return await invokeCommand<CliModelMapping>('cli_model_mapping_update', { id, input })
  } catch {
    return null
  }
}

export async function deleteCliModelMapping(id: string): Promise<boolean> {
  try {
    await invokeCommand<void>('cli_model_mapping_delete', { id })
    return true
  } catch {
    return false
  }
}

// ===== CLI 配置文件内容读写 =====

/** 读取 CLI 配置文件内容（不含 Secret 正文） */
export async function readCliConfigFile(
  cliType: CliType,
  configuredPath?: string
): Promise<CliConfigFileContent | null> {
  try {
    return await invokeCommand<CliConfigFileContent>('cli_config_read', {
      cliType,
      configuredPath: configuredPath?.trim() || null,
    })
  } catch {
    return null
  }
}

/** 保存 CLI 配置文件内容 */
export async function saveCliConfigFile(
  cliType: CliType,
  configuredPath: string | undefined,
  content: string
): Promise<CliConfigFileContent | null> {
  try {
    return await invokeCommand<CliConfigFileContent>('cli_config_save', {
      cliType,
      configuredPath: configuredPath?.trim() || null,
      content,
    })
  } catch {
    return null
  }
}
