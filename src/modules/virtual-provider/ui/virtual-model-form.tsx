"use client"

import { useState, useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Separator } from '@/components/ui/separator'
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { WideDialogContent } from '@/components/ui/wide-dialog'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ModelTransferList, RouteSettingsList } from './model-transfer-list'
import { useProviderList } from '@/hooks/use-provider-list'
import { useAllModels } from '@/hooks/use-virtual-provider'
import { saveVirtualModel } from '@/hooks/use-virtual-provider-mutation'
import { invokeCommand } from '@/hooks/use-command'
import { toast } from 'sonner'
import { toIcodeError } from '@/core/errors'
import type {
  VirtualModel,
  VirtualModelRoute,
  SaveVirtualModelRouteInput,
} from '@/modules/virtual-provider/types'
import type { SelectedModelDetail } from './model-transfer-list'

const schema = z.object({
  modelId: z.string().min(1, '模型 ID 不能为空'),
  displayName: z.string().optional(),
  isEnabled: z.boolean(),
})

type FormValues = z.infer<typeof schema>

interface VirtualModelDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 所属虚拟供应商 ID */
  virtualProviderId: string
  /** 虚拟供应商策略；load_balance 时显示 weight 输入 */
  strategy?: 'fallback' | 'on_all' | 'load_balance'
  /** 传入 model 表示编辑模式；不传表示新建模式 */
  model?: VirtualModel | null
  onSuccess?: () => void
}

/**
 * 虚拟模型弹窗（新建/编辑共用，超宽分栏版）
 *
 * 左侧填写父级虚拟模型信息，中间竖线分隔，右侧通过模型穿梭框选择子级真实模型。
 * 提交时一次性把虚拟模型与全部子级路由发给后端，后端在事务中重新关联子级路由。
 */
