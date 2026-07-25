import { createFileRoute } from '@tanstack/react-router'
import { ModelList } from '@/modules/ai-gateway/ui/model-list'

/**
 * AI Gateway 模型管理页
 */
function ModelsPage() {

  return (
    <div className="h-full overflow-auto p-6">
      {/* <div className="mb-6 flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold">{t('aiGateway.models')}</h1>
          <p className="text-muted-foreground text-sm">管理对外暴露的 Gateway 模型</p>
        </div>
        <Button size="sm" className="h-8 text-xs">
          <i className={cn('fa-solid fa-plus', 'mr-1.5')} />
          {t('aiGateway.addModel')}
        </Button>
      </div> */}

{/* 模型统计页 */}
      <div className="h-[calc(100%-5rem)]">
        <ModelList />
      </div>
    </div>
  )
}

export const Route = createFileRoute('/gateways/models')({
  component: ModelsPage,
})
