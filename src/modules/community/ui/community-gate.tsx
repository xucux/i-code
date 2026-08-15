/**
 * 社区门禁页（§8.2）
 *
 * 未开启社区时整页高斯模糊 + 居中卡片 + Switch 确认开启；
 * D8：侧栏无任何提示，仅在用户点入社区后展示本页。
 */

import { useTranslation } from '@/modules/i18n/use-translation'
import { Card, CardContent } from '@/components/ui/card'
import { Switch } from '@/components/ui/switch'

export interface CommunityGateProps {
  /** 开启请求执行中（禁用开关防重复提交） */
  pending: boolean
  /** 开启社区（成功后由父级引导设置资料） */
  onEnable: () => void
}

export function CommunityGate({ pending, onEnable }: CommunityGateProps) {
  const { t } = useTranslation('community')

  return (
    // 整页模糊背景：内容区实际渲染在卡片下方，保证模糊效果可见
    <div className="relative h-full w-full overflow-hidden">
      {/* 占位内容：被模糊的示意层（帖子列表轮廓） */}
      <div className="absolute inset-0 space-y-3 p-6 opacity-60" aria-hidden>
        <div className="h-8 w-1/3 rounded-md bg-muted" />
        {[80, 62, 70, 55, 66].map((w, i) => (
          <div key={i} className="h-20 rounded-md border bg-card" style={{ width: `${w}%` }} />
        ))}
      </div>

      {/* 居中说明卡片 */}
      <div className="absolute inset-0 flex items-center justify-center backdrop-blur-lg">
        <Card className="w-96">
          <CardContent className="space-y-4 p-6">
            <div className="flex items-center gap-2">
              <i className="fa-solid fa-users text-primary size-4" />
              <h2 className="text-base font-semibold">{t('gate.title')}</h2>
            </div>

            <p className="text-muted-foreground text-xs leading-relaxed">{t('gate.desc')}</p>

            <div className="bg-muted/50 rounded-md border p-3">
              <p className="text-muted-foreground text-xs leading-relaxed">
                <i className="fa-solid fa-shield-halved mr-1.5 size-3" />
                {t('gate.privacy')}
              </p>
            </div>

            <div className="flex items-center justify-between">
              <span className="text-xs">{t('gate.enable')}</span>
              <Switch disabled={pending} onCheckedChange={onEnable} />
            </div>

            <p className="text-muted-foreground text-[11px]">{t('gate.reopen')}</p>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
