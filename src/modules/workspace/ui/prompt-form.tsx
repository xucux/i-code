import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import type { WorkspacePrompt } from '@/modules/workspace/types'

const schema = z.object({
  name: z.string().min(1, '名称不能为空'),
  content: z.string().min(1, '内容不能为空'),
  sortOrder: z.coerce.number().int(),
})

type FormValues = z.infer<typeof schema>

interface PromptFormProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  prompt?: WorkspacePrompt | null
  onSubmit: (values: FormValues) => void
}

/**
 * Prompt 新增/编辑表单
 */
export function PromptForm({ open, onOpenChange, prompt, onSubmit }: PromptFormProps) {
  const { t } = useTranslation()
  const isEdit = Boolean(prompt)

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      name: '',
      content: '',
      sortOrder: 0,
    },
  })

  useEffect(() => {
    if (prompt) {
      form.reset({
        name: prompt.name,
        content: prompt.content,
        sortOrder: prompt.sortOrder,
      })
    } else {
      form.reset({
        name: '',
        content: '',
        sortOrder: 0,
      })
    }
  }, [prompt, form])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="text-base">{isEdit ? '编辑 Prompt' : '新增 Prompt'}</DialogTitle>
          <DialogDescription className="text-xs">配置注入到 CLI 的提示词内容</DialogDescription>
        </DialogHeader>

        <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
          <div className="grid grid-cols-[1fr_80px] gap-4">
            <div className="space-y-1.5">
              <Label className="text-xs">名称</Label>
              <Input {...form.register('name')} className="h-8 text-xs" />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs">排序</Label>
              <Input type="number" {...form.register('sortOrder')} className="h-8 text-xs" />
            </div>
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs">内容</Label>
            <Textarea {...form.register('content')} className="min-h-[120px] text-xs" placeholder="输入 Prompt 文本..." />
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
