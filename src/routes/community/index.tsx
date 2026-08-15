import { createFileRoute } from '@tanstack/react-router'
import { CommunityPage } from '@/modules/community/ui/community-page'

export const Route = createFileRoute('/community/')({
  component: CommunityPage,
})
