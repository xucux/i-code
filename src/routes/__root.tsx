import { createRootRoute, Outlet, useRouterState } from '@tanstack/react-router'
import { Toaster } from '@/components/ui/sonner'
import { TitleBar } from '@/components/ui/title-bar'
import { TitleBarInfoContainer } from '@/components/ui/title-bar-info-container'
import { AppLayout } from '@/components/layout/app-layout'
import { UpdateCheckIndicator } from '@/modules/settings/ui/update-check'

function RootComponent() {
  const location = useRouterState({ select: (s) => s.location })
  // 迷你面板路由不渲染标题栏、侧边栏和顶部间距
  const isMiniPanel = location.pathname === '/mini-panel'

  if (isMiniPanel) {
    return (
      <div className="h-screen w-screen overflow-hidden">
        <Outlet />
        <Toaster />
      </div>
    )
  }

  return (
    // pt-9 为固定定位的标题栏留出高度，防止内容被遮挡
    <div className="h-screen w-screen overflow-hidden pt-9">
      {/* 全局自定义标题栏，所有页面共享；中间根据设置展示信息胶囊，右侧含迷你面板入口与关于入口 */}
      <TitleBar
        info={<TitleBarInfoContainer />}
        leftExtra={<UpdateCheckIndicator />}
      />
      {/* 主布局：左侧导航 + 右侧内容区 */}
      <AppLayout>
        <Outlet />
      </AppLayout>
      <Toaster />
    </div>
  )
}

export const Route = createRootRoute({
  component: RootComponent,
})
