/**
 * 额度监控配置表单组件
 *
 * 替代原先的 JSON 文本输入，提供结构化的额度监控配置界面：
 * - 选择监控方法（BalanceMethod）
 * - 根据方法动态展示额外配置字段（如 NewAPI 的 userId/systemToken）
 * - 自定义脚本分组：仅列出 status=active 的模板
 * - 自动序列化为 BalanceConfig JSON 字符串
 */

import { useMemo } from 'react'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { useActiveScriptTemplates } from '@/hooks/use-script-templates'
import { useTranslation } from '@/modules/i18n/use-translation'
import type { BalanceMethod, BalanceConfig } from '@/modules/balance/types'

/** 内置监控方法选项（不含 Grok Build，其 label 走 i18n）；`labelKey` 标记需要 i18n 的选项，其余为品牌名直接展示 */
const BUILTIN_METHOD_OPTIONS: { value: BalanceMethod; label?: string; labelKey?: string }[] = [
  { value: 'none', labelKey: 'balanceForm.methodNone' },
  { value: 'deepseek', label: 'DeepSeek' },
  { value: 'openrouter', label: 'OpenRouter' },
  { value: 'siliconflow', labelKey: 'balanceForm.methodSiliconflow' },
  { value: 'moonshot-ai', label: 'Moonshot AI' },
  { value: 'kimi-code', label: 'Kimi Code' },
  { value: 'newapi', label: 'New API' },
  { value: 'aihubmix', label: 'AIHubMix' },
  { value: 'claude-relay-service', label: 'Claude Relay' },
  { value: 'minimax', label: 'MiniMax' },
  { value: 'antigravity', label: 'Antigravity' },
  { value: 'gemini-cli', label: 'Gemini CLI' },
  { value: 'codex', label: 'Codex' },
  // { value: 'synthetic', label: '测试数据' },
]

export interface BalanceConfigFormProps {
  /** 当前配置的 JSON 字符串（从 provider.balanceProviderJson 解析） */
  value: string
  /** 配置变更回调，返回新的 JSON 字符串 */
  onChange: (json: string) => void
}

/**
 * 额度监控配置表单
 *
 * 将 JSON 字符串解析为 BalanceConfig 对象，提供结构化编辑，
 * 变更时自动序列化回 JSON 字符串。
 */
