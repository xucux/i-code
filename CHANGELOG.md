# Changelog

## [0.0.16] - 2026-08-02

### 新增

- **供应商附加请求头编辑与管理**：供应商表单「高级」页签新增「附加请求头」编辑器，支持增删改请求头，网关转发到上游时注入且可覆盖默认头
  - 值支持模板变量：`${uuid()}`（每次请求随机）、`${uuid_by_day()}`（当天固定）、`${variables["key"]}`（供应商扩展变量）；`$SECRET` 引用原样保存、转发时自动解密
  - 新增 `gateway_provider_extra_headers_list` Command，编辑供应商时加载并回填现有请求头
  - 创建供应商时自动回填内置预设的默认附加请求头（如 OpenCode Zen Free 的 `Authorization: Bearer public` 与 `x-opencode-*` 系列）
- **OAuth Token 续期原地更新**：OAuth 续期成功时新 token 原地更新原 Secret 引用（保留 id），避免每次续期产生孤儿 Secret 记录
- **TLS 信任系统证书库**：reqwest 切换为 `rustls-tls-native-roots`，TLS 校验读取系统证书存储（Windows 证书库），支持信任用户安装的代理 MITM 根证书（如 Proxypin CA）

### 修复

- **附加请求头创建时丢失**：修复从内置预设创建供应商时 `defaultExtraHeaders` 未回填表单导致附加请求头被丢弃、网关无头可注入的问题
- **附加请求头无法编辑**：修复编辑供应商时 `UpdateProviderInput` 缺少 `extra_headers` 字段导致附加请求头保存被忽略的问题

### 变更

- `UpdateProviderInput` 新增三态 `extra_headers` 字段（传对象=全量替换、传 null=清空、不传=不修改）

## [0.0.15] - 2026-08-02

### 新增

- **可调时间窗口图表**：仪表盘新增 Token 消耗与请求次数两张概览图，右上角提供显示/隐藏切换（状态持久化到 localStorage）
  - 按时间窗口长度自动选择聚合粒度：≤2 小时用 30 秒桶、≤12 小时用 2 分钟桶、>12 小时用 5 分钟桶
  - 新增 `twoMinutes` / `fiveMinutes` 两种聚合粒度（`StatsGranularity` 扩展），实时聚合从 `model_call_logs` 明细表动态生成时间桶，空桶补 0 保持时间轴连续
  - 图表时间窗口与粒度选择逻辑抽离为 `chart-utils.ts`（`getWindowConfig` / `generateBuckets`），供网关趋势/流量/Token 图表复用
  - 模型曲线颜色基于主题 `--primary` 色相生成（±32° 范围内均分），饱和度与亮度沿渐变递增，并针对浅色/深色主题调整亮度区间，保证与主题协调且可读
- **仪表盘网关操作**：网关状态卡片新增一键启动/停止按钮（`useGatewayStatus.start/stop`），操作结果以 toast 提示；网关运行时显示接口文档弹窗入口
- **迷你面板数据窗口扩展**：数据统计窗口从最近 1 小时扩展为 24 小时，趋势图改用 `hourly` 粒度展示 Token 用量，并修复全 0 数据时 SVG 面积图除零导致平线异常的问题

### 变更

- 仪表盘图标统一应用 `text-muted-foreground` 颜色，弱化视觉层级

## [0.0.14] - 2026-08-01

### 新增

- **脚本公共存储（script-storage）**：所有脚本模板共享的键值存储，供额度脚本在多次执行之间读写状态与缓存
  - 新增 Host Functions：`storage::get / set / delete / keys / has / clear / incr / set_ns / get_ns / delete_ns / keys_ns`，同时提供扁平别名（`storage_get` / `storage_set` 等），编辑器侧边文档同步补充 storage 分组与示例
  - 存储文件位于应用数据目录 `script-storage.json`（与 `i-code.db` 同目录），文件不存在时后端自动创建；明文 JSON 存储、无需脱敏（与 Secret 体系区分，禁止写入 API Key / Token）
  - 值类型支持任意可 JSON 序列化的 Rhai 值（字符串 / 数字 / 布尔 / map / 数组）
  - **TTL 过期**：`storage::set(key, value, ttl_ms)` 可设置过期时间；读取时惰性清理，应用启动时批量清理过期项
  - **命名空间**：`set_ns / get_ns / delete_ns / keys_ns` 以 `ns:key` 前缀隔离不同模板的 key，避免冲突；`__ttl__` 为系统保留键禁止写入
  - **大小上限**：单值 ≤ 64 KiB，总量 ≤ 1 MiB，超出报错
  - **并发安全**：进程内全局单例 `Arc<Mutex<...>>` 跨脚本共享，写入采用「临时文件 + rename」原子落盘，避免并发写互相覆盖与写一半损坏
