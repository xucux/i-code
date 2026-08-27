---
editRules:
  - 禁止将i18n文件变更带入变更文档
  - 禁止将敏感信息带入变更文档
---

# Changelog

## [release-version-tempalte]

### 🚀新增

### 🐞修复

### 🔄变更

## [0.2.13] - 2026-08-27

### 🚀新增

- **帖子点赞**：帖子详情页新增点赞按钮（心形图标 + 计数），点击点赞、再次点击取消；作者不能给自己的帖子点赞（自己的帖子不展示点赞按钮）；帖子列表卡片同步展示点赞数与已赞状态（已赞帖子心形高亮）
- **点赞为作者加积分**：每次点赞为帖子作者 +1 积分并计入积分流水（可追溯）；取消点赞时同步扣回，防止反复点赞刷分
- **点赞防刷与级联清理**：点赞 / 取消点赞应用业务限流防连点刷分；帖子被删除时其全部点赞关系一并清理

### 🐞修复

### 🔄变更

## [0.2.12] - 2026-08-26

> [!IMPORTANT]
> 本版本社区新增外链分享；分享链接访问配额按档位消耗积分，发布前请知悉定价规则。

### 🚀新增

- **帖子外链分享**：帖子详情页新增「分享」按钮（仅作者本人可见），弹窗可设置访问次数上限（默认 1000 次，1~10000 次）并生成公网直链，一键复制；弹窗内展示当前积分与本次发起所需积分，余额不足时无法发起
- **分享按档位计费**：发起分享消耗的积分按次数档位收取——≤1000 次消耗 100 积分、1000~4000 次消耗 200 积分、4000~10000 次消耗 500 积分；撤销不返还积分，消耗计入积分流水便于追溯
- **直链浅色阅读页**：浏览器打开分享直链返回浅色主题的 Markdown 渲染页（标题 / 作者 / 板块 / 正文，支持表格、任务列表、任务勾选、GitHub 风格提示引用块等），访问计数原子递增；达到次数上限后展示「访问次数已用完/链接失效」页面并延时跳转到 i-code 项目
- **管理端分享管理**：社区管理员页面新增「分享」页签，可查看全站分享记录（发起人、帖子、次数用量、状态、直链，可按帖子筛选）并撤销任意分享（撤销后直链失效，积分不返还）
- **分享级联清理**：帖子被删除时，其关联的外链分享一并失效

### 🐞修复

### 🔄变更

## [0.2.11] - 2026-08-25

### 🚀新增

- **社区 Markdown 编辑器语法工具栏**：发帖正文与一级评论编辑框顶部新增语法工具栏，支持粗体、斜体、删除线、标题、引用、行内代码、代码块、链接、图片、任务列表、无序 / 有序列表、表格、分隔线、**换行**，在光标处或选中处插入对应 Markdown 语法
- **社区换行与空格渲染**：正文单换行直接渲染为换行（不再被折叠成空格），段落内多空格原样保留
- **社区代码块超长折叠**：帖子正文与一级评论中超过 12 行的代码块默认折叠，显示「展开全部 N 行 / 收起」按钮，点击可展开查看完整内容
- **社区代码块操作菜单**：渲染出的代码块右上角新增「⋯」菜单，点开提供**复制**（一键拷贝代码原文，临时绿色对勾反馈）与**自动换行**（软换行开关，联动菜单对勾指示）两项操作
- **社区代码块语法高亮**：代码块按语言高亮（java / html / css / shell / powershell / bash / xml / yaml / javascript / typescript / python / go / rust / c / cpp 等，未标注语言自动探测），并提供与软件亮 / 暗主题联动的两套配色
- **发帖弹窗全屏放大（系统全屏 + 分屏预览）**：发帖 / 回复弹窗右上角（close 按钮旁）新增全屏放大入口，使用系统级全屏（Fullscreen API，参考脚本编辑器 / 模型统计）放大整个弹窗至全屏，ESC / 还原按钮退出；全屏态下 Markdown 编辑器自动切换为「左编辑 / 右预览」左右分屏，工具栏横跨顶部
- **发帖标题帮助提示**：发帖标题右侧新增 helpicon 帮助图标，点击弹出字数限制说明（标题 80 字、正文 5000 字以内）

### 🐞修复

### 🔄变更

- **社区 Markdown 渲染器隔离**：社区帖子正文、一级评论、发帖与回复预览改用独立的社区渲染器，与软件内全局渲染器解耦，代码块折叠等社区特有能力不再影响更新日志等场景
- **编辑帖子弹窗升级**：「我的帖子」编辑与管理端编辑帖子弹窗对齐新建帖子弹窗——支持新 Markdown 编辑器（语法工具栏 / 编辑-预览 / 全屏分屏）、系统全屏放大、紧凑布局与标题帮助提示
- **发帖弹窗布局紧凑化**：缩小发帖弹窗内板块选择、标题、正文等元素的间距，整体更紧凑

## [0.2.10] - 2026-08-24

### 🚀新增

- **管理端举报治理**：举报列表每条新增「详情」入口，可跳转到目标帖子并在详情内编辑 / 删除帖子与任意回复；「处理」改为措施弹窗，支持对被举报人**封禁 / 禁言**（禁言可选时长或永久）、**修改内容**（帖子标题 / 正文 / 板块，或回复正文）、**忽略**，任一处置完成后自动将该举报标记为已处理

### 🐞修复

