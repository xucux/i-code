/**
 * 额度监控配置表单组件
 *
 * 替代原先的 JSON 文本输入，提供结构化的额度监控配置界面：
 * - 选择监控方法（BalanceMethod）
 * - 根据方法动态展示额外配置字段（如 NewAPI 的 userId/systemToken）
 * - 自动序列化为 BalanceConfig JSON 字符串
 */

import { useMemo } from 'react'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { BalanceMethod, BalanceConfig } from '@/modules/balance/types'

/** 监控方法选项 */
const BALANCE_METHOD_OPTIONS: { value: BalanceMethod; label: string; description: string }[] = [
  { value: 'none', label: '不监控', description: '不查询额度' },
  { value: 'deepseek', label: 'DeepSeek', description: '用户余额 API' },
  { value: 'openrouter', label: 'OpenRouter', description: 'Credits API' },
  { value: 'siliconflow', label: '硅基流动', description: '用户信息 API' },
  { value: 'moonshot-ai', label: 'Moonshot AI', description: 'Kimi 国内站余额 API' },
  { value: 'kimi-code', label: 'Kimi Code', description: 'Kimi 国际站 usages API' },
  { value: 'newapi', label: 'New API', description: 'OneAPI 系分支，需额外配置' },
  { value: 'aihubmix', label: 'AIHubMix', description: 'remain API' },
  { value: 'claude-relay-service', label: 'Claude Relay', description: 'apiStats 系列，可自定义地址' },
  { value: 'minimax', label: 'MiniMax', description: 'coding plan 余额 API' },
  { value: 'antigravity', label: 'Antigravity', description: 'Google Code Assist 配额' },
  { value: 'gemini-cli', label: 'Gemini CLI', description: 'Google Code Assist 配额' },
  { value: 'codex', label: 'Codex', description: 'OpenAI usage API' },
  { value: 'synthetic', label: '测试数据', description: '合成数据，用于调试' },
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
  // 解析当前配置
  const config = useMemo<BalanceConfig>(() => {
    if (!value.trim()) return { method: 'none' }
    try {
      return JSON.parse(value) as BalanceConfig
    } catch {
      return { method: 'none' }
    }
  }, [value])

  const method = config.method

  /** 更新配置并序列化 */
  const updateConfig = (newConfig: BalanceConfig) => {
    onChange(JSON.stringify(newConfig))
  }

  /** 切换监控方法 */
  const handleMethodChange = (newMethod: string) => {
    // 切换方法时保留该方法的特有字段，其他字段清空
    switch (newMethod as BalanceMethod) {
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
      default:
        updateConfig({ method: newMethod as BalanceMethod })
    }
  }

  const selectedOption = BALANCE_METHOD_OPTIONS.find((o) => o.value === method)

  return (
    <div className="space-y-3">
      {/* 监控方法选择 */}
      <div className="space-y-1.5">
        <Label className="text-xs">监控方法</Label>
        <Select value={method} onValueChange={handleMethodChange}>
          <SelectTrigger className="h-8 text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {BALANCE_METHOD_OPTIONS.map((opt) => (
              <SelectItem key={opt.value} value={opt.value} className="text-xs">
                <span className="font-medium">{opt.label}</span>
                <span className="text-muted-foreground ml-1.5 text-[10px]">{opt.description}</span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {selectedOption && method !== 'none' && (
          <p className="text-muted-foreground text-[10px]">
            {selectedOption.description}
          </p>
        )}
      </div>

      {/* New API 特有配置 */}
      {method === 'newapi' && (
        <div className="space-y-2 rounded-md border p-3">
          <div className="text-xs font-medium">New API 配置</div>
          <div className="space-y-1.5">
            <Label className="text-[11px]">用户 ID</Label>
            <Input
              value={(config as { method: 'newapi'; userId?: string }).userId ?? ''}
              onChange={(e) =>
                updateConfig({ ...config as Extract<BalanceConfig, { method: 'newapi' }>, userId: e.target.value || undefined })
              }
              className="h-7 text-xs"
              placeholder="NewAPI 用户 ID"
            />
          </div>
          <div className="space-y-1.5">
            <Label className="text-[11px]">系统 Token</Label>
            <Input
              type="password"
              value={(config as { method: 'newapi'; systemToken?: string }).systemToken ?? ''}
              onChange={(e) =>
                updateConfig({ ...config as Extract<BalanceConfig, { method: 'newapi' }>, systemToken: e.target.value || undefined })
              }
              className="h-7 text-xs"
              placeholder="系统管理 Token"
            />
            <p className="text-muted-foreground text-[10px]">用于查询 API 管理端额度</p>
          </div>
        </div>
      )}

      {/* Claude Relay Service 特有配置 */}
      {method === 'claude-relay-service' && (
        <div className="space-y-2 rounded-md border p-3">
          <div className="text-xs font-medium">Claude Relay 配置</div>
          <div className="space-y-1.5">
            <Label className="text-[11px]">自定义 API 地址</Label>
            <Input
              value={(config as { method: 'claude-relay-service'; baseUrl?: string }).baseUrl ?? ''}
              onChange={(e) =>
                updateConfig({ ...config as Extract<BalanceConfig, { method: 'claude-relay-service' }>, baseUrl: e.target.value || undefined })
              }
              className="h-7 text-xs font-mono"
              placeholder="https://your-relay.example.com/api/stats"
            />
            <p className="text-muted-foreground text-[10px]">覆盖默认的 apiStats API 地址</p>
          </div>
        </div>
      )}

      {/* 已实现的供应商提示 */}
      {method !== 'none' && method !== 'synthetic' && (
        <p className="text-muted-foreground text-[10px]">
          <i className="fa-solid fa-circle-check mr-1 text-emerald-500" />
          该供应商额度查询已实现，保存后可刷新查看额度
        </p>
      )}
    </div>
  )
}
