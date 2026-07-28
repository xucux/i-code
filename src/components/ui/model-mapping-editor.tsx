"use client"

import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import { Label } from "@/components/ui/label"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { useTranslation } from "@/modules/i18n/use-translation"

/**
 * 单个 CLI 模型映射项
 *
 * 用于将 CLI（如 Claude Code、Codex）内部使用的模型角色映射到实际请求模型，
 * 并可声明是否支持 1M 上下文窗口。
 */
export interface ModelMappingItem {
  /** 映射行唯一标识 */
  id: string
  /** CLI 内部模型角色，如 Sonnet / Opus / Fable / Haiku */
  role: string
  /** 前端展示名称，可编辑 */
  displayName: string
  /** 实际请求模型 ID，如 claude-opus-4-8 */
  actualModel: string
  /** 是否声明支持 1M 上下文 */
  supports1M: boolean
}

export interface ModelMappingEditorProps {
  /** 映射列表 */
  mappings: ModelMappingItem[]
  /** 默认兜底模型，当 CLI 请求未命中任何角色时使用 */
  fallbackModel: string
  /**
   * 当前可用的模型列表。
   * 当列表非空时，每行【实际请求模型】输入框右侧显示下拉图标，
   * 点击后可选择列表中的模型填充到当前行。
   */
  availableModels?: string[]
  /** 映射变更回调（仅 UI，业务逻辑由调用方实现） */
  onMappingsChange?: (mappings: ModelMappingItem[]) => void
  /** 兜底模型变更回调 */
  onFallbackChange?: (value: string) => void
  /** 一键设置回调 */
  onAutoSetup?: () => void
  /** 获取模型列表回调，provider 为下拉选择项 */
  onFetchModels?: (provider: string) => void
  /** 是否展示 1M 上下文开关列（Codex 等客户端可关闭） */
  showSupports1M?: boolean
  /** 删除单条映射回调；提供时会在每行末尾显示删除按钮 */
  onDeleteMapping?: (id: string) => void
  /** 自定义类名 */
  className?: string
}

/**
 * 模型映射编辑器
 *
 * 参考 Claude Code CLI 的模型映射界面，提供：
 * - 角色 → 实际请求模型 的表格映射
 * - 显示名称编辑
 * - 1M 上下文声明开关（可通过 showSupports1M 关闭）
 * - 默认兜底模型输入
 * - 一键设置 / 获取模型列表（纯 UI 回调，暂不绑定业务逻辑）
 * - 当 availableModels 有数据时，实际请求模型列右侧出现下拉选择
 * - 可选的单条映射删除按钮
 *
 * 全部使用 shadcn 组件并适配当前主题色。
 */