- **脚本存储 UI 浏览器**：脚本模板页新增「存储」入口按钮，打开 `ScriptStorageDialog` 可视化查看 / 新建 / 编辑 / 删除 / 清空全部条目，展示 key、值预览与剩余 TTL（可读化显示，支持设置过期时间）
- **备份集成**：创建备份时若存在 `script-storage.json` 则一并打包；恢复时随数据库一并还原（旧备份无此文件时自动跳过）


## [0.0.13] - 2026-08-01

### 新增

- **网关外部请求认证开关**：`gateway_settings` 表新增 `auth_enabled` 字段，可在 AI Gateway 设置页切换外部请求认证
  - `auth_enabled = true`（默认）：外部请求需携带有效 API Key（`gateway_auth_keys` 或默认 Gateway Key）
  - `auth_enabled = false`：开放模式，外部请求无需认证直接放行
  - 内部 CLI 始终豁免，不受此开关影响
- **网关通配地址 LAN 解析**：网关监听 `0.0.0.0` / `::` 时，自动解析本机可访问的 LAN 地址用于展示与复制
  - 新增 `gateway_list_local_ips` Command，枚举本机网卡 IPv4 地址并剔除 loopback 与 link-local
  - 新增 `useLocalIps` Hook，前端按 LAN 优先级（`192.168.0.0/24` > `192.168/16` > `172.16/12` > `10/8`）排序地址
  - 网关状态栏展示解析后的 LAN 地址，并显示「通配监听」提示徽章
- **网关接口文档弹窗**：网关页面新增 `GatewayApiDocsDialog`，列出网关对外暴露的接口清单（`/health`、`/readyz`、`/v1/models`、`/v1/chat/completions`、`/v1/messages`），支持选择本机地址拼装完整 URL 一键复制
- **请求头模板变量解析器**：新增 `header_variable_resolver` 模块，网关转发时解析供应商附加请求头中的模板占位符为运行时实际值
  - `${uuid()}`：每次请求生成新的 UUID v4
  - `${uuid_by_day()}`：基于当天日期生成确定性 UUID v5（全天不变，跨天自动变化）
  - `${variables["key"]}`：从供应商扩展变量中取值，与额度脚本 `variables` 共享键空间
  - 内置 OpenCode Zen Free 模板的附加请求头改用模板变量（`ses_${uuid_by_day()}` / `msg_${uuid()}`），并同步更新 User-Agent 对齐 opencode 1.18.3
- **xAI Grok Build OAuth 模式完善**：模型拉取与网关转发双路径支持 OAuth 专用请求头
  - OAuth 模式固定通过 `https://cli-chat-proxy.grok.com/v1/models` 拉取模型列表；API Key 模式走标准 OpenAI 兼容接口
  - 网关转发为 `XaiGrokOauth` 认证注入专用身份标识头（`X-XAI-Token-Auth`、`x-grok-client-version`、`User-Agent`），对齐官方 CLI chat-proxy 请求头格式

### 变更

- **日志迁移至 tracing 收尾**：将 `main.rs` 启动流程、OAuth 回调、代理决策、调度器等剩余 `log::` 调用全部迁移到 `tracing` 生态
  - 网关 tauri-plugin-log 输出器重构为 tracing 结构化日志输出器，网关日志改用结构化字段（`http.method` / `http.url` / `http.status_code` / `duration_ms` / `tokens` / `request_id` 等），便于检索与过滤

## [0.0.12] - 2026-07-31

### 新增

- **OAuth 认证支持与定时任务调度系统**：新增 scheduler 定时任务模块，实现 OAuth token 自动续期功能
  - 新增定时任务调度系统，每 10 分钟扫描快过期的 OAuth 供应商并自动刷新 token
  - 为 `providers` 表新增 `auth_expires_at` 和 `auth_method` 字段，优化认证信息查询效率
  - 实现供应商级附加请求头功能，支持自定义请求头配置与自动注入到网关转发请求
  - 新增 provider token 解密查看功能，支持前端安全查看明文认证配置
  - 扩展 `OAuth2TokenData` 结构，新增 `is_renewable` 字段用于判断续期能力
  - 新增模型路由 ID 复制功能，优化开发者体验
  - 新增 OpenCode Zen Free 内置供应商模板
  - 优化前后端认证流程，OAuth 续期成功后自动回调更新供应商对象并同步冗余字段

