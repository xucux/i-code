import { useEffect, useState } from 'react'
import { invokeCommand } from '@/hooks/use-command'
import type {
  CliConfigFileInspection,
  CliModelMapping,
  CliProfile,
  CliProvider,
  CliType,
} from '@/modules/cli-management/types'

/**
 * 获取 CLI 档案列表
 *
 * 调用后端 `cli_profile_list` 命令。
 */
export function useCliProfiles(): {
  profiles: CliProfile[]
  loading: boolean
  refetch: () => void
} {
  const [profiles, setProfiles] = useState<CliProfile[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    setLoading(true)
    try {
      const result = await invokeCommand<CliProfile[]>('cli_profile_ensure_defaults')
      setProfiles(result)
    } catch {
      setProfiles([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [])

  return { profiles, loading, refetch: load }
}

/**
 * 获取指定 CLI 档案下的供应商绑定列表
 *
 * 调用后端 `cli_provider_list` 命令。
 */
export function useCliProviders(profileId: string | null): {
  providers: CliProvider[]
  loading: boolean
  refetch: () => void
} {
  const [providers, setProviders] = useState<CliProvider[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    if (!profileId) {
      setProviders([])
      return
    }
    setLoading(true)
    try {
      const result = await invokeCommand<CliProvider[]>('cli_provider_list', {
        cliProfileId: profileId,
      })
      setProviders(result)
    } catch {
      setProviders([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [profileId])

  return { providers, loading, refetch: load }
}

/**
 * 获取指定 CLI 供应商下的模型映射列表
 *
 * 调用后端 `cli_model_mapping_list` 命令。
 */
export function useCliModelMappings(providerId: string | null): {
  mappings: CliModelMapping[]
  loading: boolean
  refetch: () => void
} {
  const [mappings, setMappings] = useState<CliModelMapping[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    if (!providerId) {
      setMappings([])
      return
    }
    setLoading(true)
    try {
      const result = await invokeCommand<CliModelMapping[]>('cli_model_mapping_list', {
        cliProviderId: providerId,
      })
      setMappings(result)
    } catch {
      setMappings([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [providerId])

  return { mappings, loading, refetch: load }
}

/** 探测 CLI 配置文件位置与语法状态，不读取配置正文到前端。 */
export function inspectCliConfigFile(
  cliType: CliType,
  configuredPath?: string
): Promise<CliConfigFileInspection> {
  return invokeCommand<CliConfigFileInspection>('cli_config_inspect', {
    cliType,
    configuredPath: configuredPath?.trim() || null,
  })
}

/** 检测指定客户端 CLI 是否在 PATH 中可用 */
export function checkCliClient(cliType: CliType): Promise<boolean> {
  return invokeCommand<boolean>('cli_client_check', { cliType })
}
