/**
 * 脚本模板系统变量 / 函数目录
 *
 * 供右侧文档面板与 CodeMirror 联想补全共用，保持一致。
 */

export interface ScriptCatalogItem {
  /** 补全标签 / 展示名 */
  label: string
  /** 插入文本 */
  insert: string
  /** 说明 */
  detail: string
  /** 补全类型 */
  type: 'variable' | 'function' | 'keyword'
  /** 文档面板分组 */
  group: 'variable' | 'http' | 'json' | 'log' | 'string' | 'math' | 'util'
}

/** 系统变量 */
export const SCRIPT_VARIABLES: ScriptCatalogItem[] = [
  { label: 'api_key', insert: 'api_key', detail: '已解密 API Key / OAuth access_token', type: 'variable', group: 'variable' },
  { label: 'now_ms', insert: 'now_ms', detail: '当前 UTC 毫秒时间戳', type: 'variable', group: 'variable' },
  { label: 'provider', insert: 'provider', detail: '供应商 map：id/slug/name/base_url/provider_type/is_enabled', type: 'variable', group: 'variable' },
  { label: 'provider.id', insert: 'provider.id', detail: '供应商 UUID', type: 'variable', group: 'variable' },
  { label: 'provider.slug', insert: 'provider.slug', detail: '路由 slug', type: 'variable', group: 'variable' },
  { label: 'provider.name', insert: 'provider.name', detail: '展示名', type: 'variable', group: 'variable' },
  { label: 'provider.base_url', insert: 'provider.base_url', detail: 'API Base URL', type: 'variable', group: 'variable' },
  { label: 'provider.provider_type', insert: 'provider.provider_type', detail: '协议类型', type: 'variable', group: 'variable' },
  { label: 'provider.is_enabled', insert: 'provider.is_enabled', detail: '是否启用', type: 'variable', group: 'variable' },
  { label: 'auth', insert: 'auth', detail: '认证摘要 map（不含完整 secret）', type: 'variable', group: 'variable' },
  { label: 'auth.method', insert: 'auth.method', detail: '认证方法', type: 'variable', group: 'variable' },
  { label: 'auth.project_id', insert: 'auth.project_id', detail: '可选 project id', type: 'variable', group: 'variable' },
  { label: 'auth.managed_project_id', insert: 'auth.managed_project_id', detail: '可选 managed project id', type: 'variable', group: 'variable' },
  { label: 'auth.account_id', insert: 'auth.account_id', detail: '可选 account id', type: 'variable', group: 'variable' },
  { label: 'template', insert: 'template', detail: '当前模板元信息 map', type: 'variable', group: 'variable' },
  { label: 'template.id', insert: 'template.id', detail: '当前模板 ID', type: 'variable', group: 'variable' },
  { label: 'template.name', insert: 'template.name', detail: '当前模板名称', type: 'variable', group: 'variable' },
  { label: 'template.kind', insert: 'template.kind', detail: '模板类型', type: 'variable', group: 'variable' },
]

