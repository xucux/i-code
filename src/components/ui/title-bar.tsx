import { useEffect, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { cn } from '@/lib/utils'
import { isTauri } from '@/core/utils'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

/** 应用版本号，与 package.json / tauri.conf.json 保持同步 */
const APP_VERSION = '0.2.11'

interface TitleBarProps {
  /** 标题栏中间展示的可选信息内容（如 MemoryInfo 胶囊） */
  info?: React.ReactNode
  /** 是否显示迷你面板入口（闪电 icon），点击后通过 Tauri Command 打开独立迷你窗口 */
  showMiniPanel?: boolean
  /** 是否显示关于入口（问号 icon），点击后弹出关于对话框 */
  showAbout?: boolean
  /** 渲染在 i-code 标题右侧的附加内容（如更新指示器），由路由层注入以避免 UI 组件耦合业务模块 */
  leftExtra?: React.ReactNode
}

/**
 * 自定义标题栏
 *
 * 布局：[左侧: 应用标识] [中间: 信息展示 + 拖拽区] [右侧: 闪电/关于 + 窗口控制]
 * - 固定定位于页面顶部（z-50），高度 h-9，不随内容滚动
 * - 背景采用主题渐变，与当前主题色保持一致
 * - 非按钮区域支持 data-tauri-drag-region 拖拽
 * - 窗口控制按钮（最小化/最大化/关闭）统一使用 Font Awesome 图标
 */
export function TitleBar({
  info,
  showMiniPanel = true,
  // showAbout = true,
  leftExtra,
}: TitleBarProps) {
  const [isMaximized, setIsMaximized] = useState(false)
  const [aboutOpen, setAboutOpen] = useState(false)
  /** 仅在 Tauri 环境下获取窗口实例 */
  const appWindow = isTauri() ? getCurrentWindow() : null

  // 监听窗口最大化状态变化，动态切换 最大化/还原 按钮图标
  useEffect(() => {
    if (!appWindow) return
    const update = async () => {
      setIsMaximized(await appWindow.isMaximized())
    }
    update()
    const unlisten = appWindow.onResized(update)
    return () => {
      void unlisten.then((fn) => fn())
    }
  }, [appWindow])

  /** 通过 Tauri Command 打开迷你面板独立窗口 */
  const openMiniPanel = async () => {
    try {
      await invoke('open_mini_panel')
    } catch (err) {
      // eslint-disable-next-line no-console
      console.error('Failed to open mini panel:', err)
    }
  }

  return (
    <>
      <div
        data-tauri-drag-region
        className={cn(
          'fixed left-0 right-0 top-0 z-50 flex h-9 select-none items-center justify-between border-b pl-3 pr-0'
        )}
        style={{
          background:
            'linear-gradient(90deg, hsl(var(--background)) 0%, hsl(var(--primary) / 0.04) 50%, hsl(var(--muted) / 0.35) 100%)',
        }}
      >
        {/* 左侧：应用图标与标题，不参与拖拽 */}
        <div
          className="flex items-center gap-2"
          style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
        >
          <div className="bg-primary text-primary-foreground flex size-5 items-center justify-center rounded text-[10px] font-bold">
            i
          </div>
          <span className="text-xs font-medium">i-code</span>
          {leftExtra}
        </div>

        {/* 中间：拖拽区域，可插入信息展示胶囊 */}
        <div className="flex flex-1 items-center justify-center gap-4 px-4" data-tauri-drag-region>
          {info}
        </div>

        {/* 右侧：功能入口 + 窗口控制按钮，不参与拖拽 */}
        <div
          className="flex items-center"
          style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
        >
          {/* 迷你面板入口：闪电 icon，打开独立悬浮小窗口 */}
          {showMiniPanel && (
            <button
              type="button"
              onClick={openMiniPanel}
              className="flex h-9 w-9 items-center justify-center text-muted-foreground transition-colors hover:bg-black/5 hover:text-foreground"
              aria-label="打开迷你面板"
              title="迷你面板"
            >
              <i className="fa-solid fa-bolt h-3.5 w-3.5 text-xs" />
            </button>
          )}

          {/* 关于入口：问号 icon，弹出关于对话框 */}
          {/* {showAbout && (
            <button
              type="button"
              onClick={() => setAboutOpen(true)}
              className="flex h-9 w-9 items-center justify-center text-muted-foreground transition-colors hover:bg-black/5 hover:text-foreground"
              aria-label="关于"
              title="关于"
            >
              <i className="fa-regular fa-circle-question h-4 w-4 text-xs" />
            </button>
          )} */}

          {/* 窗口控制按钮：仅在 Tauri 桌面环境中显示 */}
          {appWindow && (
            <>
              {/* 最小化 */}
              <button
                type="button"
                onClick={() => appWindow.minimize()}
                className="flex h-9 w-10 items-center justify-center text-muted-foreground transition-colors hover:bg-black/5 hover:text-foreground"
                aria-label="最小化"
              >
                <i className="fa-solid fa-minus h-3.5 w-3.5 text-xs" />
              </button>

              {/* 最大化 / 还原：图标根据当前状态切换 */}
              <button
                type="button"
                onClick={() => (isMaximized ? appWindow.unmaximize() : appWindow.maximize())}
                className="flex h-9 w-10 items-center justify-center text-muted-foreground transition-colors hover:bg-black/5 hover:text-foreground"
                aria-label={isMaximized ? '还原' : '最大化'}
              >
                <i
                  className={cn(
                    'h-3.5 w-3.5 text-xs',
                    isMaximized ? 'fa-regular fa-clone' : 'fa-regular fa-square'
                  )}
                />
              </button>

              {/* 关闭：隐藏到系统托盘（而非退出进程） */}
              <button
                type="button"
                onClick={() => appWindow.hide()}
                className="flex h-9 w-10 items-center justify-center text-muted-foreground transition-colors hover:bg-red-500 hover:text-white"
                aria-label="关闭"
              >
                <i className="fa-solid fa-xmark h-4 w-4 text-xs" />
              </button>
            </>
          )}
        </div>
      </div>

      {/* 关于对话框 */}
      <Dialog open={aboutOpen} onOpenChange={setAboutOpen}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <div className="flex items-center gap-3">
              <div className="bg-primary text-primary-foreground flex size-10 items-center justify-center rounded-lg text-lg font-bold">
                i
              </div>
              <div>
                <DialogTitle className="text-lg">i-code</DialogTitle>
                <DialogDescription className="text-sm text-muted-foreground">
                  AI 网关管理与监控工具
                </DialogDescription>
              </div>
            </div>
          </DialogHeader>
          <div className="space-y-2 text-sm text-muted-foreground">
            <p>版本 {APP_VERSION}</p>
            <p>基于 Tauri 2.x + React 19 构建</p>
            <p className="text-xs">多供应商 AI 模型聚合网关，提供统一接口、额度监控与智能路由。</p>
          </div>
        </DialogContent>
      </Dialog>
    </>
  )
}
