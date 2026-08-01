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
  group: 'variable' | 'http' | 'json' | 'log' | 'storage' | 'string' | 'math' | 'util'
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
  { label: 'variables', insert: 'variables', detail: '供应商扩展模板变量 map（按 key 取值）', type: 'variable', group: 'variable' },
  { label: 'variables["key"]', insert: 'variables["key"]', detail: '按 key 取模板变量值（如 variables["cookie"]）', type: 'variable', group: 'variable' },
  { label: 'template', insert: 'template', detail: '当前模板元信息 map', type: 'variable', group: 'variable' },
  { label: 'template.id', insert: 'template.id', detail: '当前模板 ID', type: 'variable', group: 'variable' },
  { label: 'template.name', insert: 'template.name', detail: '当前模板名称', type: 'variable', group: 'variable' },
  { label: 'template.kind', insert: 'template.kind', detail: '模板类型', type: 'variable', group: 'variable' },
  // proxy
  { label: 'proxy', insert: 'proxy', detail: '代理配置 map：provider_type/provider_url/global_enabled/global_type/global_url', type: 'variable', group: 'variable' },
  { label: 'proxy.provider_type', insert: 'proxy.provider_type', detail: '供应商代理策略："global"/"direct"/"socks"/"http"，未配置时不存在', type: 'variable', group: 'variable' },
  { label: 'proxy.provider_url', insert: 'proxy.provider_url', detail: '供应商代理 URL（仅 socks/http 时存在）', type: 'variable', group: 'variable' },
  { label: 'proxy.global_enabled', insert: 'proxy.global_enabled', detail: '全局代理开关是否启用', type: 'variable', group: 'variable' },
  { label: 'proxy.global_type', insert: 'proxy.global_type', detail: '全局代理策略："direct"/"system"/"http"/"socks"', type: 'variable', group: 'variable' },
  { label: 'proxy.global_url', insert: 'proxy.global_url', detail: '全局代理 URL（仅 http/socks 时存在）', type: 'variable', group: 'variable' },
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
  {
    label: 'http::set_proxy',
    insert: 'http::set_proxy(${proxy_url})',
    detail: '设置代理 URL，后续 http 请求都走该代理（如 "socks5://127.0.0.1:1080"）',
    type: 'function',
    group: 'http',
  },
  // Proxied HTTP
  {
    label: 'proxied_http::get',
    insert: 'proxied_http::get(${url}, ${headers})',
    detail: '自动走应用代理的 GET 请求（供应商代理 > 全局代理 > 直连）',
    type: 'function',
    group: 'http',
  },
  {
    label: 'proxied_http::post',
    insert: 'proxied_http::post(${url}, ${body}, ${headers})',
    detail: '自动走应用代理的 POST 请求',
    type: 'function',
    group: 'http',
  },
  {
    label: 'proxied_http::request',
    insert: 'proxied_http::request(${method}, ${url})',
    detail: '自动走应用代理的通用 HTTP 请求',
    type: 'function',
    group: 'http',
  },
  {
    label: 'proxied_http::get_json',
    insert: 'proxied_http::get_json(${url})',
    detail: '自动走应用代理的 GET 并解析 JSON；非 2xx 抛错',
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
  // storage（公共存储）
  {
    label: 'storage::get',
    insert: 'storage::get(${key})',
    detail: '读取公共存储中的值（key 不存在或已过期返回 ()）；存储于应用数据目录 script-storage.json，明文不脱敏',
    type: 'function',
    group: 'storage',
  },
  {
    label: 'storage::set',
    insert: 'storage::set(${key}, ${value})',
    detail: '写入公共存储（key 不存在新增，存在覆盖），立即落盘；所有脚本共享同一存储',
    type: 'function',
    group: 'storage',
  },
  {
    label: 'storage::set (TTL)',
    insert: 'storage::set(${key}, ${value}, ${ttlMs})',
    detail: '写入并设置过期时间（毫秒，须 > 0）；到期后 get/has/keys 自动清理',
    type: 'function',
    group: 'storage',
  },
  {
    label: 'storage::delete',
    insert: 'storage::delete(${key})',
    detail: '删除公共存储中的 key（幂等，key 不存在不报错），立即落盘',
    type: 'function',
    group: 'storage',
  },
  {
    label: 'storage::has',
    insert: 'storage::has(${key})',
    detail: 'key 是否存在（未过期）',
    type: 'function',
    group: 'storage',
  },
  {
    label: 'storage::keys',
    insert: 'storage::keys()',
    detail: '列出全部 key（不含保留键，已过期项自动清理）',
    type: 'function',
    group: 'storage',
  },
  {
    label: 'storage::clear',
    insert: 'storage::clear()',
    detail: '清空全部数据（含 TTL 记录）',
    type: 'function',
    group: 'storage',
  },
  {
    label: 'storage::incr',
    insert: 'storage::incr(${key}, ${delta})',
    detail: '原子自增/自减（整数）；key 不存在视为 0，返回新值；保留已有 TTL',
    type: 'function',
    group: 'storage',
  },
  {
    label: 'storage::set_ns',
    insert: 'storage::set_ns(${ns}, ${key}, ${value})',
    detail: '写入命名空间（内部 key = ns:key），不同模板可隔离同名 key',
    type: 'function',
    group: 'storage',
  },
  {
    label: 'storage::get_ns',
    insert: 'storage::get_ns(${ns}, ${key})',
    detail: '读取命名空间下的值（key 不存在或已过期返回 ()）',
    type: 'function',
    group: 'storage',
  },
  {
    label: 'storage::delete_ns',
    insert: 'storage::delete_ns(${ns}, ${key})',
    detail: '删除命名空间下的 key',
    type: 'function',
    group: 'storage',
  },
  {
    label: 'storage::keys_ns',
    insert: 'storage::keys_ns(${ns})',
    detail: '列出命名空间下全部 key（去掉 ns: 前缀）',
    type: 'function',
    group: 'storage',
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
          : f.group === 'http' && f.label === 'http::set_proxy'
            ? 'http::set_proxy("socks5://127.0.0.1:1080");\n'
            : f.group === 'http' && f.label === 'proxied_http::get'
              ? 'let resp = proxied_http::get(url, headers);\n'
              : f.group === 'http' && f.label === 'proxied_http::post'
                ? 'let resp = proxied_http::post(url, body, headers);\n'
                : f.group === 'http' && f.label === 'proxied_http::get_json'
                  ? 'let data = proxied_http::get_json(url);\n'
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
                            : f.group === 'storage' && f.label === 'storage::get'
                              ? 'let v = storage::get("key");\n'
                              : f.group === 'storage' && f.label === 'storage::set'
                                ? 'storage::set("key", value);\n'
                                : f.group === 'storage' && f.label === 'storage::set (TTL)'
                                  ? 'storage::set("key", value, 60000);\n'
                                  : f.group === 'storage' && f.label === 'storage::delete'
                                    ? 'storage::delete("key");\n'
                                    : f.group === 'storage' && f.label === 'storage::has'
                                      ? 'if storage::has("key") { }\n'
                                      : f.group === 'storage' && f.label === 'storage::incr'
                                        ? 'let n = storage::incr("counter", 1);\n'
                                        : f.group === 'storage' && f.label === 'storage::get_ns'
                                          ? 'let v = storage::get_ns("ns", "key");\n'
                                          : f.group === 'storage' && f.label === 'storage::set_ns'
                                            ? 'storage::set_ns("ns", "key", value);\n'
                                            : f.group === 'storage' && f.label === 'storage::keys_ns'
                                              ? 'let keys = storage::keys_ns("ns");\n'
                                              : `${f.insert.replace(/\$\{(\w+)\}/g, '$1')};\n`,
}))
