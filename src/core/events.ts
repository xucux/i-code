/**
 * 事件总线与后端 Tauri 事件名常量
 *
 * 事件流方向：
 * ```
 * 后端状态变化 → Tauri Event emit → 前端 listen() → Zustand store 更新 → UI 重渲染
 *                                                       ↕
 *                                                 mitt 内部事件（模块间通信）
 * ```
 *
 * - `eventBus`（mitt）：仅在前端内部使用，用于模块间 UI 状态同步（如弹窗关闭后刷新列表）。
 * - `BACKEND_EVENTS`：后端通过 `app_handle.emit()` 推送的事件名常量，前端通过 `listen()` 接收。
 */

import mitt from 'mitt'

/**
 * 前端内部事件名常量
 * 命名规范：`模块:动作`，例如 `provider:changed`
 */
export const EVENT_NAMES = {
  PROVIDER_CHANGED: 'provider:changed',
  WORKSPACE_SWITCHED: 'workspace:switched',
  WORKSPACE_APPLIED: 'workspace:applied',
  GATEWAY_STATUS_CHANGED: 'gateway:status-changed',
  SETTINGS_CHANGED: 'settings:changed',
  LOCALE_CHANGED: 'locale:changed',
  THEME_CHANGED: 'theme:changed',
  /** CLI 供应商额度刷新后触发，UI 更新展示 */
  BALANCE_REFRESHED: 'balance:refreshed',
  /** 虚拟供应商健康状态变化 */
  VIRTUAL_PROVIDER_HEALTH_CHANGED: 'virtual-provider:health-changed',
  /** Secret 引用被新增/删除，UI 提示用户 */
  SECRET_CHANGED: 'secret:changed',
} as const

/**
 * 前端 mitt 事件类型映射
 * 每个事件名对应的 payload 类型，保证类型安全。
 */
export type FrontendEvents = {
  [EVENT_NAMES.PROVIDER_CHANGED]: { providerId?: string; action?: 'create' | 'update' | 'delete' }
  [EVENT_NAMES.WORKSPACE_SWITCHED]: { workspaceId: string }
  [EVENT_NAMES.WORKSPACE_APPLIED]: { workspaceId: string; appliedAt: string }
  [EVENT_NAMES.GATEWAY_STATUS_CHANGED]: { isRunning: boolean; error?: string }
  [EVENT_NAMES.SETTINGS_CHANGED]: { key: string; value: unknown }
  [EVENT_NAMES.LOCALE_CHANGED]: { locale: string }
  [EVENT_NAMES.THEME_CHANGED]: { theme: string }
  [EVENT_NAMES.BALANCE_REFRESHED]: { cliProviderId: string }
  [EVENT_NAMES.VIRTUAL_PROVIDER_HEALTH_CHANGED]: { routeId: string; isHealthy: boolean }
  [EVENT_NAMES.SECRET_CHANGED]: { secretId: string; action?: 'create' | 'delete' }
}

/** 前端事件总线实例 */
export const eventBus = mitt<FrontendEvents>()

/**
 * 后端 Tauri 事件名常量
 * 与 Rust 侧 `app_handle.emit(name, payload)` 中的 name 字符串保持一致。
 */
export const BACKEND_EVENTS = {
  /** Gateway 启停或异常状态变化 */
  GATEWAY_STATUS_CHANGED: 'gateway:status-changed',
  /** 托盘菜单或前端请求切换网关开关（toggle），后端监听后执行启停 */
  GATEWAY_TOGGLE_REQUEST: 'gateway:toggle-request',
  /** 后端推送新日志条目，前端日志面板实时追加 */
  LOG_NEW_ENTRY: 'log:new-entry',
  /** 日志缓冲区被清空 */
  LOG_CLEARED: 'log:cleared',
  /** 工作区应用完成（写入 CLI 配置文件） */
  WORKSPACE_APPLIED: 'workspace:applied',
  /** 进程内存使用情况，用于标题栏 MemoryInfo 展示 */
  MEMORY_USAGE: 'memory-usage',
  /** 模型调用记录写入完成，用于仪表盘统计更新 */
  CALL_RECORD_UPDATED: 'call-record:updated',
  /** 今日 Tokens 消耗数据，由托盘后台线程每 10 秒推送 */
  TODAY_TOKENS: 'today-tokens',
  /** 余额快照刷新完成 */
  BALANCE_SNAPSHOT_UPDATED: 'balance:snapshot-updated',
  /** 启动期版本更新检查完成（无论是否有更新都推送），前端据此控制标题栏更新图标 */
  UPDATE_CHECK_RESULT: 'update-check-result',
  /** 应用设置变更完成，用于标题栏信息配置实时更新 */
  SETTINGS_CHANGED: 'settings:changed',
  /** 聊天流式增量 */
  CHAT_STREAM_CHUNK: 'chat:stream-chunk',
  /** 聊天流式/HTTP 完成 */
  CHAT_STREAM_DONE: 'chat:stream-done',
  /** 聊天流式/HTTP 错误 */
  CHAT_STREAM_ERROR: 'chat:stream-error',
  /** 更新下载进度 */
  UPDATE_DOWNLOAD_PROGRESS: 'update-download-progress',
} as const
