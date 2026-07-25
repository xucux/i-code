import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
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
import type { CliModelMapping } from '@/modules/cli-management/types'

const schema = z.object({
  cliModelAlias: z.string().min(1, 'CLI 模型别名不能为空'),
  gatewayModelId: z.string().optional(),
  rawModelId: z.string().optional(),
  inputMode: z.enum(['select', 'manual']),
})

type FormValues = z.infer<typeof schema>

interface ModelMappingFormProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  providerId: string
  mapping?: CliModelMapping | null
  routeMode?: number
  availableModels?: Array<{ value: string; label: string }>
  onSubmit: (values: FormValues) => void
}

/**
 * CLI 模型映射新增/编辑表单
 */
export function ModelMappingForm({
  open,
  onOpenChange,
  mapping,
  routeMode = 1,
  availableModels = [],
  onSubmit,
}: ModelMappingFormProps) {
  const { t } = useTranslation()
  const isEdit = Boolean(mapping)

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      cliModelAlias: '',
      gatewayModelId: '',
      rawModelId: '',
      inputMode: 'select',
    },
  })

  useEffect(() => {
    if (mapping) {
      form.reset({
        cliModelAlias: mapping.cliModelAlias,
        gatewayModelId: mapping.gatewayModelId ?? '',
        rawModelId: mapping.rawModelId ?? '',
        inputMode: routeMode === 1 ? 'select' : 'manual',
      })
    } else {
      form.reset({
        cliModelAlias: '',
        gatewayModelId: '',
        rawModelId: '',
        inputMode: routeMode === 1 ? 'select' : 'manual',
      })
    }
  }, [mapping, routeMode, form])

  const inputMode = form.watch('inputMode')

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="text-base">
            {isEdit ? t('cli.mappingForm.editTitle') : t('cli.mappingForm.createTitle')}
          </DialogTitle>
          <DialogDescription className="text-xs">{t('cli.mappingForm.description')}</DialogDescription>
        </DialogHeader>

        <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
          <div className="space-y-1.5">
            <Label className="text-xs">{t('cli.mappingForm.alias')}</Label>
            <Input
              {...form.register('cliModelAlias')}
              className="h-8 text-xs"
              placeholder={t('cli.mappingForm.aliasPlaceholder')}
            />
          </div>

          {inputMode === 'select' ? (
            <div className="space-y-1.5">
              <Label className="text-xs">{t('cli.mappingForm.gatewayModel')}</Label>
              <Select
                value={form.watch('gatewayModelId')}
                onValueChange={(value) => form.setValue('gatewayModelId', value)}
                disabled={availableModels.length === 0}
              >
                <SelectTrigger className="h-8 text-xs">
                  <SelectValue placeholder={t('cli.mappingForm.selectGatewayModel')} />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {availableModels.map((model) => (
                      <SelectItem key={model.value} value={model.value} className="text-xs">
                        {model.label}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
              {availableModels.length === 0 && (
                <p className="text-muted-foreground text-xs">{t('cli.mappingForm.noGatewayModels')}</p>
              )}
            </div>
          ) : (
            <div className="space-y-1.5">
              <Label className="text-xs">{t('cli.mappingForm.rawModel')}</Label>
              <Input
                {...form.register('rawModelId')}
                className="h-8 text-xs"
                placeholder={t('cli.mappingForm.rawModelPlaceholder')}
              />
            </div>
          )}

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
