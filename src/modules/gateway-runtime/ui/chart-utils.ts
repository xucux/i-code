import type { StatsGranularity } from '@/modules/call-records/types'

/** 图表时间窗口配置：根据窗口小时数决定聚合粒度、桶宽与标签格式 */
export interface WindowConfig {
  granularity: StatsGranularity
  bucketSeconds: number
  formatLabel: (d: Date) => string
}

const pad = (n: number) => n.toString().padStart(2, '0')

/**
 * 根据时间窗口长度选择最合适的聚合粒度
 *
 * - 1-2 小时：30 秒桶
 * - 2-12 小时：2 分钟桶
 * - 12-24 小时：5 分钟桶
 */
export function getWindowConfig(windowHours: number): WindowConfig {
  if (windowHours <= 2) {
    return {
      granularity: 'thirtySeconds',
      bucketSeconds: 30,
      formatLabel: (d) => `${pad(d.getHours())}:${pad(d.getMinutes())}`,
    }
  }
  if (windowHours <= 12) {
    return {
      granularity: 'twoMinutes',
      bucketSeconds: 120,
      formatLabel: (d) => `${pad(d.getHours())}:${pad(d.getMinutes())}`,
    }
  }
  return {
    granularity: 'fiveMinutes',
    bucketSeconds: 300,
    formatLabel: (d) =>
      `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`,
  }
}

/**
 * 生成时间桶序列
 *
 * 结束时间对齐到桶宽边界，向前生成覆盖整个窗口的桶。
 * 用于在图表中保持时间轴连续（空桶补 0）。
 */
export function generateBuckets(now: Date, windowHours: number, bucketSeconds: number): Date[] {
  const buckets: Date[] = []
  const ms = now.getTime()
  const alignedMs = Math.floor(ms / (bucketSeconds * 1000)) * (bucketSeconds * 1000)
  const aligned = new Date(alignedMs)

  const totalBuckets = (windowHours * 60 * 60) / bucketSeconds
  for (let i = totalBuckets - 1; i >= 0; i--) {
    buckets.push(new Date(aligned.getTime() - i * bucketSeconds * 1000))
  }
  return buckets
}

/** 模型曲线颜色：围绕主题色相的最大偏移（±度） */
const HUE_SPREAD = 32

/**
 * 生成围绕主题色的模型曲线颜色（替代色相环 360° 均分）
 *
 * 读取当前主题 `--primary` 的色相作为基准，在 ±HUE_SPREAD 范围内按模型数量均分，
 * 避免颜色跳变到与主题无关的色相；饱和度与亮度沿渐变方向递增，
 * 并针对浅色/深色主题调整亮度区间（浅色主题偏深、深色主题偏亮），
 * 保证各颜色在对应背景下可读且整体与主题协调。
 */
export function getChartColor(index: number, total: number): string {
  // 读取主题主色相（Tailwind HSL 空格语法：`222.2 47.4% 11.2%`）
  const raw = getComputedStyle(document.documentElement).getPropertyValue('--primary').trim()
  const hueMatch = raw.match(/^([\d.]+)/)
  const baseHue = hueMatch ? parseFloat(hueMatch[1]) : 222

  // 深色主题判断：主题类名形如 theme-dark / theme-claude-dark
  const themeClass = Array.from(document.documentElement.classList).find((c) =>
    c.startsWith('theme-')
  )
  const isDark = themeClass?.includes('dark') ?? false

  const hue = total <= 1
    ? baseHue
    : baseHue - HUE_SPREAD + (HUE_SPREAD * 2 * index) / (total - 1)
  const progress = total <= 1 ? 0.5 : index / (total - 1)
  // 饱和度与亮度沿渐变方向递增，相邻模型既有色相差又有明暗差，便于区分
  const sat = Math.round(58 + progress * 22)
  const light = isDark
    ? Math.round(55 + progress * 14)
    : Math.round(34 + progress * 16)
  const normalizedHue = ((hue % 360) + 360) % 360

  return `hsl(${normalizedHue} ${sat}% ${light}%)`
}

/** 解析当前主题 `--primary` 变量的 HSL 分量（Tailwind 空格语法：`222.2 47.4% 11.2%`） */
function readPrimaryHsl(): { hue: number; sat: number; light: number } {
  const raw = getComputedStyle(document.documentElement).getPropertyValue('--primary').trim()
  const parts = raw.split(/\s+/)
  const hue = parts[0] !== undefined ? parseFloat(parts[0]) : 222
  const sat = parts[1] !== undefined ? parseFloat(parts[1]) : 60
  const light = parts[2] !== undefined ? parseFloat(parts[2]) : 40
  return { hue, sat, light }
}

/**
 * 生成 Token 累计柱状图的双色（围绕主题主色的明暗配对）
 *
 * - `total`：当天总 Token 消耗（亮一档）
 * - `cached`：当天缓存命中 Token 消耗（深一档，比 total 更深）
 *
 * 深色主题下主色本身偏亮，缓存色在主色基础上压低亮度实现"更深"；
 * 浅色主题下主色偏深（如 11% 亮度），总消耗使用提亮后的版本、缓存色压得更深。
 */
export function getTokenBarColors(): { total: string; cached: string } {
  const { hue, sat, light } = readPrimaryHsl()
  const themeClass = Array.from(document.documentElement.classList).find((c) =>
    c.startsWith('theme-')
  )
  const isDark = themeClass?.includes('dark') ?? false

  if (isDark) {
    // 深色主题：主色偏亮 → 总消耗用主色，缓存命中压低亮度（更深更重）
    const cachedLight = Math.max(light - 26, 15)
    return {
      total: `hsl(${hue} ${sat}% ${light}%)`,
      cached: `hsl(${hue} ${Math.min(sat + 5, 95)}% ${cachedLight}%)`,
    }
  }
  // 浅色主题：主色偏深 → 总消耗提亮一档、缓存命中的亮度再压低（保持明显更深）
  const totalLight = Math.min(light + 30, 55)
  const cachedLight = Math.max(light - 10, 9)
  return {
    total: `hsl(${hue} ${Math.max(sat - 8, 35)}% ${totalLight}%)`,
    cached: `hsl(${hue} ${Math.min(sat + 5, 95)}% ${cachedLight}%)`,
  }
}