### 🔄变更

## [0.2.9] - 2026-08-23

### 🚀新增

- **社区消息通知**：新增消息通知体系——他人回复你的帖子 / 评论、管理员对你封禁 / 禁言时都会生成通知记录；社区首页「发帖」右侧新增消息铃铛按钮（无文字），存在未读通知时展示小红点，点击进入通知列表并自动将全部通知标记为已读，回复类通知可点击跳转到对应帖子
- **一级评论 Markdown**：帖子详情页一级评论（顶层评论）正文支持 Markdown 渲染；发表顶层评论 / 回复顶层评论改用支持 Markdown 编辑与预览的新回复弹窗，二级（楼中楼）评论保持纯文本并使用原回复弹窗
- **楼中楼 @昵称**：楼中楼回复若回复的是另一条二级评论，会在其作者昵称后追加显示 @目标昵称，便于理清回复对象

### 🐞修复

### 🔄变更

## [0.2.8] - 2026-08-21

### 🚀新增

- **社区签到排行**：社区右侧个人栏在「积分排行」下方新增「签到排行」入口，支持累计签到与连续签到两套排行（各自分页、前三名高亮、可加载更多 / 刷新）；封禁用户已过滤，禁言用户仍正常展示；连续签到按当前连续天数（今日或昨日仍连续）统计
- **预设模型扩充**：内置模型预设新增 7 个模型——智谱 `glm-5.3`、Meta `muse-spark-1.2-contributor`、通义 `qwen3.8-max` / `qwen3.8-27b`、谷歌 `gemini-3.7-flash` / `gemini-3.6-flash` / `gemini-3.5-flash-lite`

### 🐞修复

### 🔄变更

## [0.2.7] - 2026-08-19

### 🚀新增

### 🐞修复

### 🔄变更

- **Toast 位置与关闭**：系统 Toast 弹窗默认移动到上方居中展示，新增常驻右上角「关闭」角标
- **Toast 滑动删除**：toast 弹窗支持向左 / 右拖动删除
- **用户资料缓存**：社区「我的信息」缓存有效期由 15 秒延长至 6 小时，降低社区接口调用频率

## [0.2.6] - 2026-08-18

### 🚀新增

- **社区链接跳转**：社区 md 文章中的外部链接（http/https/mailto/tel）点击后弹出选择菜单，支持「在应用内打开」或「在浏览器打开」，避免链接跳转打断应用
- **站内专属浏览器窗口**：「在应用内打开」会新开一个专属浏览器窗口，顶部提供返回、刷新按钮与地址栏，下方以内嵌网页展示目标内容；目标站点禁止内嵌时可改用「在浏览器打开」兜底
- **全局右键菜单**：应用内所有界面接管右键，弹出自定义菜单，提供粘贴、复制选中文本、刷新界面、回到主页操作（顶替 WebView2 原生右键菜单；粘贴通过后端读取系统剪贴板，兼容 Windows / macOS / Linux 各平台）

### 🐞修复

### 🔄变更

## [0.2.5] - 2026-08-16

### 🚀新增

- **社区签到积分**：社区新增每日签到获取积分功能，每次签到随机获得 10~50 积分（服务端加密随机源），连续满 5 天额外奖励 200 积分；积分流水单独落表（`points_ledger`）便于追溯，用户信息接口同步返回累计积分，个人栏展示积分与连续签到天数
- **积分排行**：社区右侧个人栏新增「积分排行」入口，展示全站用户按累计积分降序的排行榜（前三名高亮，支持加载更多 / 刷新）；封禁用户已过滤，禁言用户仍正常展示

### 🐞修复

### 🔄变更

## [0.2.4] - 2026-08-15

### 🚀新增

- **用户禁言**：管理员可对用户设置指定时长或永久禁言（懒删除，到期自动失效，无需定时任务）；禁言期间仅可浏览、无法发帖 / 回复；禁言徽章在个人栏与帖子、评论的作者昵称旁展示，禁言原因可选展示给用户
- **管理员登录保护**：登录接口增加防爆破（按日历日累计凭证错误达阈值即永久封禁来源 IP，仅人工从数据库恢复）；登录成功后前端缓存会话 6 小时，避免频繁重复登录

### 🐞修复

### 🔄变更

- **管理员会话有效期调整**：服务端会话时长与前端缓存时长对齐并留有缓冲，避免会话提前失效导致重复登录

## [0.2.3] - 2026-08-15

> [!NOTE]
> 本次更新新增社区(beta)模块，默认关闭

### 🚀新增

- **社区模块完整功能**：新增社区入口导航与国际化配置，社区路由页与全量业务组件（帖子列表 / 详情 / 评论区 / 个人栏 / 发帖 / 点赞 / 签到 / 资料设置），后端新增社区服务、Rust Command 与类型定义，并引入设备身份（`MachineGuid` 加盐哈希）与 Worker REST 客户端
- **管理员帖子与回复管理**：管理员页面新增「帖子管理」Tab，支持所有用户的帖子分页列表（游标分页 + 板块过滤）、帖子编辑 / 删除；进入详情可查看帖子全文与评论区，并对任意回复进行行内编辑 / 删除（删除顶层评论级联楼中楼）
- **站点治理与帖子锁定**：管理员页面新增「站点治理」Tab，提供**全站禁言 / 禁止发帖 / 禁止评论回复**三开关（`muteAll` 优先级最高）+ 帖子级锁定（锁定后禁止新增评论回复、存量保留展示）；用户端在治理或锁定命中时禁用发帖 / 回复入口并展示提示，帖子列表与详情展示锁定状态
- **帖子列表缓存**：帖子列表新增 30 秒内板块切换缓存复用与缓存清空方法，发帖后刷新列表避免触发 API 限流
- **Markdown 组件泛化**：将 Markdown 渲染组件重构为通用全局组件，供社区与更新检查等模块复用
- **回复弹窗交互**：帖子详情以回复弹窗替换原底部固定输入区，优化评论交互体验

