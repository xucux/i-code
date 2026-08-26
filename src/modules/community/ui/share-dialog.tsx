/**
 * 帖子分享弹窗（2026-08-26 分享迭代）
 *
 * 仅帖子作者可打开（入口见 post-detail.tsx）：
 * - 次数设置：访问配额上限，默认 1000，范围 1 ~ 10000；
 * - 成本提示：每次发起分享扣 100 积分（points_ledger reason='share_link'），撤销不返还；
 * - 当前积分：打开时经 /users/me（命中 6h 资料缓存）展示，不足 100 禁用发起；
 * - 生成后展示直链（只读 + 复制）并刷新本帖已有分享列表（仅展示 / 复制，撤销归管理员 §4.2）。
 */

import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Badge } from '@/components/ui/badge'
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
import { Label } from '@/components/ui/label'
import { cn } from '@/lib/utils'
import { formatCommunityTime } from '@/hooks/use-community'
import {
  createCommunityShareLink,
  getCommunityPostShareLinks,
  getCommunityProfile,
} from '@/hooks/use-community'
import type { ShareLink } from '@/modules/community/types'

/** 访问配额区间（与 Worker / Rust 保持一致） */
const MAX_VIEWS_MIN = 1
const MAX_VIEWS_MAX = 10000
const MAX_VIEWS_DEFAULT = 1000

/**
 * 分享档位收费（与 Worker `shareCostPoints` 保持一致，见 docs/proposals/community-post-share.md §5）
 * - ≤ 1000 次 100 积分；1000 < 次数 ≤ 4000 为 200；4000 < 次数 ≤ 10000 为 500
 */
const SHARE_TIERS: { threshold: number; cost: number }[] = [
  { threshold: 1000, cost: 100 },
  { threshold: 4000, cost: 200 },
  { threshold: MAX_VIEWS_MAX, cost: 500 },
]
function shareCostPoints(maxViews: number): number {
  const tier = SHARE_TIERS.find((t) => maxViews <= t.threshold)
  return tier?.cost ?? 0
}

