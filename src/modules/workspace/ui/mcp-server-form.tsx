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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { WorkspaceMcpServer, McpTransport } from '@/modules/workspace/types'

const schema = z.object({
  name: z.string().min(1, '名称不能为空'),
  transport: z.enum(['stdio', 'sse', 'http']),
  configJson: z.string().min(1, '配置 JSON 不能为空'),
  isEnabled: z.boolean(),
})

type FormValues = z.infer<typeof schema>

interface McpServerFormProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  server?: WorkspaceMcpServer | null
  onSubmit: (values: FormValues) => void
}

/**
 * MCP Server 新增/编辑表单
 *
 * `configJson` 需要是合法 JSON，由后端在写入配置文件时透传给 CLI。
 * v0.1 只做文本级 JSON 输入与基础结构校验，后续可扩展为结构化表单。
 */
export function McpServerForm({ open, onOpenChange, server, onSubmit }: McpServerFormProps) {
  const { t } = useTranslation()
  const isEdit = Boolean(server)

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      name: '',
      transport: 'stdio',
      configJson: '',
      isEnabled: true,
    },
  })

  useEffect(() => {
    if (server) {
      form.reset({
        name: server.name,
        transport: server.transport as McpTransport,
        configJson: server.configJson,
        isEnabled: server.isEnabled,
      })
    } else {
      form.reset({
        name: '',
        transport: 'stdio',
        configJson: '',
        isEnabled: true,
      })
    }
  }, [server, form])

  const transport = form.watch('transport')

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="text-base">{isEdit ? '编辑 MCP Server' : '新增 MCP Server'}</DialogTitle>
          <DialogDescription className="text-xs">配置 MCP 服务器连接参数</DialogDescription>
        </DialogHeader>

        <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
          <div className="space-y-1.5">
            <Label className="text-xs">名称</Label>
            <Input {...form.register('name')} className="h-8 text-xs" />
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs">传输方式</Label>
            <Select value={transport} onValueChange={(v) => form.setValue('transport', v as McpTransport)}>
              <SelectTrigger className="h-8 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="stdio" className="text-xs">stdio</SelectItem>
                <SelectItem value="sse" className="text-xs">sse</SelectItem>
                <SelectItem value="http" className="text-xs">http</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs">配置 JSON</Label>
            <Textarea
              {...form.register('configJson')}
              className="min-h-[120px] font-mono text-xs"
              placeholder='{"command":"npx","args":["-y","@modelcontextprotocol/server-filesystem"],"env":{}}'
            />
          </div>

          <div className="flex items-center gap-2">
            <Switch checked={form.watch('isEnabled')} onCheckedChange={(v) => form.setValue('isEnabled', v)} />
            <Label className="text-xs">启用该 MCP Server</Label>
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