## [0.0.11] - 2026-07-30

### 新增

- **OAuth 授权流程完整实现**：全面重构 OAuth 授权体系，支持浏览器跳转授权、回调服务器注册表与强管理
  - 新增全局回调服务器注册表，管理 OAuth 本地回调服务生命周期，支持动态端口与固定端口（Claude 54545 / Codex 1455 / xAI 56121）
  - 新增回调服务器管理面板，展示活跃实例（端口、供应商、状态、启动时间），支持实时刷新与强制关闭
  - 新增重新授权确认流程，支持保留/清空历史授权信息，提供灵活的二次授权体验
  - 新增 GitHub Copilot 授权后自动拉取账户信息并展示
  - 新增端口占用检测与跨平台清理指引弹窗，支持自动复制命令
  - 新增清空供应商 OAuth token 接口与前端调用，支持手动清除失效授权
  - 优化授权状态展示，支持过期状态提示与账户信息展示
  - 新增 OAuth 授权日志与错误追踪，关键状态变化写入自研 logger
  - 新增 `callback_registry.rs` 全局内存注册表模块，追踪活跃回调服务器实例
- **OpenCode CLI Agent 配置管理**：CLI 管理页新增 Agent 配置弹窗，支持对 `opencode.json` 中的 agent 配置进行增删改查
  - 完整 Agent 配置表单：Agent ID、描述、模型、Prompt、权限、工具、高级选项 JSON
  - 左侧列表选择 + 右侧编辑的布局，支持新增和删除 Agent
  - 数据仅写入配置文件，不涉及数据库

### 修复

- **模型列表视图模式翻译 key 错误**：修复模型列表视图模式切换按钮的国际化 key 引用错误


## [0.0.10] - 2026-07-30

### 新增

- **供应商网络连通性检测**：供应商列表工具栏新增「检测」下拉按钮，支持直连/代理两种模式 ping 所有供应商 URL
  - 后端新增 `gateway_provider_ping` Command，逐个检测供应商并实时推送事件
  - 前端点击后立即弹出检测对话框，逐条接收 `provider:ping-result` 事件实时追加表格行
  - 检测完成后接收 `provider:ping-done` 汇总事件，展示成功/失败/总数
  - 每条结果写入自研 logger（source=system），便于日志页面按 system 来源筛选查看
  - 任意 HTTP 响应（含 4xx/5xx）视为可达；仅网络错误（超时、DNS 失败、连接拒绝）视为失败
- **供应商增删改事件广播**：供应商创建/更新/删除后通过 `provider:changed` 事件通知前端，列表自动刷新
- **全局代理配置保存校验**：设置页全局代理开启时，新增保存按钮替代失焦自动保存，避免误触发无效代理配置
  - 保存时写入脱敏代理日志到自研 logger（system 来源）

### 修复

- **翻译 key 不匹配与 i18n 命名空间误用**：修复部分组件翻译键名与命名空间引用不一致导致文案缺失的问题
- **额度快照过滤逻辑**：关闭额度监控后立即隐藏对应数据，不再展示过期的残留快照
- **虚拟路由 hook 渲染时序**：简化依赖管理，修复潜在渲染时序问题

### 变更

- **虚拟模型展示格式优化**：显示完整的 `供应商slug/模型ID` 路径，便于识别模型来源
- **虚拟模型表单布局重构**：拆分路由设置为独立 Tab 页，提升交互体验与表单可读性

## [0.0.9] - 2026-07-29

### 新增

- **日志框架迁移至 tracing**：从 `tauri-plugin-log` 迁移到 `tracing` 生态，统一日志基础设施，支持全链路追踪
  - 新增 `trace_id` 模块：生成与传播唯一追踪 ID（base32 编码），用于跨日志/调用记录关联
  - 新增 `TraceIdLayer`：将 trace_id 注入线程上下文，供转发器、调用记录等复用
  - 新增 `size_aware_appender`：大小感知的日志文件追加器，按文件大小滚动切分
  - 新增 `atomic_filter`：原子级别过滤器，支持运行时动态调整日志级别
  - 新增 `tracing_webview` 模块：将日志事件通过 `CONSOLE_LOG` 事件转发到 WebView 控制台
  - 集成 `tower-http` 的 `TraceLayer`：为 HTTP 网关请求自动注入 trace_id，实现请求级追踪
  - 前端新增 `registerConsoleLogForwarder` 监听器，在 `App` 组件中注册
  - 新增迁移方案文档 `docs/plan/log-migration-tracing.md`
