import { useCallback, useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ScrollPage } from '@/components/ui/scroll-page'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { useTranslation } from '@/modules/i18n/use-translation'

// ── OpenCode Agent 类型定义 ──

/** Agent 权限值：allow / deny / ask，未设置时使用 undefined */
export type OpenCodeAgentPermissionValue = 'allow' | 'deny' | 'ask'

/** Agent 工具开关值 */
export type OpenCodeAgentToolValue = boolean

/**
 * OpenCode Agent 配置条目
 *
 * 对应 opencode.json 中 `agent` 对象下的单个条目：
 * ```json
 * "agent": {
 *   "backend-dev": { "description": "...", "model": "...", ... }
 * }
 * ```
 */
export interface OpenCodeAgent {
  description?: string
  model?: string
  options?: Record<string, unknown>
  /** 权限映射：工具名 → allow/deny/ask */
  permission?: Record<string, string>
  prompt?: string
  /** 工具开关映射：工具名 → true/false */
  tools?: Record<string, boolean>
}

/** opencode.json 中 `agent` 字段的类型：键名 → Agent 配置 */
export type OpenCodeAgents = Record<string, OpenCodeAgent>

// ── 常量 ──

/** 常见权限工具名（按 opencode 约定） */
const COMMON_PERMISSION_KEYS = ['bash', 'edit', 'webfetch'] as const

/** 常见工具开关名（按 opencode 约定） */
const COMMON_TOOL_KEYS = ['bash', 'write', 'edit', 'read', 'ls'] as const

/** 权限下拉选项 */
const PERMISSION_OPTIONS: { value: string; label: string }[] = [
  { value: 'unset', label: '未设置' },
  { value: 'allow', label: 'allow' },
  { value: 'deny', label: 'deny' },
  { value: 'ask', label: 'ask' },
]

// ── 组件 Props ──

export interface OpenCodeAgentDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 当前 opencode.json 中的 agent 配置（由父组件持有） */
  agents: OpenCodeAgents
  /** 可用模型列表（`providerId/modelId` 格式，从当前 Provider 派生） */
  availableModels: string[]
  /** 保存回调：将编辑后的 agents 写回父组件状态 */
  onSave: (agents: OpenCodeAgents) => void
}

/**
 * OpenCode Agent 配置弹窗（软件全屏）
 *
 * 功能：
 * - 左侧 Agent 列表，支持新增 / 选中 / 删除
 * - 右侧编辑表单：ID、描述、模型、Prompt、权限、工具开关、高级 Options
 * - 编辑过程中保持本地工作副本，保存时一次性回写父组件
 * - 数据不进入数据库，仅修改 opencode.json 的 `agent` 字段
 */
