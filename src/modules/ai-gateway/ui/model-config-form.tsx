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
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import type { ModelConfig } from '@/modules/ai-gateway/types'

/** 分词器选项列表（使用 tokenizer 命名空间） */
function getTokenizerOptions(t: (key: string, options?: Record<string, unknown> | string) => string) {
  return [
    { value: 'default', label: t('default', { ns: 'tokenizer' }) },
    { value: 'char4', label: t('char4', { ns: 'tokenizer' }) },
    { value: 'conservative', label: t('conservative', { ns: 'tokenizer' }) },
    { value: 'openai', label: t('openai', { ns: 'tokenizer' }) },
    { value: 'deepseek', label: t('deepseek', { ns: 'tokenizer' }) },
  ]
}

const schema = z.object({
  name: z.string().min(1, 'aiGateway.modelConfigForm.validation.nameRequired'),
  family: z.string().optional(),
  maxInputTokens: z.coerce.number().int().optional(),
  maxOutputTokens: z.coerce.number().int().optional(),
  tokenizer: z.string().optional(),
  tokenCountMultiplier: z.coerce.number().positive().optional(),
  stream: z.boolean(),
})

type FormValues = z.infer<typeof schema>

interface ModelConfigFormProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  config?: ModelConfig | null
  onSubmit: (values: FormValues) => void
}

/**
 * 模型配置新增/编辑表单
 */
export function ModelConfigForm({ open, onOpenChange, config, onSubmit }: ModelConfigFormProps) {
  const { t } = useTranslation()
  const isEdit = Boolean(config)

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      name: '',
      family: '',
      maxInputTokens: undefined,
      maxOutputTokens: undefined,
      tokenizer: 'default',
      tokenCountMultiplier: 1.0,
      stream: true,
    },
  })

  useEffect(() => {
    if (config) {
      form.reset({
        name: config.name,
        family: config.family ?? '',
        maxInputTokens: config.maxInputTokens,
        maxOutputTokens: config.maxOutputTokens,
        tokenizer: config.tokenizer ?? 'default',
        tokenCountMultiplier: config.tokenCountMultiplier ?? 1.0,
        stream: config.stream ?? true,
      })
    } else {
      form.reset({
        name: '',
        family: '',
        maxInputTokens: undefined,
        maxOutputTokens: undefined,
        tokenizer: 'default',
        tokenCountMultiplier: 1.0,
        stream: true,
      })
    }
  }, [config, form])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="text-base">{isEdit ? t('aiGateway.modelConfigForm.editTitle') : t('aiGateway.modelConfigForm.createTitle')}</DialogTitle>
          <DialogDescription className="text-xs">{t('aiGateway.modelConfigForm.description')}</DialogDescription>
        </DialogHeader>

        <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
          <div className="space-y-1.5">
            <Label className="text-xs">{t('aiGateway.modelConfigForm.name')}</Label>
            <Input {...form.register('name')} className="h-8 text-xs" />
            {form.formState.errors.name && <p className="text-destructive text-[10px]">{t(form.formState.errors.name.message || '')}</p>}
          </div>
          <div className="space-y-1.5">
            <Label className="text-xs">{t('aiGateway.modelConfigForm.family')}</Label>
            <Input {...form.register('family')} className="h-8 text-xs" placeholder={t('aiGateway.modelConfigForm.familyPlaceholder')} />
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <Label className="text-xs">{t('aiGateway.modelConfigForm.maxInputTokens')}</Label>
              <Input type="number" {...form.register('maxInputTokens')} className="h-8 text-xs" />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs">{t('aiGateway.modelConfigForm.maxOutputTokens')}</Label>
              <Input type="number" {...form.register('maxOutputTokens')} className="h-8 text-xs" />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <Label className="text-xs">{t('aiGateway.modelConfigForm.tokenizer')}</Label>
              <Select
                value={form.watch('tokenizer') ?? 'default'}
                onValueChange={(v) => form.setValue('tokenizer', v)}
              >
                <SelectTrigger className="h-8 text-xs">
                  <SelectValue placeholder={t('aiGateway.modelConfigForm.tokenizerPlaceholder')} />
                </SelectTrigger>
                <SelectContent>
                  {getTokenizerOptions(t).map((opt) => (
                    <SelectItem key={opt.value} value={opt.value} className="text-xs">
                      {opt.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs">{t('aiGateway.modelConfigForm.tokenCountMultiplier')}</Label>
              <Input
                type="number"
                step="0.1"
                {...form.register('tokenCountMultiplier')}
                className="h-8 text-xs"
                placeholder={t('aiGateway.modelConfigForm.tokenCountMultiplierPlaceholder')}
              />
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Switch checked={form.watch('stream')} onCheckedChange={(v) => form.setValue('stream', v)} />
            <Label className="text-xs">{t('aiGateway.modelConfigForm.stream')}</Label>
          </div>

          <DialogFooter>
            <Button type="submit" size="sm" className="h-8 text-xs">
              <i className="fa-solid fa-check mr-1.5" />
              {isEdit ? t('aiGateway.modelConfigForm.confirmUpdate') : t('aiGateway.modelConfigForm.confirmCreate')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
