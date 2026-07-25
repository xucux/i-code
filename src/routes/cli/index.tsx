import { createFileRoute } from '@tanstack/react-router'
import { useMemo, useState } from 'react'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { useCliProfiles } from '@/hooks/use-cli-profiles'
import { useTranslation } from '@/modules/i18n/use-translation'
import { ClaudeCliPanel } from '@/modules/cli-management/ui/claude-cli-panel'
import { CodexPanel } from '@/modules/cli-management/ui/codex-panel'
import { OpenCodePanel } from '@/modules/cli-management/ui/opencode-panel'
import { CliSettingsPanel } from '@/modules/cli-management/ui/cli-settings-panel'

/**
 * CLI 管理首页
 *
 * 按固定客户端维护供应商、模型映射和配置文件位置。
 * 顶部仅保留 Tab 切换 + 帮助说明，去除冗余标题。
 */
function CliIndexPage() {
  const { t } = useTranslation()
  const [activeTab, setActiveTab] = useState('claude-code')
  const { profiles, refetch } = useCliProfiles()
  const [pageHeight, pageRef] = useAvailableHeight()
  const [headerHeight, headerRef] = useAvailableHeight()
  const contentHeight = useMemo(
    () => Math.max(0, pageHeight - headerHeight - 32),
    [headerHeight, pageHeight]
  )

  const profileBySlug = (slug: string) => profiles.find((profile) => profile.slug === slug)

  return (
    <div ref={pageRef} className="h-full overflow-hidden p-4">
      <TooltipProvider delayDuration={200}>
        <Tabs value={activeTab} onValueChange={setActiveTab}>
          <div ref={headerRef} className="flex items-center justify-between gap-3">
            <TabsList className="h-8 shrink-0">
              <TabsTrigger value="claude-code" className="h-7 gap-1.5 px-2.5 text-xs">
                <i className="fa-solid fa-terminal" />
                {t('cli.tabs.claude')}
              </TabsTrigger>
              <TabsTrigger value="codex" className="h-7 gap-1.5 px-2.5 text-xs">
                <i className="fa-solid fa-code" />
                {t('cli.tabs.codex')}
              </TabsTrigger>
              <TabsTrigger value="opencode" className="h-7 gap-1.5 px-2.5 text-xs">
                <i className="fa-solid fa-code-branch" />
                {t('cli.tabs.opencode')}
              </TabsTrigger>
              <TabsTrigger value="settings" className="h-7 gap-1.5 px-2.5 text-xs">
                <i className="fa-solid fa-gear" />
                {t('cli.tabs.settings')}
              </TabsTrigger>
            </TabsList>

            <Tooltip>
              <TooltipTrigger asChild>
                <button
                  type="button"
                  className="flex size-7 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                  aria-label={t('cli.help.title')}
                >
                  <i className="fa-solid fa-circle-question text-sm" />
                </button>
              </TooltipTrigger>
              <TooltipContent side="bottom" align="end" className="max-w-xs text-xs">
                <p className="font-medium">{t('cli.help.title')}</p>
                <p className="mt-1 whitespace-pre-line text-muted-foreground">
                  {t('cli.help.content')}
                </p>
              </TooltipContent>
            </Tooltip>
          </div>

        <TabsContent value="claude-code" className="overflow-hidden">
          <ClaudeCliPanel profile={profileBySlug('claude-code')} height={contentHeight} />
        </TabsContent>
        <TabsContent value="codex" className="overflow-hidden">
          <CodexPanel profile={profileBySlug('codex')} height={contentHeight} />
        </TabsContent>
        <TabsContent value="opencode" className="overflow-hidden">
          <OpenCodePanel profile={profileBySlug('opencode')} height={contentHeight} />
        </TabsContent>
        <TabsContent value="settings" className="min-h-0 overflow-hidden">
          <CliSettingsPanel
            profiles={profiles}
            height={contentHeight}
            onProfilesChange={refetch}
          />
        </TabsContent>
        </Tabs>
      </TooltipProvider>
    </div>
  )
}

export const Route = createFileRoute('/cli/')({
  component: CliIndexPage,
})
