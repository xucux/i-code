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

/** 模型曲线颜色：围绕主题色相的最大偏移（±度），最外圈色相偏移量 */
const HUE_SPREAD = 32

/** 判断当前是否为深色主题（主题类名形如 theme-dark / theme-claude-dark） */
function isDarkTheme(): boolean {
  const themeClass = Array.from(document.documentElement.classList).find((c) =>
    c.startsWith('theme-')
  )
  return themeClass?.includes('dark') ?? false
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
 * 生成主题色对（围绕当前主题主色的两个配色）
 *
 * - `primary`：主题色——深色主题直接用 `--primary`（本身偏亮），浅色主题提亮一档
 * - `deep`：主题深色——与主题色同色相、更深一档（浅色主题直接用 `--primary`）
 *
 * Token 累计柱状图（总消耗 / 缓存命中）与模型折线图（前两条线）共用此配色，
 * 保证全站图表与主题主色一致。
 */
export function getThemeColorPair(): { primary: string; deep: string } {
  const { hue, sat, light } = readPrimaryHsl()
  if (isDarkTheme()) {
    // 深色主题：主色偏亮 → 主题色用主色，深色压缩低亮度（更重）
    const deepLight = Math.max(light - 26, 15)
    return {
      primary: `hsl(${hue} ${sat}% ${light}%)`,
      deep: `hsl(${hue} ${Math.min(sat + 5, 95)}% ${deepLight}%)`,
    }
  }
  // 浅色主题：主色偏深 → 主题色提亮一档、主题深色直接用主色
  const primaryLight = Math.min(light + 30, 55)
  const deepLight = Math.max(light - 10, 9)
  return {
    primary: `hsl(${hue} ${Math.max(sat - 8, 35)}% ${primaryLight}%)`,
    deep: `hsl(${hue} ${Math.min(sat + 5, 95)}% ${deepLight}%)`,
  }
}

/**
 * 生成 Token 累计柱状图的双色
 *
 * - `total`：当天总 Token 消耗（主题色）
 * - `cached`：当天缓存命中 Token 消耗（主题深色，比 total 更深）
 */
export function getTokenBarColors(): { total: string; cached: string } {
  const pair = getThemeColorPair()
  return { total: pair.primary, cached: pair.deep }
}

/**
 * 生成围绕主题色的模型曲线颜色（包括主题色与主题深色）
 *
 * 颜色表固定包含两组主题色：
 * - `index 0`：主题色（与 Token 累计柱状图「总消耗」一致）
 * - `index 1`：主题深色（与 Token 累计柱状图「缓存命中」一致）
 * 其余模型色围绕主题色相在两侧渐变分布（避开中心主题色对），
 * 饱和度与亮度沿渐变方向递增，并针对浅色/深色主题调整亮度区间，
 * 保证各颜色可读且整体与主题协调。
 */
export function getChartColor(index: number, total: number): string {
  const pair = getThemeColorPair()
  if (total <= 1 || index === 0) return pair.primary
  if (index === 1) return pair.deep

  // 其余模型色：按「外圈 → 内圈」交替取色相偏移，始终避开中心主题色对
  const { hue: baseHue } = readPrimaryHsl()
  const k = index - 1 // 剩余模型序号（0 起）
  const remaining = total - 2
  const maxRings = Math.max(Math.ceil(remaining / 2), 1)
  const ring = Math.floor(k / 2)
  // 最外圈偏移 HUE_SPREAD，每向内一圈收缩，最小 8°（避免与主题色对重叠）
  const offset = Math.round(
    HUE_SPREAD - (ring * (HUE_SPREAD - 8)) / Math.max(maxRings - 1, 1)
  )
  const sign = k % 2 === 0 ? 1 : -1
  const hue = baseHue + sign * offset
  const pos = remaining <= 1 ? 0.5 : k / (remaining - 1)
  const sat = Math.round(58 + pos * 22)
  const light = isDarkTheme()
    ? Math.round(55 + pos * 14)
    : Math.round(34 + pos * 16)
  const normalizedHue = ((hue % 360) + 360) % 360

  return `hsl(${normalizedHue} ${sat}% ${light}%)`
}
