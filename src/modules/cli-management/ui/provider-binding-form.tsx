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
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { CliProvider } from '@/modules/cli-management/types'
import type { CatalogProvider } from '@/modules/gateway-runtime/types'

const schema = z
  .object({
    displayName: z.string().min(1, '显示名称不能为空'),
    providerId: z.string().optional(),
    routeMode: z.number(),
    gatewayBaseUrl: z.string().optional(),
    directBaseUrl: z.string().optional(),
    sortOrder: z.coerce.number().int(),
    isDefault: z.boolean(),
  })
  .superRefine((values, context) => {
    if (values.routeMode === 1 && !values.providerId) {
      context.addIssue({ code: z.ZodIssueCode.custom, path: ['providerId'], message: '请选择供应商' })
    }
    if (values.routeMode === 0 && !values.directBaseUrl?.trim()) {
      context.addIssue({ code: z.ZodIssueCode.custom, path: ['directBaseUrl'], message: '请输入直连地址' })
    }
  })

type FormValues = z.infer<typeof schema>

interface ProviderBindingFormProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  profileId: string
  binding?: CliProvider | null
  providers: CatalogProvider[]
  onSubmit: (values: FormValues) => void
}

/**
 * CLI 供应商绑定新增/编辑表单
 */
export function ProviderBindingForm({
  open,
  onOpenChange,
  binding,
  providers,
  onSubmit,
}: ProviderBindingFormProps) {
  const { t } = useTranslation()
  const isEdit = Boolean(binding)

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      displayName: '',
      providerId: '',
      routeMode: 1,
      gatewayBaseUrl: '',
      directBaseUrl: '',
      sortOrder: 0,
      isDefault: false,
    },
  })

  useEffect(() => {
    if (binding) {
      form.reset({
        displayName: binding.displayName,
        providerId: binding.providerId ?? '',
        routeMode: binding.routeMode,
        gatewayBaseUrl: binding.gatewayBaseUrl ?? '',
        directBaseUrl: binding.directBaseUrl ?? '',
        sortOrder: binding.sortOrder,
        isDefault: binding.isDefault,
      })
    } else {
      form.reset({
        displayName: '',
        providerId: '',
        routeMode: 1,
        gatewayBaseUrl: '',
        directBaseUrl: '',
        sortOrder: 0,
        isDefault: false,
      })
    }
  }, [binding, form])

  const routeMode = form.watch('routeMode')
  const providerId = form.watch('providerId')

  /** 当前选中的目录供应商（虚拟供应商仅支持网关路由） */
  const selectedProviderIsVirtual = providers.find((p) => p.id === providerId)?.isVirtual ?? false

  // 选中虚拟供应商时强制走网关路由（route_mode=1），并同步 baseUrl 默认值
  useEffect(() => {
    if (!selectedProviderIsVirtual) return
    form.setValue('routeMode', 1)
  }, [selectedProviderIsVirtual, form])

  /**
   * 当选择直连模式且已选供应商时，直连 Base URL 跟随所选 Gateway 供应商的 baseUrl 同步调整。
   */
  useEffect(() => {
    if (routeMode !== 0 || !providerId) return
    const provider = providers.find((p) => p.id === providerId)
    if (!provider?.baseUrl) return
    form.setValue('directBaseUrl', provider.baseUrl)
  }, [routeMode, providerId, providers, form])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="text-base">
            {isEdit ? t('cli.providerForm.editTitle') : t('cli.providerForm.createTitle')}
          </DialogTitle>
          <DialogDescription className="text-xs">{t('cli.providerForm.description')}</DialogDescription>
        </DialogHeader>

        <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
          <div className="space-y-1.5">
            <Label className="text-xs">{t('cli.providerForm.displayName')}</Label>
            <Input {...form.register('displayName')} className="h-8 text-xs" />
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs">{t('cli.providerForm.gatewayProvider')}</Label>
            <Select
              value={form.watch('providerId')}
              onValueChange={(v) => form.setValue('providerId', v)}
            >
              <SelectTrigger className="h-8 text-xs">
                <SelectValue placeholder={t('cli.providerForm.selectProvider')} />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  {providers.map((p) => (
                    <SelectItem key={p.id} value={p.id} className="text-xs">
                      {p.displayName}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs">{t('cli.providerForm.routeMode')}</Label>
            <Select
              value={String(routeMode)}
              onValueChange={(v) => form.setValue('routeMode', Number(v))}
            >
              <SelectTrigger className="h-8 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="1" className="text-xs">{t('cli.route.gateway')}</SelectItem>
                  <SelectItem value="0" className="text-xs" disabled={selectedProviderIsVirtual}>
                    {t('cli.route.direct')}
                  </SelectItem>
                </SelectGroup>
              </SelectContent>
            </Select>
          </div>

          {routeMode === 1 ? (
            <div className="space-y-1.5">
              <Label className="text-xs">{t('cli.providerForm.gatewayBaseUrl')}</Label>
              <Input {...form.register('gatewayBaseUrl')} className="h-8 text-xs" placeholder="http://127.0.0.1:54321" />
            </div>
          ) : (
            <div className="space-y-1.5">
              <Label className="text-xs">{t('cli.providerForm.directBaseUrl')}</Label>
              <Input {...form.register('directBaseUrl')} className="h-8 text-xs" placeholder="https://api.example.com" />
            </div>
          )}

          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <Label className="text-xs">{t('cli.providerForm.sortOrder')}</Label>
              <Input type="number" {...form.register('sortOrder')} className="h-8 text-xs" />
            </div>
            <div className="flex items-end pb-1.5">
              <label className="flex items-center gap-2 text-xs">
                <Switch checked={form.watch('isDefault')} onCheckedChange={(v) => form.setValue('isDefault', v)} />
                {t('cli.providerForm.defaultBinding')}
              </label>
            </div>
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
