"use client"

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Card, CardContent, CardDescription, CardHeader } from '@/components/ui/card'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { invokeCommand } from '@/hooks/use-command'
import { useTranslation } from '@/modules/i18n/use-translation'
import { parseAuthConfig, type Provider } from '@/modules/ai-gateway/types'
import { toast } from 'sonner'
import { toIcodeError } from '@/core/errors'

/**
 * 单个 Claude CLI 模型映射项
 */
export interface ClaudeModelMappingItem {
  /** 映射行唯一标识 */
  id: string
  /** Claude CLI 固定模型角色 */
  role: ClaudeModelRole
  /** 前端展示名称 */
  displayName: string
  /** 实际请求模型 ID */
  actualModel: string
  /** 是否声明支持 1M 上下文 */
  supports1M: boolean
}

/** Claude CLI 固定模型角色 */
export type ClaudeModelRole = 'Sonnet' | 'Opus' | 'Fable' | 'Haiku'

interface ClaudeModelMappingEditorProps {
  /** 后端已保存的映射列表（组件会合并为固定 4 角色） */
  mappings: ClaudeModelMappingItem[]
  /** 默认兜底模型 */
  fallbackModel: string
  /** 当前可用网关模型列表，用于下拉选择 */
  availableModels?: string[]
  /** 映射变更回调 */
  onMappingsChange?: (mappings: ClaudeModelMappingItem[]) => void
  /** 兜底模型变更回调 */
  onFallbackChange?: (value: string) => void
  /** 当前绑定的 Gateway 供应商，用于拉取模型与导入 API Key */
  gatewayProvider?: Provider
  /** 当前 CLI 供应商路由模式：1=走本地网关；0=直连上游 */
  routeMode?: number
  /** Anthropic API Key（明文） */
  apiKey?: string
  /** API Key 变更回调 */
  onApiKeyChange?: (value: string) => void
  /** 自定义类名 */
  className?: string
}

/** 固定角色与默认模型 */
const DEFAULT_ROLES: Omit<ClaudeModelMappingItem, 'id'>[] = [
  { role: 'Sonnet', displayName: 'claude-opus-4-8', actualModel: 'claude-opus-4-8', supports1M: true },
  { role: 'Opus', displayName: 'claude-opus-4-8', actualModel: 'claude-opus-4-8', supports1M: true },
  { role: 'Fable', displayName: 'claude-opus-4-8', actualModel: 'claude-opus-4-8', supports1M: true },
  { role: 'Haiku', displayName: 'claude-sonnet-4-5-20250929', actualModel: 'claude-sonnet-4-5-20250929', supports1M: false },
]

/** 默认兜底模型 */
export const DEFAULT_FALLBACK_MODEL = 'claude-opus-4-8'

/**
 * Claude CLI 专属模型映射编辑器
 *
 * 固定展示 Sonnet / Opus / Fable / Haiku 四个角色，支持：
 * - 显示名称编辑
 * - 实际请求模型编辑或下拉选择
 * - 1M 上下文声明开关
 * - 默认兜底模型输入
 * - 一键恢复默认映射
 *
 * 风格贴合 CLI 管理界面：紧凑、使用 shadcn 组件与 Font Awesome 图标。
 */
