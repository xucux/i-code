/**
 * 聊天会话列表（左侧栏）
 *
 * ## 界面描述
 *
 * - 顶栏：标题 +「新建」按钮。
 * - 列表项：会话标题（可截断）+ 模型 ID 小字；选中项 `primary` 浅底。
 * - 删除：行内垃圾桶，hover 显示；点击不冒泡到选中。
 * - 空/加载态居中小字提示。
 * - 宽度与高度由父级 `style` 传入（默认宽约 200px）。
 *
 * ## 逻辑描述
 *
 * - 纯展示 + 回调：选中 / 删除 / 新建均由 `ChatPage` 处理。
 * - 删除仅触发 `onDelete(id)`，二次确认对话框在父级。
 * - `activeId` 与列表项 `id` 比较决定高亮；无业务副作用。
 */

import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useTranslation } from '@/modules/i18n/use-translation'
import type { ChatSessionSummary } from '@/modules/chat/types'

export interface SessionListProps {
  sessions: ChatSessionSummary[]
  /** 当前选中会话；null 表示尚未选中（仍可自动建草稿） */
  activeId: string | null
  loading?: boolean
  onSelect: (id: string) => void
  onDelete: (id: string) => void
  onCreate: () => void
  style?: React.CSSProperties
}

export function SessionList({
  sessions,
  activeId,
  loading,
  onSelect,
  onDelete,
  onCreate,
  style,
}: SessionListProps) {
  const { t } = useTranslation('chat')

  return (
    <div className="flex h-full min-h-0 flex-col border-r bg-card" style={style}>
      <div className="flex  h-12 items-center justify-between gap-2 border-b px-2 py-2">
        <span className="text-xs font-medium text-foreground">{t('sessions.title')}</span>
        <Button size="sm" variant="outline" className="h-7 px-2 text-xs" onClick={onCreate}>
          <i className="fa-solid fa-plus mr-1 size-3" />
          {t('sessions.new')}
        </Button>
      </div>

      <ScrollArea className="min-h-0 flex-1">
        <div className="flex flex-col gap-0.5 p-1.5">
          {loading && sessions.length === 0 && (
            <p className="px-2 py-4 text-center text-xs text-muted-foreground">
              {t('sessions.loading')}
            </p>
          )}
          {!loading && sessions.length === 0 && (
            <p className="px-2 py-4 text-center text-xs text-muted-foreground">
              {t('sessions.empty')}
            </p>
          )}
          {sessions.map((s) => {
            const active = s.id === activeId
            return (
              <div
                key={s.id}
                className={cn(
                  'group flex cursor-pointer items-start gap-1 rounded-md px-2 py-1.5 transition-colors',
                  active
                    ? 'bg-primary/15 text-primary'
                    : 'text-foreground hover:bg-accent hover:text-accent-foreground'
                )}
                onClick={() => onSelect(s.id)}
              >
                <div className="min-w-0 flex-1">
                  <div className="truncate text-xs font-medium">{s.title || t('sessions.untitled')}</div>
                  <div className="mt-0.5 truncate text-[10px] text-muted-foreground tabular-nums">
                    {s.model}
                  </div>
                </div>
                <button
                  type="button"
                  className={cn(
                    'mt-0.5 shrink-0 rounded p-1 text-muted-foreground opacity-0 transition-opacity hover:bg-destructive/15 hover:text-destructive group-hover:opacity-100',
                    active && 'opacity-70'
                  )}
                  title={t('sessions.delete')}
                  onClick={(e) => {
                    e.stopPropagation()
                    onDelete(s.id)
                  }}
                >
                  <i className="fa-solid fa-trash-can size-3 text-[10px]" />
                </button>
              </div>
            )
          })}
        </div>
      </ScrollArea>
    </div>
  )
}
