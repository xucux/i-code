import { useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { invokeCommand } from '@/hooks/use-command'
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
import { toast } from 'sonner'
import { toIcodeError } from '@/core/errors'
import type { VirtualHealthCheckConfig } from '@/modules/virtual-provider/types'

/** 分钟输入允许范围：0.5 ~ 1440 分钟 */
const MIN_MINUTES = 0.5
const MAX_MINUTES = 1440

/** 路由历史保留天数允许范围：1 ~ 365 天（与后端校验一致） */
const MIN_KEEP_DAYS = 1
const MAX_KEEP_DAYS = 365

/** 将保留天数收拢到允许范围内；非法值回落到最小值 */
function clampKeepDays(value: number): number {
  if (!Number.isFinite(value)) return MIN_KEEP_DAYS
  return Math.min(MAX_KEEP_DAYS, Math.max(MIN_KEEP_DAYS, Math.round(value)))
}

/** 秒 → 分钟文本（保留两位小数，去掉多余的 0） */
function toMinutesText(secs: number): string {
  return String(Math.round((secs / 60) * 100) / 100)
}

/** 将分钟值收拢到允许范围内；非法值回落到最小值 */
function clampMinutes(value: number): number {
  if (!Number.isFinite(value)) return MIN_MINUTES
  return Math.min(MAX_MINUTES, Math.max(MIN_MINUTES, value))
}

/**
 * 探活设置入口按钮
 *
 * 打开一个紧凑弹窗，编辑虚拟供应商运行时的全局配置：
 * - 探活间隔：调度器对已降级路由发起探活的间隔；
 * - 全部失活兜底间隔：所有路由都永久失活时，最多按该间隔尝试 1 条最久未尝试的路由；
 * - 路由历史保留天数：`virtual_route_attempts` 表按该天数清理历史记录。
 * 间隔 UI 以分钟编辑，提交时转换为秒；保留天数直接按天提交。
 */
export function HealthCheckSettingsButton() {
  const { t } = useTranslation('virtualProvider')
  const [open, setOpen] = useState(false)
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [healthCheckMinutes, setHealthCheckMinutes] = useState('60')
  const [fallbackMinutes, setFallbackMinutes] = useState('30')
  const [keepDays, setKeepDays] = useState('3')

  const fetchConfig = async () => {
    setLoading(true)
    try {
      const config = await invokeCommand<VirtualHealthCheckConfig>(
        'virtual_health_check_config_get',
        {}
      )
      setHealthCheckMinutes(toMinutesText(config.healthCheckIntervalSecs))
      setFallbackMinutes(toMinutesText(config.allFailedFallbackIntervalSecs))
      setKeepDays(String(config.attemptsKeepDays))
    } catch (err) {
      toast.error(toIcodeError(err).message)
    } finally {
      setLoading(false)
    }
  }

  const handleOpenChange = (next: boolean) => {
    setOpen(next)
    if (next) void fetchConfig()
  }

  const handleSave = async () => {
    const healthCheck = clampMinutes(Number(healthCheckMinutes))
    const fallback = clampMinutes(Number(fallbackMinutes))
    const days = clampKeepDays(Number(keepDays))
    // 回填收拢后的值，避免用户看到越界输入
    setHealthCheckMinutes(toMinutesText(healthCheck * 60))
    setFallbackMinutes(toMinutesText(fallback * 60))
    setKeepDays(String(days))

    setSaving(true)
    try {
      await invokeCommand('virtual_health_check_config_set', {
        input: {
          healthCheckIntervalSecs: Math.round(healthCheck * 60),
          allFailedFallbackIntervalSecs: Math.round(fallback * 60),
          attemptsKeepDays: days,
        },
      })
      toast.success(t('healthCheckSettingsSaved'))
      await fetchConfig()
      setOpen(false)
    } catch (err) {
      toast.error(toIcodeError(err).message)
    } finally {
      setSaving(false)
    }
  }

  const disabled = loading || saving

  return (
    <>
      <Button
        size="sm"
        variant="outline"
        className="h-8 text-xs"
        onClick={() => handleOpenChange(true)}
      >
        <i className="fa-solid fa-heart-pulse mr-1.5" />
        {t('healthCheckSettingsButton')}
      </Button>

      <Dialog open={open} onOpenChange={handleOpenChange}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle className="text-base">{t('healthCheckSettings')}</DialogTitle>
            <DialogDescription className="text-xs leading-relaxed">
              {t('healthCheckSettingsHint')}
            </DialogDescription>
          </DialogHeader>

          {loading ? (
            <div className="text-muted-foreground flex items-center gap-1.5 py-4 text-xs">
              <i className="fa-solid fa-circle-notch fa-spin" />
              {t('loading')}
            </div>
          ) : (
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label className="text-xs" htmlFor="health-check-interval">
                  {t('healthCheckInterval')}
                </Label>
                <div className="relative">
                  <Input
                    id="health-check-interval"
                    type="number"
                    step="0.5"
                    min={MIN_MINUTES}
                    max={MAX_MINUTES}
                    className="h-8 pr-9 text-xs tabular-nums"
                    value={healthCheckMinutes}
                    disabled={disabled}
                    onChange={(e) => setHealthCheckMinutes(e.target.value)}
                  />
                  <span className="text-muted-foreground pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 text-[10px]">
                    {t('intervalMinutes')}
                  </span>
                </div>
              </div>

              <div className="space-y-1.5">
                <Label className="text-xs" htmlFor="all-failed-fallback-interval">
                  {t('allFailedFallbackInterval')}
                </Label>
                <div className="relative">
                  <Input
                    id="all-failed-fallback-interval"
                    type="number"
                    step="0.5"
                    min={MIN_MINUTES}
                    max={MAX_MINUTES}
                    className="h-8 pr-9 text-xs tabular-nums"
                    value={fallbackMinutes}
                    disabled={disabled}
                    onChange={(e) => setFallbackMinutes(e.target.value)}
                  />
                  <span className="text-muted-foreground pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 text-[10px]">
                    {t('intervalMinutes')}
                  </span>
                </div>
              </div>

              <div className="space-y-1.5">
                <Label className="text-xs" htmlFor="attempts-keep-days">
                  {t('attemptsKeepDays')}
                </Label>
                <div className="relative">
                  <Input
                    id="attempts-keep-days"
                    type="number"
                    step="1"
                    min={MIN_KEEP_DAYS}
                    max={MAX_KEEP_DAYS}
                    className="h-8 pr-7 text-xs tabular-nums"
                    value={keepDays}
                    disabled={disabled}
                    onChange={(e) => setKeepDays(e.target.value)}
                  />
                  <span className="text-muted-foreground pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 text-[10px]">
                    {t('keepDaysUnit')}
                  </span>
                </div>
              </div>
            </div>
          )}

          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-8 text-xs"
              onClick={() => handleOpenChange(false)}
            >
              {t('cancel', { ns: 'common' })}
            </Button>
            <Button
              type="button"
              size="sm"
              className="h-8 text-xs"
              disabled={disabled}
              onClick={handleSave}
            >
              {saving ? (
                <i className="fa-solid fa-circle-notch fa-spin mr-1.5" />
              ) : (
                <i className="fa-solid fa-check mr-1.5" />
              )}
              {t('save', { ns: 'common' })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
