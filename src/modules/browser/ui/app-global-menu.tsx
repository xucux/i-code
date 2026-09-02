/**
 * 全局右键接管 + 外部链接接管
 *
 * 1) 右键接管：在应用任意界面右键弹出自定义菜单（复制 / 刷新 / 回到主页），
 *    顶替 WebView2 原生的右键菜单。
 * 2) 外部链接接管：点击 http(s) / mailto / tel 链接时，阻止默认跳转，
 *    弹出「在应用内打开 / 在浏览器打开」选择菜单（对应应用需求一、二）。
 *
 * 通过 window 捕获事件挂载于根布局，所有页面（含 Markdown 中的链接）共用。
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import { useNavigate } from '@tanstack/react-router'
import { useTranslation } from '@/modules/i18n/use-translation'
import { invokeCommand } from '@/hooks/use-command'
import { isTauri } from '@/core/utils'
import { cn } from '@/lib/utils'

/** 菜单单项 */
interface MenuItem {
  key: string
  /** i18n 文案 */
  label: string
  /** Font Awesome 图标类名（不含前缀） */
  icon: string
  disabled?: boolean
  onSelect: () => void
}

/** 当前展示的菜单状态与位置 */
interface MenuState {
  x: number
  y: number
  items: MenuItem[]
}

/** 链接类型判断结果 */
function classifyLink(href: string): 'web' | 'other' | null {
  const h = href.trim().toLowerCase()
  if (/^https?:\/\//.test(h)) return 'web'
  if (/^(mailto:|tel:)/.test(h)) return 'other'
  return null
}

/** 判断元素是否为可编辑目标（文本输入框 / 文本域 / 可编辑富文本） */
function isEditableTarget(el: Element | null): el is HTMLElement {
  if (!el) return false
  const target = el as HTMLElement
  if (target.isContentEditable) return true
  const tag = el.tagName
  if (tag === 'TEXTAREA') return true
  if (tag === 'INPUT') {
    const type = (el as HTMLInputElement).type
    return !['checkbox', 'radio', 'button', 'submit', 'reset', 'file', 'image', 'color', 'range'].includes(
      type
    )
  }
  return false
}

/** 将文本插入到可编辑目标的光标位置（input/textarea 手动回填并触发 input 事件，兼容 React 受控组件） */
function insertTextToEditable(el: HTMLElement, text: string): void {
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    const start = el.selectionStart ?? el.value.length
    const end = el.selectionEnd ?? start
    const next = el.value.slice(0, start) + text + el.value.slice(end)
    const setter = Object.getOwnPropertyDescriptor(el.constructor.prototype, 'value')?.set
    if (setter) setter.call(el, next)
    el.setSelectionRange(start + text.length, start + text.length)
    // 触发 input 事件，保证 React 受控组件能同步状态
    el.dispatchEvent(new Event('input', { bubbles: true }))
  } else {
    // contenteditable：优先使用 execCommand 以保留撤销栈
    document.execCommand('insertText', false, text)
  }
}

/** 读取系统剪贴板文本并粘贴到当前聚焦的可编辑目标；无可编辑目标时静默忽略 */
async function pasteIntoActive(): Promise<void> {
  let text: string
  try {
    text = await invokeCommand<string>('clipboard_read_text')
  } catch {
    return
  }
  if (!text) return

  const active = document.activeElement
  if (isEditableTarget(active)) {
    active.focus()
    insertTextToEditable(active, text)
  }
}

