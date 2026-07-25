import { useTranslation } from '@/modules/i18n/use-translation'
import type { Locale as DateFnsLocale } from 'date-fns'
import { enUS, zhCN } from 'date-fns/locale'
import { zhCN as rdpZhCN } from 'react-day-picker/locale'

/**
 * 根据当前 i18n 语言返回日期组件所需的 locale 对象。
 *
 * - dateFnsLocale 用于 date-fns 的 format
 * - reactDayPickerLocale 用于 react-day-picker 的 DayPicker
 */
export function useDateLocale(): {
  dateFnsLocale: DateFnsLocale
  reactDayPickerLocale: typeof rdpZhCN | undefined
} {
  const { i18n } = useTranslation()
  const isZh = i18n.language === 'zh-CN'

  return {
    dateFnsLocale: isZh ? zhCN : enUS,
    reactDayPickerLocale: isZh ? rdpZhCN : undefined,
  }
}