/** 最近创建的分享（新直链框展示用；pid 为空 = 本次弹窗内尚未创建） */
interface ShareDialogProps {
  postId: number
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function ShareDialog({ postId, open, onOpenChange }: ShareDialogProps) {
  const { t } = useTranslation('community')

  // 当前用户积分（null = 尚未加载 / 加载失败）
  const [points, setPoints] = useState<number | null>(null)
  // 次数输入（字符串便于限制非法字符）
  const [maxViews, setMaxViews] = useState(String(MAX_VIEWS_DEFAULT))
  const [creating, setCreating] = useState(false)
  // 本次弹窗内刚生成的分享（展示直链 + 复制）
  const [created, setCreated] = useState<ShareLink | null>(null)
  // 本帖已有分享列表（仅作者本人可见）
  const [shares, setShares] = useState<ShareLink[]>([])
  const [listLoading, setListLoading] = useState(false)
  // 刚复制链接的 pid（用于对勾反馈，1.5s 还原）
  const [copiedPid, setCopiedPid] = useState<string | null>(null)

  /** 拉取本帖分享列表（成功即覆盖，失败按空列表处理并以 toast 提示） */
  const loadShares = useCallback(async () => {
    setListLoading(true)
    try {
      const data = await getCommunityPostShareLinks(postId)
      setShares(data.items)
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setListLoading(false)
    }
  }, [postId])

  // 打开时：重置表单 + 拉取当前积分与已有分享
  useEffect(() => {
    if (!open) return
    setCreated(null)
    setMaxViews(String(MAX_VIEWS_DEFAULT))
    getCommunityProfile()
      .then((profile) => setPoints(profile.stats.points))
      .catch(() => setPoints(null))
    void loadShares()
  }, [open, loadShares])

  /** 将文本写入剪贴板（失败静默，保持无感知） */
  const copyUrl = async (url: string, pid: string) => {
    try {
      await navigator.clipboard.writeText(url)
    } catch {
      return
    }
    setCopiedPid(pid)
    toast.success(t('share.copied'))
    setTimeout(() => setCopiedPid((prev) => (prev === pid ? null : prev)), 1500)
  }

  /** 发起分享（次数合法 + 积分充足 + 非提交中） */
  const handleCreate = async () => {
    const v = Number(maxViews)
    if (!Number.isInteger(v) || v < MAX_VIEWS_MIN || v > MAX_VIEWS_MAX) {
      toast.error(t('share.invalidMaxViews'))
      return
    }
    if (points != null && points < shareCostPoints(v)) return
    setCreating(true)
    try {
      const link = await createCommunityShareLink(postId, v)
      setCreated(link)
      await loadShares()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setCreating(false)
    }
  }

  /** 当前次数是否可提交（默认 1000 内置占位合法） */
  const viewsValid = (() => {
    const v = Number(maxViews)
    return Number.isInteger(v) && v >= MAX_VIEWS_MIN && v <= MAX_VIEWS_MAX
  })()

  /** 本次发起所需积分（输入非法时按 0 处理，避免误报不足） */
  const currentCost = viewsValid ? shareCostPoints(Number(maxViews)) : 0

  /** 积分不足提示（points 加载完成且不足时展示） */
  const insufficient = points != null && points < currentCost

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="text-sm">
            <i className="fa-solid fa-share-nodes text-primary mr-1.5 size-3" />
            {t('share.title')}
          </DialogTitle>
          <DialogDescription className="text-xs">{t('share.desc')}</DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-1">
          {/* 次数设置 + 成本提示 */}
          <div className="space-y-1">
            <div className="flex items-center justify-between">
              <Label htmlFor="share-max-views" className="text-xs">
                {t('share.maxViews')}
              </Label>
              <span className="text-muted-foreground text-[10px]">
                {Number(maxViews) || 0}/{MAX_VIEWS_MAX}
              </span>
            </div>
            <Input
              id="share-max-views"
              type="number"
              min={MAX_VIEWS_MIN}
              max={MAX_VIEWS_MAX}
              value={maxViews}
              onChange={(e) => setMaxViews(e.target.value.replace(/[^\d]/g, ''))}
              className="h-8 text-xs"
            />
            <p className="text-muted-foreground flex items-center gap-1 text-[11px]">
              <i className="fa-solid fa-coins size-2.5" />
              {t('share.tiers', { c1: 100, c2: 200, c3: 500 })}
            </p>
            <p className="text-primary flex items-center gap-1 text-[11px]">
              <i className="fa-solid fa-coins size-2.5" />
              {t('share.costForThis', { cost: currentCost, maxViews: Number(maxViews) || 0 })}
            </p>
          </div>

          {/* 当前积分 */}
          <div className="flex items-center justify-between rounded-md border bg-muted/40 px-3 py-2 text-xs">
            <span className="text-muted-foreground">{t('share.currentPoints')}</span>
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
              {t('share.insufficientPoints', { cost: currentCost })}
            </p>
          )}

          {/* 刚生成的直链（可全选 + 复制） */}
          {created && (
            <div className="space-y-1 rounded-md border border-primary/40 bg-primary/5 p-2">
              <div className="flex items-center gap-1.5">
                <Label className="text-primary shrink-0 text-[11px]">{t('share.link')}</Label>
                <Badge variant="secondary" className="h-4 px-1 font-mono text-[10px]">
                  {created.pid}
                </Badge>
              </div>
              <div className="flex items-center gap-1.5">
                <Input readOnly value={created.url} onFocus={(e) => e.currentTarget.select()} className="h-8 flex-1 font-mono text-[11px]" />
                <Button
                  variant="outline"
                  size="sm"
                  className="h-8 shrink-0 px-2 text-[11px]"
                  onClick={() => void copyUrl(created.url, created.pid)}
                >
                  {copiedPid === created.pid ? (
                    <i className="fa-solid fa-check text-green-500 mr-1 size-2.5" />
                  ) : (
                    <i className="fa-regular fa-copy mr-1 size-2.5" />
                  )}
                  {t('share.copy')}
                </Button>
              </div>
            </div>
          )}

          {/* 本帖已有分享列表（仅展示 / 复制；撤销归管理员） */}
          <div className="space-y-1">
            <p className="text-muted-foreground flex items-center gap-1 text-[11px]">
              <i className="fa-solid fa-link size-2.5" />
              {t('share.linkList')}
              <span className="text-[10px] tabular-nums">({shares.length})</span>
            </p>
            {listLoading && !shares.length ? (
              <div className="text-muted-foreground flex h-10 items-center justify-center gap-2 text-xs">
                <i className="fa-solid fa-spinner fa-spin size-3" />
                {t('loadError.loading')}
              </div>
            ) : shares.length === 0 ? (
              <p className="text-muted-foreground text-[11px]">{t('share.empty')}</p>
            ) : (
              <div className="max-h-44 space-y-1 overflow-y-auto pr-1 custom-scrollbar">
                {shares.map((share) => {
                  const exhausted = share.views >= share.maxViews
                  return (
                    <div key={share.pid} className="flex items-center gap-1.5 rounded-md border bg-background px-2 py-1.5">
                      <span className="w-24 shrink-0 truncate font-mono text-[11px]">{share.pid}</span>
                      <span className="text-muted-foreground shrink-0 text-[11px] tabular-nums">
                        {t('share.usage', { views: share.views, maxViews: share.maxViews })}
                      </span>
                      <Badge
                        variant={exhausted ? 'outline' : 'secondary'}
                        className={cn('h-4 shrink-0 px-1 text-[10px]', exhausted && 'text-muted-foreground')}
                      >
                        {t(exhausted ? 'share.statusExhausted' : 'share.statusNormal')}
                      </Badge>
                      <span className="text-muted-foreground ml-auto shrink-0 text-[10px] tabular-nums">
                        {formatCommunityTime(share.createdAt, t)}
                      </span>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="text-muted-foreground hover:text-foreground h-6 shrink-0 px-1.5 text-[11px]"
                        title={t('share.copy')}
                        onClick={() => void copyUrl(share.url, share.pid)}
                      >
                        {copiedPid === share.pid ? (
                          <i className="fa-solid fa-check text-green-500 size-2.5" />
                        ) : (
                          <i className="fa-regular fa-copy size-2.5" />
                        )}
                      </Button>
                    </div>
                  )
                })}
              </div>
            )}
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" size="sm" className="h-8 text-xs" onClick={() => onOpenChange(false)}>
            {t('post.cancel')}
          </Button>
          <Button
            size="sm"
            className="h-8 text-xs"
            disabled={!viewsValid || creating || insufficient}
            onClick={handleCreate}
          >
            {creating && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
            <i className="fa-solid fa-share-nodes mr-1.5 size-2.5" />
            {t('share.create')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}