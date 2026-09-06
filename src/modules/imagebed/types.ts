/**
 * imagebed 模块类型定义（社区图床上传）
 *
 * - `ImagebedProvider`：内置/可用的图床列表项（与后端 `imagebed_list` Command 返回一致）
 * - `ImagebedLinkReady`：后端 `imagebed:link-ready` 事件 payload
 */

/** 图床 provider（安全字段，不含注入脚本） */
export interface ImagebedProvider {
  id: string
  name: string
  url: string
}

/** 图床外链就绪事件 payload（后端轮询标题桥接推送） */
export interface ImagebedLinkReady {
  providerId: string
  /** 图片直链 URL */
  url: string
  /** 可直接插入编辑器的 Markdown 片段（![alt](url)） */
  markdown: string
  /** 毫秒时间戳 */
  createdAt: number
}