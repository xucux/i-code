import { useEffect, useRef, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { useFailedChains } from '@/hooks/use-virtual-provider'
import { ScrollPage } from '@/components/ui/scroll-page'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { cn } from '@/lib/utils'
import { formatDateTime } from '@/core/utils'
import type {
  RequestAttemptChain,
  RequestAttemptStep,
} from '@/modules/virtual-provider/types'

interface FailedChainsTabProps {
  /** 当前选中的虚拟供应商 alias，传 null 时不加载 */
  alias: string | null
  /**
   * 刷新信号：由父级 Tab 栏的刷新按钮自增，
   * 变化时重新拉取失败请求列表（父级无法直接调用本组件内部的 refetch）
   */
  refreshKey?: number
}

/**
 * 失败请求 Tab
 *
 * 按网关请求（request_id）聚合展示故障转移的完整尝试链：
 * 每条请求可展开查看依次尝试了哪些路由、每次的状态码 / 耗时与失败原因。
 *
 * 数据来源：后端 `virtual_provider_failed_chains` 命令。
 */
export function FailedChainsTab({ alias, refreshKey = 0 }: FailedChainsTabProps) {
  const { t } = useTranslation('virtualProvider')
  const { chains, loading, refetch } = useFailedChains(alias)

  // 父级 Tab 栏的刷新按钮通过自增 refreshKey 触发重新拉取
  const lastRefreshKey = useRef(refreshKey)
  useEffect(() => {
    if (lastRefreshKey.current === refreshKey) return
    lastRefreshKey.current = refreshKey
    void refetch()
  }, [refreshKey, refetch])

  return (
    <div className="flex h-full flex-col">
      <ScrollPage variant="borderless" scrollbarVisible="auto" className="min-h-0 flex-1">
        <div className="space-y-2 px-1 pt-1">
          {loading && (
            <div className="text-muted-foreground py-6 text-center text-xs">
              {t('loading')}
            </div>
          )}

          {!loading && chains.length === 0 && (
            <div className="text-muted-foreground py-8 text-center text-xs">
              {t('failedRequestsEmpty')}
            </div>
          )}

          {!loading &&
            chains.map((chain) => <ChainRow key={chain.requestId} chain={chain} />)}
        </div>
      </ScrollPage>
    </div>
  )
}

interface ChainRowProps {
  chain: RequestAttemptChain
}

/**
 * 单条失败请求，点击头部展开尝试链明细。
 */
function ChainRow({ chain }: ChainRowProps) {
  const [open, setOpen] = useState(false)

  return (
    <Collapsible open={open} onOpenChange={setOpen} className="rounded-md border">
      <CollapsibleTrigger asChild>
        <button
          type="button"
          className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-muted/50"
        >
          <i
            className={cn(
              'fa-solid fa-chevron-right shrink-0 text-[10px] text-muted-foreground transition-transform',
              open && 'rotate-90',
            )}
          />
          <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
            {formatDateTime(chain.startedAt)}
          </span>
          <span
            className="max-w-[200px] truncate font-mono text-[10px] text-muted-foreground"
            title={chain.requestId}
          >
            {chain.requestId}
          </span>
          <span className="ml-auto flex shrink-0 items-center gap-1 text-[10px] tabular-nums text-muted-foreground">
            <i className="fa-solid fa-code-branch text-[9px]" />
            {chain.attempts.length}
          </span>
        </button>
      </CollapsibleTrigger>

      <CollapsibleContent>
        <div className="space-y-1 border-t px-2 py-1.5">
          {chain.attempts.map((step) => (
            <AttemptRow
              key={`${step.attemptIndex}-${step.virtualRouteId}`}
              step={step}
            />
          ))}
        </div>
      </CollapsibleContent>
    </Collapsible>
  )
}

interface AttemptRowProps {
  step: RequestAttemptStep
}

/**
 * 单次路由尝试：序号 / 结果 / 目标供应商与模型 / 状态码 / 耗时 / 错误。
 */
function AttemptRow({ step }: AttemptRowProps) {
  const providerLabel =
    step.targetProviderName ?? step.targetProviderId ?? step.virtualRouteId

  return (
    <div className="flex items-center gap-2 text-[10px]">
      <span className="w-5 shrink-0 text-right tabular-nums text-muted-foreground">
        #{step.attemptIndex + 1}
      </span>
      <i
        className={cn(
          'shrink-0',
          step.success
            ? 'fa-solid fa-circle-check text-primary'
            : 'fa-solid fa-circle-xmark text-destructive',
        )}
      />
      <span className="min-w-0 max-w-[150px] truncate text-xs">{providerLabel}</span>
      <span className="min-w-0 max-w-[170px] truncate font-mono text-[10px] text-muted-foreground">
        {step.targetModelId ?? '—'}
      </span>
      <span
        className={cn(
          'shrink-0 tabular-nums',
          step.success
            ? 'text-primary'
            : 'text-destructive',
        )}
      >
        {step.statusCode ?? 'network'}
      </span>
      <span className="w-12 shrink-0 text-right tabular-nums text-muted-foreground">
        {step.durationMs}ms
      </span>
      {step.errorMessage && (
        <span
          className="min-w-0 flex-1 truncate text-destructive"
          title={step.errorMessage}
        >
          {step.errorMessage}
        </span>
      )}
    </div>
  )
}