- **额度脚本代理支持**：Rhai 脚本运行时新增代理配置能力，与全局/供应商代理策略对齐
  - 新增 `proxied_http` 模块：自动应用供应商与全局代理配置，支持 GET/POST/通用请求/JSON 解析
  - 新增 `http::set_proxy` host function：支持手动配置脚本代理 URL
  - 向脚本注入 `proxy` 系统变量，提供供应商与全局代理配置信息
  - 新增 3 个内置脚本 snippet 演示代理使用方式
  - 新增脱敏 URL 工具函数，避免代理 URL 中的认证信息泄露

### 变更

- **转发请求 ID 复用 trace_id**：网关 forwarder 复用 `TraceIdSpan` 注入的 trace_id 作为 `request_id`，使转发日志、调用记录与 tracing 日志的 tid 保持一致，便于全链路关联；兜底使用自动生成的 trace id 作为 fallback
- **脚本 host 白名单校验逻辑优化**：区分市场脚本与本地脚本，仅对公共市场脚本强制执行 host 白名单校验，本地脚本不做强制限制
  - 新增 `is_marketplace` 方法判断脚本来源
  - 新增 `enforce_host_whitelist` 参数控制是否强制校验
  - 统一市场脚本 `snippet_id` 前缀常量使用
  - 标记废弃的旧阻塞代理函数

## [0.0.8] - 2026-07-28

### 新增

- **脚本模板市场**：新增脚本模板市场模块，支持从公共 GitHub 仓库拉取模板列表、预览和一键应用为本地草稿
  - 后端新增 `script_template_marketplace_list` / `get_detail` / `apply` 三个 Command，包含缓存、校验与冲突处理
  - 前端新增 `ScriptTemplateMarketplaceDialog` 组件，支持筛选、搜索、预览和一键应用
  - 新增 `useScriptTemplateMarketplace` Hook，封装市场列表拉取与详情查询
  - 新增市场提案文档 `docs/proposals/script-template-marketplace.md`
- **Claude CLI 配置一键应用**：新增 `cli_apply_claude_config` 命令与服务实现，支持一键将当前供应商配置写入 Claude Code 的 `settings.json`
  - 支持自动同步网关/直连模式的 Base URL 与认证信息
  - 支持配置开关、模型映射、兜底模型等完整 CLI 选项
  - 保留原有保存功能的同时新增独立的应用配置入口
  - 新增 `ApplyClaudeConfigInput` / `ApplyClaudeConfigResult` DTO
- **Codex 模型映射**：Codex CLI 面板支持模型映射配置，UI 与后端逻辑调整对齐 Claude CLI 模式

### 修复

- **供应商表单 API Key 显示**：优化 API Key 显示与编辑体验，支持留空不修改原有密钥
- **额度监控展示**：修复额度监控展示逻辑，仅在非 `none` 模式下展示相关 UI

### 变更

- **模型映射编辑器重构**：将通用模型映射编辑能力抽离为公共组件 `ClaudeModelMapping`，供 Claude CLI 和 Codex 复用
- **代码编辑器自适应高度**：为 `CodeEditor` 组件新增 `autoHeight` 属性，实现基于内容的自适应高度；CLI 设置面板和脚本模板预览面板的编辑器高度样式同步调整
- **托盘额度菜单更新逻辑**：重构托盘额度菜单更新逻辑，提取公共函数 `update_tray_balance_items` 复用代码

## [0.0.7] - 2026-07-28

### 新增

- **供应商扩展模板变量**：供应商表单新增「扩展」Tab，支持管理 `key/value/isSecret/label` 的变量列表，运行时以 `variables["key"]` 注入额度脚本
  - 后端新增 `script_variables_json` 字段及迁移 `V002__provider_script_variables.sql`
  - 敏感变量值由 Secret 模块加密后存储为 `$SECRET:{uuid}$` 引用
  - 余额脚本上下文注入 `variables` map，可在脚本中读取扩展变量
  - 前端新增 `ScriptVariablesEditor` 组件，支持增删改、敏感开关、key 格式校验与重复校验