export function BalanceConfigForm({ value, onChange }: BalanceConfigFormProps) {
  const { t } = useTranslation('scriptTemplate')
  const { items: activeScripts } = useActiveScriptTemplates()

  /** 内置监控方法选项（`labelKey` 与 Grok Build 的 label 走 i18n） */
  const builtinMethodOptions = useMemo(() => {
    const options: { value: BalanceMethod; label: string }[] = BUILTIN_METHOD_OPTIONS.map((opt) => ({
      value: opt.value,
      label: opt.labelKey ? t(opt.labelKey) : (opt.label ?? opt.value),
    }))
    options.push({ value: 'grok-build', label: t('balanceForm.methodGrokBuild') })
    return options
  }, [t])

  const config = useMemo<BalanceConfig>(() => {
    if (!value.trim()) return { method: 'none' }
    try {
      return JSON.parse(value) as BalanceConfig
    } catch {
      return { method: 'none' }
    }
  }, [value])

  const method = config.method
  const scriptTemplateId =
    method === 'script'
      ? (config as Extract<BalanceConfig, { method: 'script' }>).scriptTemplateId
      : ''
  const scriptTimeoutMs =
    method === 'script'
      ? (config as Extract<BalanceConfig, { method: 'script' }>).timeoutMs
      : undefined

  const scriptStillActive = useMemo(() => {
    if (method !== 'script' || !scriptTemplateId) return true
    return activeScripts.some((s) => s.id === scriptTemplateId)
  }, [method, scriptTemplateId, activeScripts])

  const updateConfig = (newConfig: BalanceConfig) => {
    onChange(JSON.stringify(newConfig))
  }

  const selectValue =
    method === 'script' && scriptTemplateId
      ? `script:${scriptTemplateId}`
      : method === 'script'
        ? 'script:__missing__'
        : method

  const handleMethodChange = (newValue: string) => {
    if (newValue.startsWith('script:')) {
      const id = newValue.slice('script:'.length)
      if (id === '__missing__' || id === '__empty__') return
      updateConfig({
        method: 'script',
        scriptTemplateId: id,
        timeoutMs: scriptTimeoutMs,
      })
      return
    }

    const newMethod = newValue as BalanceMethod
    switch (newMethod) {
      case 'newapi':
        updateConfig({
          method: 'newapi',
          userId: (config as { method: 'newapi'; userId?: string }).userId,
          systemToken: (config as { method: 'newapi'; systemToken?: string }).systemToken,
        })
        break
      case 'claude-relay-service':
        updateConfig({
          method: 'claude-relay-service',
          baseUrl: (config as { method: 'claude-relay-service'; baseUrl?: string }).baseUrl,
        })
        break
      case 'script':
        break
      default:
        updateConfig({ method: newMethod as Exclude<BalanceMethod, 'script' | 'newapi' | 'claude-relay-service'> })
    }
  }

  return (
    <div className="space-y-3">
      <div className="space-y-1.5">
        <Label className="text-xs">{t('balanceForm.methodLabel')}</Label>
        <Select value={selectValue} onValueChange={handleMethodChange}>
          <SelectTrigger className="h-8 text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              <SelectLabel className="text-[10px]">{t('balanceForm.builtinGroup')}</SelectLabel>
              {builtinMethodOptions.map((opt) => (
                <SelectItem key={opt.value} value={opt.value} className="text-xs">
                  <span className="font-medium">{opt.label}</span>
                </SelectItem>
              ))}
            </SelectGroup>
            <SelectGroup>
              <SelectLabel className="text-[10px]">{t('balanceForm.customScripts')}</SelectLabel>
              {activeScripts.length === 0 ? (
                <SelectItem value="script:__empty__" disabled className="text-xs">
                  {t('balanceForm.manageHint')}
                </SelectItem>
              ) : (
                activeScripts.map((s) => (
                  <SelectItem key={s.id} value={`script:${s.id}`} className="text-xs">
                    <span className="font-medium">{s.name}</span>
                    <span className="text-muted-foreground ml-1.5 font-mono text-[10px]">
                      {s.slug}
                    </span>
                  </SelectItem>
                ))
              )}
              {method === 'script' && scriptTemplateId && !scriptStillActive && (
                <SelectItem value={`script:${scriptTemplateId}`} className="text-xs">
                  <span className="text-destructive font-medium">
                    {scriptTemplateId.slice(0, 8)}…
                  </span>
                </SelectItem>
              )}
            </SelectGroup>
          </SelectContent>
        </Select>
        {method === 'script' && (
          <p className="text-muted-foreground text-[10px]">{t('balanceForm.activeOnly')}</p>
        )}
        {method === 'script' && !scriptStillActive && (
          <p className="text-destructive text-[10px]">{t('balanceForm.unavailable')}</p>
        )}
      </div>

      {method === 'script' && (
        <div className="space-y-2 rounded-md border p-3">
          <div className="text-xs font-medium">{t('balanceForm.customScripts')}</div>
          <div className="space-y-1.5">
            <Label className="text-[11px]">{t('balanceForm.timeout')}</Label>
            <Input
              type="number"
              value={scriptTimeoutMs ?? ''}
              onChange={(e) => {
                const v = e.target.value
                updateConfig({
                  method: 'script',
                  scriptTemplateId,
                  timeoutMs: v ? Number(v) : undefined,
                })
              }}
              className="h-7 text-xs tabular-nums"
              placeholder="15000"
              min={1000}
              max={30000}
            />
          </div>
        </div>
      )}

      {method === 'newapi' && (
        <div className="space-y-2 rounded-md border p-3">
          <div className="text-xs font-medium">{t('balanceForm.newapiConfig')}</div>
          <div className="space-y-1.5">
            <Label className="text-[11px]">{t('balanceForm.newapiUserId')}</Label>
            <Input
              value={(config as { method: 'newapi'; userId?: string }).userId ?? ''}
              onChange={(e) =>
                updateConfig({
                  ...(config as Extract<BalanceConfig, { method: 'newapi' }>),
                  userId: e.target.value || undefined,
                })
              }
              className="h-7 text-xs"
              placeholder={t('balanceForm.newapiUserIdPlaceholder')}
            />
          </div>
          <div className="space-y-1.5">
            <Label className="text-[11px]">{t('balanceForm.newapiSystemToken')}</Label>
            <Input
              type="password"
              value={(config as { method: 'newapi'; systemToken?: string }).systemToken ?? ''}
              onChange={(e) =>
                updateConfig({
                  ...(config as Extract<BalanceConfig, { method: 'newapi' }>),
                  systemToken: e.target.value || undefined,
                })
              }
              className="h-7 text-xs"
              placeholder={t('balanceForm.newapiSystemTokenPlaceholder')}
            />
            <p className="text-muted-foreground text-[10px]">{t('balanceForm.newapiSystemTokenHint')}</p>
          </div>
        </div>
      )}

      {method === 'claude-relay-service' && (
        <div className="space-y-2 rounded-md border p-3">
          <div className="text-xs font-medium">{t('balanceForm.claudeRelayConfig')}</div>
          <div className="space-y-1.5">
            <Label className="text-[11px]">{t('balanceForm.claudeRelayBaseUrl')}</Label>
            <Input
              value={
                (config as { method: 'claude-relay-service'; baseUrl?: string }).baseUrl ?? ''
              }
              onChange={(e) =>
                updateConfig({
                  ...(config as Extract<BalanceConfig, { method: 'claude-relay-service' }>),
                  baseUrl: e.target.value || undefined,
                })
              }
              className="h-7 text-xs font-mono"
              placeholder="https://your-relay.example.com/api/stats"
            />
            <p className="text-muted-foreground text-[10px]">{t('balanceForm.claudeRelayBaseUrlHint')}</p>
          </div>
        </div>
      )}

      {method !== 'none' && method !== 'synthetic' && method !== 'script' && (
        <p className="text-muted-foreground text-[10px]">
          <i className="fa-solid fa-circle-check mr-1 text-emerald-500" />
          {t('balanceForm.implementedHint')}
        </p>
      )}
    </div>
  )
}