### 🐞修复

### 🔄变更

- **社区页面布局重构**：统一使用原生滚动替换 ScrollPage 组件，调整列表容器样式；重构社区路由配置，修正路由路径；调整弹窗、表单、按钮样式与尺寸，统一 UI 风格

## [0.2.2] - 2026-08-14

> [!IMPORTANT]
> 本次更新包含数据库结构变更（V007 虚拟路由迭代：健康检查元数据列、路由权重列、路由尝试历史表），升级后自动应用迁移。

### 🚀新增

- **虚拟路由字段补全**：`virtual_model_routes` 的 `timeout_ms` / `extra_headers_json` / `extra_body_json` 列真正可用；网关转发时合并路由级额外请求头与请求体（路由级覆盖供应商级）；表单支持单独禁用某条路由、配置超时与高级 JSON 编辑器
- **虚拟路由主动健康检查**：调度器新增 `health_check_loop`，每 60 秒对已降级或最近失败的路由发起轻量探活（GET /v1/models，5s 超时）；探活成功恢复健康，连续失败 3 次自动降级；路由列表展示连续失败次数与失败原因 badge
- **load_balance 策略实现**：新增 `RouteSelector` trait 与 `FallbackSelector` / `LoadBalanceSelector` / `OnAllSelector` 三个实现；`load_balance` 策略按 `weight` 字段加权随机选择 1 条健康路由；`on_all` 暂降级为 fallback 顺序尝试（并发实现留待后续迭代）；路由表单在 load_balance 策略下显示权重输入框
- **虚拟路由尝试历史落库与可视化**：新增 `virtual_route_attempts` 表记录每次路由尝试（成功/失败、状态码、耗时、错误原因）；`VirtualForwarder` 在每条路由尝试结束后异步写入历史；调度器每 24 小时自动清理 30 天前历史；虚拟供应商详情页新增「路由历史」Tab，展示路由维度统计（成功率/平均耗时/最近失败原因）与选中路由的最近 50 次尝试明细
- **虚拟模型图节点 Tooltip**：子级路由节点 hover 展示 Radix Tooltip，结构化显示优先级 / 权重 / 健康状态（颜色编码：健康=绿/不健康=红/禁用=灰/未知=主色）/ 上次探活时间 / 探活耗时 / 连续失败次数 / 失败原因
- **单条路由测试**：`RouteSettingsList` 每行新增「测试」按钮（fa-bolt 图标），调用新增 `virtual_provider_route_test` Command 对目标供应商发起轻量探活请求（GET /v1/models，5s 超时），结果用 toast 展示（HTTP 状态码 + 耗时 + 错误信息）；不写入 `virtual_route_attempts` 避免污染统计
- **alias 变更影响检查**：新增 `virtual_provider_check_alias_impact` Command，统计 `cli_model_mappings` 中 `gateway_model_id` 以旧别名为前缀的记录数；编辑虚拟供应商时别名变化后防抖 300ms 自动检查，有影响时在字段下方展示黄色警告提示
- **路由拖拽排序**：`RouteSettingsList` 引入 @dnd-kit（core + sortable + utilities，合计约 19KB gzip）实现拖拽排序；行首新增 grip 拖拽把手（仅把手响应拖拽，避免与行内输入框/开关冲突），拖拽结束后按新顺序自动重算 `priority`

### 🐞修复

- **虚拟模型路由图统计修复**：「N 条路由 · 已启用 N」改为仅统计子级真实路由，不再误把父级虚拟模型节点计入路由数

### 🔄变更

- 数据库 schema 升级至 V007；本次迭代三块 schema 变更（探活元数据列、路由权重列、路由尝试历史表）合并为单个迁移 `V007__virtual_route_iteration.sql`
- `VirtualForwarder::run` 路由解析入口由 `resolve_fallback_routes` 改为 `resolve_routes_by_strategy`，按虚拟供应商策略选择路由
- `SaveVirtualModelRouteInput` 新增 `timeoutMs` / `extraHeaders` / `extraBody` / `weight` 字段；`enabled` 由前端传入（原硬编码 true）

## [0.2.1] - 2026-08-13

### 🚀新增

- **日期选择类组件支持清空**：DatePicker / DateTimePicker / DateRangePicker / DateTimeRangePicker 新增清空按钮，仅在已有值时显示，点击后完全移除已选值并关闭弹窗
- **模型统计时间范围扩展**：新增「12 小时」与「今天」两个快捷选项，时间范围选择逻辑重构为基于 ID 匹配，避免数组引用变化导致定时刷新节奏被打乱
- **端口占用弹窗 Windows winnat 指引**：端口占用提示弹窗 Windows 部分新增第 3 步——重启 winnat 服务（`net stop winnat; net start winnat`），用于解决 Hyper-V / WSL 保留端口导致固定端口无法绑定的问题
- **网关启动失败复用端口帮助弹窗**：网关启动绑定端口失败（端口被占用 / 权限不足 / 地址不可用）时，复用 PortInUseDialog 展示分平台排查指引，而非仅展示一条失败 toast

