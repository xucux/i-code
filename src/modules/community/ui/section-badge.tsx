/**
 * 帖子板块徽标（闲聊 / 领鸡蛋 / 技术）
 *
 * 列表卡片、帖子详情、我的内容等场景复用；
 * 名称走 i18n（community.section.*），未知板块回退「闲聊」样式。
 */

import { useTranslation } from '@/modules/i18n/use-translation'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import type { CommunitySection } from '@/modules/community/types'

/** 板块图标（Font Awesome free solid） */
const SECTION_ICON: Record<CommunitySection, string> = {
  chat: 'fa-comments',
  eggs: 'fa-egg',
  tech: 'fa-laptop-code',
}

/** 板块配色（CSS 变量，禁止硬编码色值） */
const SECTION_STYLE: Record<CommunitySection, string> = {
  chat: 'border-primary/30 bg-primary/10 text-primary',
  eggs: 'border-amber-500/30 bg-amber-500/10 text-amber-600 dark:text-amber-400',
  tech: 'border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400',
}

export interface SectionBadgeProps {
  section: CommunitySection
  className?: string
}

export function SectionBadge({ section, className }: SectionBadgeProps) {
  const { t } = useTranslation('community')
  return (
    <Badge
      variant="outline"
      className={cn('h-4 gap-1 px-1.5 text-[10px] font-medium', SECTION_STYLE[section], className)}
      title={t(`section.${section}`)}
    >
      <i className={cn('fa-solid size-2.5', SECTION_ICON[section])} />
      {t(`section.${section}`)}
    </Badge>
  )
}
