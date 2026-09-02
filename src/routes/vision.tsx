import { createFileRoute } from '@tanstack/react-router'
import { VisionPage } from '@/modules/media-generation/ui/vision-page'

/** 视觉生成页面（文生图工作台 + 画廊） */
export const Route = createFileRoute('/vision')({
  component: VisionPage,
})
