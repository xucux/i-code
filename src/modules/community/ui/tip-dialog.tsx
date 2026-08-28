/**
 * 帖子打赏弹窗（2026-08-28 打赏迭代）
 *
 * 打赏规则（Worker 校验）：
 * - 单次 1 ~ 66 积分；不能打赏自己的帖子；
 * - 每人每帖仅可打赏一次，不可撤销（重复打赏 Worker 返回 409）。
 *
 * 交互：
 * - 快捷金额（1 / 6 / 18 / 66）+ 自定义数字输入；
 * - 打开时经 /users/me（命中 6h 资料缓存）展示当前积分，不足所选金额时禁用确认；
 * - 打赏成功回调 `onTipped`（返回打赏后最新人数 / 总额），由调用方刷新页面状态。
 */

import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import { getCommunityProfile, tipCommunityPost } from '@/hooks/use-community'
import type { PostTipData } from '@/modules/community/types'

/** 单次打赏积分区间（与 Worker TIPS_MAX / Rust / 文档保持一致） */
const TIP_MAX = 66
const TIP_MIN = 1

/** 单日打赏次数上限（与 Worker TIPS_MAX_PER_DAY 保持一致） */
const TIP_DAILY_MAX = 3

/** 快捷金额档位 */
const TIP_PRESETS = [1, 6, 18, 66] as const

export interface TipDialogProps {
  postId: number
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 打赏成功回调（参数为打赏后最新人数 / 总额，供调用方更新页面状态） */
  onTipped: (data: PostTipData) => void
}

export function TipDialog({ postId, open, onOpenChange, onTipped }: TipDialogProps) {
  const { t } = useTranslation('community')

  // 当前用户积分（null = 尚未加载 / 加载失败）
  const [points, setPoints] = useState<number | null>(null)
  // 选中档位（null = 自定义输入）；点击档位时写入 amountText
  const [preset, setPreset] = useState<number | null>(null)
  // 金额输入（字符串便于限制非法字符）
  const [amountText, setAmountText] = useState(String(TIP_PRESETS[0]))
  const [tipping, setTipping] = useState(false)

  // 打开时：重置表单 + 拉取当前积分
  useEffect(() => {
    if (!open) return
    setTipping(false)
    setPreset(TIP_PRESETS[0])
    setAmountText(String(TIP_PRESETS[0]))
    getCommunityProfile()
      .then((profile) => setPoints(profile.stats.points))
      .catch(() => setPoints(null))
  }, [open])

  /** 当前金额是否合法（1~66 整数） */
  const amountValid = (() => {
    const v = Number(amountText)
    return Number.isInteger(v) && v >= TIP_MIN && v <= TIP_MAX
  })()

  const amount = amountValid ? Number(amountText) : 0
  /** 积分不足提示（points 加载完成且不足时展示；输入非法时按 0 处理避免误报） */
  const insufficient = points != null && points < amount

  /** 点击快捷档位：选中并写入金额 */
  const pickPreset = (value: number) => {
    setPreset(value)
    setAmountText(String(value))
  }

  /** 确认打赏（金额合法 + 积分充足 + 非提交中） */
  const handleTip = async () => {
    if (!amountValid || tipping) return
    if (points != null && points < amount) return
    setTipping(true)
    try {
      const data = await tipCommunityPost(postId, amount)
      onTipped(data)
      toast.success(t('success.tipped', { amount }))
      onOpenChange(false)
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setTipping(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="text-sm">
            <i className="fa-solid fa-gift text-primary mr-1.5 size-3" />
            {t('tip.title')}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {t('tip.desc', { min: TIP_MIN, max: TIP_MAX, daily: TIP_DAILY_MAX })}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-1">
          {/* 快捷金额档位 */}
          <div className="grid grid-cols-4 gap-1.5">
            {TIP_PRESETS.map((value) => (
              <button
                key={value}
                type="button"
                onClick={() => pickPreset(value)}
                className={cn(
                  'rounded-md border py-1.5 text-sm font-medium transition-colors tabular-nums',
                  preset === value
                    ? 'border-primary bg-primary/10 text-primary'
                    : 'hover:bg-muted text-foreground'
                )}
              >
                {value}
              </button>
            ))}
          </div>

          {/* 自定义金额输入 */}
          <div className="space-y-1">
            <div className="flex items-center justify-between">
              <span className="text-muted-foreground text-xs">{t('tip.amountLabel')}</span>
              <span className="text-muted-foreground text-[10px] tabular-nums">
                {Number(amountText) || 0}/{TIP_MAX}
              </span>
            </div>
            <Input
              type="number"
              min={TIP_MIN}
              max={TIP_MAX}
              value={amountText}
              placeholder={t('tip.customPlaceholder', { min: TIP_MIN, max: TIP_MAX })}
              onChange={(e) => {
                // 去除非法字符；输入变化后取消档位高亮（视为自定义）
                const next = e.target.value.replace(/[^\d]/g, '')
                setAmountText(next)
                setPreset(null)
              }}
              className="h-8 text-xs"
            />
          </div>

          {/* 当前积分 */}
          <div className="flex items-center justify-between rounded-md border bg-muted/40 px-3 py-2 text-xs">
            <span className="text-muted-foreground">{t('tip.currentPoints')}</span>
            <span className="font-medium tabular-nums">
              {points == null ? (
                <i className="fa-solid fa-spinner fa-spin text-muted-foreground size-3" />
              ) : (
                <span className={cn(insufficient && 'text-destructive')}>{points}</span>
              )}
            </span>
          </div>
          {insufficient && (
            <p className="text-destructive flex items-center gap-1 text-[11px]">
              <i className="fa-solid fa-triangle-exclamation size-2.5" />
              {t('tip.insufficientPoints', { amount })}
            </p>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" size="sm" className="h-8 text-xs" onClick={() => onOpenChange(false)}>
            {t('post.cancel')}
          </Button>
          <Button
            size="sm"
            className="h-8 text-xs"
            disabled={!amountValid || tipping || insufficient}
            onClick={() => void handleTip()}
          >
            {tipping && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
            <i className="fa-solid fa-gift mr-1.5 size-2.5" />
            {t('tip.confirm', { amount })}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}