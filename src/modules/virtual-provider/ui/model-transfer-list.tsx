"use client"

import * as React from "react"

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
import { useTranslation } from "@/modules/i18n/use-translation"
import type { ExposedModel, Provider } from "@/modules/ai-gateway/types"
import type { VirtualModelRoute } from "@/modules/virtual-provider/types"

/** 单条已选模型的可编辑属性 */
export interface SelectedModelDetail {
  /** 模型键：{provider_id}/{model_id} */
  key: string
  /** 优先级（数值越小越优先） */
  priority: number
  /** 是否健康 */
  isHealthy: boolean
  /** 最大重试次数 */
  maxRetries: number
  /** 重试间隔毫秒数 */
  retryIntervalMs: number
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
  /** 已选模型可编辑属性变化回调 */
  onDetailChange?: (details: SelectedModelDetail[]) => void
  /** 已选模型可编辑属性（受控） */
  details?: SelectedModelDetail[]
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
  onDetailChange,
  details: controlledDetails,
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
        label: model.displayName || model.modelId,
        description: provider.displayName,
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

  // 已选模型属性（受控/非受控）
  const details: SelectedModelDetail[] = React.useMemo(() => {
    if (controlledDetails) {
      return controlledDetails
    }
    // 从 routes 派生默认值
    return selectedIds.map((key, index) => {
      // 查找匹配路由
      const route = routes.find((r) => {
        const provider = providerById.get(r.targetProviderId)
        return provider && `${provider.id}/${r.targetModelId}` === key
      })
      return {
        key,
        priority: route ? Number(route.priority) : index,
        isHealthy: route ? route.isHealthy : true,
        maxRetries: route ? Number(route.maxRetries) : 3,
        retryIntervalMs: route ? Number(route.retryIntervalMs) : 1000,
      }
    })
  }, [controlledDetails, selectedIds, routes, providerById])

  const handleDetailChange = (idx: number, field: keyof SelectedModelDetail, value: number | boolean) => {
    const newDetails = [...details]
    newDetails[idx] = { ...newDetails[idx], [field]: value }
    onDetailChange?.(newDetails)
  }

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
      {/* 已选模型属性编辑区 */}
      {selectedIds.length > 0 && (
        <div className="mt-3">
          <span className="text-xs font-medium">{t('routeSettings')}</span>
          <div className="mt-1.5 space-y-1.5">
            {details.map((detail, idx) => {
              const [pid, mid] = detail.key.split('/')
              const provider = providerById.get(pid)
              const model = providerModels.find(
                (m) => m.providerSlug === provider?.slug && m.modelId === mid
              )
              const label = model?.displayName || mid
              const providerLabel = provider?.displayName || pid

              return (
                <div
                  key={detail.key}
                  className="flex items-center gap-2 rounded-md border px-2 py-1.5"
                >
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-xs font-medium">{label}</div>
                    <div className="text-muted-foreground truncate text-[10px]">{providerLabel}</div>
                  </div>
                  <div className="flex items-center gap-1.5">
                    <Label className="text-[10px]">{t('priority')}</Label>
                    <Input
                      type="number"
                      value={detail.priority}
                      onChange={(e) => handleDetailChange(idx, 'priority', Number(e.target.value))}
                      className="h-6 w-14 text-[10px]"
                      min={0}
                    />
                  </div>
                  <div className="flex items-center gap-1">
                    <Switch
                      checked={detail.isHealthy}
                      onCheckedChange={(v) => handleDetailChange(idx, 'isHealthy', v)}
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
                      onChange={(e) => handleDetailChange(idx, 'maxRetries', Number(e.target.value))}
                      className="h-6 w-14 text-[10px]"
                      min={0}
                    />
                  </div>
                  <div className="flex items-center gap-1.5">
                    <Label className="text-[10px]">{t('retryInterval')}</Label>
                    <Input
                      type="number"
                      value={detail.retryIntervalMs}
                      onChange={(e) => handleDetailChange(idx, 'retryIntervalMs', Number(e.target.value))}
                      className="h-6 w-20 text-[10px]"
                      min={0}
                    />
                    <span className="text-muted-foreground text-[10px]">ms</span>
                  </div>
                </div>
              )
            })}
          </div>
        </div>
      )}
    </div>
  )
}
