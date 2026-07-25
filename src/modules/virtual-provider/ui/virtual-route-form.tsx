import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { useProviderGatewayModels } from '@/hooks/use-virtual-provider'
import type { VirtualModelRoute } from '@/modules/virtual-provider/types'
import type { Provider } from '@/modules/ai-gateway/types'

const schema = z.object({
  targetProviderId: z.string().min(1, '请选择目标供应商'),
  targetModelId: z.string().min(1, '请选择目标模型'),
  priority: z.coerce.number().int(),
  enabled: z.boolean(),
  maxRetries: z.coerce.number().int(),
  timeoutMs: z.coerce.number().int().optional(),
})

type FormValues = z.infer<typeof schema>

interface VirtualRouteFormProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  route?: VirtualModelRoute | null
  providers: Provider[]
  onSubmit: (values: FormValues) => void
}

/**
 * 虚拟模型路由新增/编辑表单
 *
 * 选择目标真实供应商及其模型，作为虚拟模型故障转移的一条候选路由。
 */
export function VirtualRouteForm({ open, onOpenChange, route, providers, onSubmit }: VirtualRouteFormProps) {
  const { t } = useTranslation()
  const isEdit = Boolean(route)

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      targetProviderId: '',
      targetModelId: '',
      priority: 0,
      enabled: true,
      maxRetries: 0,
      timeoutMs: undefined,
    },
  })

  const targetProviderId = form.watch('targetProviderId')
  const { models: targetModels, refetch: refetchTargetModels } = useProviderGatewayModels(targetProviderId || null)

  useEffect(() => {
    if (route) {
      form.reset({
        targetProviderId: route.targetProviderId,
        targetModelId: route.targetModelId,
        priority: Number(route.priority),
        enabled: route.enabled,
        maxRetries: Number(route.maxRetries),
        timeoutMs: route.timeoutMs ?? undefined,
      })
    } else {
      form.reset({
        targetProviderId: '',
        targetModelId: '',
        priority: 0,
        enabled: true,
        maxRetries: 0,
        timeoutMs: undefined,
      })
    }
  }, [route, form])

  // 切换目标供应商时刷新模型列表
  useEffect(() => {
    if (targetProviderId) {
      void refetchTargetModels()
      // 如果当前选中的模型不在新供应商下，清空选择
      const exists = targetModels.some((m) => m.modelId === form.getValues('targetModelId'))
      if (!exists) {
        form.setValue('targetModelId', '')
      }
    }
  }, [targetProviderId, refetchTargetModels, form, targetModels])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="text-base">{isEdit ? '编辑路由' : '新增路由'}</DialogTitle>
          <DialogDescription className="text-xs">选择一条真实供应商模型作为故障转移候选</DialogDescription>
        </DialogHeader>

        <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
          <div className="space-y-1.5">
            <Label className="text-xs">目标供应商</Label>
            <Select value={targetProviderId} onValueChange={(v) => form.setValue('targetProviderId', v)}>
              <SelectTrigger className="h-8 text-xs">
                <SelectValue placeholder="选择供应商" />
              </SelectTrigger>
              <SelectContent>
                {providers.map((provider) => (
                  <SelectItem key={provider.id} value={provider.id} className="text-xs">
                    {provider.displayName}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs">目标模型</Label>
            <Select
              value={form.watch('targetModelId')}
              onValueChange={(v) => form.setValue('targetModelId', v)}
              disabled={!targetProviderId}
            >
              <SelectTrigger className="h-8 text-xs">
                <SelectValue placeholder={targetProviderId ? '选择模型' : '请先选择供应商'} />
              </SelectTrigger>
              <SelectContent>
                {targetModels.map((model) => (
                  <SelectItem key={model.id} value={model.modelId} className="text-xs">
                    {model.displayName ?? model.modelId}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <Label className="text-xs">优先级</Label>
              <Input type="number" {...form.register('priority')} className="h-8 text-xs" />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs">最大重试</Label>
              <Input type="number" {...form.register('maxRetries')} className="h-8 text-xs" />
            </div>
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs">超时（毫秒，留空表示不限制）</Label>
            <Input type="number" {...form.register('timeoutMs')} className="h-8 text-xs" placeholder="30000" />
          </div>

          <div className="flex items-center gap-2">
            <Switch checked={form.watch('enabled')} onCheckedChange={(v) => form.setValue('enabled', v)} />
            <Label className="text-xs">启用该路由</Label>
          </div>

          <DialogFooter>
            <Button type="button" variant="ghost" size="sm" className="h-8 text-xs" onClick={() => onOpenChange(false)}>
              {t('common.cancel')}
            </Button>
            <Button type="submit" size="sm" className="h-8 text-xs">
              {t('common.save')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
