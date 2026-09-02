/**
 * 模型选择 + 思考强度子级面板组合组件
 *
 * ## 界面描述
 *
 * 触发按钮外观与下拉框一致（展示当前模型）；点击后弹出 Popover 浮层：
 *
 * ```
 * ┌────────────────────┬───────────────────────────┐
 * │ 模型列表            │  思考强度面板              │
 * │  ○ model-a         │  [开关] 开启思考           │
 * │  ✓ model-b (选中)  │  [默认][low][high][none]   │
 * │  ○ model-c         │  自定义力度输入             │
 * └────────────────────┴───────────────────────────┘
 * ```
 *
 * - 左列为可选模型：顶部本地搜索框过滤；条目悬浮展示模型 ID 全称；点击即选中（回调 `onModelChange`）。
 * - 选中后右侧自动浮动展示该模型的思考强度操作面板（顶部展示选中模型 ID 全称）；
 *   选项优先取模型配置 `thinking_json.thinkingEffortOptions`，缺省回退默认列表。
 * - 面板开关控制「开启思考」（`thinkingEnabled`），力度选项写入 `thinkingEffort`；
 *   关闭思考时力度清空（发送时不注入 reasoning_effort）。
 * - 模型无思考配置时面板显示提示，力度操作仍可用（回退默认列表）。
 *
 * 主题色全部走 CSS 变量；文案走 i18n。
 */

import { useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { useTranslation } from '@/modules/i18n/use-translation'
import type { ModelThinkingConfig } from '@/modules/ai-gateway/types'
import { cn } from '@/lib/utils'

/** 推理力度默认可选列表；模型配置未声明 thinkingEffortOptions 时回退 */
export const DEFAULT_EFFORT_OPTIONS = ['low', 'medium', 'high', 'none']

/** 候选模型条目：`value` 为路由 ID，`thinkingJson` 为模型思考配置 JSON */
export interface ModelThinkingPickerModel {
  value: string
  label: string
  thinkingJson?: string
}

export interface ModelThinkingPickerProps {
  /** 候选模型列表（含各模型 thinkingJson） */
  models: ModelThinkingPickerModel[]
  /** 当前选中模型（路由 ID） */
  selectedModel: string
  onModelChange: (model: string) => void
  /** 是否开启思考（推理） */
  thinkingEnabled: boolean
  onThinkingEnabledChange: (enabled: boolean) => void
  /** 本轮推理力度（reasoning_effort），空串表示不指定 */
  thinkingEffort: string
  onThinkingEffortChange: (effort: string) => void
  disabled?: boolean
}

/**
 * 解析模型思考配置 JSON（来自 model_configs.thinking_json）
 *
 * - `enabled`：thinking_json.type === 'enabled'，默认开启思考
 * - `effort`：开启时默认推理力度（如 "high"）
 * - `effortOptions`：可选推理力度列表（thinkingEffortOptions），为空时调用方回退默认列表
 */
export function parseModelThinking(thinkingJson?: string): {
  enabled: boolean
  effort: string
  effortOptions: string[]
} {
  const empty = { enabled: false, effort: '', effortOptions: [] as string[] }
  if (!thinkingJson) return empty
  try {
    const cfg = JSON.parse(thinkingJson) as ModelThinkingConfig
    return {
      enabled: cfg.type === 'enabled',
      effort: cfg.effort ?? '',
      effortOptions: cfg.thinkingEffortOptions ?? [],
    }
  } catch {
    return empty
  }
}

/**
 * 模型选择 + 思考强度子级面板
 */
export function ModelThinkingPicker({
  models,
  selectedModel,
  onModelChange,
  thinkingEnabled,
  onThinkingEnabledChange,
  thinkingEffort,
  onThinkingEffortChange,
  disabled,
}: ModelThinkingPickerProps) {
  const { t } = useTranslation('chat')
  const [open, setOpen] = useState(false)
  /** 左列模型本地搜索关键字 */
  const [search, setSearch] = useState('')
  /** 面板中「自定义力度」输入值 */
  const [customDraft, setCustomDraft] = useState('')

  /** 按关键字过滤模型（匹配展示名或路由 ID，不区分大小写） */
  const filteredModels = useMemo(() => {
    const kw = search.trim().toLowerCase()
    if (!kw) return models
    return models.filter(
      (m) => m.label.toLowerCase().includes(kw) || m.value.toLowerCase().includes(kw),
    )
  }, [models, search])

  const selected = models.find((m) => m.value === selectedModel)
  const selectedThinking = useMemo(
    () => parseModelThinking(selected?.thinkingJson),
    [selected?.thinkingJson],
  )
  /** 面板力度选项：优先模型配置声明，缺省回退默认列表 */
  const effortOptions = selectedThinking.effortOptions.length
    ? selectedThinking.effortOptions
    : DEFAULT_EFFORT_OPTIONS

  /** 面板开关：开启时默认取模型配置 effort；关闭时清空力度 */
  const handleToggle = (next: boolean) => {
    onThinkingEnabledChange(next)
    onThinkingEffortChange(next ? selectedThinking.effort : '')
  }

  /** 点击力度选项：隐式开启思考并写入力度 */
  const handlePickEffort = (effort: string) => {
    if (!thinkingEnabled) onThinkingEnabledChange(true)
    onThinkingEffortChange(effort)
  }

  /** 提交自定义力度（回车或确认按钮） */
  const submitCustom = () => {
    const v = customDraft.trim()
    if (!v) return
    handlePickEffort(v)
    setCustomDraft('')
  }

  return (
    <Popover open={open} onOpenChange={(next) => {
      setOpen(next)
      // 关闭浮层时清空搜索，避免下次打开仍处于过滤状态
      if (!next) setSearch('')
    }}>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="outline"
          disabled={disabled || models.length === 0}
          className="h-7 w-[min(100%,220px)] justify-between gap-1 px-2 text-xs font-normal"
          title={selected?.label || t('input.selectModel')}
        >
          <span className="min-w-0 flex-1 truncate text-left">
            {selected?.label || (models.length === 0 ? t('input.noModels') : t('input.selectModel'))}
          </span>
          <i className="fa-solid fa-chevron-down shrink-0 text-[10px] text-muted-foreground" />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" sideOffset={6} className="w-[470px] p-1.5">
        <div className="flex gap-1.5">
          {/* 左列：本地搜索框 + 模型列表 */}
          <div className="flex max-h-[300px] w-[210px] shrink-0 flex-col gap-1">
            <Input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={t('input.searchModel')}
              className="h-6 shrink-0 text-xs"
              autoFocus
            />
            <div className="min-h-0 flex-1 overflow-y-auto">
              {filteredModels.map((m) => {
                const active = m.value === selectedModel
                return (
                  <button
                    key={m.value}
                    type="button"
                    title={m.value}
                    className={cn(
                      'flex w-full items-center gap-1.5 rounded px-2 py-1.5 text-left text-xs transition-colors hover:bg-accent',
                      active && 'bg-accent',
                    )}
                    onClick={() => onModelChange(m.value)}
                  >
                    <span className="min-w-0 flex-1 truncate">{m.label}</span>
                    {active && <i className="fa-solid fa-check shrink-0 text-[10px] text-primary" />}
                  </button>
                )
              })}
              {filteredModels.length === 0 && (
                <p className="px-2 py-3 text-center text-xs text-muted-foreground">
                  {models.length === 0 ? t('input.noModels') : t('input.searchNoMatch')}
                </p>
              )}
            </div>
          </div>

          {/* 右列：选中模型的思考强度操作面板 */}
          <div className="min-w-0 flex-1 space-y-2.5 border-l pl-3">
            {/* 选中模型 ID 全称（不截断，过长换行展示） */}
            {selectedModel && (
              <p className="break-all text-[10px] leading-4 text-muted-foreground">{selectedModel}</p>
            )}
            <p className="text-xs font-medium">{t('input.thinkingIntensity')}</p>

            {/* 开启思考开关 */}
            <label className="flex items-center gap-2 text-xs">
              <Switch
                checked={thinkingEnabled}
                onCheckedChange={handleToggle}
                disabled={disabled}
                className="h-[18px] w-[32px] shrink-0 [&>span]:h-[14px] [&>span]:w-[14px] [&>span]:data-[state=checked]:translate-x-[15px]"
              />
              {t('input.thinkingToggle')}
            </label>

            {/* 力度选项：默认 + 模型配置枚举 */}
            <div className="flex flex-wrap gap-1" role="group" aria-label={t('input.thinkingIntensity')}>
              <button
                type="button"
                disabled={!thinkingEnabled || disabled}
                className={cn(
                  'rounded border px-2 py-0.5 text-xs transition-colors disabled:opacity-40',
                  !thinkingEffort ? 'border-primary bg-primary/10 text-primary' : 'hover:bg-accent',
                )}
                onClick={() => onThinkingEffortChange('')}
              >
                {t('input.thinkingEffortDefault')}
              </button>
              {effortOptions.map((v) => (
                <button
                  key={v}
                  type="button"
                  disabled={!thinkingEnabled || disabled}
                  className={cn(
                    'rounded border px-2 py-0.5 text-xs transition-colors disabled:opacity-40',
                    thinkingEffort === v ? 'border-primary bg-primary/10 text-primary' : 'hover:bg-accent',
                  )}
                  onClick={() => handlePickEffort(v)}
                >
                  {v}
                </button>
              ))}
            </div>

            {/* 自定义力度输入 */}
            <div className="flex items-center gap-1">
              <Input
                value={customDraft}
                onChange={(e) => setCustomDraft(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault()
                    submitCustom()
                  }
                }}
                placeholder={t('input.thinkingEffortCustom')}
                disabled={!thinkingEnabled || disabled}
                className="h-6 min-w-0 flex-1 text-xs"
              />
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-6 shrink-0 px-1.5 text-xs"
                disabled={!thinkingEnabled || disabled || !customDraft.trim()}
                onClick={submitCustom}
                title={t('input.thinkingEffortApply')}
              >
                <i className="fa-solid fa-check text-[10px]" />
              </Button>
            </div>

            {/* 模型未声明思考配置提示 */}
            {!selected?.thinkingJson && (
              <p className="text-[10px] leading-4 text-muted-foreground">{t('input.thinkingNoConfig')}</p>
            )}
          </div>
        </div>
      </PopoverContent>
    </Popover>
  )
}
