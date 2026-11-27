/**
 * `formatCompactCount` 单位换算单测
 *
 * 背景：该函数曾使用 `W = 万 / B = 亿`，与模型统计页的 `M = 百万` 混排，
 * 导致同一份数据在标题栏显示 `1.02B`（实为 1.02 亿）、在统计卡显示 `102.18M`，
 * `B` 一词两义且相差一个数量级。现已统一为 K / M / B（千 / 百万 / 十亿），
 * 此处锁定换算关系，避免回退。
 */

import { describe, expect, it } from 'vitest'
import { formatCompactCount } from './utils'

describe('formatCompactCount', () => {
  it('小于 1000 时原样展示', () => {
    expect(formatCompactCount(0)).toBe('0')
    expect(formatCompactCount(999)).toBe('999')
  })

  it('K = 千', () => {
    expect(formatCompactCount(1_000)).toBe('1.0K')
    expect(formatCompactCount(9_999)).toBe('10.0K')
    expect(formatCompactCount(12_345)).toBe('12.3K')
  })

  it('M = 百万', () => {
    expect(formatCompactCount(1_000_000)).toBe('1.00M')
    expect(formatCompactCount(102_178_202)).toBe('102.18M')
  })

  it('B = 十亿（1e9），而不是亿（1e8）', () => {
    expect(formatCompactCount(1_000_000_000)).toBe('1.00B')
    expect(formatCompactCount(1_021_782_020)).toBe('1.02B')
    // 1.02 亿应显示为 102.18M，不能再出现 1.02B
    expect(formatCompactCount(102_178_202)).not.toContain('B')
  })

  it('同一个数值在任意入参类型下结果一致', () => {
    expect(formatCompactCount(102_178_202n)).toBe('102.18M')
    expect(formatCompactCount('102178202')).toBe('102.18M')
  })

  it('非整数（图表刻度）不会因 BigInt 转换失败而降级为 —', () => {
    expect(formatCompactCount(1234.5)).toBe('1.2K')
    expect(formatCompactCount(1_500_000.7)).toBe('1.50M')
  })

  it('非法输入返回占位符', () => {
    expect(formatCompactCount(null)).toBe('—')
    expect(formatCompactCount(-1)).toBe('—')
    expect(formatCompactCount(Number.NaN)).toBe('—')
  })
})
