/**
 * 社区右侧个人栏（§8.3）
 *
 * 个人卡：头像 + 昵称 + 编辑、签到按钮、发帖/数据统计、
 * 我的帖子 / 我的回复入口（切换左列视图）、关闭社区。
 */

import { useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import { cn } from '@/lib/utils'
import { getCommunityAvatar } from '@/modules/community/avatars'
import { MuteBadge } from '@/modules/community/ui/mute-badge'
import type { CommunityView, ProfileData } from '@/modules/community/types'

export interface CommunityProfilePanelProps {
  profile: ProfileData
  activeView: CommunityView
  /** 切换左列视图（点击已激活项则回到最新） */
  onViewChange: (view: CommunityView) => void
  onEditProfile: () => void
  onCheckIn: () => void
  checkInPending: boolean
  onCloseCommunity: () => void
}

export function CommunityProfilePanel({
  profile,
  activeView,
  onViewChange,
  onEditProfile,
  onCheckIn,
  checkInPending,
  onCloseCommunity,
}: CommunityProfilePanelProps) {
  const { t } = useTranslation('community')
  const [confirmClose, setConfirmClose] = useState(false)
  const { user, stats } = profile

  return (
    <div className="flex h-full flex-col gap-3">
      {/* 个人卡 */}
      <div className="rounded-lg border bg-card p-3">
        <div className="flex items-center gap-2.5">
          <div className="bg-muted flex size-10 shrink-0 items-center justify-center rounded-full border text-xl">
            {getCommunityAvatar(user.avatarIndex)}
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1">
              <span className="truncate text-sm font-medium">{user.nickname}</span>
              {user.banned && (
                <Badge variant="destructive" className="h-4 px-1 text-[10px]">
                  {t('profile.banned')}
                </Badge>
              )}
              <MuteBadge muted={user.muted} until={user.muteUntil} />
            </div>
            {/* 统计：帖子 / 回复 */}
            <div className="text-muted-foreground mt-0.5 flex items-center gap-2 text-[11px] tabular-nums">
              <span title={t('profile.posts')}>
                <i className="fa-solid fa-file-lines mr-0.5 size-2.5" />
                {stats.postCount}
              </span>
              <span title={t('profile.replies')}>
                <i className="fa-solid fa-comment mr-0.5 size-2.5" />
                {stats.replyCount}
              </span>
              <span title={t('profile.points')} className="text-amber-600">
                <i className="fa-solid fa-coins mr-0.5 size-2.5" />
                {stats.points}
              </span>
            </div>
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="text-muted-foreground size-7"
            title={t('action.editProfile')}
            onClick={onEditProfile}
          >
            <i className="fa-solid fa-pen size-3" />
          </Button>
        </div>

        {user.banned && user.banReason && (
          <p className="text-destructive mt-2 text-[11px]">{t('profile.banReason', { reason: user.banReason })}</p>
        )}
        {user.muted && user.muteReason && (
          <p className="text-amber-600 mt-2 text-[11px]">{t('profile.muteReason', { reason: user.muteReason })}</p>
        )}

        <Separator className="my-3" />

        {/* 签到区：累计 + 连续 + 按钮 */}
        <div className="space-y-2">
          <div className="text-muted-foreground flex items-center justify-between text-[11px]">
            <span>{t('profile.totalCheckIns', { count: stats.totalCheckIns })}</span>
            <span className="flex items-center gap-1">
              <i className="fa-solid fa-fire text-orange-500 size-3" />
              {t('profile.streak', { count: stats.streakDays })}
            </span>
          </div>
          <Button
            size="sm"
            className="h-7 w-full text-xs"
            disabled={stats.todayCheckedIn || checkInPending || user.banned}
            onClick={onCheckIn}
          >
            {checkInPending && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
            <i className="fa-solid fa-calendar-check mr-1.5 size-3" />
            {stats.todayCheckedIn ? t('action.checkedIn') : t('action.checkIn')}
          </Button>
        </div>
      </div>

      {/* 入口：我的帖子 / 我的回复 */}
      <div className="overflow-hidden rounded-lg border bg-card">
        <EntryButton
          icon="fa-solid fa-file-lines"
          label={t('tab.myPosts')}
          active={activeView === 'myPosts'}
          onClick={() => onViewChange('myPosts')}
        />
        <EntryButton
          icon="fa-solid fa-comment"
          label={t('tab.myReplies')}
          active={activeView === 'myReplies'}
          onClick={() => onViewChange('myReplies')}
        />
        <EntryButton
          icon="fa-solid fa-trophy"
          label={t('tab.leaderboard')}
          active={activeView === 'leaderboard'}
          onClick={() => onViewChange('leaderboard')}
        />
      </div>

      {/* 关闭社区（二次确认，防误触） */}
      <div className="rounded-lg border bg-card p-1">
        {confirmClose ? (
          <div className="space-y-1.5">
            <p className="text-muted-foreground text-[11px]">{t('gate.closeConfirm')}</p>
            <div className="flex gap-1.5">
              <Button variant="destructive" size="sm" className="h-6 flex-1 text-[11px]" onClick={onCloseCommunity}>
                {t('gate.closeYes')}
              </Button>
              <Button variant="outline" size="sm" className="h-6 flex-1 text-[11px]" onClick={() => setConfirmClose(false)}>
                {t('gate.closeNo')}
              </Button>
            </div>
          </div>
        ) : (
          <Button
            variant="ghost"
            size="sm"
            className="text-muted-foreground h-7 w-full justify-start text-xs hover:text-destructive"
            onClick={() => setConfirmClose(true)}
          >
            <i className="fa-solid fa-door-closed mr-1.5 size-3" />
            {t('action.closeCommunity')}
          </Button>
        )}
      </div>
    </div>
  )
}

/** 入口按钮（我的帖子 / 我的回复） */
function EntryButton({
  icon,
  label,
  active,
  onClick,
}: {
  icon: string
  label: string
  active: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'flex w-full items-center gap-2 px-3 py-2 text-left text-xs transition-colors hover:bg-accent',
        active ? 'text-primary font-medium' : 'text-muted-foreground hover:text-foreground'
      )}
    >
      <i className={cn(icon, 'size-3.5')} />
      {label}
    </button>
  )
}