export function OpenCodeAgentDialog({
  open,
  onOpenChange,
  agents,
  availableModels,
  onSave,
}: OpenCodeAgentDialogProps) {
  const { t } = useTranslation()

  // ── 本地工作副本 ──
  const [workingAgents, setWorkingAgents] = useState<OpenCodeAgents>({})
  const [selectedKey, setSelectedKey] = useState<string | null>(null)
  // 新增 Agent 时的临时 ID 输入
  const [addingNew, setAddingNew] = useState(false)
  const [newAgentKey, setNewAgentKey] = useState('')

  // ── 同步外部 agents 到工作副本（弹窗打开时） ──
  useEffect(() => {
    if (open) {
      // 深拷贝，避免直接修改父组件状态
      const copy: OpenCodeAgents = {}
      for (const [key, agent] of Object.entries(agents)) {
        copy[key] = {
          ...agent,
          permission: agent.permission ? { ...agent.permission } : undefined,
          tools: agent.tools ? { ...agent.tools } : undefined,
          options: agent.options ? { ...agent.options } : undefined,
        }
      }
      setWorkingAgents(copy)
      const firstKey = Object.keys(copy)[0] ?? null
      setSelectedKey(firstKey)
      setAddingNew(false)
      setNewAgentKey('')
    }
  }, [open, agents])

  // ── 当前选中的 Agent ──
  const selectedAgent = selectedKey ? workingAgents[selectedKey] : undefined
  const agentKeys = useMemo(() => Object.keys(workingAgents), [workingAgents])

  // ── 新增 Agent ──
  const handleStartAdd = useCallback(() => {
    setAddingNew(true)
    setNewAgentKey('')
    setSelectedKey(null)
  }, [])

  const handleConfirmAdd = useCallback(() => {
    const trimmed = newAgentKey.trim()
    if (!trimmed) {
      toast.error(t('cli.opencode.agent.keyRequired'))
      return
    }
    if (workingAgents[trimmed]) {
      toast.error(t('cli.opencode.agent.keyExists'))
      return
    }
    setWorkingAgents((prev) => ({
      ...prev,
      [trimmed]: {
        description: '',
        model: '',
        prompt: '',
        permission: {},
        tools: {},
        options: {},
      },
    }))
    setSelectedKey(trimmed)
    setAddingNew(false)
    setNewAgentKey('')
  }, [newAgentKey, workingAgents, t])

  const handleCancelAdd = useCallback(() => {
    setAddingNew(false)
    setNewAgentKey('')
    // 若已有 Agent，回到第一个
    const firstKey = Object.keys(workingAgents)[0] ?? null
    setSelectedKey(firstKey)
  }, [workingAgents])

  // ── 删除 Agent ──
  const handleDeleteAgent = useCallback(
    (key: string) => {
      setWorkingAgents((prev) => {
        const next = { ...prev }
        delete next[key]
        return next
      })
      if (selectedKey === key) {
        const remaining = Object.keys(workingAgents).filter((k) => k !== key)
        setSelectedKey(remaining[0] ?? null)
      }
      toast.success(t('cli.opencode.agent.deleted'))
    },
    [selectedKey, workingAgents, t]
  )

  // ── 更新当前选中 Agent 的字段 ──
  const updateSelectedAgent = useCallback(
    (updater: (agent: OpenCodeAgent) => OpenCodeAgent) => {
      if (!selectedKey) return
      setWorkingAgents((prev) => {
        const current = prev[selectedKey]
        if (!current) return prev
        return { ...prev, [selectedKey]: updater(current) }
      })
    },
    [selectedKey]
  )

  // ── Agent ID 重命名 ──
  const handleRenameAgent = useCallback(
    (newKey: string) => {
      if (!selectedKey) return
      const trimmed = newKey.trim()
      if (!trimmed || trimmed === selectedKey) return
      if (workingAgents[trimmed]) {
        toast.error(t('cli.opencode.agent.keyExists'))
        return
      }
      setWorkingAgents((prev) => {
        const next: OpenCodeAgents = {}
        for (const [k, v] of Object.entries(prev)) {
          next[k === selectedKey ? trimmed : k] = v
        }
        return next
      })
      setSelectedKey(trimmed)
    },
    [selectedKey, workingAgents, t]
  )

  // ── 权限操作 ──
  const setPermission = useCallback(
    (permKey: string, value: string) => {
      updateSelectedAgent((agent) => {
        const permission = { ...(agent.permission ?? {}) }
        if (value === 'unset') {
          delete permission[permKey]
        } else {
          permission[permKey] = value
        }
        return { ...agent, permission }
      })
    },
    [updateSelectedAgent]
  )

  const addCustomPermission = useCallback(
    (permKey: string) => {
      const trimmed = permKey.trim()
      if (!trimmed) return
      updateSelectedAgent((agent) => {
        const permission = { ...(agent.permission ?? {}) }
        if (!(trimmed in permission)) {
          permission[trimmed] = 'ask'
        }
        return { ...agent, permission }
      })
    },
    [updateSelectedAgent]
  )

  const removePermission = useCallback(
    (permKey: string) => {
      updateSelectedAgent((agent) => {
        const permission = { ...(agent.permission ?? {}) }
        delete permission[permKey]
        return { ...agent, permission }
      })
    },
    [updateSelectedAgent]
  )

  // ── 工具开关操作 ──
  const setTool = useCallback(
    (toolKey: string, value: boolean) => {
      updateSelectedAgent((agent) => {
        const tools = { ...(agent.tools ?? {}) }
        tools[toolKey] = value
        return { ...agent, tools }
      })
    },
    [updateSelectedAgent]
  )

  const addCustomTool = useCallback(
    (toolKey: string) => {
      const trimmed = toolKey.trim()
      if (!trimmed) return
      updateSelectedAgent((agent) => {
        const tools = { ...(agent.tools ?? {}) }
        if (!(trimmed in tools)) {
          tools[trimmed] = false
        }
        return { ...agent, tools }
      })
    },
    [updateSelectedAgent]
  )

  const removeTool = useCallback(
    (toolKey: string) => {
      updateSelectedAgent((agent) => {
        const tools = { ...(agent.tools ?? {}) }
        delete tools[toolKey]
        return { ...agent, tools }
      })
    },
    [updateSelectedAgent]
  )

  // ── Options JSON 编辑 ──
  const [optionsText, setOptionsText] = useState('{}')
  const [optionsError, setOptionsError] = useState<string | null>(null)

  useEffect(() => {
    if (selectedAgent) {
      setOptionsText(JSON.stringify(selectedAgent.options ?? {}, null, 2))
      setOptionsError(null)
    }
  }, [selectedKey, selectedAgent?.options])

  const handleOptionsChange = useCallback(
    (text: string) => {
      setOptionsText(text)
      try {
        const parsed = text.trim() ? JSON.parse(text) : {}
        if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
          throw new Error('期望 JSON 对象')
        }
        setOptionsError(null)
        updateSelectedAgent((agent) => ({ ...agent, options: parsed as Record<string, unknown> }))
      } catch (err) {
        setOptionsError(err instanceof Error ? err.message : 'JSON 解析失败')
      }
    },
    [updateSelectedAgent]
  )

  // ── 模型选择 ──
  const handleModelChange = useCallback(
    (value: string) => {
      updateSelectedAgent((agent) => ({
        ...agent,
        model: value === '__manual__' ? agent.model ?? '' : value,
      }))
    },
    [updateSelectedAgent]
  )

  // ── 保存 / 取消 ──
  const handleSave = useCallback(() => {
    // 校验 Options JSON
    if (optionsError) {
      toast.error(t('cli.opencode.agent.optionsInvalid'))
      return
    }
    onSave(workingAgents)
    onOpenChange(false)
    toast.success(t('cli.opencode.agent.saved'))
  }, [optionsError, workingAgents, onSave, onOpenChange, t])

  const handleCancel = useCallback(() => {
    onOpenChange(false)
  }, [onOpenChange])

  // ── 自定义权限 / 工具输入 ──
  const [customPermKey, setCustomPermKey] = useState('')
  const [customToolKey, setCustomToolKey] = useState('')

  // ── 渲染辅助：合并常见键 + 已有自定义键 ──
  const permissionEntries = useMemo(() => {
    if (!selectedAgent?.permission) return []
    return Object.entries(selectedAgent.permission)
  }, [selectedAgent?.permission])

  const toolEntries = useMemo(() => {
    if (!selectedAgent?.tools) return []
    return Object.entries(selectedAgent.tools)
  }, [selectedAgent?.tools])

  // 所有权限键 = 常见键 ∪ 已有键（保持稳定排序）
  const allPermissionKeys = useMemo(() => {
    const set = new Set<string>(COMMON_PERMISSION_KEYS)
    for (const k of permissionEntries.map(([k]) => k)) set.add(k)
    return Array.from(set)
  }, [permissionEntries])

  // 所有工具键 = 常见键 ∪ 已有键（保持稳定排序）
  const allToolKeys = useMemo(() => {
    const set = new Set<string>(COMMON_TOOL_KEYS)
    for (const k of toolEntries.map(([k]) => k)) set.add(k)
    return Array.from(set)
  }, [toolEntries])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex h-[min(640px,92vh)] w-[min(860px,96vw)] max-w-[860px] flex-col gap-0 overflow-hidden p-0">
        {/* ── 头部 ── */}
        <DialogHeader className="shrink-0 border-b px-4 py-3">
          <DialogTitle className="flex items-center gap-2 text-sm">
            <i className="fa-solid fa-user-gear text-xs text-primary" />
            {t('cli.opencode.agent.dialogTitle')}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {t('cli.opencode.agent.dialogDescription')}
          </DialogDescription>
        </DialogHeader>

        {/* ── 主体：左右分栏 ── */}
        <div className="flex min-h-0 flex-1">
          {/* 左侧：Agent 列表 */}
          <div className="flex w-[220px] shrink-0 flex-col border-r">
            <div className="shrink-0 border-b p-2">
              <Button
                type="button"
                size="sm"
                className="h-7 w-full gap-1.5 text-xs"
                onClick={handleStartAdd}
                disabled={addingNew}
              >
                <i className="fa-solid fa-plus text-[10px]" />
                {t('cli.opencode.agent.addAgent')}
              </Button>
            </div>

            {addingNew && (
              <div className="shrink-0 border-b p-2">
                <Input
                  autoFocus
                  value={newAgentKey}
                  onChange={(e) => setNewAgentKey(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') handleConfirmAdd()
                    if (e.key === 'Escape') handleCancelAdd()
                  }}
                  placeholder={t('cli.opencode.agent.keyPlaceholder')}
                  className="h-7 text-xs"
                />
                <div className="mt-1.5 flex gap-1">
                  <Button
                    type="button"
                    size="sm"
                    className="h-6 flex-1 text-[10px]"
                    onClick={handleConfirmAdd}
                  >
                    {t('common.confirm')}
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-6 flex-1 text-[10px]"
                    onClick={handleCancelAdd}
                  >
                    {t('common.cancel')}
                  </Button>
                </div>
              </div>
            )}

            <ScrollPage
              className="min-h-0 flex-1"
              variant="borderless"
              scrollbarVisible="auto"
            >
              <div className="flex flex-col gap-0.5 p-1.5">
                {agentKeys.length === 0 && !addingNew && (
                  <div className="py-6 text-center text-[11px] text-muted-foreground">
                    {t('cli.opencode.agent.noAgents')}
                  </div>
                )}
                {agentKeys.map((key) => {
                  const agent = workingAgents[key]
                  const isSelected = key === selectedKey
                  return (
                    <div
                      key={key}
                      className={`group flex cursor-pointer items-center gap-1.5 rounded px-2 py-1.5 text-xs transition-colors ${
                        isSelected
                          ? 'bg-accent text-accent-foreground'
                          : 'hover:bg-muted'
                      }`}
                      onClick={() => {
                        setSelectedKey(key)
                        setAddingNew(false)
                      }}
                    >
                      <i
                        className={`fa-solid fa-user text-[9px] shrink-0 ${
                          isSelected ? 'text-primary' : 'text-muted-foreground'
                        }`}
                      />
                      <div className="min-w-0 flex-1">
                        <div className="truncate font-medium">{key}</div>
                        {agent?.description && (
                          <div className="truncate text-[10px] text-muted-foreground">
                            {agent.description}
                          </div>
                        )}
                      </div>
                      <Tooltip>
                        <TooltipTrigger asChild>
                          <button
                            type="button"
                            className="shrink-0 rounded p-0.5 text-muted-foreground opacity-0 transition-opacity hover:text-destructive group-hover:opacity-100"
                            onClick={(e) => {
                              e.stopPropagation()
                              handleDeleteAgent(key)
                            }}
                          >
                            <i className="fa-solid fa-trash text-[9px]" />
                          </button>
                        </TooltipTrigger>
                        <TooltipContent side="right" className="text-[11px]">
                          {t('common.delete')}
                        </TooltipContent>
                      </Tooltip>
                    </div>
                  )
                })}
              </div>
            </ScrollPage>
          </div>

          {/* 右侧：编辑表单 */}
          <div className="flex min-h-0 flex-1 flex-col">
            {!selectedAgent ? (
              <div className="flex flex-1 items-center justify-center text-xs text-muted-foreground">
                {addingNew
                  ? t('cli.opencode.agent.addingHint')
                  : t('cli.opencode.agent.selectHint')}
              </div>
            ) : (
              <ScrollPage
                className="min-h-0 flex-1"
                variant="borderless"
                scrollbarVisible="auto"
              >
                <div className="space-y-3 p-3">
                  {/* Agent ID */}
                  <div className="space-y-1.5">
                    <Label className="text-xs">
                      {t('cli.opencode.agent.agentId')}
                      <span className="ml-1 text-destructive">*</span>
                    </Label>
                    <Input
                      value={selectedKey ?? ''}
                      onChange={(e) => handleRenameAgent(e.target.value)}
                      placeholder="backend-dev"
                      className="h-8 text-xs"
                    />
                  </div>

                  {/* 描述 */}
                  <div className="space-y-1.5">
                    <Label className="text-xs">
                      {t('cli.opencode.agent.description')}
                    </Label>
                    <Input
                      value={selectedAgent.description ?? ''}
                      onChange={(e) =>
                        updateSelectedAgent((a) => ({ ...a, description: e.target.value }))
                      }
                      placeholder={t('cli.opencode.agent.descriptionPlaceholder')}
                      className="h-8 text-xs"
                    />
                  </div>

                  {/* 模型 */}
                  <div className="space-y-1.5">
                    <Label className="text-xs">
                      {t('cli.opencode.agent.model')}
                    </Label>
                    <div className="flex items-center gap-2">
                      <Select
                        value={
                          selectedAgent.model &&
                          availableModels.includes(selectedAgent.model)
                            ? selectedAgent.model
                            : '__manual__'
                        }
                        onValueChange={handleModelChange}
                      >
                        <SelectTrigger className="h-8 w-auto min-w-[180px] text-xs">
                          <SelectValue
                            placeholder={t('cli.opencode.agent.modelPlaceholder')}
                          />
                        </SelectTrigger>
                        <SelectContent>
                          {availableModels.length === 0 && (
                            <SelectItem value="__manual__" className="text-xs">
                              {t('cli.opencode.agent.manualInput')}
                            </SelectItem>
                          )}
                          {availableModels.map((modelId) => (
                            <SelectItem key={modelId} value={modelId} className="text-xs">
                              {modelId}
                            </SelectItem>
                          ))}
                          {availableModels.length > 0 && (
                            <SelectItem value="__manual__" className="text-xs">
                              {t('cli.opencode.agent.manualInput')}
                            </SelectItem>
                          )}
                        </SelectContent>
                      </Select>
                      {/* 手动输入框：当模型不在可用列表时显示当前值 */}
                      <Input
                        value={selectedAgent.model ?? ''}
                        onChange={(e) =>
                          updateSelectedAgent((a) => ({ ...a, model: e.target.value }))
                        }
                        placeholder="provider-slug/model-id"
                        className="h-8 flex-1 text-xs"
                      />
                    </div>
                  </div>

                  <Separator />

                  {/* Prompt */}
                  <div className="space-y-1.5">
                    <Label className="text-xs">
                      {t('cli.opencode.agent.prompt')}
                    </Label>
                    <Textarea
                      value={selectedAgent.prompt ?? ''}
                      onChange={(e) =>
                        updateSelectedAgent((a) => ({ ...a, prompt: e.target.value }))
                      }
                      placeholder={t('cli.opencode.agent.promptPlaceholder')}
                      className="min-h-[120px] font-mono text-xs leading-relaxed"
                    />
                  </div>

                  <Separator />

                  {/* 权限 Permission */}
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <Label className="text-xs">
                        {t('cli.opencode.agent.permission')}
                      </Label>
                      <Badge variant="secondary" className="px-1.5 py-0 text-[9px]">
                        {allPermissionKeys.length}
                      </Badge>
                    </div>
                    <div className="space-y-1">
                      {allPermissionKeys.map((permKey) => {
                        const currentValue = selectedAgent.permission?.[permKey]
                        const isCustom = !COMMON_PERMISSION_KEYS.includes(
                          permKey as (typeof COMMON_PERMISSION_KEYS)[number]
                        )
                        return (
                          <div
                            key={permKey}
                            className="flex items-center gap-2 rounded border px-2 py-1"
                          >
                            <code className="min-w-[80px] shrink-0 text-[11px]">
                              {permKey}
                            </code>
                            <Select
                              value={currentValue ?? 'unset'}
                              onValueChange={(v) => setPermission(permKey, v)}
                            >
                              <SelectTrigger className="h-6 flex-1 text-[11px]">
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent>
                                {PERMISSION_OPTIONS.map((opt) => (
                                  <SelectItem
                                    key={opt.value}
                                    value={opt.value}
                                    className="text-[11px]"
                                  >
                                    {opt.label}
                                  </SelectItem>
                                ))}
                              </SelectContent>
                            </Select>
                            {isCustom && (
                              <Tooltip>
                                <TooltipTrigger asChild>
                                  <button
                                    type="button"
                                    className="shrink-0 rounded p-0.5 text-muted-foreground hover:text-destructive"
                                    onClick={() => removePermission(permKey)}
                                  >
                                    <i className="fa-solid fa-xmark text-[10px]" />
                                  </button>
                                </TooltipTrigger>
                                <TooltipContent side="top" className="text-[11px]">
                                  {t('common.delete')}
                                </TooltipContent>
                              </Tooltip>
                            )}
                          </div>
                        )
                      })}
                    </div>
                    {/* 添加自定义权限键 */}
                    <div className="flex items-center gap-1.5">
                      <Input
                        value={customPermKey}
                        onChange={(e) => setCustomPermKey(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter' && customPermKey.trim()) {
                            addCustomPermission(customPermKey)
                            setCustomPermKey('')
                          }
                        }}
                        placeholder={t('cli.opencode.agent.customPermissionPlaceholder')}
                        className="h-6 flex-1 text-[11px]"
                      />
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        className="h-6 gap-1 px-2 text-[10px]"
                        onClick={() => {
                          if (customPermKey.trim()) {
                            addCustomPermission(customPermKey)
                            setCustomPermKey('')
                          }
                        }}
                      >
                        <i className="fa-solid fa-plus text-[8px]" />
                        {t('cli.opencode.agent.add')}
                      </Button>
                    </div>
                  </div>

                  <Separator />

                  {/* 工具开关 Tools */}
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <Label className="text-xs">
                        {t('cli.opencode.agent.tools')}
                      </Label>
                      <Badge variant="secondary" className="px-1.5 py-0 text-[9px]">
                        {allToolKeys.length}
                      </Badge>
                    </div>
                    <div className="space-y-1">
                      {allToolKeys.map((toolKey) => {
                        const currentValue = selectedAgent.tools?.[toolKey] ?? false
                        const isCustom = !COMMON_TOOL_KEYS.includes(
                          toolKey as (typeof COMMON_TOOL_KEYS)[number]
                        )
                        return (
                          <div
                            key={toolKey}
                            className="flex items-center gap-2 rounded border px-2 py-1"
                          >
                            <code className="min-w-[80px] shrink-0 text-[11px]">
                              {toolKey}
                            </code>
                            <div className="flex-1" />
                            <Switch
                              checked={currentValue}
                              onCheckedChange={(v) => setTool(toolKey, v)}
                              className="data-[state=checked]:bg-primary"
                            />
                            {isCustom && (
                              <Tooltip>
                                <TooltipTrigger asChild>
                                  <button
                                    type="button"
                                    className="shrink-0 rounded p-0.5 text-muted-foreground hover:text-destructive"
                                    onClick={() => removeTool(toolKey)}
                                  >
                                    <i className="fa-solid fa-xmark text-[10px]" />
                                  </button>
                                </TooltipTrigger>
                                <TooltipContent side="top" className="text-[11px]">
                                  {t('common.delete')}
                                </TooltipContent>
                              </Tooltip>
                            )}
                          </div>
                        )
                      })}
                    </div>
                    {/* 添加自定义工具键 */}
                    <div className="flex items-center gap-1.5">
                      <Input
                        value={customToolKey}
                        onChange={(e) => setCustomToolKey(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter' && customToolKey.trim()) {
                            addCustomTool(customToolKey)
                            setCustomToolKey('')
                          }
                        }}
                        placeholder={t('cli.opencode.agent.customToolPlaceholder')}
                        className="h-6 flex-1 text-[11px]"
                      />
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        className="h-6 gap-1 px-2 text-[10px]"
                        onClick={() => {
                          if (customToolKey.trim()) {
                            addCustomTool(customToolKey)
                            setCustomToolKey('')
                          }
                        }}
                      >
                        <i className="fa-solid fa-plus text-[8px]" />
                        {t('cli.opencode.agent.add')}
                      </Button>
                    </div>
                  </div>

                  <Separator />

                  {/* 高级 Options */}
                  <div className="space-y-1.5">
                    <Label className="text-xs">
                      {t('cli.opencode.agent.options')}
                      <span className="ml-1.5 text-[10px] text-muted-foreground">
                        {t('cli.opencode.agent.optionsHint')}
                      </span>
                    </Label>
                    <Textarea
                      value={optionsText}
                      onChange={(e) => handleOptionsChange(e.target.value)}
                      placeholder="{}"
                      className="min-h-[80px] font-mono text-[11px]"
                    />
                    {optionsError && (
                      <p className="text-[10px] text-destructive">{optionsError}</p>
                    )}
                  </div>
                </div>
              </ScrollPage>
            )}
          </div>
        </div>

        {/* ── 底部操作栏 ── */}
        <DialogFooter className="shrink-0 border-t px-4 py-2.5">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-8 text-xs"
            onClick={handleCancel}
          >
            {t('common.cancel')}
          </Button>
          <Button
            type="button"
            size="sm"
            className="h-8 gap-1.5 text-xs"
            onClick={handleSave}
            disabled={!!optionsError}
          >
            <i className="fa-solid fa-check text-[10px]" />
            {t('common.save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
