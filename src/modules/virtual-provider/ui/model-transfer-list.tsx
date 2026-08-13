"use client"

import * as React from "react"

import {
  DndContext,
  closestCenter,
  type DragEndEvent,
} from "@dnd-kit/core"
import {
  SortableContext,
  useSortable,
  verticalListSortingStrategy,
  arrayMove,
} from "@dnd-kit/sortable"
import { CSS } from "@dnd-kit/utilities"

import { TransferList, type TransferListItem } from "@/components/ui/transfer-list"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible"
import { useTranslation } from "@/modules/i18n/use-translation"
import { testRoute } from "@/hooks/use-virtual-provider-mutation"
import { toast } from "sonner"
import { cn } from "@/lib/utils"
import type { ExposedModel, Provider } from "@/modules/ai-gateway/types"
import type { VirtualModelRoute } from "@/modules/virtual-provider/types"

/** 单条已选模型的可编辑属性 */
export interface SelectedModelDetail {
  /** 模型键：{provider_id}/{model_id} */
  key: string
  /** 路由数据库 ID（新建路由时为 undefined，编辑已有路由时填充） */
  routeId?: string
  /** 优先级（数值越小越优先） */
  priority: number
  /** 是否启用该路由（原硬编码 true，改为可单独禁用） */
  enabled: boolean
  /** 是否健康 */
  isHealthy: boolean
  /** 最大重试次数 */
  maxRetries: number
  /** 重试间隔毫秒数 */
  retryIntervalMs: number
  /** 路由级超时（毫秒）；undefined 表示继承供应商级 */
  timeoutMs?: number
  /** 路由级附加请求头 JSON 字符串（编辑器原始值）；空串表示无 */
  extraHeadersJson?: string
  /** 路由级附加请求体 JSON 字符串（编辑器原始值）；空串表示无 */
  extraBodyJson?: string
  /** 负载均衡权重（默认 1，0 表示不参与轮询）；仅在 load_balance 策略下生效 */
  weight: number
}

export interface ModelTransferListProps {
  /** 所有对外暴露的真实供应商模型 */
  providerModels: ExposedModel[]
  /** 真实供应商列表，用于把 slug/id 互相映射 */
  realProviders: Provider[]
  /** 当前虚拟模型下已存在的路由 */
  routes: VirtualModelRoute[]
  /** 选择变化回调，返回新的已选模型键集合 */
  onChange: (selectedKeys: string[]) => void
  /**
   * 受控的已选模型键集合。
   * 若提供则优先使用该值；否则从 `routes` 派生（适用于即时提交场景）。
   */
  selectedIds?: string[]
  /** 自定义类名 */
  className?: string
}

/**
 * 模型穿梭框组件
 *
 * 基于通用 TransferList，把真实供应商暴露的模型作为候选池，
 * 当前虚拟模型已映射的路由作为已选项。
 * 模型键格式为 `{provider_id}/{model_id}`，便于调用方直接创建或删除路由。
 */
