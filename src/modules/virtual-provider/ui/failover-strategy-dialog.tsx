import { useState, type ReactNode } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

/** 错误判定表中一行：左侧为状态码 / 错误标识，右侧为处理方式 i18n key */
interface StrategyErrorRow {
  codes: string
  descKey: string
}

/** 错误判定表（与后端 error_classifier.rs 的判定顺序保持一致） */
const ERROR_ROWS: StrategyErrorRow[] = [
  {
    codes: '402 · 429(quota)',
    descKey: 'strategyErrorQuota',
  },
  {
    codes: '401 · 403',
    descKey: 'strategyErrorAuth',
  },
  {
    codes: '404 · 405 · 406',
    descKey: 'strategyErrorModel',
  },
  {
    codes: '429(rate limit)',
    descKey: 'strategyErrorRate',
  },
  {
    codes: '5xx · 408 · timeout',
    descKey: 'strategyErrorUpstream',
  },
  {
    codes: '400 · 413 · 422',
    descKey: 'strategyErrorRequest',
  },
  {
    codes: '400/403 content',
    descKey: 'strategyErrorContent',
  },
  {
    codes: 'other 4xx',
    descKey: 'strategyErrorUnknown',
  },
]

interface StrategySectionProps {
  /** Font Awesome 图标名（不含 `fa-solid`） */
  icon: string
  /** 小节标题 */
  title: string
  /** 小节正文 */
  children: ReactNode
}

/** 策略说明小节（标题 + 正文，统一边框与紧凑间距） */
function StrategySection({ icon, title, children }: StrategySectionProps) {
  return (
    <div className="rounded-md border p-2.5">
      <p className="mb-1 flex items-center gap-1.5 text-xs font-medium">
        <i className={`fa-solid ${icon} text-primary text-[10px]`} />
        {title}
      </p>
      <div className="text-muted-foreground text-[11px] leading-relaxed">{children}</div>
    </div>
  )
}

/**
 * 故障转移策略说明入口
 *
 * 放在「模型列表」标题旁的问号图标，点击弹出弹窗，
 * 集中说明选路顺序、错误判定、降级与冷却、探活与恢复、兜底以及对外错误格式。
 */
export function FailoverStrategyHelp() {
  const { t } = useTranslation('virtualProvider')
  const { t: tc } = useTranslation()
  const [open, setOpen] = useState(false)

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        className="text-muted-foreground hover:text-foreground transition-colors"
        title={t('strategyHelpButton')}
        aria-label={t('strategyHelpButton')}
      >
        <i className="fa-solid fa-circle-info size-3" />
      </button>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="max-w-xl">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-base">
              <i className="fa-solid fa-route text-primary" />
              {t('strategyHelpTitle')}
            </DialogTitle>
            <DialogDescription className="text-xs leading-relaxed">
              {t('strategyHelpIntro')}
            </DialogDescription>
          </DialogHeader>

          <div className="max-h-[60vh] space-y-2 overflow-y-auto pr-1">
            {/* 选路顺序 */}
            <StrategySection icon="fa-list-ol" title={t('strategyOrderTitle')}>
              {t('strategyOrderDesc')}
            </StrategySection>

            {/* 错误判定表 */}
            <StrategySection icon="fa-code-branch" title={t('strategyErrorsTitle')}>
              <p className="mb-1.5">{t('strategyErrorsHint')}</p>
              <div className="space-y-1">
                {ERROR_ROWS.map((row) => (
                  <div key={row.descKey} className="flex gap-2">
                    <span className="text-foreground/80 w-32 shrink-0 font-mono text-[10px]">
                      {row.codes}
                    </span>
                    <span className="flex-1">{t(row.descKey)}</span>
                  </div>
                ))}
              </div>
            </StrategySection>

            {/* 降级与冷却 */}
            <StrategySection icon="fa-temperature-arrow-down" title={t('strategyDegradeTitle')}>
              {t('strategyDegradeDesc')}
            </StrategySection>

            {/* 探活与恢复 */}
            <StrategySection icon="fa-heart-pulse" title={t('strategyProbeTitle')}>
              {t('strategyProbeDesc')}
            </StrategySection>

            {/* 手动恢复 */}
            <StrategySection icon="fa-rotate-left" title={t('strategyRecoveryTitle')}>
              {t('strategyRecoveryDesc')}
            </StrategySection>

            {/* 全部失活兜底 */}
            <StrategySection icon="fa-shield-halved" title={t('strategyFallbackTitle')}>
              {t('strategyFallbackDesc')}
            </StrategySection>

            {/* 对外错误格式 */}
            <StrategySection icon="fa-triangle-exclamation" title={t('strategyErrorFormatTitle')}>
              {t('strategyErrorFormatDesc')}
            </StrategySection>
          </div>

          <div className="flex items-center justify-between">
            <span className="text-muted-foreground text-[10px]">{t('strategySettingsHint')}</span>
            <Button size="sm" className="h-8 text-xs" onClick={() => setOpen(false)}>
              {tc('common.close')}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </>
  )
}