- **京东 JoyAgent 余额脚本 Snippet**：新增 `joyagent-balance` 内置脚本，查询可用积分、积分上限、已用积分、剩余百分比、优惠券/钱包/欠款金额及账户状态
- **字符串转换 Host Functions**：Rhai 脚本运行时新增 `str::to_float` / `str::to_int`（同时提供扁平别名 `str_to_float` / `str_to_int`），用于将接口返回的字符串数值转为数值类型

### 修复

- **供应商表单误提交**：`ScriptVariablesEditor` 中「添加变量」与「删除」按钮未声明 `type="button"`，默认触发表单提交导致弹窗意外关闭并保存；已显式指定 `type="button"`

### 变更

- **余额脚本鉴权方式调整**：内置 JoyAgent / 小米 MiMo 余额查询脚本从直接读取 `api_key` 改为通过 `variables["cookie"]` 读取扩展变量，Cookie 等动态凭证不再占用 API Key 字段，注释同步指引用户在扩展模板变量中配置 `cookie`

## [0.0.6] - 2026-07-27

### 新增

- **聊天提示词库**：ChatInput 工具栏新增提示词按钮，点击弹出提示词列表弹窗，支持选择后一键填入输入框
  - 提示词来源：用户配置目录下 `prompt/` 文件夹中的 `.md` 文件，标题取自首个 `# ` 行
  - 列表紧凑布局，标题超宽时自动横向滚动（跑马灯），每行右侧「应用」按钮
  - 超过 125000 字符自动截断并提示
  - 后端新增 `chat_prompt_list` / `chat_prompt_get` 两个 Command

### 修复

- **提示词弹窗无限请求**：`PromptPickerDialog` 中 `useTranslation` 返回的 `t` 每次渲染新建引用，导致 effect 依赖无限循环调用 `chat_prompt_list`；改用 `tRef` 稳定依赖数组

## [0.0.5] - 2026-07-27

### 修复

- **代理策略修正**：全局代理未启用时强制 `no_proxy()` 直连，不再回落到 reqwest 默认行为（读取系统 `HTTP_PROXY` / `HTTPS_PROXY` 环境变量），修复「系统设了代理环境变量但代理不可用时，直连可达的供应商也拉取/转发失败」的问题
- **模型拉取忽略代理配置**：`fetch_official_models` / `fetch_models_by_protocol` 改用 `build_provider_http_client`（含 `apply_provider_proxy`），修复此前使用裸 `reqwest::Client::new()` 导致供应商代理策略全部失效的问题
- **供应商无法切回全局代理**：前端 `provider-form.tsx` 始终序列化 `proxyJson`（含 `global` 模式），修复此前 `global` 时返回 `undefined` 被 Tauri invoke 省略导致后端跳过更新、DB 保留旧代理配置的问题
- **OAuth 代理不一致**：`oauth2.rs::new_for_provider` 改用 `apply_provider_proxy`，修复此前 `Global` 分支不应用全局代理的缺陷

### 变更

- **代理逻辑统一到 `shared` 层**：新增 `apply_provider_proxy`，供 `ai_gateway`（模型拉取 / OAuth）与 `gateway_runtime`（网关转发）共用，保证两条网络路径策略一致；详细设计见 [`docs/proxy.md`](docs/proxy.md)
- **代理日志增强**：代理决策全链路增加 `tauri-plugin-log` 的 `trace` / `error` 级别日志，含策略来源、最终决策、URL（脱敏认证信息）；`error` 日志输出完整 reqwest 错误链（含网络栈），便于排障

## [0.0.4] - 2026-07-26

### 新增

- 全局代理现在应用于所有出站网络请求，包括供应商 API 调用、额度脚本 HTTP 请求、更新检测等

### 修复

- 版本号更新脚本修复正则 `g` 标志导致静态版本引用未同步的问题

### 变更

- **全局代理配置重构**：代理类型从 `direct / custom / system / vscode` 简化为 `direct / system / http / socks`，移除已废弃的 `authorization`、`strictSSL` 字段，HTTP/SOCKS 代理 URL 支持直接包含认证信息（如 `http://user:pass@host:port`）
- **全局代理统一应用**：将 `apply_global_proxy` 从 `update_version` 模块提取到 `shared` 模块，供网关运行时、额度脚本 HTTP 调用、更新检测等所有出站请求复用；新增 `apply_global_proxy_blocking` 供同步阻塞客户端（Rhai 脚本）使用
- 设置页网络卡片 i18n 键名从扁平 `settings.network` 重构为嵌套 `settings.network.title`，全局代理描述文案同步更新

