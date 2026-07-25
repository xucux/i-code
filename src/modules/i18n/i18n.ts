import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import LanguageDetector from 'i18next-browser-languagedetector'
import zhCN from './locales/zh-CN.json'
import en from './locales/en.json'

// 翻译资源：按命名空间聚合各语言 JSON 文件
const resources = {
  'zh-CN': { translation: zhCN },
  en: { translation: en },
}

// 初始化 i18next：使用浏览器语言检测并缓存到 localStorage
i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    fallbackLng: 'zh-CN',
    interpolation: {
      escapeValue: false,
    },
    detection: {
      order: ['localStorage', 'navigator'],
      caches: ['localStorage'],
      lookupLocalStorage: 'icode-locale',
    },
  })

export default i18n

// 主动切换当前语言
export function setLocale(locale: 'zh-CN' | 'en') {
  void i18n.changeLanguage(locale)
}

// 获取当前语言，兜底返回中文
export function getLocale(): 'zh-CN' | 'en' {
  return (i18n.language as 'zh-CN' | 'en') ?? 'zh-CN'
}
