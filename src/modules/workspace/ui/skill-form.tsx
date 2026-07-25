import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
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
import type { WorkspaceSkill } from '@/modules/workspace/types'

const schema = z
  .object({
    name: z.string().min(1, '名称不能为空'),
    sourcePath: z.string().optional(),
  content: z.string().optional(),
  isEnabled: z.boolean(),
})
  .superRefine((data, ctx) => {
    if (!data.sourcePath?.trim() && !data.content?.trim()) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        message: '文件路径与内联内容至少填写一项',
        path: ['content'],
      })
    }
  })

type FormValues = z.infer<typeof schema>

interface SkillFormProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  skill?: WorkspaceSkill | null
  onSubmit: (values: FormValues) => void
}

/**
 * Skill 新增/编辑表单
 *
 * Skill 来源有两种：
 * 1. `sourcePath`：指向本地 skill 文件路径；
 * 2. `content`：直接编辑内联内容。
 * 两者至少填一项，应用到 CLI 时由后端决定优先级（当前以内联内容优先）。
 */
export function SkillForm({ open, onOpenChange, skill, onSubmit }: SkillFormProps) {
  const { t } = useTranslation()
  const isEdit = Boolean(skill)

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      name: '',
      sourcePath: '',
      content: '',
      isEnabled: true,
    },
  })

  useEffect(() => {
    if (skill) {
      form.reset({
        name: skill.name,
        sourcePath: skill.sourcePath ?? '',
        content: skill.content ?? '',
        isEnabled: skill.isEnabled,
      })
    } else {
      form.reset({
        name: '',
        sourcePath: '',
        content: '',
        isEnabled: true,
      })
    }
  }, [skill, form])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="text-base">{isEdit ? '编辑 Skill' : '新增 Skill'}</DialogTitle>
          <DialogDescription className="text-xs">配置 Skill 文件路径或内联内容</DialogDescription>
        </DialogHeader>

        <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
          <div className="space-y-1.5">
            <Label className="text-xs">名称</Label>
            <Input {...form.register('name')} className="h-8 text-xs" />
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs">本地文件路径</Label>
            <Input {...form.register('sourcePath')} className="h-8 text-xs" placeholder="/path/to/skill.md" />
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs">内联内容</Label>
            <Textarea {...form.register('content')} className="min-h-[120px] text-xs" placeholder="输入 Skill 内容..." />
          </div>

          <div className="flex items-center gap-2">
            <Switch checked={form.watch('isEnabled')} onCheckedChange={(v) => form.setValue('isEnabled', v)} />
            <Label className="text-xs">启用该 Skill</Label>
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