export function ModelMappingEditor({
  mappings,
  fallbackModel,
  availableModels = [],
  onMappingsChange,
  onFallbackChange,
  onAutoSetup,
  onFetchModels,
  showSupports1M = true,
  onDeleteMapping,
  className,
}: ModelMappingEditorProps) {
  const { t } = useTranslation()

  /**
   * 更新单条映射字段
   *
   * 使用 patch 对象一次性更新多个字段，避免连续调用时因闭包读取同一旧 props
   * 而导致后一次更新覆盖前一次更新（如下拉选择模型时需同时设置 actualModel 与 displayName）。
   */
  const updateMapping = (id: string, patch: Partial<Pick<ModelMappingItem, 'displayName' | 'actualModel' | 'supports1M'>>) => {
    onMappingsChange?.(
      mappings.map((item) => (item.id === id ? { ...item, ...patch } : item))
    )
  }

  /** 是否有可选择的模型列表 */
  const hasAvailableModels = availableModels.length > 0

  /** 根据是否展示 1M 列和删除按钮计算网格列宽 */
  const gridCols = showSupports1M
    ? onDeleteMapping
      ? "grid-cols-[2fr_2fr_4fr_2fr_2fr]"
      : "grid-cols-[2fr_3fr_5fr_2fr]"
    : onDeleteMapping
      ? "grid-cols-[2fr_3fr_5fr_2fr]"
      : "grid-cols-[2fr_3fr_7fr]"

  return (
    <Card className={cn("w-full", className)}>
      {/* 头部：标题 + 操作按钮 */}
      <CardHeader className="flex flex-row items-start justify-between gap-4 pb-3">
        <div className="space-y-1">
          <CardTitle className="text-base">{t('cli.modelMappingEditor.title')}</CardTitle>
          <CardDescription className="text-xs">
            {t('cli.modelMappingEditor.description')}
          </CardDescription>
        </div>
        <div className="flex items-center gap-2">
          {onAutoSetup && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-7 gap-1 px-2 text-xs"
              onClick={onAutoSetup}
            >
              <i className="fa-solid fa-bolt text-[10px]" />
              {t('cli.modelMappingEditor.autoSetup')}
            </Button>
          )}

          {onFetchModels && (
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-7 gap-1 px-2 text-xs"
                >
                  <i className="fa-solid fa-cloud-arrow-down text-[10px]" />
                  {t('cli.modelMappingEditor.fetchModels')}
                  <i className="fa-solid fa-chevron-down text-[10px]" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem onClick={() => onFetchModels("anthropic-native")}>
                  {t('cli.modelMappingEditor.fetchModelsNative')}
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => onFetchModels("openai-compatible")}>
                  {t('cli.modelMappingEditor.fetchModelsCompatible')}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          )}
        </div>
      </CardHeader>

      <CardContent className="space-y-4">
        {/* 映射表头 */}
        <div className={cn("grid gap-2 px-1 text-xs text-muted-foreground", gridCols)}>
          <div>{t('cli.modelMappingEditor.roleHeader')}</div>
          <div>{t('cli.modelMappingEditor.displayNameHeader')}</div>
          <div>{t('cli.modelMappingEditor.actualModelHeader')}</div>
          {showSupports1M && <div className="text-right">{t('cli.modelMappingEditor.supports1MHeader')}</div>}
          {onDeleteMapping && <div className="text-right">{t('cli.modelMappingEditor.operationHeader')}</div>}
        </div>

        {/* 映射行 */}
        <div className="space-y-2">
          {mappings.map((item) => (
            <div
              key={item.id}
              className={cn(
                "grid items-center gap-2 rounded-md border bg-background/50 px-1 py-1.5 transition-colors hover:bg-accent/30",
                gridCols
              )}
            >
              {/* 角色 */}
              <div className="px-2 text-sm font-medium">{item.role}</div>

              {/* 显示名称 */}
              <div>
                <Input
                  value={item.displayName}
                  onChange={(e) => updateMapping(item.id, { displayName: e.target.value })}
                  className="h-7 text-xs"
                  placeholder={t('cli.modelMappingEditor.displayNamePlaceholder')}
                />
              </div>

              {/* 实际请求模型：输入框 + 可选下拉 */}
              <div>
                <div className="relative flex items-center">
                  <Input
                    value={item.actualModel}
                    onChange={(e) => updateMapping(item.id, { actualModel: e.target.value })}
                    className={cn(
                      "h-7 text-xs",
                      // 有下拉图标时右侧留出空间，避免文字被遮挡
                      hasAvailableModels && "pr-7"
                    )}
                    placeholder={t('cli.modelMappingEditor.actualModelPlaceholder')}
                  />
                  {/* 仅当 availableModels 非空时展示下拉图标 */}
                  {hasAvailableModels && (
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <button
                          type="button"
                          className="absolute right-1 top-1/2 flex h-5 w-5 -translate-y-1/2 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                          aria-label={t('cli.modelMappingEditor.selectModel')}
                          title={t('cli.modelMappingEditor.selectModel')}
                        >
                          <i className="fa-solid fa-chevron-down text-[10px]" />
                        </button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end" className="max-h-60 overflow-y-auto">
                        {availableModels.map((model) => (
                          <DropdownMenuItem
                            key={`${item.id}-${model}`}
                            onClick={() =>
                              updateMapping(item.id, {
                                actualModel: model,
                                displayName: model.replace(/^[^/]+\//, ""),
                              })
                            }
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
              {showSupports1M && (
                <div className="flex items-center justify-end gap-2 pr-2">
                  <Switch
                    id={`${item.id}-supports-1m`}
                    checked={item.supports1M}
                    onCheckedChange={(checked) => updateMapping(item.id, { supports1M: checked })}
                    className="data-[state=checked]:bg-primary"
                  />
                  <Label
                    htmlFor={`${item.id}-supports-1m`}
                    className="cursor-pointer text-xs tabular-nums text-muted-foreground"
                  >
                    1M
                  </Label>
                </div>
              )}

              {/* 删除按钮 */}
              {onDeleteMapping && (
                <div className="flex items-center justify-end pr-2">
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="size-6 text-destructive hover:text-destructive"
                    onClick={() => onDeleteMapping(item.id)}
                    aria-label={t('cli.modelMappingEditor.delete')}
                    title={t('cli.modelMappingEditor.delete')}
                  >
                    <i className="fa-solid fa-trash text-[10px]" />
                  </Button>
                </div>
              )}
            </div>
          ))}
        </div>

        {/* 默认兜底模型 */}
        <div className="space-y-1.5 border-t pt-3">
          <Label htmlFor="fallback-model" className="text-xs font-medium">
            {t('cli.modelMappingEditor.fallbackModel')}
          </Label>
          <div className="relative flex items-center">
            <Input
              id="fallback-model"
              value={fallbackModel}
              onChange={(e) => onFallbackChange?.(e.target.value)}
              className={cn('h-8 text-xs', hasAvailableModels && 'pr-7')}
              placeholder={t('cli.modelMappingEditor.fallbackPlaceholder')}
            />
            {hasAvailableModels && (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <button
                    type="button"
                    className="absolute right-1 top-1/2 flex h-5 w-5 -translate-y-1/2 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                    aria-label={t('cli.modelMappingEditor.selectModel')}
                    title={t('cli.modelMappingEditor.selectModel')}
                  >
                    <i className="fa-solid fa-chevron-down text-[10px]" />
                  </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="max-h-60 overflow-y-auto">
                  {availableModels.map((model) => (
                    <DropdownMenuItem key={`fallback-${model}`} onClick={() => onFallbackChange?.(model)}>
                      {model}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
            )}
          </div>
          <p className="text-xs text-muted-foreground">
            {t('cli.modelMappingEditor.fallbackHint')}
          </p>
        </div>
      </CardContent>
    </Card>
  )
}
