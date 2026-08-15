/**
 * 社区资料设置弹层（昵称 + 预设 emoji 头像，§8.2 / §8.5）
 *
 * 两个场景复用：
 * - setup：首次开启门禁后本地无昵称 → 引导设置
 * - edit：个人栏「编辑资料」
 */

import { useEffect, useMemo, useState } from 'react'
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
import { Label } from '@/components/ui/label'
import { ScrollPage } from '@/components/ui/scroll-page'
import { cn } from '@/lib/utils'
import {
  COMMUNITY_AVATAR_CATEGORIES,
  getCommunityAvatar,
} from '@/modules/community/avatars'

/** 昵称长度上限（前后端一致校验） */
const NICKNAME_MAX = 20

export interface ProfileSetupDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** setup = 首次引导（不可关闭）；edit = 编辑资料 */
  mode: 'setup' | 'edit'
  defaultNickname?: string
  defaultAvatarIndex?: number | null
  /** 提交（调用方执行 update_profile 并处理错误 toast），resolve 表示成功 */
  onSubmit: (nickname: string, avatarIndex: number) => Promise<unknown>
}

export function ProfileSetupDialog({
  open,
  onOpenChange,
  mode,
  defaultNickname,
  defaultAvatarIndex,
  onSubmit,
}: ProfileSetupDialogProps) {
  const { t } = useTranslation('community')
  const [nickname, setNickname] = useState(defaultNickname ?? '')
  const [avatarIndex, setAvatarIndex] = useState(defaultAvatarIndex ?? 0)
  const [submitting, setSubmitting] = useState(false)
  // 当前激活的头像类别 key（默认跟随已有头像所在类别）
  const [activeCat, setActiveCat] = useState(COMMUNITY_AVATAR_CATEGORIES[0].key)

  // 各类别在扁平数组中的起始下标（渲染全局 index 用）
  const catOffsets = useMemo(() => {
    const offsets = new Map<string, number>()
    let acc = 0
    for (const cat of COMMUNITY_AVATAR_CATEGORIES) {
      offsets.set(cat.key, acc)
      acc += cat.emojis.length
    }
    return offsets
  }, [])

  // 根据下标定位所属类别 key
  const catOfIndex = useMemo(() => {
    const keys: string[] = []
    for (const cat of COMMUNITY_AVATAR_CATEGORIES) {
      for (let i = 0; i < cat.emojis.length; i++) keys.push(cat.key)
    }
    return keys
  }, [])

  // 弹层每次打开时以最新默认值重置表单，并跳到已有头像所在类别
  useEffect(() => {
    if (open) {
      setNickname(defaultNickname ?? '')
      setAvatarIndex(defaultAvatarIndex ?? 0)
      setActiveCat(catOfIndex[defaultAvatarIndex ?? 0] ?? COMMUNITY_AVATAR_CATEGORIES[0].key)
    }
  }, [open, defaultNickname, defaultAvatarIndex, catOfIndex])

  const activeEmojis =
    COMMUNITY_AVATAR_CATEGORIES.find((c) => c.key === activeCat)?.emojis ?? []
  const activeOffset = catOffsets.get(activeCat) ?? 0

  const trimmed = nickname.trim()
  const valid = trimmed.length > 0 && trimmed.length <= NICKNAME_MAX

  const handleSubmit = async () => {
    if (!valid || submitting) return
    setSubmitting(true)
    try {
      await onSubmit(trimmed, avatarIndex)
      // 成功后由调用方关闭弹层并 toast
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="text-sm">
            {mode === 'setup' ? t('profile.setupTitle') : t('profile.editTitle')}
          </DialogTitle>
          <DialogDescription className="text-xs">{t('profile.setupDesc')}</DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          {/* 当前头像预览 + 昵称 */}
          <div className="flex items-center gap-3">
            <div className="bg-muted flex size-12 shrink-0 items-center justify-center rounded-full border text-2xl">
              {getCommunityAvatar(avatarIndex)}
            </div>
            <div className="flex-1 space-y-1">
              <Label htmlFor="community-nickname" className="text-xs">
                {t('profile.nickname')}
              </Label>
              <Input
                id="community-nickname"
                value={nickname}
                maxLength={NICKNAME_MAX}
                placeholder={t('profile.nicknamePlaceholder')}
                onChange={(e) => setNickname(e.target.value)}
                className="h-8 text-xs"
              />
            </div>
          </div>

          {/* emoji 头像选择器：类别 chips + 全量滚动网格 */}
          <div className="space-y-1.5">
            <Label className="text-xs">{t('profile.avatar')}</Label>
            <div className="flex flex-wrap gap-1">
              {COMMUNITY_AVATAR_CATEGORIES.map((cat) => (
                <button
                  key={cat.key}
                  type="button"
                  onClick={() => setActiveCat(cat.key)}
                  className={cn(
                    'rounded-full border px-2 py-0.5 text-[11px] transition-colors hover:bg-accent',
                    cat.key === activeCat
                      ? 'border-primary bg-primary/10 text-primary font-medium'
                      : 'border-border text-muted-foreground'
                  )}
                >
                  {t(`avatarCat.${cat.key}`)}
                </button>
              ))}
            </div>
            {/* 固定高度滚动区（弹窗内固定像素高度，避免内容撑爆弹层） */}
            <ScrollPage className="h-52" variant="default" scrollbarVisible="auto">
              <div className="grid grid-cols-8 pt-1">
                {activeEmojis.map((emoji, i) => {
                  const index = activeOffset + i
                  return (
                    <button
                      key={index}
                      type="button"
                      title={emoji}
                      onClick={() => setAvatarIndex(index)}
                      className={cn(
                        'mx-auto my-0.5 flex size-8 items-center justify-center rounded-md border text-lg transition-colors hover:bg-accent',
                        index === avatarIndex
                          ? 'border-primary bg-primary/10'
                          : 'border-transparent bg-transparent'
                      )}
                    >
                      {emoji}
                    </button>
                  )
                })}
              </div>
            </ScrollPage>
          </div>
        </div>

        <DialogFooter>
          {mode === 'setup' ? (
            // 首次引导允许跳过（Worker 懒注册，默认昵称兜底），后续可从个人栏再设置
            <Button variant="ghost" size="sm" className="text-muted-foreground h-8 text-xs" onClick={() => onOpenChange(false)}>
              {t('profile.later')}
            </Button>
          ) : (
            <Button variant="outline" size="sm" className="h-8 text-xs" onClick={() => onOpenChange(false)}>
              {t('post.cancel')}
            </Button>
          )}
          <Button size="sm" className="h-8 text-xs" disabled={!valid || submitting} onClick={handleSubmit}>
            {submitting && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
            {t('profile.save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
