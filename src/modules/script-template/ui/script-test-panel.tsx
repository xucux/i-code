/**
 * 脚本试运行面板
 */

import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { Provider } from '@/modules/ai-gateway/types'
import type { ScriptTemplateTestResult } from '@/modules/script-template/types'

export interface ScriptTestPanelProps {
  providers: Provider[]
  providerId: string
  onProviderChange: (id: string) => void
  onRun: () => void
  running: boolean
  result: ScriptTemplateTestResult | null
  labels: {
    provider: string
    run: string
    running: string
    result: string
    duration: string
    noResult: string
  }
}

export function ScriptTestPanel({
  providers,
  providerId,
  onProviderChange,
  onRun,
  running,
  result,
  labels,
}: ScriptTestPanelProps) {
  return (
    <div className="space-y-2 rounded-md border p-3">
      <div className="flex flex-wrap items-end gap-2">
        <div className="min-w-[180px] flex-1 space-y-1">
          <Label className="text-[11px]">{labels.provider}</Label>
          <Select value={providerId || undefined} onValueChange={onProviderChange}>
            <SelectTrigger className="h-8 text-xs">
              <SelectValue placeholder={labels.provider} />
            </SelectTrigger>
            <SelectContent>
              {providers.map((p) => (
                <SelectItem key={p.id} value={p.id} className="text-xs">
                  {p.displayName}
                  <span className="text-muted-foreground ml-1.5 font-mono text-[10px]">
                    {p.slug}
                  </span>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <Button
          type="button"
          size="sm"
          className="h-8 text-xs"
          disabled={!providerId || running}
          onClick={onRun}
        >
          {running ? (
            <>
              <i className="fa-solid fa-spinner fa-spin mr-1.5" />
              {labels.running}
            </>
          ) : (
            <>
              <i className="fa-solid fa-play mr-1.5" />
              {labels.run}
            </>
          )}
        </Button>
      </div>

      <div className="space-y-1">
        <div className="text-muted-foreground text-[11px]">{labels.result}</div>
        {!result ? (
          <p className="text-muted-foreground text-[11px]">{labels.noResult}</p>
        ) : (
          <div className="space-y-1">
            <div className="flex items-center gap-2 text-xs">
              <i
                className={
                  result.ok
                    ? 'fa-solid fa-circle-check text-emerald-500'
                    : 'fa-solid fa-circle-xmark text-destructive'
                }
              />
              <span className="tabular-nums">
                {labels.duration}: {result.durationMs}ms
              </span>
            </div>
            {result.error && (
              <pre className="text-destructive max-h-28 overflow-auto rounded bg-muted/50 p-2 font-mono text-[10px] whitespace-pre-wrap">
                {result.error}
              </pre>
            )}
            {result.snapshot && (
              <pre className="max-h-40 overflow-auto rounded bg-muted/50 p-2 font-mono text-[10px] whitespace-pre-wrap">
                {JSON.stringify(result.snapshot, null, 2)}
              </pre>
            )}
            {result.logs && result.logs.length > 0 && (
              <pre className="text-muted-foreground max-h-24 overflow-auto rounded bg-muted/30 p-2 font-mono text-[10px] whitespace-pre-wrap">
                {result.logs.join('\n')}
              </pre>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