export function ClaudeModelMappingEditor({
  mappings,
  fallbackModel,
  availableModels = [],
  onMappingsChange,
  onFallbackChange,
  gatewayProvider,
  routeMode = 1,
  apiKey = '',
  onApiKeyChange,
  className,
}: ClaudeModelMappingEditorProps) {
  const { t } = useTranslation()
  // 使用 ref 避免 callback 引用变化导致初始同步 effect 重复执行
  const onMappingsChangeRef = useRef(onMappingsChange)
  useEffect(() => {
    onMappingsChangeRef.current = onMappingsChange
  }, [onMappingsChange])

  /** 将传入映射与固定角色合并，确保四行始终可见；显示名称与 1M 声明使用默认值 */
  const mergedMappings = useMemo<ClaudeModelMappingItem[]>(() => {
    return DEFAULT_ROLES.map((defaultRole) => {
      const backend = mappings.find((m) => m.role === defaultRole.role)
      return {
        id: backend?.id ?? `role-${defaultRole.role.toLowerCase()}`,
        role: defaultRole.role,
        displayName: defaultRole.displayName,
        actualModel: backend?.actualModel ?? defaultRole.actualModel,
        supports1M: defaultRole.supports1M,
      }
    })
  }, [mappings])

  /** 本地编辑态：组件卸载或传入变化时同步 */
  const [localMappings, setLocalMappings] = useState<ClaudeModelMappingItem[]>(mergedMappings)
  const [localFallbackModel, setLocalFallbackModel] = useState(fallbackModel)

  useEffect(() => {
    setLocalMappings(mergedMappings)
    // 初始加载或外部 mappings 变化时，把合并后的完整 4 角色映射回传父级
    onMappingsChangeRef.current?.(mergedMappings)
  }, [mergedMappings])

  useEffect(() => {
    setLocalFallbackModel(fallbackModel)
  }, [fallbackModel])

  /** 通过拉取获得的模型列表，与可用网关模型合并后供下拉选择 */
  const [fetchedModels, setFetchedModels] = useState<string[]>([])
  const [fetching, setFetching] = useState(false)
  const [fetchError, setFetchError] = useState<string | null>(null)

  /** 当前是否为网关路由模式 */
  const isGatewayRoute = routeMode === 1

  /** 切换 Gateway 供应商时清空拉取结果，避免把上一个供应商的模型映射到当前供应商 */
  useEffect(() => {
    setFetchedModels([])
    setFetchError(null)
  }, [gatewayProvider?.id])

  /** 将供应商返回的原始模型 ID 转换为前端实际使用的模型值 */
  const normalizeFetchedId = useCallback(
    (id: string) => {
      if (isGatewayRoute && gatewayProvider?.slug) {
        return `${gatewayProvider.slug}/${id}`
      }
      return id
    },
    [isGatewayRoute, gatewayProvider?.slug]
  )

  /** 下拉可选项：已暴露网关模型 + 拉取结果，去重 */
  const allModelOptions = useMemo(() => {
    const list = [...availableModels, ...fetchedModels]
    const seen = new Set<string>()
    const result: string[] = []
    for (const m of list) {
      if (!m || seen.has(m)) continue
      seen.add(m)
      result.push(m)
    }
    return result
  }, [availableModels, fetchedModels])

  /** 为固定角色从模型列表中挑选最佳匹配 */
  const findBestModelForRole = useCallback(
    (role: ClaudeModelRole, models: string[]) => {
      if (models.length === 0) return undefined
      const token = role.toLowerCase()
      const match = models.find((m) => m.toLowerCase().includes(token))
      if (match) return match
      // Fable 没有对应官方模型时回退到第一个可用模型
      if (role === 'Fable') return models[0]
      return undefined
    },
    []
  )

  /** 应用拉取到的模型列表：自动映射到固定角色并扩展下拉选项 */
  const applyFetchedModels = useCallback(
    (models: string[]) => {
      setFetchedModels(models)
      if (models.length === 0) return
      const next = localMappings.map((item) => {
        const best = findBestModelForRole(item.role, models)
        if (!best) return item
        return {
          ...item,
          actualModel: best,
          displayName: best.replace(/^[^/]+\//, ''),
        }
      })
      setLocalMappings(next)
      onMappingsChange?.(next)
    },
    [findBestModelForRole, localMappings, onMappingsChange]
  )

  /** 按指定协议从 Gateway 供应商实时拉取模型列表 */
  const handleFetchByProtocol = useCallback(
    async (protocol: 'openai-compatible' | 'anthropic-native') => {
      if (!gatewayProvider) {
        toast.error(t('cli.claude.modelMapping.noProvider'))
        return
      }
      setFetching(true)
      setFetchError(null)
      try {
        const models = await invokeCommand<string[]>('gateway_fetch_models_by_protocol', {
          providerId: gatewayProvider.id,
          protocol,
        })
        applyFetchedModels(models.map(normalizeFetchedId))
      } catch (err) {
        const message = toIcodeError(err).message
        setFetchError(message)
        toast.error(t('cli.claude.modelMapping.fetchFailed'))
      } finally {
        setFetching(false)
      }
    },
    [gatewayProvider, normalizeFetchedId, applyFetchedModels, t]
  )

  /** 从 Gateway 供应商 authJson 中导入 API Key 并解密为明文 */
  const handleImportApiKey = useCallback(async () => {
    if (!gatewayProvider) {
      toast.error(t('cli.claude.modelMapping.noProvider'))
      return
    }
    const auth = parseAuthConfig(gatewayProvider)
    if (auth?.method !== 'api-key' || !auth.apiKey) {
      toast.error(t('cli.claude.modelMapping.noApiKeyInProvider'))
      return
    }
    try {
      const plaintext = await invokeCommand<string>('secret_decrypt_text', { value: auth.apiKey })
      onApiKeyChange?.(plaintext)
      toast.success(t('cli.claude.modelMapping.apiKeyImported'))
    } catch (err) {
      toast.error(toIcodeError(err).message)
    }
  }, [gatewayProvider, onApiKeyChange, t])

  /** 更新单条映射字段并触发回调 */
  const updateMapping = useCallback(
    (id: string, field: keyof ClaudeModelMappingItem, value: string | boolean) => {
      const next = localMappings.map((item) =>
        item.id === id ? { ...item, [field]: value } : item
      )
      setLocalMappings(next)
      onMappingsChange?.(next)
    },
    [localMappings, onMappingsChange]
  )

  /** 更新兜底模型 */
  const updateFallback = useCallback(
    (value: string) => {
      setLocalFallbackModel(value)
      onFallbackChange?.(value)
    },
    [onFallbackChange]
  )

  /** 一键恢复默认映射 */
  const handleAutoSetup = useCallback(() => {
    const next = DEFAULT_ROLES.map((role) => ({
      ...role,
      id: `role-${role.role.toLowerCase()}`,
    }))
    setLocalMappings(next)
    setLocalFallbackModel(DEFAULT_FALLBACK_MODEL)
    onMappingsChange?.(next)
    onFallbackChange?.(DEFAULT_FALLBACK_MODEL)
  }, [onMappingsChange, onFallbackChange])

  const hasModelOptions = allModelOptions.length > 0

  return (
    <Card className={cn('w-full', className)}>
      <CardHeader className="flex flex-row items-start justify-between items-center gap-4 p-0 pb-3">
        <div className="space-y-1">
          {/* <CardTitle className="text-sm">模型映射</CardTitle> */}
          <CardDescription className="text-xs">
            {t('cli.claude.modelMapping.description')}
          </CardDescription>
        </div>
        <div className="flex items-center gap-2">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-7 gap-1 px-2 text-xs"
                disabled={fetching}
              >
                <i className={cn('fa-solid text-[10px]', fetching ? 'fa-spinner fa-spin' : 'fa-cloud-arrow-down')} />
                {t('cli.claude.modelMapping.fetchModels')}
                <i className="fa-solid fa-chevron-down text-[10px]" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem
                onClick={() => handleFetchByProtocol('anthropic-native')}
                disabled={!gatewayProvider}
              >
                {t('cli.claude.modelMapping.anthropicNative')}
              </DropdownMenuItem>
              <DropdownMenuItem
                onClick={() => handleFetchByProtocol('openai-compatible')}
                disabled={!gatewayProvider}
              >
                {t('cli.claude.modelMapping.openaiCompatible')}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 gap-1 px-2 text-xs"
            onClick={handleAutoSetup}
          >
            <i className="fa-solid fa-bolt text-[10px]" />
            {t('cli.claude.modelMapping.autoSetup')}
          </Button>
        </div>
      </CardHeader>

      <CardContent className="space-y-4 p-0">
        {/* 映射表头 */}
        <div className="grid grid-cols-12 gap-2 px-1 text-xs text-muted-foreground">
          <div className="col-span-2">{t('cli.claude.modelMapping.roleHeader')}</div>
          <div className="col-span-3">{t('cli.claude.modelMapping.displayNameHeader')}</div>
          <div className="col-span-5">{t('cli.claude.modelMapping.actualModelHeader')}</div>
          <div className="col-span-2 text-right">{t('cli.claude.modelMapping.supports1MHeader')}</div>
        </div>

        {/* 映射行 */}
        <div className="space-y-2">
          {localMappings.map((item) => (
            <div
              key={item.id}
              className="grid grid-cols-12 items-center gap-2 rounded-md border bg-background/50 py-1.5 transition-colors hover:bg-accent/30"
            >
              {/* 角色 */}
              <div className="col-span-2 px-1 text-xs font-medium">{item.role}</div>

              {/* 显示名称 */}
              <div className="col-span-3">
                <Input
                  value={item.displayName}
                  onChange={(e) => updateMapping(item.id, 'displayName', e.target.value)}
                  className="h-7 text-xs"
                  placeholder={t('cli.claude.modelMapping.displayNamePlaceholder')}
                />
              </div>

              {/* 实际请求模型：输入框 + 可选下拉 */}
              <div className="col-span-5">
                <div className="relative flex items-center">
                  <Input
                    value={item.actualModel}
                    onChange={(e) => updateMapping(item.id, 'actualModel', e.target.value)}
                    className={cn('h-7 text-xs', hasModelOptions && 'pr-7')}
                    placeholder={t('cli.claude.modelMapping.actualModelPlaceholder')}
                  />
                  {hasModelOptions && (
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <button
                          type="button"
                          className="absolute right-1 top-1/2 flex h-5 w-5 -translate-y-1/2 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                          aria-label={t('cli.claude.modelMapping.selectModel')}
                          title={t('cli.claude.modelMapping.selectModel')}
                        >
                          <i className="fa-solid fa-chevron-down text-[10px]" />
                        </button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end" className="max-h-60 overflow-y-auto">
                        {allModelOptions.map((model) => (
                          <DropdownMenuItem
                            key={`${item.id}-${model}`}
                            onClick={() => updateMapping(item.id, 'actualModel', model)}
                          >
                            {model}
                          </DropdownMenuItem>
                        ))}
                      </DropdownMenuContent>
                    </DropdownMenu>
                  )}
                </div>
              </div>

              {/* 1M 支持开关 */}
              <div className="col-span-2 flex items-center justify-end gap-2 pr-2">
                <Switch
                  id={`${item.id}-supports-1m`}
                  checked={item.supports1M}
                  onCheckedChange={(checked) => updateMapping(item.id, 'supports1M', checked)}
                  className="data-[state=checked]:bg-primary"
                />
                <Label
                  htmlFor={`${item.id}-supports-1m`}
                  className="cursor-pointer text-xs tabular-nums text-muted-foreground"
                >
                  1M
                </Label>
              </div>
            </div>
          ))}
        </div>

        {/* 拉取错误提示 */}
        {fetchError && (
          <p className="text-xs text-destructive">
            {t('cli.claude.modelMapping.fetchError')}: {fetchError}
          </p>
        )}

        {/* Anthropic API Key */}
        <div className="space-y-1.5 rounded-md border bg-background/50 px-3 py-2.5">
          <div className="flex items-center justify-between">
            <Label htmlFor="claude-api-key" className="text-xs font-medium text-muted-foreground">
              {t('cli.claude.modelMapping.apiKeyLabel')}
            </Label>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-6 gap-1 px-2 text-[11px]"
              onClick={handleImportApiKey}
              disabled={!gatewayProvider}
            >
              <i className="fa-solid fa-key text-[10px]" />
              {t('cli.claude.modelMapping.importApiKey')}
            </Button>
          </div>
          <Input
            id="claude-api-key"
            type="password"
            value={apiKey}
            onChange={(e) => onApiKeyChange?.(e.target.value)}
            placeholder={t('cli.claude.modelMapping.apiKeyPlaceholder')}
            className="h-8 text-xs"
          />
          <p className="text-xs text-muted-foreground">
            {t('cli.claude.modelMapping.apiKeyDesc')}
          </p>
        </div>

        {/* 默认兜底模型 */}
        <div className="space-y-1.5 border-t pt-3">
          <Label htmlFor="claude-fallback-model" className="text-xs font-medium">
            {t('cli.claude.modelMapping.fallbackModel')}
          </Label>
          <Input
            id="claude-fallback-model"
            value={localFallbackModel}
            onChange={(e) => updateFallback(e.target.value)}
            className="h-8 text-xs"
            placeholder={t('cli.claude.modelMapping.fallbackPlaceholder')}
          />
          <p className="text-xs text-muted-foreground">
            {t('cli.claude.modelMapping.fallbackHint')}
          </p>
        </div>
      </CardContent>
    </Card>
  )
}