### 🐞修复

- **供应商网络检测弹窗滚动布局修复**：移除外层 Radix ScrollArea（其注入的 `display: table` 包裹层导致固定高度失效），改用原生 div overflow-auto + min-h-0 + DialogContent max-h-[80vh]
- **网络检测错误信息截断与详情查看**：Ping 错误信息过长时默认截断（max-width 220px），点击展开 Popover 查看完整错误内容并支持复制
- **状态字段文字竖排修复**：调整列宽（w-20）并添加 whitespace-nowrap，防止表头与单元格文字垂直换行

### 🔄变更

- 版本号升级至 0.2.1
- PortInUseDialog 组件泛化：新增可选 `title` / `description` props，供 OAuth 授权与网关启动两个场景复用
- 供应商列表移除顶部「刷新」按钮，保留新增 / 导入 / 网络检测等操作

## [0.2.0] - 2026-08-10

### 🚀新增

### 🐞修复

- **WebDAV 备份路径拼接错位修复**：修复 WebDAV 备份在 `base_url` 自带 path（如 `https://dav.jianguoyun.com/dav/` 的 `/dav`）时，服务器返回的 href 与 `base_url` 同名前缀重复拼接（如 `/dav/dav/...`），导致 PUT / GET / DELETE 请求路径错位触发 409 等错误
  - 新增 `extract_path_from_url` / `href_to_remote_path` 路径归一化工具函数，将服务器返回的 href 转换为相对于 `base_url` 的路径（仅当剩余部分为空或以 `/` 开头时才去除前缀，避免 `/dav` 误匹配 `/dav-backup` 等场景）

### 🔄变更

## [0.1.10] - 2026-08-09

### 🚀新增

- **网关 Token 累计消耗图**：网关总览页在「Token 消耗」与「请求趋势」卡片之间新增「Token 累计消耗」卡片，展示最近 31 天（按天聚合）的 Token 消耗柱状图
  - 支持按模型 ID 筛选（默认全部模型，下拉选项来自统计期内实际出现过的模型）
  - 双色柱：亮色为当天总 Token 消耗，深色为当天缓存命中 Token 消耗；颜色随主题主色自适应明暗
  - 数据来自预聚合查询 `model_call_stats_daily`，随图表自动刷新周期更新

### 🐞修复

### 🔄变更

- **模型折线图配色升级**：Token 消耗图与请求趋势图的模型配色表前两位固定为「主题色」与「主题深色」（与 Token 累计消耗柱状图的「总消耗 / 缓存命中」一致），其余模型色围绕主题色相由外圈向内圈交替渐变，全站图表配色与主题主色统一

## [0.1.9] - 2026-08-09

> [!IMPORTANT]
> 数据库结构变更：`gateway_settings` 表新增 DeepSeek 思考修复配置列（V006 迁移，应用启动时自动执行，无需手动操作）。

### 🚀新增

- **DeepSeek 思考修复（网关侧）**：网关设置新增「DeepSeek 思考修复」开关，解决 DeepSeek V4 思考模式下多轮对话报错（上游要求 assistant 消息必须携带 `reasoning_content`，但模型有时仅返回 tool_calls 而不产生该字段，导致下一轮请求被 400 拒绝）
  - `gateway_settings` 表新增 `deepseek_thinking_fix` JSON 配置列（`enabled` / `keyword` / `matchMode`，默认关键字 `deepseek`、默认匹配模式 `contains`），网关设置页新增配置卡片：开关 + 模型匹配关键字输入 + 匹配模式下拉（`contains` / `equals` / `prefix` / `suffix`），附帮助气泡说明
  - 开启后，网关转发器在请求体预处理之后、协议桥接之前，为匹配模型的 assistant 消息补注入空 `reasoning_content` 兜底（忽略大小写匹配，仅对 Chat Completions 协议生效）
- **供应商网络检测新增「按供应商配置」模式**：供应商列表网络检测下拉新增第三种模式，按各供应商自身代理配置发起检测
  - `gateway_provider_ping` Command 支持 `config` 模式：按供应商 `proxy_json` 配置应用代理（未配置或 `global` 类型回退到全局代理，`direct` 类型直连，`socks` / `http` 类型使用各自代理），与既有「直连」「代理」模式并列

### 🐞修复

- **DeepSeek V4 思考模式多轮对话报错（聊天侧）**：聊天模块回传历史消息时，将助手消息的 thinking 思考内容作为 `reasoning_content` 字段一并回传。此前思考模式下缺少该字段会导致多轮对话被上游拒绝、对话中断

### 🔄变更

- **内置供应商列表排序调整**：内置供应商列表改为按 `sortOrder` 倒序排列（值越大越靠前），推荐供应商优先展示
- **网关认证界面调整**：API Key 管理卡片移除头部描述文字，卡片右侧新增网关启动后的访问地址，点击一键复制带 `/v1` 的完整 URL
- **网关访问地址记忆**：接口文档弹窗中最后选中的访问地址会持久化，网关配置状态卡片、仪表盘与 API Key 管理处的地址展示均优先跟随该选择；未选择时仍按排序规则展示

## [0.1.8] - 2026-08-08

### 🚀新增