/** 系统函数（含字符串 / 数学） */
export const SCRIPT_FUNCTIONS: ScriptCatalogItem[] = [
  // HTTP
  {
    label: 'http::get',
    insert: 'http::get(${url}, ${headers})',
    detail: 'GET 请求，返回 #{ status, body, headers }',
    type: 'function',
    group: 'http',
  },
  {
    label: 'http::post',
    insert: 'http::post(${url}, ${body}, ${headers})',
    detail: 'POST 请求',
    type: 'function',
    group: 'http',
  },
  {
    label: 'http::request',
    insert: 'http::request(${method}, ${url})',
    detail: '通用 HTTP 请求',
    type: 'function',
    group: 'http',
  },
  {
    label: 'http::get_json',
    insert: 'http::get_json(${url})',
    detail: 'GET 并解析 JSON；非 2xx 抛错',
    type: 'function',
    group: 'http',
  },
  // JSON
  {
    label: 'json::parse',
    insert: 'json::parse(${text})',
    detail: '字符串 → 对象',
    type: 'function',
    group: 'json',
  },
  {
    label: 'json::stringify',
    insert: 'json::stringify(${value})',
    detail: '对象 → 字符串',
    type: 'function',
    group: 'json',
  },
  {
    label: 'json::stringify_pretty',
    insert: 'json::stringify_pretty(${value})',
    detail: '对象 → 美化 JSON 字符串',
    type: 'function',
    group: 'json',
  },
  // log / control
  {
    label: 'log::info',
    insert: 'log::info(${msg})',
    detail: '写入 info 日志（自动脱敏）',
    type: 'function',
    group: 'log',
  },
  {
    label: 'log::warn',
    insert: 'log::warn(${msg})',
    detail: '写入 warn 日志',
    type: 'function',
    group: 'log',
  },
  {
    label: 'log::error',
    insert: 'log::error(${msg})',
    detail: '写入 error 日志',
    type: 'function',
    group: 'log',
  },
  {
    label: 'error',
    insert: 'error(${msg})',
    detail: '中止执行并返回业务错误',
    type: 'function',
    group: 'util',
  },
  {
    label: 'url_join',
    insert: 'url_join(${base}, ${path})',
    detail: '安全拼接 URL 路径',
    type: 'function',
    group: 'util',
  },
  // string
  {
    label: 'str::contains',
    insert: 'str::contains(${text}, ${sub})',
    detail: '判断 text 是否包含 sub',
    type: 'function',
    group: 'string',
  },
  {
    label: 'str::replace',
    insert: 'str::replace(${text}, ${from}, ${to})',
    detail: '替换全部 from → to',
    type: 'function',
    group: 'string',
  },
  {
    label: 'str::starts_with',
    insert: 'str::starts_with(${text}, ${prefix})',
    detail: '是否以 prefix 开头',
    type: 'function',
    group: 'string',
  },
  {
    label: 'str::ends_with',
    insert: 'str::ends_with(${text}, ${suffix})',
    detail: '是否以 suffix 结尾',
    type: 'function',
    group: 'string',
  },
  {
    label: 'str::trim',
    insert: 'str::trim(${text})',
    detail: '去除首尾空白',
    type: 'function',
    group: 'string',
  },
  {
    label: 'str::to_lower',
    insert: 'str::to_lower(${text})',
    detail: '转小写',
    type: 'function',
    group: 'string',
  },
  {
    label: 'str::to_upper',
    insert: 'str::to_upper(${text})',
    detail: '转大写',
    type: 'function',
    group: 'string',
  },
  {
    label: 'str::len',
    insert: 'str::len(${text})',
    detail: '字符串长度',
    type: 'function',
    group: 'string',
  },
  {
    label: 'str::sub_string',
    insert: 'str::sub_string(${text}, ${start}, ${end})',
    detail: '截取子串 [start, end)',
    type: 'function',
    group: 'string',
  },
  // math
  {
    label: 'math::abs',
    insert: 'math::abs(${x})',
    detail: '绝对值',
    type: 'function',
    group: 'math',
  },
  {
    label: 'math::min',
    insert: 'math::min(${a}, ${b})',
    detail: '取较小值',
    type: 'function',
    group: 'math',
  },
  {
    label: 'math::max',
    insert: 'math::max(${a}, ${b})',
    detail: '取较大值',
    type: 'function',
    group: 'math',
  },
  {
    label: 'math::floor',
    insert: 'math::floor(${x})',
    detail: '向下取整',
    type: 'function',
    group: 'math',
  },
  {
    label: 'math::ceil',
    insert: 'math::ceil(${x})',
    detail: '向上取整',
    type: 'function',
    group: 'math',
  },
  {
    label: 'math::round',
    insert: 'math::round(${x})',
    detail: '四舍五入',
    type: 'function',
    group: 'math',
  },
  {
    label: 'math::sqrt',
    insert: 'math::sqrt(${x})',
    detail: '平方根',
    type: 'function',
    group: 'math',
  },
  {
    label: 'math::pow',
    insert: 'math::pow(${base}, ${exp})',
    detail: '幂运算 base^exp',
    type: 'function',
    group: 'math',
  },
]

/** 全部补全项 */
export const SCRIPT_COMPLETIONS: ScriptCatalogItem[] = [
  ...SCRIPT_VARIABLES,
  ...SCRIPT_FUNCTIONS,
]

/** 文档面板：变量列表 */
export const DOC_VARIABLES = SCRIPT_VARIABLES.map((v) => ({
  name: v.label,
  desc: v.detail,
}))

/** 文档面板：函数列表（含 insert 片段） */
export const DOC_FUNCTIONS = SCRIPT_FUNCTIONS.map((f) => ({
  name: f.label,
  desc: f.detail,
  insert: f.insert.includes('${')
    ? f.insert
        .replace(/\$\{(\w+)\}/g, (_, name: string) => name)
        .concat(f.type === 'function' && !f.insert.endsWith(';') ? '' : '')
    : f.insert,
  /** 点击插入时用更友好的默认片段 */
  insertText:
    f.group === 'http' && f.label === 'http::get'
      ? 'let resp = http::get(url, headers);\n'
      : f.group === 'http' && f.label === 'http::post'
        ? 'let resp = http::post(url, body, headers);\n'
        : f.group === 'http' && f.label === 'http::get_json'
          ? 'let data = http::get_json(url);\n'
          : f.group === 'json' && f.label === 'json::parse'
            ? 'let data = json::parse(resp.body);\n'
            : f.group === 'json' && f.label === 'json::stringify'
              ? 'let s = json::stringify(value);\n'
              : f.group === 'log'
                ? `${f.label}("debug");\n`
                : f.label === 'error'
                  ? 'error("失败原因");\n'
                  : f.label === 'url_join'
                    ? 'let url = url_join(provider.base_url, "/v1/user/balance");\n'
                    : `${f.insert.replace(/\$\{(\w+)\}/g, '$1')};\n`,
}))