## [0.0.3] - 2026-07-26

### 新增

#### 额度监控脚本模块

- **数据库**：新增 `script_templates` 表，支持模板名称、slug、类型、状态（draft/active/disabled）、脚本正文、引擎、超时、host 白名单、试运行记录等字段
- **后端 CRUD**：完整 10 个 Command（`script_template_list`、`get`、`create`、`update`、`delete`、`set_status`、`test`、`list_active_for_select`、`list_snippets`、`list_refs`），全部注册在 `main.rs`
- **状态机**：`draft → active → disabled` 三态迁移，`publish`/`disable`/`revert_to_draft`，启用前校验脚本非空，删除时检查供应商引用
- **Rhai 运行时**：纯 Rust 脚本引擎，每请求新建 Engine + Scope 避免状态泄漏，`spawn_blocking` 避免阻塞 tokio
- **系统变量注入**：`api_key`（已解密）、`provider`（id/slug/name/base_url/type/is_enabled）、`auth`（method/project_id/account_id 白名单）、`now_ms`、`template`（id/name/kind）
- **Host Functions**：
  - `http.get/post/request/get_json` — 基于 reqwest，host 白名单校验、超时（默认 15s，上限 30s）、响应 body 2MB 上限
  - `json.parse/stringify/stringify_pretty` — 字符串与 Dynamic 互转
  - `log.info/warn/error` — 自研 logger，自动脱敏 API Key
  - `error(msg)` — 中止执行并转为业务错误
  - `str.contains/replace`、`url.join` — 工具函数
- **沙箱策略**：禁止文件/进程/环境变量访问，最大执行步数 100_000，脚本正文上限 64 KiB，并行脚本数信号量控制
- **Dynamic → BalanceSnapshot 映射**：校验返回结构合法性，`updatedAt` 缺省时自动补 `now_ms`
- **内置 Snippet**（6 个）：余额 GET + Bearer、返回 items 骨架、Bearer 请求头、小米 MiMo 按量计费、小米 MiMo TokenPlan、Grok Usage
- **BalanceMethod 扩展**：新增 `Script` 方法，`BalanceConfig::Script` 含 `scriptTemplateId`、`timeoutMs`、`allowedHosts`
- **额度刷新适配**：`dispatch_refresh` 中识别 Script 分支，加载 active 模板后执行脚本，失败返回明确错误信息
- **前端类型**：`ScriptTemplate`、`ScriptTemplateStatus`、`CreateScriptTemplateInput`、`UpdateScriptTemplateInput`、`ScriptTemplateTestResult`、`ScriptSnippet` 等完整 DTO
- **前端 Hooks**：`useScriptTemplateList`（含 kind/status/keyword 筛选）、`useActiveScriptTemplates`（仅 active 模板）、`useScriptSnippets`（内置 snippet 列表）、`useScriptTemplateMutation`（CRUD + 状态迁移 + 试运行 + 引用查询）
- **前端组件**：
  - `ScriptTemplateList` — 列表页，支持类型/状态/搜索筛选、紧凑 900×700 布局、空状态引导
  - `ScriptTemplateEditor` — 全功能编辑对话框，元数据字段、CodeMirror 编辑器（JS 高亮近似 Rhai）、系统全屏切换、状态徽章、发布/禁用/恢复草稿
  - `ScriptSidebarDocs` — 右侧文档面板，系统变量/函数/Snippet/返回结构/示例 五个 Tab，点击插入编辑器
  - `ScriptTestPanel` — 试运行面板，供应商 Select 下拉选择、执行结果展示（snapshot / 错误 / 耗时 / 日志）
  - `ScriptTemplateStatusBadge` — 状态徽章（草稿/启用/禁用）
  - `BalanceConfigForm` — 供应商额度配置表单下拉分组「内置」+「自定义脚本」，选择脚本模板后支持 timeoutMs 覆盖，模板禁用时显示警告
- **安全设计**：API Key 仅内存注入，禁止明文落库或写入日志，host 白名单默认跟随 `base_url`，日志脱敏，导出备份不含密钥

### 修复

