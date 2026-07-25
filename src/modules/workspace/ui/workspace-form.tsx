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
import type { Workspace } from '@/modules/workspace/types'

const schema = z.object({
  slug: z.string().min(1, 'slug 不能为空').regex(/^[a-z0-9-]+$/, 'slug 只能包含小写字母、数字和横线'),
  displayName: z.string().min(1, '显示名称不能为空'),
  rootPath: z.string().min(1, '工作区目录不能为空'),
  isActive: z.boolean(),
})

type FormValues = z.infer<typeof schema>

interface WorkspaceFormProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  workspace?: Workspace | null
  onSubmit: (values: FormValues) => void
}

/**
 * 工作区新增/编辑表单
 *
 * v0.1 仅通过文本输入目录路径；后续可接入 Tauri 的目录选择对话框以提升体验。
 */
export function WorkspaceForm({ open, onOpenChange, workspace, onSubmit }: WorkspaceFormProps) {
  const { t } = useTranslation()
  const isEdit = Boolean(workspace)

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      slug: '',
      displayName: '',
      rootPath: '',
      isActive: false,
    },
  })

  useEffect(() => {
    if (workspace) {
      form.reset({
        slug: workspace.slug,
        displayName: workspace.displayName,
        rootPath: workspace.rootPath,
        isActive: workspace.isActive,
      })
    } else {
      form.reset({
        slug: '',
        displayName: '',
        rootPath: '',
        isActive: false,
      })
    }
  }, [workspace, form])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="text-base">{isEdit ? '编辑工作区' : '新建工作区'}</DialogTitle>
          <DialogDescription className="text-xs">按项目或目录隔离 CLI 配置</DialogDescription>
        </DialogHeader>

        <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <Label className="text-xs">slug</Label>
              <Input {...form.register('slug')} disabled={isEdit} className="h-8 text-xs" placeholder="my-project" />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs">显示名称</Label>
              <Input {...form.register('displayName')} className="h-8 text-xs" />
            </div>
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs">工作区目录</Label>
            <Input {...form.register('rootPath')} className="h-8 text-xs" placeholder="/path/to/project" />
          </div>

          <div className="flex items-center gap-2">
            <Switch checked={form.watch('isActive')} onCheckedChange={(v) => form.setValue('isActive', v)} />
            <Label className="text-xs">创建后立即激活</Label>
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
