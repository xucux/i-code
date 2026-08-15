import { createFileRoute } from '@tanstack/react-router'
import { CommunityAdmin } from '@/modules/community/ui/community-admin'

export const Route = createFileRoute('/community/admin')({
  component: CommunityAdmin,
})