export function VirtualModelDialog({
  open,
  onOpenChange,
  virtualProviderId,
  strategy,
  model,
  onSuccess,
}: VirtualModelDialogProps) {
  const { t } = useTranslation('virtualProvider')
  const { providers: realProviders } = useProviderList()
  const { models: allModels } = useAllModels()
  const isEdit = Boolean(model)

  // 编辑模式下拉取该虚拟模型已有的路由（内部状态，不使用 useVirtualRoutes hook）
  const [routesForEdit, setRoutesForEdit] = useState<VirtualModelRoute[]>([])
  const [routesLoading, setRoutesLoading] = useState(false)

  const [selectedKeys, setSelectedKeys] = useState<string[]>([])
  const [routeDetails, setRouteDetails] = useState<SelectedModelDetail[]>([])
  const [submitting, setSubmitting] = useState(false)

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      modelId: '',
      displayName: '',
      isEnabled: true,
    },
  })

  // 弹窗打开时：重置表单 + 加载路由数据并填充选中态
  // 直接用 invokeCommand + await，避免 useVirtualRoutes 缓存与 effect 时序问题
  useEffect(() => {
    if (!open) return

    // 重置表单
    if (model) {
      form.reset({
        modelId: model.modelId,
        displayName: model.displayName ?? '',
        isEnabled: model.isEnabled,
      })
    } else {
      form.reset({
        modelId: '',
        displayName: '',
        isEnabled: true,
      })
    }

    // 清空选中态
    setSelectedKeys([])
    setRouteDetails([])

    // 编辑模式：直接调用后端命令加载最新路由，await 后用结果填充
    if (!model?.id) return
    let cancelled = false
    setRoutesLoading(true)
    void invokeCommand<VirtualModelRoute[]>('virtual_provider_route_list', {
      virtualModelId: model.id,
    })
      .then((result) => {
        if (cancelled) return
        setRoutesForEdit(result)
        // 直接用返回结果填充选中态，不依赖 state 时序
        const keys = result.map((route) => `${route.targetProviderId}/${route.targetModelId}`)
        setSelectedKeys(keys)
        setRouteDetails(
          result.map((route) => ({
            key: `${route.targetProviderId}/${route.targetModelId}`,
            routeId: route.id,
            priority: Number(route.priority),
            enabled: route.enabled,
            isHealthy: route.isHealthy,
            maxRetries: Number(route.maxRetries),
            retryIntervalMs: Number(route.retryIntervalMs),
            timeoutMs: route.timeoutMs != null ? Number(route.timeoutMs) : undefined,
            extraHeadersJson: route.extraHeadersJson ?? '',
            extraBodyJson: route.extraBodyJson ?? '',
            weight: Number(route.weight ?? 1),
          }))
        )
      })
      .catch(() => {
        if (cancelled) return
        setRoutesForEdit([])
      })
      .finally(() => {
        if (!cancelled) setRoutesLoading(false)
      })

    return () => {
      cancelled = true
    }
  }, [open, model, form])

  const errors = form.formState.errors

  /**
   * 穿梭框选择变化时同步 selectedKeys 与 routeDetails
   * - 新增的 key：创建默认 detail（priority 按顺序、isHealthy=true、maxRetries=3、retryIntervalMs=1000）
   * - 移除的 key：删除对应 detail
   * - 保留的 key：沿用已有 detail
   */
  const handleSelectionChange = (keys: string[]) => {
    setSelectedKeys(keys)
    setRouteDetails((prev) => {
      const prevMap = new Map(prev.map((d) => [d.key, d]))
      return keys.map((key, index) => {
        const existing = prevMap.get(key)
        if (existing) return existing
        // 查找 routesForEdit 中是否已有该路由的配置（编辑模式下首次加载）
        const slashIdx = key.indexOf('/')
        const pid = slashIdx >= 0 ? key.slice(0, slashIdx) : key
        const mid = slashIdx >= 0 ? key.slice(slashIdx + 1) : ''
        const route = routesForEdit.find(
          (r) => r.targetProviderId === pid && r.targetModelId === mid
        )
        return {
          key,
          priority: route ? Number(route.priority) : index,
          enabled: route ? route.enabled : true,
          isHealthy: route ? route.isHealthy : true,
          maxRetries: route ? Number(route.maxRetries) : 3,
          retryIntervalMs: route ? Number(route.retryIntervalMs) : 1000,
          timeoutMs: route?.timeoutMs != null ? Number(route.timeoutMs) : undefined,
          extraHeadersJson: route?.extraHeadersJson ?? '',
          extraBodyJson: route?.extraBodyJson ?? '',
          weight: route ? Number(route.weight ?? 1) : 1,
        }
      })
    })
  }

  const handleSubmit = form.handleSubmit(async (values) => {
    setSubmitting(true)
    try {
      // 从 routeDetails 构建 routes，如果没有 details 则按 selectedKeys 顺序用默认值
      // key 格式为 `{providerId}/{modelId}`，modelId 可能含 '/'，用 indexOf 拆分
      const routes: SaveVirtualModelRouteInput[] = selectedKeys.map((key, index) => {
        const slashIdx = key.indexOf('/')
        const targetProviderId = slashIdx >= 0 ? key.slice(0, slashIdx) : key
        const targetModelId = slashIdx >= 0 ? key.slice(slashIdx + 1) : ''
        const detail = routeDetails.find((d) => d.key === key)

        // 解析 extraHeaders / extraBody JSON 字符串；空串或非法 JSON 跳过
        const parseJsonObj = (raw?: string): Record<string, unknown> | undefined => {
          const trimmed = (raw ?? '').trim()
          if (!trimmed) return undefined
          try {
            const parsed = JSON.parse(trimmed)
            if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
              return parsed as Record<string, unknown>
            }
          } catch {
            // 非法 JSON 直接跳过；前端可在此前置校验提示用户
          }
          return undefined
        }

        return {
          targetProviderId,
          targetModelId,
          priority: detail?.priority ?? index,
          enabled: detail?.enabled ?? true,
          isHealthy: detail?.isHealthy ?? true,
          maxRetries: detail?.maxRetries ?? 3,
          retryIntervalMs: detail?.retryIntervalMs ?? 1000,
          timeoutMs: detail?.timeoutMs,
          extraHeaders: parseJsonObj(detail?.extraHeadersJson),
          extraBody: parseJsonObj(detail?.extraBodyJson),
          weight: detail?.weight ?? 1,
        }
      })

      await saveVirtualModel({
        id: model?.id,
        virtualProviderId,
        modelId: values.modelId,
        displayName: values.displayName,
        isEnabled: values.isEnabled,
        routes,
      })

      toast.success(isEdit ? t('editModel') : t('newModel'))
      onOpenChange(false)
      form.reset()
      setSelectedKeys([])
      setRouteDetails([])
      onSuccess?.()
    } catch (err) {
      toast.error(toIcodeError(err).message)
    } finally {
      setSubmitting(false)
    }
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <WideDialogContent className="max-w-4xl">
        <DialogHeader>
          <DialogTitle className="text-base">
            {isEdit ? t('editModel') : t('newModel')}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {isEdit ? t('editModelDescription') : t('createModelDescription')}
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="flex flex-col gap-3">
          <Tabs defaultValue="association" className="min-h-0">
            <TabsList className="h-8">
              <TabsTrigger value="association" className="text-xs">
                {t('modelAssociation')}
              </TabsTrigger>
              <TabsTrigger value="routes" className="text-xs">
                {t('routeSettings')}
              </TabsTrigger>
            </TabsList>

            {/* Tab 1：模型关联（父级信息 + 穿梭框） */}
            <TabsContent value="association" className="grid grid-cols-[1fr_auto_3fr] gap-4">
              {/* 左侧：父级虚拟模型信息 */}
              <div className="space-y-2">
                <h4 className="text-xs font-medium">{t('modelInfo')}</h4>

                <div className="space-y-1.5" data-invalid={errors.modelId ? '' : undefined}>
                  <Label className="text-xs" htmlFor="vm-modelId">{t('modelId')}</Label>
                  <Input
                    id="vm-modelId"
                    {...form.register('modelId')}
                    className="h-8 text-xs"
                    placeholder={t('modelIdPlaceholder')}
                    aria-invalid={errors.modelId ? true : undefined}
                  />
                  {errors.modelId && (
                    <p className="text-destructive text-[10px]">{errors.modelId.message}</p>
                  )}
                </div>

                <div className="space-y-1.5" data-invalid={errors.displayName ? '' : undefined}>
                  <Label className="text-xs" htmlFor="vm-displayName">{t('modelDisplayName')}</Label>
                  <Input
                    id="vm-displayName"
                    {...form.register('displayName')}
                    className="h-8 text-xs"
                  />
                </div>

                <div className="flex items-center gap-2">
                  <Switch
                    id="vm-isEnabled"
                    checked={form.watch('isEnabled')}
                    onCheckedChange={(v) => form.setValue('isEnabled', v)}
                  />
                  <Label className="text-xs" htmlFor="vm-isEnabled">{t('enabledVirtualModel')}</Label>
                </div>
              </div>

              {/* 中间：竖线分隔 */}
              <Separator orientation="vertical" className="h-auto" />

              {/* 右侧：模型穿梭框 */}
              <div className="flex min-h-0 flex-col">
                {routesLoading && isEdit ? (
                  <div className="text-muted-foreground flex flex-1 items-center justify-center text-xs">
                    {t('loading')}
                  </div>
                ) : (
                  <ModelTransferList
                    providerModels={allModels}
                    realProviders={realProviders}
                    routes={routesForEdit}
                    selectedIds={selectedKeys}
                    onChange={handleSelectionChange}
                    className="min-h-0 flex-1"
                  />
                )}
              </div>
            </TabsContent>

            {/* Tab 2：路由属性设置（原生滚动） */}
            <TabsContent value="routes">
              <div className="overflow-y-auto pr-1" style={{ maxHeight: '60vh' }}>
                {routesLoading && isEdit ? (
                  <div className="text-muted-foreground py-8 text-center text-xs">
                    {t('loading')}
                  </div>
                ) : (
                  <RouteSettingsList
                    details={routeDetails}
                    onDetailChange={setRouteDetails}
                    providerModels={allModels}
                    realProviders={realProviders}
                    strategy={strategy}
                  />
                )}
              </div>
            </TabsContent>
          </Tabs>

          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-8 text-xs"
              onClick={() => onOpenChange(false)}
            >
              {t('cancel', { ns: 'common' })}
            </Button>
            <Button type="submit" size="sm" className="h-8 text-xs" disabled={submitting}>
              <i className="fa-solid fa-check mr-1.5" data-icon="inline-start" />
              {t('save', { ns: 'common' })}
            </Button>
          </DialogFooter>
        </form>
      </WideDialogContent>
    </Dialog>
  )
}