export function ModelTransferList({
  providerModels,
  realProviders,
  routes,
  onChange,
  selectedIds: controlledSelectedIds,
  className,
}: ModelTransferListProps) {
  const { t } = useTranslation('virtualProvider')
  const providerBySlug = React.useMemo(() => {
    const map = new Map<string, Provider>()
    for (const provider of realProviders) {
      map.set(provider.slug, provider)
    }
    return map
  }, [realProviders])

  const providerById = React.useMemo(() => {
    const map = new Map<string, Provider>()
    for (const provider of realProviders) {
      map.set(provider.id, provider)
    }
    return map
  }, [realProviders])

  const [sortBy, setSortBy] = React.useState<'default' | 'modelId'>('default')

  const items: TransferListItem[] = React.useMemo(() => {
    const result: TransferListItem[] = []
    for (const model of providerModels) {
      const provider = providerBySlug.get(model.providerSlug)
      if (!provider) continue
      result.push({
        id: `${provider.id}/${model.modelId}`,
        // 主文本：模型 ID；小字行：供应商名称 · 模型名称
        label: model.modelId,
        description: model.displayName
          ? `${provider.displayName} · ${model.displayName}`
          : provider.displayName,
        group: provider.displayName,
      })
    }
    if (sortBy === 'modelId') {
      result.sort((a, b) => a.id.localeCompare(b.id))
    }
    return result
  }, [providerModels, providerBySlug, sortBy])

  const selectedIds: string[] = React.useMemo(() => {
    if (controlledSelectedIds) {
      return controlledSelectedIds
    }
    const result: string[] = []
    for (const route of routes) {
      const provider = providerById.get(route.targetProviderId)
      if (!provider) continue
      result.push(`${provider.id}/${route.targetModelId}`)
    }
    return result
  }, [routes, providerById, controlledSelectedIds])

  return (
    <div className={className}>
      <div className="mb-2 flex items-center justify-between">
        <span className="text-xs font-medium">{t('childRealModels')}</span>
        <div className="flex items-center gap-1.5">
          <span className="text-muted-foreground text-[10px]">{t('sort')}</span>
          <Select value={sortBy} onValueChange={(v) => setSortBy(v as 'default' | 'modelId')}>
            <SelectTrigger className="h-7 w-28 text-[10px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="default" className="text-[10px]">{t('sortDefault')}</SelectItem>
              <SelectItem value="modelId" className="text-[10px]">{t('sortByModelId')}</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
      <TransferList
        items={items}
        selectedIds={selectedIds}
        onChange={onChange}
        titleLeft={t('availableModels')}
        titleRight={t('selectedModels')}
        leftSearchPlaceholder={t('searchModelPlaceholder')}
        rightSearchPlaceholder={t('searchSelectedPlaceholder')}
        listHeight="h-48"
      />
    </div>
  )
}

/**
 * 路由属性设置列表（独立组件）
 *
 * 展示已选模型的优先级、健康状态、重试次数、重试间隔等可编辑属性。
 * 从 ModelTransferList 拆出，便于在 Tab 布局中独立渲染与滚动。
 */
export interface RouteSettingsListProps {
  /** 已选模型可编辑属性（受控） */
  details: SelectedModelDetail[]
  /** 属性变化回调 */
  onDetailChange: (details: SelectedModelDetail[]) => void
  /** 所有对外暴露的真实供应商模型（用于显示名称映射） */
  providerModels: ExposedModel[]
  /** 真实供应商列表 */
  realProviders: Provider[]
  /** 虚拟供应商策略；load_balance 时显示 weight 输入框 */
  strategy?: 'fallback' | 'on_all' | 'load_balance'
  /** 自定义类名 */
  className?: string
}

/**
 * 可拖拽排序的单条路由行
 *
 * 通过 useSortable 让整行（含高级折叠区）随拖拽平移；
 * 拖拽把手为行首的 grip 图标，仅把手响应拖拽，避免与行内输入框/开关的点击冲突。
 */
interface SortableRouteItemProps {
  /** 单条路由可编辑属性 */
  detail: SelectedModelDetail
  /** 在列表中的索引 */
  index: number
  /** 虚拟供应商策略；load_balance 时显示 weight 输入框 */
  strategy: RouteSettingsListProps['strategy']
  /** 正在测试的路由 key（用于禁用重复点击） */
  testingId: string | null
  /** providerId -> Provider 映射，用于显示供应商名称 */
  providerById: Map<string, Provider>
  /** 所有对外暴露的真实供应商模型，用于显示模型名称 */
  providerModels: ExposedModel[]
  /** 属性变化回调 */
  onDetailChange: (
    index: number,
    field: keyof SelectedModelDetail,
    value: number | boolean | string | undefined,
  ) => void
  /** 单条路由测试回调 */
  onTest: (detail: SelectedModelDetail) => void
}

