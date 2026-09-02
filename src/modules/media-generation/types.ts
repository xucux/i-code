/**
 * 媒体生成模块类型定义
 *
 * 对应后端 `src-tauri/src/modules/media_generation/types.rs`（camelCase 对齐）。
 */

/** 生成图像请求输入 */
export interface GenerateImageInput {
  /** 视觉生成供应商 ID */
  providerId: string
  /** 模型 ID（如 `sensenova-u1-fast`） */
  modelId: string
  /** 图像描述文本 */
  prompt: string
  /** 图像尺寸（如 `2752x1536`），缺省用供应商默认值 */
  size?: string
  /** 生成图片数量 */
  n?: number
  /** 是否添加水印（缺省用供应商默认值） */
  watermark?: boolean
}

/** 图像生成历史记录 */
export interface MediaGeneration {
  id: string
  providerId: string
  providerSlug: string
  modelId: string
  prompt: string
  /** 生成参数快照（size / n / watermark 等） */
  params?: Record<string, unknown>
  /** 状态：succeeded / failed */
  status: 'succeeded' | 'failed'
  /** 本地产物相对路径（相对媒体产物根目录） */
  assetPaths: string[]
  /** 供应商返回的原始 URL（可能已过期） */
  sourceUrls: string[]
  /** 失败原因 */
  errorMessage?: string
  /** 生成耗时（毫秒） */
  durationMs?: number
  createdAt: string
}