export function AppGlobalMenu() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const [menu, setMenu] = useState<MenuState | null>(null)

  /** 展示菜单（位置在渲染后进一步钳制到视口内） */
  const showMenu = useCallback((x: number, y: number, items: MenuItem[]) => {
    setMenu({ x, y, items })
  }, [])

  /** 关闭当前菜单 */
  const dismiss = useCallback(() => setMenu(null), [])

  /** 菜单 DOM 引用，用于判断鼠标是否落在菜单内 */
  const menuRef = useRef<HTMLDivElement | null>(null)

  // 打开外部链接：在系统浏览器打开
  const openOutside = useCallback((href: string) => {
    void invokeCommand<void>('open_url', { url: href })
  }, [])

  // 打开内置浏览器窗口
  const openInApp = useCallback((href: string) => {
    void invokeCommand<void>('open_browser_window', { url: href })
  }, [])

  // 对外部链接的点击拦截（捕获阶段，确保提前于链接默认行为）
  const handleLinkClick = useCallback(
    (e: MouseEvent) => {
      if (e.button !== 0) return
      const target = (e.target as HTMLElement | null)?.closest?.('a')
      const href = target?.getAttribute('href')
      if (!href) return

      const kind = classifyLink(href)
      if (!kind) return

      // 外部链接：阻止默认跳转，弹出打开方式选择
      e.preventDefault()
      const { clientX, clientY } = e
      const items: MenuItem[] =
        kind === 'web'
          ? [
              {
                key: 'openInApp',
                label: t('browser.openInApp'),
                icon: 'fa-window-restore',
                onSelect: () => openInApp(href),
              },
              {
                key: 'openOutside',
                label: t('browser.openOutside'),
                icon: 'fa-arrow-up-right-from-square',
                onSelect: () => openOutside(href),
              },
            ]
          : [
              {
                key: 'openOutside',
                label: t('browser.openOutside'),
                icon: 'fa-arrow-up-right-from-square',
                onSelect: () => openOutside(href),
              },
            ]
      showMenu(clientX, clientY, items)
    },
    [t, showMenu, openInApp, openOutside]
  )

  // 全局右键接管：弹出 粘贴 / 复制 / 刷新 / 回到主页
  const handleContextMenu = useCallback(
    (e: MouseEvent) => {
      // 页面声明的右键接管区域（data-suppress-global-contextmenu，如视觉生成页的图片菜单）：
      // 仅抑制 WebView2 原生菜单，不弹全局菜单，由页面自行展示专属右键
      const target = e.target as HTMLElement | null
      if (target?.closest?.('[data-suppress-global-contextmenu]')) {
        e.preventDefault()
        return
      }

      e.preventDefault()
      const selection = window.getSelection()?.toString() ?? ''
      const items: MenuItem[] = [
        {
          key: 'paste',
          label: t('contextMenu.paste'),
          icon: 'fa-paste',
          onSelect: () => {
            void pasteIntoActive()
          },
        },
        {
          key: 'copy',
          label: t('contextMenu.copy'),
          icon: 'fa-copy',
          disabled: !selection,
          onSelect: () => {
            void navigator.clipboard.writeText(selection).catch(() => undefined)
          },
        },
        {
          key: 'refresh',
          label: t('contextMenu.refresh'),
          icon: 'fa-rotate',
          onSelect: () => window.location.reload(),
        },
        {
          key: 'home',
          label: t('contextMenu.home'),
          icon: 'fa-house',
          onSelect: () => void navigate({ to: '/' }),
        },
      ]
      showMenu(e.clientX, e.clientY, items)
    },
    [t, showMenu, navigate]
  )

  useEffect(() => {
    // 非 Tauri 环境（浏览器预览）保持原生行为，不接管
    if (!isTauri()) return

    // 点击 → 外部链接接管；右键 → 自定义菜单
    window.addEventListener('click', handleLinkClick, true)
    window.addEventListener('contextmenu', handleContextMenu, true)
    return () => {
      window.removeEventListener('click', handleLinkClick, true)
      window.removeEventListener('contextmenu', handleContextMenu, true)
    }
  }, [handleLinkClick, handleContextMenu])

  // 菜单打开期间，监听全局关闭事件（滚动 / 窗口尺寸变化 / 再次点击空白）
  useEffect(() => {
    if (!menu) return
    // 点击菜单内不关闭，保证菜单项点击事件能正常触发；否则先关闭菜单
    const onClose = (e: Event) => {
      if (menuRef.current?.contains(e.target as Node)) return
      dismiss()
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') dismiss()
    }
    // 带捕获阶段的 mousedown，保证点击菜单之外的任何地方都会先关闭菜单
    window.addEventListener('mousedown', onClose, true)
    window.addEventListener('scroll', onClose, true)
    window.addEventListener('resize', onClose)
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('mousedown', onClose, true)
      window.removeEventListener('scroll', onClose, true)
      window.removeEventListener('resize', onClose)
      window.removeEventListener('keydown', onKey)
    }
  }, [menu, dismiss])

  // 菜单挂载后，将 fix 定位的菜单位置钳制到视口内，并缓存 DOM 引用供 contains 判断
  const setMenuRef = useCallback(
    (el: HTMLDivElement | null) => {
      menuRef.current = el
      if (el && menu) {
        const x = Math.max(4, Math.min(menu.x, window.innerWidth - el.offsetWidth - 4))
        const y = Math.max(4, Math.min(menu.y, window.innerHeight - el.offsetHeight - 4))
        el.style.left = `${x}px`
        el.style.top = `${y}px`
      }
    },
    [menu]
  )

  return (
    <>
      {menu && (
        <div
          ref={setMenuRef}
          role="menu"
          className="text-popover-foreground z-[999] min-w-[9rem] overflow-hidden rounded-md border bg-popover p-1 shadow-md"
          style={{ position: 'fixed', left: menu.x, top: menu.y }}
          onMouseDown={(e) => e.stopPropagation()}
        >
          {menu.items.map((item) => (
            <button
              key={item.key}
              type="button"
              role="menuitem"
              disabled={item.disabled}
              className={cn(
                'flex w-full cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-xs outline-none',
                'transition-colors',
                item.disabled
                  ? 'text-muted-foreground/50'
                  : 'hover:bg-accent hover:text-accent-foreground'
              )}
              onClick={() => {
                item.onSelect()
                dismiss()
              }}
            >
              <i className={cn('fa-solid w-4 shrink-0 text-center', item.icon)} />
              {item.label}
            </button>
          ))}
        </div>
      )}
    </>
  )
}