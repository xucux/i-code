import { useState } from 'react'

import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { CodeEditor } from '@/components/preview/code-editor'
import { useTranslation } from '@/modules/i18n/use-translation'

function generateMockLogs(): string {
  const lines = [
    '[2026-07-15T10:00:01+08:00] INFO  GET /v1/models 200 12ms',
    '[2026-07-15T10:00:05+08:00] INFO  POST /v1/chat/completions 200 1456ms gpt-4o',
    '[2026-07-15T10:00:08+08:00] INFO  POST /v1/chat/completions 200 892ms claude-3-5-sonnet',
    '[2026-07-15T10:00:12+08:00] WARN  POST /v1/chat/completions 429 34ms gpt-4o-mini',
    '[2026-07-15T10:00:15+08:00] INFO  POST /v1/chat/completions 200 678ms deepseek-chat',
    '[2026-07-15T10:00:19+08:00] INFO  GET /v1/models 200 8ms',
    '[2026-07-15T10:00:23+08:00] ERROR POST /v1/chat/completions 500 1234ms qwen-max',
    '[2026-07-15T10:00:27+08:00] INFO  POST /v1/embeddings 200 45ms text-embedding-3-small',
    '[2026-07-15T10:00:31+08:00] INFO  POST /v1/chat/completions 200 2100ms gpt-4o',
    '[2026-07-15T10:00:35+08:00] INFO  GET /health 200 2ms',
    '[2026-07-15T10:00:39+08:00] INFO  POST /v1/chat/completions 200 156ms gemini-1.5-pro',
    '[2026-07-15T10:00:43+08:00] WARN  POST /v1/chat/completions 503 56ms gpt-4-turbo',
    '[2026-07-15T10:00:47+08:00] INFO  POST /v1/chat/completions 200 789ms claude-3-opus',
    '[2026-07-15T10:00:51+08:00] INFO  POST /v1/completions 200 334ms gpt-3.5-turbo',
    '[2026-07-15T10:00:55+08:00] INFO  GET /v1/models 200 11ms',
  ]
  return lines.join('\n')
}

export function GatewayLogViewer() {
  const { t } = useTranslation('aiGateway')
  const [logs, setLogs] = useState(generateMockLogs())

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0">
        <div className="flex flex-col space-y-1.5">
          <CardTitle className="text-sm">{t('logViewer.title')}</CardTitle>
          <CardDescription className="text-xs">{t('logViewer.description')}</CardDescription>
        </div>
        <Button variant="outline" size="sm" onClick={() => setLogs('')}>
          {t('logViewer.clear')}
        </Button>
      </CardHeader>
      <CardContent>
        <CodeEditor
          value={logs}
          language="text"
          readOnly={true}
          minHeight="180px"
        />
      </CardContent>
    </Card>
  )
}