- 修复全屏模式下 Select 弹出层被 Radix aria-hidden 阻断的问题：改用 `document.documentElement.requestFullscreen()` 全屏整个文档而非 DialogContent，确保 portal 渲染的弹出层可见 

### 变更

- 供应商额度监控支持使用自定义额度监控脚本
- 供应商列表优化额度信息展示


## [0.0.2] - 2026-07-26

### 新增

- 版本检测

### 修复

- 修复 CICD

## [0.0.1] - 2026-07-25


### 新增

#### AI Gateway 供应商管理
- 支持多供应商（OpenAI、Anthropic、Gemini、OpenRouter 等）的集中管理
- 供应商 CRUD：名称、Base URL、认证方式（API Key / Bearer Token / 自定义 Header）
- 供应商级别的代理配置（ProxyConfig）、重试策略（RetryConfig）、超时设置（TimeoutConfig）
- 模型 CRUD：每个供应商下可管理多个模型，支持模型 ID、名称、上下文窗口、最大输出等基本信息
- 认证方式多态类型，对齐参考项目 `vscode-unify-chat-provider` 的 well-known 数据类型

#### Secret 加密存储
- 基于 AES-GCM 的本地加密方案，API Key / Token 等敏感数据禁止明文落库
- 加密后以 `$SECRET:{uuid}$` 引用形式存入数据库和配置 JSON
- 明文仅在 Rust 后端处理，前端仅输入时接触一次，不缓存
- Secret 引用扫描与解析仅在后端完成

#### 本地 HTTP 网关运行时（部分实现）
- 基于 axum 的本地 HTTP 网关，默认监听 `127.0.0.1:54321`
- 模型路由 ID 格式：`{provider_slug}/{model_id}`
- 支持 `/v1/chat/completions`、`/v1/messages`、`/v1/models` 等接口
- SSE 流式响应原样透传，禁止二次包装
- 错误响应标准化为 OpenAI 格式 `{ error: { message, type, param, code } }`
- 网关认证中间件
- 响应拦截器异步写入 Logger 和 Call Records

#### 额度查询（Balance）
- 支持供应商级别额度查询
- 额度快照管理，支持时间范围查询
- 金额使用 string 类型避免浮点精度问题

#### 调用记录（Call Records）
- 模型调用统计与明细记录
- 聚合统计：按供应商、模型、时间维度
- 支持调用日志查看与筛选

#### 运行时日志（Logger）
- 自研内存环形缓冲区日志，可在应用内「日志」页面查看
- 同时集成 `tauri-plugin-log` 输出到终端和控制台
- 日志级别控制：全局设置影响 tauri-plugin-log，日志页面设置影响自研 logger
- 日志支持按来源、级别、时间筛选，可导出

#### 数据库备份与恢复
- 支持本地 SQLite 数据库备份与恢复
- 支持 WebDAV 远程备份
- 备份任务管理

#### 系统设置
- 主题设置：`light` / `dark` / `claude-light` / `claude-dark` / `deepseek-light` / `deepseek-dark`
- 国际化：`zh-CN` / `en` 双语支持（基于 i18next）
- 网关地址、日志级别等全局设置
- 所有颜色使用 CSS 变量，禁止硬编码

#### 内置数据
- 内置供应商与模型 seed 数据（`src-tauri/data/builtin-*.json`）
- well-known 数据转换脚本

### 技术栈

| 层级 | 技术 |
|------|------|
| 桌面 | Tauri 2.x（Rust + WebView） |
| 前端 | React 19 + TypeScript 5（严格模式） |
| 路由 | TanStack Router（文件系统路由） |
| UI | shadcn/ui + Tailwind CSS + Font Awesome |
| 状态 | Zustand（前端）+ Tauri State（后端） |
| 表单 | react-hook-form + zod |
| 国际化 | i18next（zh-CN / en） |
| 后端 HTTP 网关 | axum |
| 数据库 | rusqlite + r2d2（SQLite） |
| 加密 | AES-GCM（本地模式） |
| 类型同步 | ts-rs（Rust → TypeScript） |
| 包管理 | pnpm@11 |

### 部分实现 / 待迭代

- **网关运行时**：路由不完整，`/v1/responses` 等端点可能缺失
- **虚拟供应商**：策略枚举与文档可能不一致
- **CLI 管理**：目录与类型骨架存在，完整流程待实现
- **工作区管理**：Prompts/MCP/Skill 的编辑与应用流程待实现