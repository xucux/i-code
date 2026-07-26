/**
 * 脚本编辑器右侧文档面板：系统变量 / 函数 / Snippets / 返回结构 / 示例
 */

import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Button } from '@/components/ui/button'
import { ScrollPage } from '@/components/ui/scroll-page'
import type { ScriptSnippet } from '@/modules/script-template/types'
import { DOC_FUNCTIONS, DOC_VARIABLES } from '@/modules/script-template/script-catalog'

export interface ScriptSidebarDocsProps {
  snippets: ScriptSnippet[]
  onInsert: (text: string) => void
  style?: React.CSSProperties
  labels: {
    variables: string
    functions: string
    snippets: string
    returnShape: string
    examples: string
    insert: string
  }
}

const RETURN_SHAPE = `{
  items: [
    {
      id: "balance",
      type: "amount",          // amount|integer|token|percent|time|status
      direction: "remaining",  // amount/integer
      value: 12.34,
      currencySymbol: "¥",
      primary: true,
      label: "余额",
      period: "current"
    }
  ]
}`

export function ScriptSidebarDocs({
  snippets,
  onInsert,
  style,
  labels,
}: ScriptSidebarDocsProps) {
  return (
    <div className="flex min-h-0 flex-col overflow-hidden rounded-md border" style={style}>
      <Tabs defaultValue="variables" className="flex h-full min-h-0 flex-col">
        <TabsList className="h-15 w-full shrink-0 justify-start overflow-x-auto rounded-none border-b bg-muted/40 px-1">
          <TabsTrigger value="variables" className="text-[10px]">
            {labels.variables}
          </TabsTrigger>
          <TabsTrigger value="functions" className="text-[10px]">
            {labels.functions}
          </TabsTrigger>
          <TabsTrigger value="snippets" className="text-[10px]">
            {labels.snippets}
          </TabsTrigger>
          <TabsTrigger value="return" className="text-[10px]">
            {labels.returnShape}
          </TabsTrigger>
          <TabsTrigger value="examples" className="text-[10px]">
            {labels.examples}
          </TabsTrigger>
        </TabsList>

        <TabsContent value="variables" className="mt-0 min-h-0 flex-1">
          <ScrollPage variant="borderless" className="h-full" style={style ? { height: '100%' } : undefined}>
            <ul className="space-y-1 p-2">
              {DOC_VARIABLES.map((v) => (
                <li
                  key={v.name}
                  className="flex items-start justify-between gap-2 rounded px-1 py-1 hover:bg-muted/50"
                >
                  <div className="min-w-0">
                    <code className="text-[11px] font-medium">{v.name}</code>
                    <p className="text-muted-foreground text-[10px]">{v.desc}</p>
                  </div>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-6 shrink-0 px-1.5 text-[10px]"
                    onClick={() => onInsert(v.name)}
                  >
                    {labels.insert}
                  </Button>
                </li>
              ))}
            </ul>
          </ScrollPage>
        </TabsContent>

        <TabsContent value="functions" className="mt-0 min-h-0 flex-1">
          <ScrollPage variant="borderless" className="h-full">
            <ul className="space-y-1 p-2">
              {DOC_FUNCTIONS.map((f) => (
                <li key={f.name} className="rounded px-1 py-1 hover:bg-muted/50">
                  <div className="flex items-start justify-between gap-2">
                    <code className="text-[11px] font-medium">{f.name}</code>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="h-6 shrink-0 px-1.5 text-[10px]"
                      onClick={() => onInsert(f.insertText)}
                    >
                      {labels.insert}
                    </Button>
                  </div>
                  <p className="text-muted-foreground text-[10px]">{f.desc}</p>
                </li>
              ))}
            </ul>
          </ScrollPage>
        </TabsContent>

        <TabsContent value="snippets" className="mt-0 min-h-0 flex-1">
          <ScrollPage variant="borderless" className="h-full">
            <ul className="space-y-2 p-2">
              {snippets.map((s) => (
                <li key={s.id} className="rounded border p-2">
                  <div className="mb-1 flex items-center justify-between gap-2">
                    <span className="text-xs font-medium">{s.name}</span>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      className="h-6 text-[10px]"
                      onClick={() => onInsert(s.body)}
                    >
                      {labels.insert}
                    </Button>
                  </div>
                  <p className="text-muted-foreground text-[10px]">{s.description}</p>
                </li>
              ))}
            </ul>
          </ScrollPage>
        </TabsContent>

        <TabsContent value="return" className="mt-0 min-h-0 flex-1">
          <ScrollPage variant="borderless" className="h-full">
            <pre className="p-2 font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
              {RETURN_SHAPE}
            </pre>
          </ScrollPage>
        </TabsContent>

        <TabsContent value="examples" className="mt-0 min-h-0 flex-1">
          <ScrollPage variant="borderless" className="h-full">
            <ul className="space-y-2 p-2">
              {snippets
                .filter((s) => s.id === 'balance-get-bearer' || s.id === 'items-skeleton')
                .map((s) => (
                  <li key={s.id} className="rounded border p-2">
                    <div className="mb-1 flex items-center justify-between">
                      <span className="text-xs font-medium">{s.name}</span>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        className="h-6 text-[10px]"
                        onClick={() => onInsert(s.body)}
                      >
                        {labels.insert}
                      </Button>
                    </div>
                    <pre className="text-muted-foreground max-h-40 overflow-auto font-mono text-[10px] whitespace-pre-wrap">
                      {s.body}
                    </pre>
                  </li>
                ))}
            </ul>
          </ScrollPage>
        </TabsContent>
      </Tabs>
    </div>
  )
}
