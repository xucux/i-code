/**
 * 路由：`/chat`
 *
 * 侧栏「网关」下方的「聊天」入口；渲染 `ChatPage` 全页。
 * 会话数据不经 URL 参数，状态在页面内（`activeId`）维护。
 */

import { createFileRoute } from '@tanstack/react-router'
import { ChatPage } from '@/modules/chat/ui/chat-page'

export const Route = createFileRoute('/chat/')({
  component: ChatPage,
})