- **聊天界面支持多网关协议选择**：会话支持 `Chat` / `Messages` / `Responses` 三种网关协议类型，可在聊天界面切换，适配不同厂商 API 格式
  - 后端按协议构造请求体：`Chat` 走 `POST /v1/chat/completions`、`Messages` 走 `POST /v1/messages`（Anthropic 原生）、`Responses` 走 `POST /v1/responses`（OpenAI Responses）
  - 流式与非流式响应按协议分别解析：`extract_stream_deltas` 统一按协议提取内容 / 思考 / usage / 结束标记；`message_stop` 与 `response.completed` 判定流结束，`response.completed` 同时携带完整 usage（先合并 usage 再判结束，避免丢用量）
- **聊天记录消息模型 ID 追踪**：助手消息与 `chat:stream-done` 事件新增 `model` 字段，存储实际生成该条消息的模型 ID（`provider_slug/model_id`），避免会话内切换模型后历史气泡被改写
- **单条聊天消息删除**：新增 `chat_message_delete` Command，从会话 JSONL 中移除指定消息并回写会话摘要计数与会话更新时间（此时无 parent / 子消息引用，采用纯 retain 全量删除，注释预留未来引用关系级联校验说明）
- **聊天记录 HTML 导出**：新增 `chat_export_html` / `chat_reveal_file` 后端命令，将会话导出为内联主题样式、可离线查看的 HTML 到配置目录 `exports/`（文件名安全化，禁止路径越界），并支持在系统文件浏览器中定位导出文件
  - 前端完善导出模板：项目 Logo、页脚与响应式布局；导出成功后展示结果弹窗，支持「打开文件所在文件夹」
  - 消息操作菜单重构为独立组件并内置到气泡内，助手消息支持复制、删除操作
- **供应商表单内置预设推荐标记**：从内置预设创建供应商时，协议类型与认证方式下拉中由预设支持的选项右侧显示「拇指」推荐图标（`fa-thumbs-up`），引导用户选择与预设匹配的配置
  - `builtin-providers.json` 新增 `providerTypes`（支持的协议类型列表）与 `authMethods` / `defaultAuth`（支持的认证方式与默认认证配置）字段，seed 逻辑与前端类型定义同步贯通
- **开机自启启动同步与优雅错误处理**：应用启动时静默核对 DB 中 `auto_start_enabled` 与系统实际注册状态，不一致时修正系统侧（覆盖软件更新后路径变更、注册项缺失等场景残留）
  - 关闭自启时若遇「系统找不到指定的文件（os error 2）」类错误，视为系统侧本就无注册项、与期望状态一致，静默处理不再报错
  - 启用失败时回滚 UI 与 DB 状态，前置避免状态不一致
- **发布流程脚本**：新增 `latest.json`（Tauri 自动更新）的 `notes` 更新脚本与 GitHub Actions workflow，支持手动指定或从 CHANGELOG 中提取对应版本章节作为 update notes（与 release body 同源，兼容 beta 预览版提示）

### 🐞修复

### 🔄变更


## [0.1.7] - 2026-08-07

### 🚀新增

- **按协议拉取官方模型**：供应商表单「拉取官方模型」按钮升级为分裂按钮（split button），主按钮执行默认拉取策略，右侧下拉菜单支持按指定协议拉取
  - 主按钮：沿用原有默认策略（按供应商 `provider_type` 自动匹配）
  - 下拉菜单：支持显式选择「OpenAI 兼容协议（`/models`）」或「Anthropic 原生协议（`/v1/models`）」拉取
  - 前端通过新增 `gateway_fetch_models_by_protocol` Command 调用，传入 `providerId` 与 `protocol` 参数

### 🐞修复

- **Gemini 模型拉取不兼容 OAuth**：`fetch_gemini_models` 此前仅通过 `?key=` 查询参数传递 API Key，既不兼容 OAuth Token 认证，也无法满足只认请求头的中转网关，导致部分供应商拉取返回 `401 API key is required in Authorization / x-api-key / x-goog-api-key header`
  - 重构为支持三种认证方式：
    - **API Key**：同时写入 `?key=` 查询参数与 `x-goog-api-key` 请求头，官方 `generativelanguage.googleapis.com` 与只认 header 的中转网关均可工作
    - **OAuth Token**（`google-gemini-oauth` 等）：写入 `Authorization: Bearer {token}`
    - **其他 OAuth 变体**：token 为 JSON（含 `accessToken`）时解析后同样作为 Bearer 写入
- **OpenAI 兼容协议模型拉取认证头单写**：`fetch_openai_compatible_models` 此前仅写入 `Authorization: Bearer {key}`，部分中转网关（Anthropic 风格 / Google Gemini 风格）无法识别。改为三通道写入，兼容官方 OpenAI 与各类中转网关：
  - `Authorization: Bearer {key}`：OpenAI 标准
  - `x-api-key`：Anthropic 风格 / 部分中转网关
  - `x-goog-api-key`：Google Gemini 风格 / 部分中转网关
  - 官方端点会忽略多余的请求头，无副作用
- **通配符监听时 LAN 地址列表缺失 loopback**：`useLocalIps` 在通配符监听（`0.0.0.0` / `::`）场景下返回的 `hosts` 列表仅包含 LAN IP，本机无法直接通过 `127.0.0.1` 访问网关。现于 LAN 地址列表末尾追加 `127.0.0.1`（若未包含），追加在排序之后确保始终位于列表最末尾，方便本机直接通过本地环回地址访问网关


