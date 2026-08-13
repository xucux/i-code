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
import {
  createVirtualProvider,
  updateVirtualProvider,
  checkAliasImpact,
} from '@/hooks/use-virtual-provider-mutation'
import { toast } from 'sonner'
import { toIcodeError } from '@/core/errors'
import type { FailoverStrategy, VirtualProvider, AliasImpactResult } from '@/modules/virtual-provider/types'

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

interface VirtualProviderDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 传入 provider 表示编辑模式；不传表示新建模式 */
  provider?: VirtualProvider | null
  onSuccess?: () => void
}

/**
 * 虚拟供应商弹窗（新建/编辑共用）
 *
 * 仅维护供应商基础信息（名称、别名、展示名、策略、启用状态）。
 * 虚拟模型及其子级路由在独立的 VirtualModelDialog 中维护。
 */
export function VirtualProviderDialog({
  open,
  onOpenChange,
  provider,
  onSuccess,
}: VirtualProviderDialogProps) {
  const { t } = useTranslation('virtualProvider')
  const isEdit = Boolean(provider)
  const [submitting, setSubmitting] = useState(false)
  // alias 变更影响检查结果（仅编辑模式下有值）
  const [aliasImpact, setAliasImpact] = useState<AliasImpactResult | null>(null)
  const [aliasChecking, setAliasChecking] = useState(false)
  const aliasCheckTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      name: '',
      alias: '',
      displayName: '',
      strategy: 'fallback',
      isEnabled: true,
      maxRetries: 3,
      retryIntervalMs: 1000,
    },
  })

  // 打开弹窗或切换 provider 时重置表单
  useEffect(() => {
    if (!open) return
    setAliasImpact(null)
    if (provider) {
      form.reset({
        name: provider.name,
        alias: provider.alias,
        displayName: provider.displayName ?? '',
        strategy: provider.strategy,
        isEnabled: provider.isEnabled,
        maxRetries: provider.maxRetries ?? 3,
        retryIntervalMs: provider.retryIntervalMs ?? 1000,
      })
    } else {
      form.reset({
        name: '',
        alias: '',
        displayName: '',
        strategy: 'fallback',
        isEnabled: true,
        maxRetries: 3,
        retryIntervalMs: 1000,
      })
    }
  }, [open, provider, form])

  // 编辑模式下监听 alias 变化，防抖调用影响检查
  const watchedAlias = form.watch('alias')
  useEffect(() => {
    if (!isEdit || !provider || !open) return
    // alias 未变化时清除影响
    if (watchedAlias === provider.alias) {
      setAliasImpact(null)
      return
    }
    // alias 格式校验通过才检查
    if (!watchedAlias || !/^[a-z0-9-]+$/.test(watchedAlias)) {
      setAliasImpact(null)
      return
    }
    // 防抖 300ms
    if (aliasCheckTimer.current) clearTimeout(aliasCheckTimer.current)
    setAliasChecking(true)
    aliasCheckTimer.current = setTimeout(async () => {
      try {
        const result = await checkAliasImpact(provider.id, watchedAlias)
        setAliasImpact(result)
      } catch {
        setAliasImpact(null)
      } finally {
        setAliasChecking(false)
      }
    }, 300)
    return () => {
      if (aliasCheckTimer.current) clearTimeout(aliasCheckTimer.current)
    }
  }, [watchedAlias, isEdit, provider, open])

  const errors = form.formState.errors

  const handleSubmit = form.handleSubmit(async (values) => {
    setSubmitting(true)
    try {
      if (isEdit && provider) {
        await updateVirtualProvider(provider.id, {
          name: values.name,
          alias: values.alias,
          displayName: values.displayName,
          strategy: values.strategy,
          isEnabled: values.isEnabled,
          maxRetries: values.maxRetries,
          retryIntervalMs: values.retryIntervalMs,
        })
        toast.success(t('editProvider'))
      } else {
        await createVirtualProvider({
          name: values.name,
          alias: values.alias,
          displayName: values.displayName,
          strategy: values.strategy,
          isEnabled: values.isEnabled,
          maxRetries: values.maxRetries,
          retryIntervalMs: values.retryIntervalMs,
        })
        toast.success(t('newProvider'))
      }

      onOpenChange(false)
      form.reset()
      onSuccess?.()
    } catch (err) {
      toast.error(toIcodeError(err).message)
    } finally {
      setSubmitting(false)
    }
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="text-base">
            {isEdit ? t('editProvider') : t('newProvider')}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {isEdit ? t('editDescription') : t('createDescription')}
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5" data-invalid={errors.name ? '' : undefined}>
              <Label className="text-xs" htmlFor="vp-name">{t('name')}</Label>
              <Input
                id="vp-name"
                {...form.register('name')}
                className="h-8 text-xs"
                aria-invalid={errors.name ? true : undefined}
              />
              {errors.name && (
                <p className="text-destructive text-[10px]">{errors.name.message}</p>
              )}
            </div>

            <div className="space-y-1.5" data-invalid={errors.alias ? '' : undefined}>
              <Label className="text-xs" htmlFor="vp-alias">{t('alias')}</Label>
              <Input
                id="vp-alias"
                {...form.register('alias')}
                className="h-8 text-xs"
                placeholder={t('aliasPlaceholder')}
                aria-invalid={errors.alias ? true : undefined}
              />
              {errors.alias && (
                <p className="text-destructive text-[10px]">{errors.alias.message}</p>
              )}
              {/* alias 变更影响提示（仅编辑模式下 alias 有变化时展示） */}
              {aliasChecking && (
                <p className="text-muted-foreground text-[10px]">
                  <i className="fa-solid fa-circle-notch fa-spin mr-1" />
                  {t('aliasImpactChecking')}
                </p>
              )}
              {aliasImpact?.hasImpact && !aliasChecking && (
                <div className="flex items-start gap-1.5 rounded-md border border-amber-500/40 bg-amber-500/10 px-2 py-1.5 text-[10px] text-amber-700 dark:text-amber-400">
                  <i className="fa-solid fa-triangle-exclamation mt-px shrink-0" />
                  <span>
                    {t('aliasImpactWarning', { count: aliasImpact.affectedCliModelMappings })}
                  </span>
                </div>
              )}
            </div>
          </div>

          <div className="space-y-1.5" data-invalid={errors.displayName ? '' : undefined}>
            <Label className="text-xs" htmlFor="vp-displayName">{t('displayName')}</Label>
            <Input
              id="vp-displayName"
              {...form.register('displayName')}
              className="h-8 text-xs"
            />
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs">{t('strategy')}</Label>
            <Select
              value={form.watch('strategy')}
              onValueChange={(v) => form.setValue('strategy', v as FailoverStrategy)}
            >
              <SelectTrigger className="h-8 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="fallback" className="text-xs">{t('strategyFallback')}</SelectItem>
                <SelectItem value="on_all" className="text-xs">{t('strategyOnAll')}</SelectItem>
                <SelectItem value="load_balance" className="text-xs">{t('strategyLoadBalance')}</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="flex items-center gap-2">
            <Switch
              id="vp-isEnabled"
              checked={form.watch('isEnabled')}
              onCheckedChange={(v) => form.setValue('isEnabled', v)}
            />
            <Label className="text-xs" htmlFor="vp-isEnabled">{t('enabledVirtualProvider')}</Label>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5" data-invalid={errors.maxRetries ? '' : undefined}>
              <Label className="text-xs" htmlFor="vp-maxRetries">{t('maxRetries')}</Label>
              <Input
                id="vp-maxRetries"
                type="number"
                {...form.register('maxRetries', { valueAsNumber: true })}
                className="h-8 text-xs"
                min={0}
              />
              {errors.maxRetries && (
                <p className="text-destructive text-[10px]">{errors.maxRetries.message}</p>
              )}
            </div>

            <div className="space-y-1.5" data-invalid={errors.retryIntervalMs ? '' : undefined}>
              <Label className="text-xs" htmlFor="vp-retryIntervalMs">{t('retryInterval')}</Label>
              <Input
                id="vp-retryIntervalMs"
                type="number"
                {...form.register('retryIntervalMs', { valueAsNumber: true })}
                className="h-8 text-xs"
                min={0}
              />
              <span className="text-muted-foreground text-[10px]">ms</span>
              {errors.retryIntervalMs && (
                <p className="text-destructive text-[10px]">{errors.retryIntervalMs.message}</p>
              )}
            </div>
          </div>

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
      </DialogContent>
    </Dialog>
  )
}
