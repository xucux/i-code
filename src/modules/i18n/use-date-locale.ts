import { useTranslation } from '@/modules/i18n/use-translation'
import type { Locale as DateFnsLocale } from 'date-fns'
import { enUS, ja, zhCN, zhTW } from 'date-fns/locale'
import {
  ja as rdpJa,
  zhCN as rdpZhCN,
  zhTW as rdpZhTW,
} from 'react-day-picker/locale'

// date-fns locale 映射：与 i18n 语言键对齐
const dateFnsLocaleMap: Record<string, DateFnsLocale> = {
  'zh-CN': zhCN,
  'zh-TW': zhTW,
  ja,
  en: enUS,
}

// react-day-picker locale 映射
const reactDayPickerLocaleMap: Record<string, typeof rdpZhCN | undefined> = {
  'zh-CN': rdpZhCN,
  'zh-TW': rdpZhTW,
  ja: rdpJa,
}

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
  const lang = i18n.language

  return {
    dateFnsLocale: dateFnsLocaleMap[lang] ?? enUS,
    reactDayPickerLocale: reactDayPickerLocaleMap[lang],
  }
}

