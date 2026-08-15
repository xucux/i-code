/**
 * 帖子详情路由（/community/post/$id）
 *
 * 从本地状态取当前 user_id（用于隐藏对自己内容的举报入口），
 * 页面主体见 modules/community/ui/post-detail.tsx。
 */

import { createFileRoute } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { PostDetail } from '@/modules/community/ui/post-detail'
import { getCommunityState } from '@/hooks/use-community'

function CommunityPostPage() {
  const { id } = Route.useParams()
  const postId = Number(id)
  const [currentUserId, setCurrentUserId] = useState<string | null>(null)

  useEffect(() => {
    getCommunityState()
      .then((state) => setCurrentUserId(state.userId))
      .catch(() => setCurrentUserId(null))
  }, [])

  if (!Number.isFinite(postId)) return null
  return <PostDetail postId={postId} currentUserId={currentUserId} />
}

export const Route = createFileRoute('/community/post/$id')({
  component: CommunityPostPage,
})
