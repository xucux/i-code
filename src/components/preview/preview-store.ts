import { create } from 'zustand'

/**
 * 组件预览（开发者入口）状态
 *
 * 页面路由始终存在，但默认隐藏：
 * - 仅当用户在「设置 → 关于和更新」连续点击标题图标 5 次后展示侧边栏入口
 * - 点击预览页右上角「退出并隐藏」后恢复隐藏
 * - 内存态，不落库；应用重启后默认隐藏
 */
interface PreviewState {
  /** 是否在侧边栏展示组件预览入口 */
  visible: boolean
  setVisible: (visible: boolean) => void
  toggle: () => void
}

export const usePreviewStore = create<PreviewState>((set) => ({
  visible: false,
  setVisible: (visible) => set({ visible }),
  toggle: () => set((state) => ({ visible: !state.visible })),
}))
