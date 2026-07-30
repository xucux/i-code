import { useCallback } from 'react'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'

interface PortInUseDialogProps {
  open: boolean
  /** 被占用的端口号 */
  port: number | null
  onOpenChange: (open: boolean) => void
}

/**
 * 检测当前操作系统平台
 */
function detectPlatform(): 'windows' | 'macos' | 'linux' {
  if (typeof navigator === 'undefined') return 'linux'
  const platform = navigator.platform.toLowerCase()
  if (platform.startsWith('win')) return 'windows'
  if (platform.startsWith('mac')) return 'macos'
  return 'linux'
}

/**
 * 端口占用提示弹窗
 *
 * 当 OAuth 回调服务器的固定端口被占用时，展示分平台的进程清理指引。
 */
export function PortInUseDialog({ open, port, onOpenChange }: PortInUseDialogProps) {
  const { t } = useTranslation('aiGateway')
  const platform = detectPlatform()

  const copyToClipboard = useCallback(async (text: string) => {
    try {
      await navigator.clipboard.writeText(text)
      toast.success(t('portInUse.copied'))
    } catch {
      // 忽略复制失败
    }
  }, [t])

  const portStr = String(port ?? '')

  const sections: Array<{
    titleKey: string
    step1Key: string
    cmd: string
    step2Key: string
    killCmd: string
  }> = [
    {
      titleKey: 'portInUse.windowsTitle',
      step1Key: 'portInUse.windowsStep1',
      cmd: `netstat -ano | findstr :${portStr}`,
      step2Key: 'portInUse.windowsStep2',
      killCmd: 'taskkill /PID <PID> /F',
    },
    {
      titleKey: 'portInUse.macosTitle',
      step1Key: 'portInUse.macosStep1',
      cmd: `lsof -i :${portStr}`,
      step2Key: 'portInUse.macosStep2',
      killCmd: 'kill -9 <PID>',
    },
    {
      titleKey: 'portInUse.linuxTitle',
      step1Key: 'portInUse.linuxStep1',
      cmd: `sudo lsof -i :${portStr}`,
      step2Key: 'portInUse.linuxStep2',
      killCmd: 'sudo kill -9 <PID>',
    },
  ]

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 text-base">
            <i className="fa-solid fa-triangle-exclamation text-amber-500" />
            {t('portInUse.title')}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {t('portInUse.description', { port: portStr })}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3">
          {sections.map((section, idx) => {
            const platformKey = ['windows', 'macos', 'linux'][idx]
            const isCurrent = platform === platformKey
            return (
              <div
                key={platformKey}
                className={`rounded-md border p-2.5 ${isCurrent ? 'border-primary/40 bg-primary/5' : ''}`}
              >
                <div className="mb-1.5 flex items-center gap-2">
                  <span className="text-xs font-medium">{t(section.titleKey)}</span>
                  {isCurrent && (
                    <span className="rounded bg-primary/10 px-1.5 py-0.5 text-[10px] text-primary">
                      {t('callbackServer.statusListening')}
                    </span>
                  )}
                </div>
                <div className="space-y-1.5">
                  <div>
                    <p className="text-muted-foreground text-[11px]">{t(section.step1Key)}</p>
                    <div className="mt-0.5 flex items-center gap-1">
                      <code className="flex-1 truncate rounded bg-muted px-1.5 py-0.5 text-[11px] font-mono">
                        {section.cmd}
                      </code>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="size-6 shrink-0"
                        onClick={() => void copyToClipboard(section.cmd)}
                        title={t('portInUse.copyCmd')}
                      >
                        <i className="fa-solid fa-copy text-[10px]" />
                      </Button>
                    </div>
                  </div>
                  <div>
                    <p className="text-muted-foreground text-[11px]">{t(section.step2Key)}</p>
                    <div className="mt-0.5 flex items-center gap-1">
                      <code className="flex-1 truncate rounded bg-muted px-1.5 py-0.5 text-[11px] font-mono">
                        {section.killCmd}
                      </code>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="size-6 shrink-0"
                        onClick={() => void copyToClipboard(section.killCmd)}
                        title={t('portInUse.copyCmd')}
                      >
                        <i className="fa-solid fa-copy text-[10px]" />
                      </Button>
                    </div>
                  </div>
                </div>
              </div>
            )
          })}
        </div>

        <div className="flex justify-end">
          <Button size="sm" className="h-8 text-xs" onClick={() => onOpenChange(false)}>
            {t('portInUse.close')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