## [0.1.6] - 2026-08-06

### 新增

- **模型调用统计展示总 token**：模型统计页面的统计描述新增「总 token」展示（随当前「明细 / 汇总」Tab 联动，取当前视图数据求和）
  - 新增 `formatTokenKMB` token 格式化工具函数，支持 K（千）/ M（百万）/ B（十亿）西式紧凑单位转换；非法输入兜底返回 `0`
  - `ModelList` 新增 `activeTab` 状态追踪当前激活 Tab，据此计算对应视图的 `totalTokens` 并参与统计描述 `totalTokens` 占位符渲染

### 修复

- **模型列表默认视图模式改为滚动模式**：`ModelList` 表格视图模式默认由 `compact`（自适应换行）改为 `scroll`（固定列宽横向滚动），避免列宽撑开导致布局溢出
- **日志 URL 与实际请求 URL 不一致**：重构 `build_log_url`（`forwarding/util.rs`），改为通过 `bridge_upstream_protocol` 计算桥接后的**上游协议**再选路径，与 `AnthropicClient` / `OpenAiChatClient` 内部 `build_upstream_url` 的入参保持一致，解决桥接场景下入口协议（如 `ChatCompletions`）与上游协议（如 `AnthropicMessages`）不同导致日志展示路径误导排查的问题
- **流式桥接转换函数调用方向错误**：修正 `forwarder` 流式桥接（`apply_stream_bridge`）中两种桥接模式下的 SSE 转换函数调用方向——`OpenaiToAnthropic`（入口 O → 上游 A）响应需转换为入口 OpenAI SSE（`anthropic_sse_to_openai`），`AnthropicToOpenai`（入口 A → 上游 O）响应需转换为入口 Anthropic SSE（`openai_sse_to_anthropic`），即响应转换方向与请求转换方向**相反**，与 `apply_response_bridge` / `convert_error_body` 保持一致；同步更新注释说明转换逻辑细节

### 变更

- **Anthropic Client 凭证双写 `Authorization` 头**：配置 `ApiKey` 时，除写入 `x-api-key` 外，同步写入 `Authorization: Bearer {key}`，兼容需要双重认证的中转网关（如小米 token-plan 等）；官方 Anthropic API 只认 `x-api-key`，多出的 `Authorization` 头会被忽略无副作用，`extra_headers` 仍可在最后覆盖 `x-api-key` / `Authorization` / `anthropic-version`

## [0.1.5] - 2026-08-06

### 新增

- **协议桥接模块（Anthropic ↔ OpenAI Chat 双向转换）**：当网关入口协议与上游供应商协议不一致时，在转发前对请求 / 响应 / 流式事件做双向转换，支持 Anthropic Messages 与 OpenAI Chat Completions 协议互转
  - 新增 `gateway_runtime/bridge` 模块：`BridgeKind` 枚举与 `detect_bridge` 触发判定、`anthropic_to_openai_chat` / `openai_chat_to_anthropic` 请求体转换、非流式响应体双向转换、流式事件状态机双向转换（容错：畸形 chunk 透传并告警）
  - 关键约束：`max_tokens` 缺失时从 `model_configs.max_output_tokens` 读取，兜底 `MAX_TOKENS_FALLBACK = 200000`；O→A 时移除 `response_format` 并在 `system` 末尾追加提示；工具调用 ID 原样透传不重命名
  - `GatewayProtocol::to_upstream_with_bridge` 按桥接判定返回上游 Client 实际协议；`forwarder` 在 `execute_and_finalize` / `apply_stream_bridge` 集成桥接，非桥接场景零开销
  - 方案文档：`docs/proposals/protocol-bridge.md`（P1–P4 全部落地，v1.0.0 released）
- **脚本模板变量依赖声明（`varList`）**：公共仓 `catalog.json` / `meta.json` 新增 `varList` 字段，显式声明脚本依赖的系统变量（`api_key` / `provider.base_url` 等）与供应商「扩展模板变量」（`variables["cookie"]` 等）
  - 后端 `marketplace/types.rs` 新增 `VarDef` / `VarSource`，`RemoteCatalogItem` 与 `MarketplaceItemSummary` 透传 `var_list`
  - 前端 `MarketplaceItemSummary` 新增 `varList`；脚本模板市场详情面板移除 tag 标签展示，改为渲染变量列表（变量名 / 来源徽章 system=蓝 custom=琥珀 / 必填徽章 / 描述），无变量时显示占位
- **供应商协议类型桥接帮助提示**：供应商表单「协议类型」标签新增 `HelpIcon` 帮助气泡（`popover` / `side=top`），说明协议桥接行为；i18n 补充 `providerTypeBridge` 帮助文案

### 变更

- **日志协议标签 `websocket` 更名为 `ws`**：`protocol_tags` / `detect_response_tags` / `LogEntry` 注释统一将 `websocket` 标签更名为 `ws`，前端日志筛选器与展示同步迁移
- **桥接转发新增 `bridge` 日志标签**：`protocol_tags` 签名新增 `BridgeKind` 参数，桥接转发额外标记 `bridge`，便于日志按桥接场景筛选


### 测试

- 协议桥接模块新增 13 个单元测试（`bridge/tests.rs`，共 1773 行）：覆盖请求 / 响应 / 流式转换规则、容错策略、工具调用 ID 透传等；全部 290 个测试通过

