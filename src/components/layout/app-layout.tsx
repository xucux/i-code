import { useEffect } from 'react'
import { Link, useRouter, useRouterState } from '@tanstack/react-router'
import { listen } from '@tauri-apps/api/event'
import { cn } from '@/lib/utils'
import { useTranslation } from '@/modules/i18n/use-translation'
import { BACKEND_EVENTS } from '@/core/events'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'

/**
 * 导航菜单项配置
 *
 * 每个菜单项包含：
 * - to：TanStack Router 路由路径
 * - icon：Font Awesome 图标类名
 * - labelKey：i18n 命名空间下的翻译 key
 * - end：是否精确匹配（用于高亮判断）
 */
interface NavItem {
  to: string
  icon: string
  labelKey: string
  end?: boolean
}

/**
 * 应用主布局
 *
 * 左侧固定侧边栏导航，右侧滚动主内容区。
 * 标题栏由 __root.tsx 统一渲染，布局不再重复渲染。
 */
export function AppLayout({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation()
  const location = useRouterState({ select: (s) => s.location })
  const router = useRouter()

  // 监听托盘菜单导航请求（如「模型统计」）：跳转到后端指定的路由路径
  useEffect(() => {
    const unlisten = listen<string>(BACKEND_EVENTS.TRAY_NAVIGATE, (event) => {
      const path = event.payload
      if (typeof path === 'string' && path.startsWith('/')) {
        void router.navigate({ to: path })
      }
    })
    return () => {
      void unlisten.then((fn) => fn())
    }
  }, [router])

  // 顶部菜单：核心业务入口
  const mainNavItems: NavItem[] = [
    { to: '/', icon: 'fa-solid fa-house', labelKey: 'nav.dashboard', end: true },
    { to: '/gateways', icon: 'fa-solid fa-network-wired', labelKey: 'nav.aiGateway' },
    { to: '/chat', icon: 'fa-solid fa-comments', labelKey: 'nav.chat' },
    { to: '/cli', icon: 'fa-solid fa-terminal', labelKey: 'nav.cli' },
    // { to: '/workspaces', icon: 'fa-solid fa-briefcase', labelKey: 'nav.workspace' },
    { to: '/community', icon: 'fa-solid fa-users', labelKey: 'nav.community' },
    { to: '/logs', icon: 'fa-solid fa-file-lines', labelKey: 'nav.logs' },
    { to: '/backups', icon: 'fa-solid fa-cloud-arrow-up', labelKey: 'nav.backup' },
  ]

  // 底部菜单：系统与开发入口
  const systemNavItems: NavItem[] = [
    { to: '/settings', icon: 'fa-solid fa-gear', labelKey: 'nav.settings' },
    { to: '/preview', icon: 'fa-solid fa-palette', labelKey: 'nav.preview' },
  ]

  /**
   * 判断当前路径是否匹配菜单项
   *
   * - end 为 true 时要求路径完全相等
   * - 否则要求当前路径以菜单路径开头（支持子路由高亮父菜单）
   */
  const isActive = (item: NavItem) => {
    if (item.end) return location.pathname === item.to
    return location.pathname === item.to || location.pathname.startsWith(`${item.to}/`)
  }

  return (
    <TooltipProvider delayDuration={200}>
      <div className="flex h-screen w-screen overflow-hidden">
        {/* 左侧导航栏 */}
        <aside className="flex w-16 flex-col border-r bg-card">
          {/* 应用图标 */}
          <div className="flex h-14 items-center justify-center border-b">
            <div className="flex size-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
              <i className="fa-solid fa-code text-sm" />
            </div>
          </div>

          {/* 主菜单 */}
          <ScrollArea className="flex-1 py-2">
            <nav className="flex flex-col items-center gap-1 px-2">
              {mainNavItems.map((item) => (
                <NavButton key={item.to} item={item} active={isActive(item)} t={t} />
              ))}
            </nav>
          </ScrollArea>

          {/* 底部菜单 */}
          <div className="flex flex-col items-center gap-1 border-t py-2 px-2">
            {systemNavItems.map((item) => (
              <NavButton key={item.to} item={item} active={isActive(item)} t={t} />
            ))}
          </div>
        </aside>

        {/* 右侧主内容区 */}
        <main className="flex-1 overflow-hidden bg-background">
          {children}
        </main>
      </div>
    </TooltipProvider>
  )
}

/**
 * 单个导航按钮
 *
 * 使用 Tooltip 展示菜单名称，保持侧边栏紧凑。
 */
function NavButton({
  item,
  active,
  t,
}: {
  item: NavItem
  active: boolean
  t: (key: string) => string
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Link
          to={item.to}
          className={cn(
            'flex size-10 items-center justify-center rounded-md text-sm transition-colors',
            active
              ? 'bg-primary/15 text-primary'
              : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
          )}
        >
          <i className={cn(item.icon, 'text-base')} />
        </Link>
      </TooltipTrigger>
      <TooltipContent side="right" className="text-xs">
        {t(item.labelKey)}
      </TooltipContent>
    </Tooltip>
  )
}
