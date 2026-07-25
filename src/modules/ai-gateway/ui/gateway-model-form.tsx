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
import type { Provider, ModelConfig } from '@/modules/ai-gateway/types'

const schema = z.object({
  providerId: z.string().min(1, 'aiGateway.gatewayModelForm.validation.providerRequired'),
  modelConfigId: z.string().min(1, 'aiGateway.gatewayModelForm.validation.modelConfigRequired'),
  modelId: z.string().min(1, 'aiGateway.gatewayModelForm.validation.modelIdRequired'),
  displayName: z.string().optional(),
  family: z.string().optional(),
  isExposed: z.boolean(),
})

type FormValues = z.infer<typeof schema>

interface GatewayModelFormProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  providers: Provider[]
  configs: ModelConfig[]
  onSubmit: (values: FormValues) => void
}

/**
 * 网关模型新增表单
 *
 * 将已有模型配置挂载到指定供应商下，形成对外暴露的 Gateway 模型。
 */
export function GatewayModelForm({ open, onOpenChange, providers, configs, onSubmit }: GatewayModelFormProps) {
  const { t } = useTranslation()

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      providerId: '',
      modelConfigId: '',
      modelId: '',
      displayName: '',
      family: '',
      isExposed: true,
    },
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="text-base">{t('aiGateway.addModel')}</DialogTitle>
          <DialogDescription className="text-xs">{t('aiGateway.gatewayModelForm.description')}</DialogDescription>
        </DialogHeader>

        <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
          <div className="space-y-1.5">
            <Label className="text-xs">{t('aiGateway.gatewayModelForm.provider')}</Label>
            <Select value={form.watch('providerId')} onValueChange={(v) => form.setValue('providerId', v)}>
              <SelectTrigger className="h-8 text-xs">
                <SelectValue placeholder={t('aiGateway.gatewayModelForm.providerPlaceholder')} />
              </SelectTrigger>
              <SelectContent>
                {providers.map((p) => (
                  <SelectItem key={p.id} value={p.id} className="text-xs">
                    {p.displayName}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {form.formState.errors.providerId && <p className="text-destructive text-[10px]">{t(form.formState.errors.providerId.message || '')}</p>}
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs">{t('aiGateway.gatewayModelForm.modelConfig')}</Label>
            <Select value={form.watch('modelConfigId')} onValueChange={(v) => form.setValue('modelConfigId', v)}>
              <SelectTrigger className="h-8 text-xs">
                <SelectValue placeholder={t('aiGateway.gatewayModelForm.modelConfigPlaceholder')} />
              </SelectTrigger>
              <SelectContent>
                {configs.map((c) => (
                  <SelectItem key={c.id} value={c.id} className="text-xs">
                    {c.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {form.formState.errors.modelConfigId && <p className="text-destructive text-[10px]">{t(form.formState.errors.modelConfigId.message || '')}</p>}
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs">{t('aiGateway.gatewayModelForm.modelId')}</Label>
            <Input {...form.register('modelId')} className="h-8 text-xs" placeholder={t('aiGateway.gatewayModelForm.modelIdPlaceholder')} />
            {form.formState.errors.modelId && <p className="text-destructive text-[10px]">{t(form.formState.errors.modelId.message || '')}</p>}
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <Label className="text-xs">{t('aiGateway.gatewayModelForm.displayName')}</Label>
              <Input {...form.register('displayName')} className="h-8 text-xs" />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs">{t('aiGateway.gatewayModelForm.family')}</Label>
              <Input {...form.register('family')} className="h-8 text-xs" />
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Switch checked={form.watch('isExposed')} onCheckedChange={(v) => form.setValue('isExposed', v)} />
            <Label className="text-xs">{t('aiGateway.gatewayModelForm.isExposed')}</Label>
          </div>

          <DialogFooter>
            <Button type="submit" size="sm" className="h-8 text-xs">
              <i className="fa-solid fa-check mr-1.5" />
              {t('aiGateway.gatewayModelForm.confirm')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
