/**
 * 聊天模块渲染偏好（localStorage 持久化）
 *
 * ## 为什么放在前端而不是 `app_settings` 表
 *
 * 这里只承载**纯渲染偏好**——后端不读取、不影响任何请求行为，
 * 因此不进数据库、不参与备份恢复，与仪表盘图表显隐、迷你窗配置等
 * 既有 UI 偏好的做法保持一致（见 `routes/index.tsx`、`routes/mini-panel.tsx`）。
 * 若后续需要随备份同步，再迁移到 `app_settings` 即可。
 *
 * ## 逻辑描述
 *
 * - 读取时做逐字段校验，非法/损坏的缓存回退默认值，避免脏数据导致渲染异常。
 * - 写入失败（隐私模式、配额满）时内存态仍然生效，仅不持久化。
 */

import { create } from 'zustand'

/** localStorage 键名，与主题（`icode-theme`）、语言（`icode-locale`）保持同一命名风格 */
const STORAGE_KEY = 'icode-chat-prefs'

/** 聊天渲染偏好 */
export interface ChatPrefs {
  /** 助手气泡按 Markdown 渲染（标题/列表/代码块/表格等）；关闭后按纯文本原样显示 */
  markdownEnabled: boolean
}

/** 默认偏好：默认开启 Markdown 渲染 */
export const DEFAULT_CHAT_PREFS: ChatPrefs = {
  markdownEnabled: true,
}

/** 从 localStorage 读取偏好；缺失或损坏时回退默认值 */
function readPrefs(): ChatPrefs {
  if (typeof window === 'undefined') return DEFAULT_CHAT_PREFS
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return DEFAULT_CHAT_PREFS
    const parsed = JSON.parse(raw) as Partial<ChatPrefs> | null
    if (!parsed || typeof parsed !== 'object') return DEFAULT_CHAT_PREFS
    return {
      markdownEnabled:
        typeof parsed.markdownEnabled === 'boolean'
          ? parsed.markdownEnabled
          : DEFAULT_CHAT_PREFS.markdownEnabled,
    }
  } catch {
    // 缓存被手工改坏或写入中断：静默回退默认值
    return DEFAULT_CHAT_PREFS
  }
}

interface ChatPrefsState extends ChatPrefs {
  setMarkdownEnabled: (enabled: boolean) => void
}

export const useChatPrefsStore = create<ChatPrefsState>((set) => ({
  ...readPrefs(),
  setMarkdownEnabled: (markdownEnabled) => {
    set({ markdownEnabled })
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ markdownEnabled } satisfies ChatPrefs))
    } catch {
      // localStorage 不可用（隐私模式 / 配额满）：内存态生效，仅不持久化
    }
  },
}))