function SortableRouteItem({
  detail,
  index,
  strategy,
  testingId,
  providerById,
  providerModels,
  onDetailChange,
  onTest,
}: SortableRouteItemProps) {
  const { t } = useTranslation('virtualProvider')
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: detail.key })

  // 拖拽时用 transform 平移整行（含高级折叠区），过渡动画平滑
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
  }

  // key 格式为 `{providerId}/{modelId}`，modelId 可能含 '/'，用 indexOf 拆分
  const slashIdx = detail.key.indexOf('/')
  const pid = slashIdx >= 0 ? detail.key.slice(0, slashIdx) : detail.key
  const mid = slashIdx >= 0 ? detail.key.slice(slashIdx + 1) : ''
  const provider = providerById.get(pid)
  const model = providerModels.find(
    (m) => m.providerSlug === provider?.slug && m.modelId === mid
  )
  // 主文本：模型 ID；小字行：供应商名称 · 模型名称
  const label = mid
  const subLabel = model?.displayName
    ? `${provider?.displayName || pid} · ${model.displayName}`
    : (provider?.displayName || pid)

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={cn(isDragging && 'opacity-60')}
    >
      <Collapsible>
        <div className="flex items-center gap-2 rounded-md border px-2 py-1.5">
          {/* 拖拽把手：仅把手响应拖拽，避免与行内输入框/开关冲突 */}
          <button
            type="button"
            {...attributes}
            {...listeners}
            className="text-muted-foreground hover:text-foreground cursor-grab text-[10px] active:cursor-grabbing"
            title={t('dragToReorder')}
          >
            <i className="fa-solid fa-grip-vertical size-3" />
          </button>
          <div className="min-w-0 flex-1">
            <div className="truncate text-xs font-medium">{label}</div>
            <div className="text-muted-foreground truncate text-[10px]">{subLabel}</div>
          </div>
          <div className="flex items-center gap-1.5">
            <Label className="text-[10px]">{t('priority')}</Label>
            <Input
              type="number"
              value={detail.priority}
              onChange={(e) => onDetailChange(index, 'priority', Number(e.target.value))}
              className="h-6 w-14 text-[10px]"
              min={0}
            />
          </div>
          <div className="flex items-center gap-1">
            <Switch
              checked={detail.enabled}
              onCheckedChange={(v) => onDetailChange(index, 'enabled', v)}
              className="scale-75"
              title={t('enabled')}
            />
            <Label className="text-[10px]">
              {detail.enabled ? t('enabled') : t('disabled')}
            </Label>
          </div>
          <div className="flex items-center gap-1">
            <Switch
              checked={detail.isHealthy}
              onCheckedChange={(v) => onDetailChange(index, 'isHealthy', v)}
              className="scale-75"
            />
            <Label className="text-[10px]">
              {detail.isHealthy ? t('healthy') : t('unhealthy')}
            </Label>
          </div>
          <div className="flex items-center gap-1.5">
            <Label className="text-[10px]">{t('maxRetries')}</Label>
            <Input
              type="number"
              value={detail.maxRetries}
              onChange={(e) => onDetailChange(index, 'maxRetries', Number(e.target.value))}
              className="h-6 w-14 text-[10px]"
              min={0}
            />
          </div>
          <div className="flex items-center gap-1.5">
            <Label className="text-[10px]">{t('retryInterval')}</Label>
            <Input
              type="number"
              value={detail.retryIntervalMs}
              onChange={(e) => onDetailChange(index, 'retryIntervalMs', Number(e.target.value))}
              className="h-6 w-20 text-[10px]"
              min={0}
            />
            <span className="text-muted-foreground text-[10px]">ms</span>
          </div>
          {strategy === 'load_balance' && (
            <div className="flex items-center gap-1.5">
              <Label className="text-[10px]">{t('weight')}</Label>
              <Input
                type="number"
                value={detail.weight}
                onChange={(e) => onDetailChange(index, 'weight', Number(e.target.value))}
                className="h-6 w-12 text-[10px]"
                min={0}
                title={t('weightHint')}
              />
            </div>
          )}
          {/* 测试按钮：对目标供应商发起探活请求，仅已保存路由可用 */}
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation()
              void onTest(detail)
            }}
            disabled={testingId === detail.key || !detail.routeId}
            className={cn(
              'text-muted-foreground hover:text-foreground ml-1 text-[10px] transition-colors',
              'disabled:cursor-not-allowed disabled:opacity-40',
            )}
            title={t('testRoute')}
          >
            <i className={cn('fa-solid fa-bolt size-3', testingId === detail.key && 'animate-pulse text-amber-500')} />
          </button>
          <CollapsibleTrigger
            className="text-muted-foreground hover:text-foreground ml-1 text-[10px] underline-offset-2 hover:underline"
            title={t('advanced')}
          >
            <i className="fa-solid fa-sliders size-3" />
          </CollapsibleTrigger>
        </div>
        <CollapsibleContent>
          <div className="border-border/60 mt-1 grid grid-cols-3 gap-2 rounded-md border bg-muted/30 px-2 py-1.5 text-[10px]">
            <div className="flex items-center gap-1.5">
              <Label className="text-[10px] whitespace-nowrap">{t('timeoutMs')}</Label>
              <Input
                type="number"
                value={detail.timeoutMs ?? ''}
                onChange={(e) => {
                  const v = e.target.value
                  onDetailChange(index, 'timeoutMs', v === '' ? undefined : Number(v))
                }}
                placeholder={t('inherit')}
                className="h-6 w-20 text-[10px]"
                min={0}
              />
              <span className="text-muted-foreground">ms</span>
            </div>
            <div className="col-span-2 flex flex-col gap-1">
              <Label className="text-[10px]">{t('extraHeaders')}</Label>
              <textarea
                value={detail.extraHeadersJson ?? ''}
                onChange={(e) => onDetailChange(index, 'extraHeadersJson', e.target.value)}
                placeholder='{"X-Custom":"value"}'
                className="h-10 w-full resize-y rounded border bg-background px-1.5 py-1 font-mono text-[10px]"
              />
            </div>
            <div className="col-span-3 flex flex-col gap-1">
              <Label className="text-[10px]">{t('extraBody')}</Label>
              <textarea
                value={detail.extraBodyJson ?? ''}
                onChange={(e) => onDetailChange(index, 'extraBodyJson', e.target.value)}
                placeholder='{"temperature":0.3}'
                className="h-12 w-full resize-y rounded border bg-background px-1.5 py-1 font-mono text-[10px]"
              />
            </div>
          </div>
        </CollapsibleContent>
      </Collapsible>
    </div>
  )
}

