import { useEffect } from 'react'
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
import type { VirtualProvider, FailoverStrategy } from '@/modules/virtual-provider/types'

const schema = z.object({
  name: z.string().min(1, '名称不能为空'),
  alias: z.string().min(1, '别名不能为空').regex(/^[a-z0-9-]+$/, '别名只能包含小写字母、数字和横线'),
  displayName: z.string().optional(),
  strategy: z.enum(['fallback', 'on_all', 'load_balance']),
  isEnabled: z.boolean(),
  maxRetries: z.coerce.number().int().min(0).default(3),
  retryIntervalMs: z.coerce.number().int().min(0).default(1000),
})

type FormValues = z.infer<typeof schema>

interface VirtualProviderFormProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  provider?: VirtualProvider | null
  onSubmit: (values: FormValues) => void
}

/**
 * 虚拟供应商新增/编辑表单
 *
 * 聚合多个真实供应商并按策略故障转移。
 * 使用 shadcn/ui 标准表单模式。
 */
export function VirtualProviderForm({ open, onOpenChange, provider, onSubmit }: VirtualProviderFormProps) {
  const { t } = useTranslation()
  const isEdit = Boolean(provider)

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      name: '',
      alias: '',
      displayName: '',
      strategy: 'fallback',
      isEnabled: true,
    },
  })

  useEffect(() => {
    if (provider) {
      form.reset({
        name: provider.name,
        alias: provider.alias,
        displayName: provider.displayName ?? '',
        strategy: provider.strategy,
        isEnabled: provider.isEnabled,
      })
    } else {
      form.reset({
        name: '',
        alias: '',
        displayName: '',
        strategy: 'fallback',
        isEnabled: true,
      })
    }
  }, [provider, form])

  const errors = form.formState.errors

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="text-base">{isEdit ? '编辑虚拟供应商' : '新建虚拟供应商'}</DialogTitle>
          <DialogDescription className="text-xs">聚合多个真实供应商并按策略故障转移</DialogDescription>
        </DialogHeader>

        <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            {/* 名称 */}
            <div className="space-y-1.5" data-invalid={errors.name ? '' : undefined}>
              <Label className="text-xs" htmlFor="name">名称</Label>
              <Input
                id="name"
                {...form.register('name')}
                className="h-8 text-xs"
                aria-invalid={errors.name ? true : undefined}
              />
              {errors.name && (
                <p className="text-destructive text-[10px]">{errors.name.message}</p>
              )}
            </div>
            {/* 别名 */}
            <div className="space-y-1.5" data-invalid={errors.alias ? '' : undefined}>
              <Label className="text-xs" htmlFor="alias">别名（路由用）</Label>
              <Input
                id="alias"
                {...form.register('alias')}
                disabled={isEdit}
                className="h-8 text-xs"
                placeholder="my-virtual"
                aria-invalid={errors.alias ? true : undefined}
              />
            </div>
          </div>

          <div className="space-y-1.5" data-invalid={errors.displayName ? '' : undefined}>
            <Label className="text-xs" htmlFor="displayName">展示名称</Label>
            <Input
              id="displayName"
              {...form.register('displayName')}
              className="h-8 text-xs"
            />
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs">故障转移策略</Label>
            <Select
              value={form.watch('strategy')}
              onValueChange={(v) => form.setValue('strategy', v as FailoverStrategy)}
            >
              <SelectTrigger className="h-8 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="fallback" className="text-xs">顺序回退（fallback）</SelectItem>
                <SelectItem value="on_all" className="text-xs">同时请求（on_all）</SelectItem>
                <SelectItem value="load_balance" className="text-xs">负载均衡（load_balance）</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="flex items-center gap-2">
            <Switch
              id="isEnabled"
              checked={form.watch('isEnabled')}
              onCheckedChange={(v) => form.setValue('isEnabled', v)}
            />
            <Label className="text-xs" htmlFor="isEnabled">启用该虚拟供应商</Label>
          </div>

          <Separator />

          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-8 text-xs"
              onClick={() => onOpenChange(false)}
            >
              {t('common.cancel')}
            </Button>
            <Button type="submit" size="sm" className="h-8 text-xs">
              <i className="fa-solid fa-check mr-1.5" data-icon="inline-start" />
              {t('common.save')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
