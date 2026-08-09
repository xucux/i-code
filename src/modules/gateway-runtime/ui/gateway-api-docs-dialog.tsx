import { useEffect, useMemo, useState } from 'react'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'
import { useTranslation } from '@/modules/i18n/use-translation'
import { setPreferredGatewayHost } from '@/hooks/use-local-ips'

/** 网关暴露的单个 HTTP 接口描述 */
interface GatewayEndpoint {
  /** HTTP 方法（GET / POST 等） */
  method: 'GET' | 'POST'
  /** 路径，如 `/v1/models` */
  path: string
  /** 接口名称 i18n key（相对 aiGateway 命名空间） */
  nameKey: string
  /** 接口描述 i18n key */
  descKey: string
  /** 是否需要认证（用于 UI 提示） */
  requireAuth: boolean
}

/** 网关当前支持的所有接口（与后端 router.rs 路由清单保持一致） */
const ENDPOINTS: GatewayEndpoint[] = [
  {
    method: 'GET',
    path: '/health',
    nameKey: 'gatewayApiDocs.endpoints.health.name',
    descKey: 'gatewayApiDocs.endpoints.health.desc',
    requireAuth: false,
  },
  {
    method: 'GET',
    path: '/readyz',
    nameKey: 'gatewayApiDocs.endpoints.readyz.name',
    descKey: 'gatewayApiDocs.endpoints.readyz.desc',
    requireAuth: false,
  },
  {
    method: 'GET',
    path: '/v1/models',
    nameKey: 'gatewayApiDocs.endpoints.models.name',
    descKey: 'gatewayApiDocs.endpoints.models.desc',
    requireAuth: true,
  },
  {
    method: 'POST',
    path: '/v1/chat/completions',
    nameKey: 'gatewayApiDocs.endpoints.chatCompletions.name',
    descKey: 'gatewayApiDocs.endpoints.chatCompletions.desc',
    requireAuth: true,
  },
  {
    method: 'POST',
    path: '/v1/responses',
    nameKey: 'gatewayApiDocs.endpoints.responses.name',
    descKey: 'gatewayApiDocs.endpoints.responses.desc',
    requireAuth: true,
  },
  {
    method: 'POST',
    path: '/v1/messages',
    nameKey: 'gatewayApiDocs.endpoints.messages.name',
    descKey: 'gatewayApiDocs.endpoints.messages.desc',
    requireAuth: true,
  },
]

interface GatewayApiDocsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 所有可访问网关的地址列表（已按优先级排序） */
  hosts: string[]
  /** 默认展示地址（一般为列表中第一个） */
  defaultHost: string
  /** 网关监听端口 */
  port?: number
}

/**
 * 网关接口文档弹窗
 *
 * 列出网关对外暴露的所有 HTTP 接口、方法、路径与说明，
 * 并基于当前选中的本机地址拼装完整 URL 供复制。
 *
 * 当网关监听 `0.0.0.0` 时，`hosts` 包含多个 LAN 地址，
 * 用户可在顶部下拉中切换以查看不同地址对应的访问 URL。
 */
export function GatewayApiDocsDialog({
  open,
  onOpenChange,
  hosts,
  defaultHost,
  port,
}: GatewayApiDocsDialogProps) {
  const { t } = useTranslation('aiGateway')
  const { t: tc } = useTranslation()

  const [selectedHost, setSelectedHost] = useState<string>(defaultHost)

  // 弹窗打开或默认地址变化时，重置选中地址
  // defaultHost 已优先「接口文档中最后选中的地址」，因此重开后仍保持用户上次的选择
  useEffect(() => {
    if (open) {
      setSelectedHost(defaultHost)
    }
  }, [open, defaultHost])

  const baseUrl = useMemo(() => {
    if (!selectedHost || !port) return ''
    return `http://${selectedHost}:${port}`
  }, [selectedHost, port])

  const copyText = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text)
      toast.success(tc('common.copied'))
    } catch {
      toast.error(tc('common.copyFailed'))
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 text-base">
            <i className="fa-solid fa-book-open text-primary" />
            {t('gatewayApiDocs.title')}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {t('gatewayApiDocs.description')}
          </DialogDescription>
        </DialogHeader>

        {/* 地址选择：多地址时下拉切换，单地址时直接展示 */}
        <div className="flex items-center gap-2">
          <span className="text-muted-foreground text-xs">{t('gatewayApiDocs.currentHost')}</span>
          {hosts.length > 1 ? (
            <Select
              value={selectedHost}
              onValueChange={(h) => {
                setSelectedHost(h)
                // 记录「接口文档中最后选中的地址」，全局展示地址优先跟随
                setPreferredGatewayHost(h)
              }}
            >
              <SelectTrigger className="h-7 w-auto min-w-48 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {hosts.map((h) => (
                  <SelectItem key={h} value={h} className="text-xs">
                    <span className="font-mono">{h}</span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          ) : (
            <span className="font-mono text-xs">{selectedHost}</span>
          )}
          {port != null && (
            <span className="text-muted-foreground text-xs">
              <span className="mx-1">:</span>
              <span className="font-mono">{port}</span>
            </span>
          )}
          {baseUrl && (
            <Button
              variant="ghost"
              size="icon"
              className="size-6"
              title={tc('common.copy')}
              onClick={() => void copyText(`${baseUrl}/v1`)}
            >
              <i className="fa-solid fa-copy text-xs" />
            </Button>
          )}
        </div>

        {/* 接口列表 */}
        <div className="max-h-72 space-y-2 overflow-y-auto pr-1">
          {ENDPOINTS.map((ep) => (
            <div
              key={`${ep.method}-${ep.path}`}
              className="rounded-md border p-2.5"
            >
              <div className="flex items-center gap-2">
                <Badge
                  variant={ep.method === 'GET' ? 'secondary' : 'default'}
                  className={cn(
                    'shrink-0 px-1.5 py-0 text-[10px] font-semibold',
                    ep.method === 'GET'
                      ? 'bg-emerald-500/15 text-emerald-600 dark:text-emerald-400'
                      : 'bg-blue-500/15 text-blue-600 dark:text-blue-400'
                  )}
                >
                  {ep.method}
                </Badge>
                <code className="flex-1 truncate font-mono text-xs">{ep.path}</code>
                {ep.requireAuth && (
                  <span className="shrink-0 rounded bg-amber-500/15 px-1.5 py-0.5 text-[10px] text-amber-600 dark:text-amber-400">
                    {t('gatewayApiDocs.requireAuth')}
                  </span>
                )}
              </div>
              <div className="mt-1.5 space-y-0.5">
                <p className="text-xs font-medium">{t(ep.nameKey)}</p>
                <p className="text-muted-foreground text-[11px]">{t(ep.descKey)}</p>
              </div>
              {baseUrl && (
                <div className="mt-1.5 flex items-center gap-1">
                  <code className="flex-1 truncate rounded bg-muted px-1.5 py-0.5 text-[11px] font-mono">
                    {baseUrl}{ep.path}
                  </code>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="size-6 shrink-0"
                    title={tc('common.copy')}
                    onClick={() => void copyText(`${baseUrl}${ep.path}`)}
                  >
                    <i className="fa-solid fa-copy text-[10px]" />
                  </Button>
                </div>
              )}
            </div>
          ))}
        </div>

        <div className="flex justify-end">
          <Button size="sm" className="h-8 text-xs" onClick={() => onOpenChange(false)}>
            {tc('common.close')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