### 备注

- 方案文档：`docs/proposals/protocol-bridge.md`；`docs/gateway-runtime.md` 已同步桥接模块说明与实现状态


## [0.1.4] - 2026-08-05

### 新增

- **自研 logger 请求头展示（去敏）**：网关（inbound 入站请求头）与供应商 API（outbound 出站请求头，缺失时回退到入站头）日志展示请求头 JSON（敏感头值替换为 `***`），位于「模型 ID」下方一行，随导出写入 CSV / JSON
  - 新增 `request_headers_to_json` 去敏序列化（`logging/headers.rs`）：将 `HeaderMap` 序列化为有序 JSON；头名称（不区分大小写、子串匹配）命中 `authorization / api-key / token / secret / credential / cookie / auth` 任一敏感片段时值替换为 `***`；非 UTF-8 二进制值序列化为 `<binary>`；无请求头时返回 `None`
  - `UpstreamClient::execute` 签名改为 `&mut UpstreamContext`，各 Client（openai_chat / anthropic / openai_responses / websocket）在发送前对真实出站请求头做去敏快照写入 `UpstreamContext.request_headers_json`，供转发（provider-api）日志展示
  - 网关四个对外 handler（`chat` / `responses` / `messages` / `models`）捕获入站 `HeaderMap` 并透传到网关日志与转发日志
  - `LogRecord` / `LogEntry` 新增 `request_headers: Option<String>` 字段；CSV 导出新增 `requestHeaders` 列
  - 前端 `LogViewer` 以 JSON 缩进格式展示请求头，i18n（zh-CN / en / ja / zh-TW）补充 `requestHeaders` 文案
- **SSE chunk 专属日志文件（按小时滚动）**：SSE chunk 日志通过独立 target `i_code::sse` 写入单独按小时滚动的 `i-code-sse.*.log`（前缀 `i-code-sse`），不混入主日志文件，避免高频 chunk 刷屏
  - SSE 专属 fmt layer 使用 `TraceIdFormat::without_location()`，文件内不打印 target 与 file:line
  - 常规主日志过滤器（`MainLogFilter`）排除 `i_code::sse` target，确保 chunk 只进专属文件

### 变更

- **日志分段大小写入加固**（`SizeAwareFileAppender`）：写入超限时不只判断滚动，改为按 `max_size` 拆分 buffer——先写满当前文件、滚动到新分段后继续写剩余部分，保证每个分段不超过 `max_size`（修复单块大写入出现 ~40MB 分段的隐患）；使用 `create_new`（O_EXCL）独占创建分段并跳过已被占用的序号，从根上避免多进程 / 多次重启把同一分段追加撑大或序号复用

### 修复

- **复制的网关 API 地址缺少 `/v1` 路径**：网关 API 文档弹窗与网关首页的「复制 API 地址」按钮此前只复制 `http://host:port`，现统一补充 `/v1`，复制完整的 OpenAI 兼容基础地址


## [0.1.3] - 2026-08-04

### 新增

- **Grok Build（xAI 订阅）额度监控**：新增 `grok-build` 余额监控方法，调用 Grok CLI 内部 chat-proxy billing 端点查询周/月额度（`GET /billing?format=credits` / `GET /billing`）
  - 认证分支：`xai-grok-oauth`（OAuth 账号，免费档 / SuperGrok）走 `cli-chat-proxy.grok.com/v1/billing`；API Key 回退 `api.x.ai` 健康探测（`/v1/me` + `/v1/chat/completions`），仅提供可用性状态，无法给出精确剩余额度
  - 请求头对齐 `gateway_runtime/auth_resolver.rs` 的 xAI Grok OAuth 解析（`x-grok-client-version` / `xai-grok-cli` 等）；金额字段单位统一为分，输出转换为美元字符串传输，避免浮点精度问题
  - 方案文档：`docs/proposals/grok-build-billing-monitoring.md`

### 变更

- **余额查询统一走全局代理**：新增 `build_balance_http_client()` 统一构造额度查询 HTTP 客户端，复用应用全局代理配置（`shared::apply_global_proxy`，对齐 `docs/proxy.md`）；全部 12 个内置 balance provider（DeepSeek、Codex、Kimi、MiniMax、Grok Build、OpenRouter 等）从 `reqwest::Client::new()` 切换为统一构造，全局代理未启用时按规范强制直连（不读系统环境变量代理），连接超时 10s、总超时 30s
- **Grok Build 字段解析增强**：金额/对象/百分比字段候选键兼容 camelCase / snake_case（`creditUsagePercent` / `credit_usage_percent`、`currentPeriod` / `current_period`、`productUsage` / `product_usage`、`monthlyLimit` / `monthly_limit` 等）；新增「月度用量百分比」指标（`used / monthlyLimit × 100`，仅额度上限 > 0 时输出）
- **脚本模板重命名**：公益 Grok 监控 snippet 更名「公益Grok监控(第三方)」（id `grok-usage` → `grok-usage-thirdparty`），与官方 Grok Build 订阅监控区分
- **隐藏「测试数据」内置监控方法**：余额配置表单中的 `synthetic`（测试数据）选项不再展示

## [0.1.2] - 2026-08-04

### 新增

