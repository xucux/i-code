import { invokeCommand } from '@/hooks/use-command'
import type { ClearStatsInput } from '@/modules/call-records/types'

/**
 * Call Records 模块写入操作
 *
 * 调用后端 Tauri Commands 完成统计数据清空与今日 tokens 查询。
 */

/** 清空模型调用统计数据
 *
 * - 不传参数时清空全部（明细表 + 两张聚合表）
 * - 传 startAt/endAt 时仅清空指定时间范围
 *
 * 返回受影响的行数（三表合计）。
 */
export async function clearCallStats(input?: ClearStatsInput): Promise<number> {
  return invokeCommand<number>('call_records_clear_stats', { input })
}

/** 获取今日消耗的 total_tokens 总数 */
export async function getTodayTokens(): Promise<number> {
  return invokeCommand<number>('call_records_today_tokens')
}
