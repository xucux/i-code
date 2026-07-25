import { createFileRoute } from '@tanstack/react-router'
import { ProviderList } from '@/modules/ai-gateway/ui/provider-list'
import { VirtualProviderList } from '@/modules/virtual-provider/ui/virtual-provider-list'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { useTranslation } from '@/modules/i18n/use-translation'

/**
 * AI Gateway 供应商管理页
 *
 * 包含真实供应商与虚拟供应商两个页签：
 * - 真实供应商：管理上游 API 供应商，支持从内置预设选择或手动新增。
 * - 虚拟供应商：聚合多个真实供应商并按策略进行故障转移。
 */
function ProvidersPage() {
  const { t } = useTranslation('aiGateway')

  return (
    <div className="h-full overflow-auto p-6">
      <Tabs defaultValue="real" className="h-[calc(100%-5rem)]">
        <div className="mb-4 flex items-center justify-between">
          <TabsList>
            <TabsTrigger value="real" className="text-xs">{t('providersPage.tabs.real')}</TabsTrigger>
            <TabsTrigger value="virtual" className="text-xs">{t('providersPage.tabs.virtual')}</TabsTrigger>
          </TabsList>
          {/* 页面级帮助：点击问号 icon 展示本页所有注意事项 */}
          <Popover>
            <PopoverTrigger asChild>
              <button
                type="button"
                className="flex size-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                aria-label={t('help', { ns: 'common' })}
              >
                <i className="fa-regular fa-circle-question text-sm" />
              </button>
            </PopoverTrigger>
            <PopoverContent side="bottom" align="end" className="w-72 text-xs">
              <ul className="space-y-1.5">
                <li>• {t('providersPage.tabs.real')}：{t('providersPage.help.real')}</li>
                <li>• {t('providersPage.tabs.virtual')}：{t('providersPage.help.virtual')}</li>
                <li>• {t('providersPage.help.slug')}</li>
                <li>• {t('providersPage.help.apiKey')}</li>
                <li>• {t('providersPage.help.builtin')}</li>
              </ul>
            </PopoverContent>
          </Popover>
        </div>
        <TabsContent value="real" className="h-[calc(100%-3rem)]">
          <ProviderList />
        </TabsContent>
        <TabsContent value="virtual" className="h-[calc(100%-3rem)]">
          <VirtualProviderList />
        </TabsContent>
      </Tabs>
    </div>
  )
}

export const Route = createFileRoute('/gateways/providers')({
  component: ProvidersPage,
})