- **日语与繁体中文语言支持**：应用语言新增「日本語」「繁體中文」选项，i18n 模块、日期组件（date-fns / react-day-picker）、设置页面语言列表、后端 `Locale` 枚举同步扩展；翻译文件 `ja.json` / `zh-TW.json` 新增
- **转发重试事件双通道日志**：网关转发器（`forwarder`）的重试事件（首次请求、退避重试、状态码判定、重试耗尽、网络错误等）同时写入 `tracing`（tauri-plugin-log，终端/日志文件）和自研内存 logger（应用内「日志」页面可见），便于运行时诊断与开发调试

## [0.1.1] - 2026-08-03

### 新增

- **内置预设自动关联默认模型**：`builtin-providers.json` 新增 `defaultModels` 属性（`modelId` 实际发送给供应商、`matchModelId` 对应 `builtin-models.json` 中的模型 id、`displayName` 展示名），从内置预设创建供应商（如 OpenCode Zen Free、Cline Free）时自动按 `matchModelId` 匹配内置模型并创建 `model_config` + `gateway_model`（`source = builtin`），无需创建后再手动添加
  - `BuiltinProvider` / `CreateProviderInput` 两端新增 `defaultModels` 字段；后端匹配逻辑与前端 `findBuiltinByModelId` 一致（精确 > 前缀 > 包含回退），匹配不到时跳过该条目并告警，不阻断供应商创建

### 修复

- **供应商导出丢失附加请求头**：`gateway_provider_export` 此前将 `extra_headers` 硬编码为 `None` 直接丢弃。现改为读取 `provider_extra_headers` 表并写入导出数据（版本升至 `1.1`，旧 `1.0` 数据导入兼容）
  - 导出（带密钥）：`$SECRET` 引用解析为明文
  - 导出（不带密钥）：含 `$SECRET` 引用的条目跳过，避免导入后悬空引用导致转发失败；普通明文与模板变量占位符（`${uuid()}` 等）原样保留
  - 导入侧逻辑原已就绪（`import_provider` 会将 `extraHeaders` 写入 `provider_extra_headers` 表），本次补齐导出侧后闭环


## [0.1.0] - 2026-08-03

### 新增

- **网关支持 OpenAI Responses API（`POST /v1/responses`）**：本地网关新增对外端点与完整转发链路，支持 Agent 场景的 Responses 协议调用（SSE 事件流 / 非流式）
  - 新增 `responses` handler（`router.rs`），错误体沿用 OpenAI 标准 `{error:{message,type,param,code}}` 格式，自动纳入 API Key 认证与网关日志
  - `GatewayProtocol` / `UpstreamProtocol` 新增 `Responses` 变体，`openai-responses` 供应商类型从 WebSocket 占位改为真实可用
  - 新增 `OpenAiResponsesClient`（`client/openai_responses_client.rs`）：默认走 HTTP/SSE 透传（路径 `/responses`），认证复用 OpenAI 兼容解析（Bearer + extra headers）
  - usage 提取支持 Responses 格式：非流式兼容 `input_tokens` / `input_tokens_details.cached_tokens`；流式解析 `response.completed` 事件中的 `response.usage`
  - `estimate_prompt_tokens` 支持 Responses 请求体 `input` 字段（字符串 / item 数组），调用记录 token 估算不再缺失
- **Responses WebSocket 传输**：`openai-responses` 供应商配置 `transport = websocket` 时，网关以 WebSocket 连接上游（`wss://{base}/responses`），支持 `response.create` 事件与热事件流
  - 引入 `tokio-tungstenite` 依赖（`rustls-tls-native-roots`，与 reqwest 一致读取系统证书库）
  - `UpstreamResponse` 新增 `WebSocketStream` 变体：Client 将 WS 文本帧转换为 SSE 格式字节流（`data: {json}\n\n`），收到 `response.completed` / `response.failed` / `response.incomplete` / `error` 终止事件后发送 Close 帧
  - `response_handler` 重构 `build_sse_from_stream` 统一 SSE 构造入口，`Streaming` 与 `WebSocketStream` 共用字节流透传与 usage 拦截逻辑
- **供应商传输方式配置**：`CreateProviderInput` / `UpdateProviderInput` 新增 `transport` 字段（`auto` / `sse` / `websocket`）并贯通 repository 读写；供应商表单新增「传输方式」下拉（仅 `openai-responses` 协议显示），`auto` / `sse` 走 HTTP+SSE，`websocket` 走 WebSocket
- **网关接口文档更新**：`GatewayApiDocsDialog` 新增 `POST /v1/responses` 端点条目，i18n（zh-CN / en）同步补充


### 变更

- **日志协议标签修正**：`protocol_tags` 增加 `transport` 参数，仅当供应商显式配置 `transport = websocket` 时才打 `websocket` 标签，HTTP 透传场景统一标记为 `sse`，不再对 `openai-responses` 一律误标
- **WebSocket 客户端占位范围收窄**：`openai-responses` 已由真实实现接管，`WebSocketClient` 占位仅保留服务 `openai-codex` / `websocket` 类型

### 测试

- `usage_extractor` 新增 5 个单元测试：Responses 非流式 usage、Chat Completions 回归、Responses 流式 `response.completed`、中途事件不污染、缺失 usage 兜底

### 备注

- 有状态 API（`previous_response_id`、`GET /v1/responses/{id}` 等）仅透传上游，网关不维护本地会话状态；WebSocket 传输暂不应用代理配置
- 方案文档：`docs/proposals/responses-api-support.md`；`docs/gateway-runtime.md` 已同步路由清单、协议标签与实现状态

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