export function RouteSettingsList({
  details,
  onDetailChange,
  providerModels,
  realProviders,
  strategy = 'fallback',
  className,
}: RouteSettingsListProps) {
  const { t } = useTranslation('virtualProvider')
  const [testingId, setTestingId] = React.useState<string | null>(null)

  const providerById = React.useMemo(() => {
    const map = new Map<string, Provider>()
    for (const provider of realProviders) {
      map.set(provider.id, provider)
    }
    return map
  }, [realProviders])

  const handleDetailChange = (idx: number, field: keyof SelectedModelDetail, value: number | boolean | string | undefined) => {
    const newDetails = [...details]
    newDetails[idx] = { ...newDetails[idx], [field]: value }
    onDetailChange(newDetails)
  }

  /** 测试单条路由：调用后端探活命令，结果用 toast 展示 */
  const handleTest = async (detail: SelectedModelDetail) => {
    if (!detail.routeId) {
      toast.warning(t('testRouteNotSaved'))
      return
    }
    setTestingId(detail.key)
    try {
      const result = await testRoute(detail.routeId)
      if (result.success) {
        toast.success(
          t('testRouteSuccess', { status: result.statusCode ?? '200', duration: result.durationMs }),
        )
      } else {
        toast.error(
          t('testRouteFailed', {
            status: result.statusCode ?? '—',
            duration: result.durationMs,
            error: result.errorMessage ?? '',
          }),
        )
      }
    } catch (err) {
      toast.error(String(err))
    } finally {
      setTestingId(null)
    }
  }

  /** 拖拽排序结束：按新顺序重排 details 并自动重算 priority（数值越小越优先） */
  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event
    if (!over || active.id === over.id) return
    const oldIndex = details.findIndex((d) => d.key === active.id)
    const newIndex = details.findIndex((d) => d.key === over.id)
    if (oldIndex === -1 || newIndex === -1) return
    const next = arrayMove(details, oldIndex, newIndex).map((d, i) => ({
      ...d,
      priority: i,
    }))
    onDetailChange(next)
  }

  if (details.length === 0) {
    return (
      <div className={className}>
        <div className="text-muted-foreground py-8 text-center text-xs">
          {t('noModels')}
        </div>
      </div>
    )
  }

  return (
    <div className={className}>
      <DndContext collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext
          items={details.map((d) => d.key)}
          strategy={verticalListSortingStrategy}
        >
          <div className="space-y-1.5">
            {details.map((detail, idx) => (
              <SortableRouteItem
                key={detail.key}
                detail={detail}
                index={idx}
                strategy={strategy}
                testingId={testingId}
                providerById={providerById}
                providerModels={providerModels}
                onDetailChange={handleDetailChange}
                onTest={handleTest}
              />
            ))}
          </div>
        </SortableContext>
      </DndContext>
    </div>
  )
}
