"use client"

import { useState, useEffect, useRef } from 'react'
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
import { ModelTransferList } from './model-transfer-list'
import { useProviderList } from '@/hooks/use-provider-list'
import { useAllModels, useVirtualRoutes } from '@/hooks/use-virtual-provider'
import { saveVirtualModel } from '@/hooks/use-virtual-provider-mutation'
import { toast } from 'sonner'
import { toIcodeError } from '@/core/errors'
import type {
  VirtualModel,
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
  model,
  onSuccess,
}: VirtualModelDialogProps) {
  const { t } = useTranslation('virtualProvider')
  const { providers: realProviders } = useProviderList()
  const { models: allModels } = useAllModels()
  const isEdit = Boolean(model)

  // 编辑模式下拉取该虚拟模型已有的路由
  const { routes: routesForEdit, loading: routesLoading } = useVirtualRoutes(model?.id ?? null)

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

  const prevOpenRef = useRef(false)

  // 打开弹窗或切换 model 时重置表单
  useEffect(() => {
    if (!open) return
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
  }, [open, model, form])

  // 仅在弹窗从关闭切换到打开时初始化穿梭框选中态
  useEffect(() => {
    if (open && !prevOpenRef.current) {
      if (isEdit && routesForEdit.length > 0) {
        const keys = routesForEdit.map((route) => `${route.targetProviderId}/${route.targetModelId}`)
        setSelectedKeys(keys)
        setRouteDetails(
          routesForEdit.map((route) => ({
            key: `${route.targetProviderId}/${route.targetModelId}`,
            priority: Number(route.priority),
            isHealthy: route.isHealthy,
            maxRetries: Number(route.maxRetries),
            retryIntervalMs: Number(route.retryIntervalMs),
          }))
        )
      } else {
        setSelectedKeys([])
        setRouteDetails([])
      }
    }
    prevOpenRef.current = open
  }, [open, isEdit, routesForEdit])

  // 路由数据加载完成后，若当前仍未选中（首次打开编辑模式），自动回显已有路由
  useEffect(() => {
    if (isEdit && routesForEdit.length > 0 && selectedKeys.length === 0 && open) {
      const keys = routesForEdit.map((route) => `${route.targetProviderId}/${route.targetModelId}`)
      setSelectedKeys(keys)
      setRouteDetails(
        routesForEdit.map((route) => ({
          key: `${route.targetProviderId}/${route.targetModelId}`,
          priority: Number(route.priority),
          isHealthy: route.isHealthy,
          maxRetries: Number(route.maxRetries),
          retryIntervalMs: Number(route.retryIntervalMs),
        }))
      )
    }
  }, [isEdit, routesForEdit, selectedKeys.length, open])

  const errors = form.formState.errors

  const handleSubmit = form.handleSubmit(async (values) => {
    setSubmitting(true)
    try {
      // 从 routeDetails 构建 routes，如果没有 details 则按 selectedKeys 顺序用默认值
      const routes: SaveVirtualModelRouteInput[] = selectedKeys.map((key, index) => {
        const [targetProviderId, targetModelId] = key.split('/')
        const detail = routeDetails.find((d) => d.key === key)
        return {
          targetProviderId,
          targetModelId,
          priority: detail?.priority ?? index,
          enabled: true,
          isHealthy: detail?.isHealthy ?? true,
          maxRetries: detail?.maxRetries ?? 3,
          retryIntervalMs: detail?.retryIntervalMs ?? 1000,
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

        <form onSubmit={handleSubmit} className="grid grid-cols-[2fr_auto_3fr] gap-4">
          {/* 左侧：父级虚拟模型信息 */}
          <div className="space-y-3">
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
                onChange={setSelectedKeys}
                details={routeDetails}
                onDetailChange={setRouteDetails}
                className="min-h-0 flex-1"
              />
            )}
          </div>

          <DialogFooter className="col-span-3">
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
