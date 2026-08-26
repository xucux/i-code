# 社区帖子外链分享（Community Post Share）设计提案

> 状态：**✅ 已完成（2026-08-26 三端开发与本地验收通过；线上部署步骤见 §10.1）**
> 关联：
> - `i-code-community-worker/doc/community.md` —— 社区 Worker 设计与现有端点约定
> - `i-code-community-worker/doc/points-checkin.md` —— 积分体系（`points_ledger` 流水表）
> - `src-tauri/src/modules/community/{types,client,service,commands}.rs` —— Rust 客户端
> - `src/modules/community/{types.ts,ui/*}`、`src/hooks/use-community.ts` —— 前端
> - `docs/proposals/script-template-marketplace.md` —— 提案文档格式参考
>
> 说明：分享功能横跨 Worker（D1 迁移 + 新端点 + 直链 HTML 渲染）、Rust 客户端（新 Command）、
> 前端（详情页分享弹窗 + 管理员分享 Tab）三层，与现有社区模块同构，不改变既有架构边界。

---

## 1. 背景与需求

### 1.1 需求（用户原话梳理）

| # | 需求 | 说明 |
|---|------|------|
| R1 | 帖子界面增加「分享」按钮 | 一键生成分享直链地址 |
| R2 | 创建外链分享表 | 建立 `帖子 id ↔ 外链 pid` 映射；默认最多访问 **1000 次**；发起时可设置次数，上限 **10000 次** |
| R3 | 直链访问文章时 Worker 直接返回 HTML | 尽可能贴近当前应用内 Markdown 渲染；主题固定**浅色**；超过次数返回「已超分享次数」页面，并**间隔一段时间后重定向**到 i-code GitHub 项目 |

### 1.2 现状梳理（改动前确认）

- 社区为两层架构：Tauri 主仓（React + Rust client，`reqwest` 访问 Worker）＋ `i-code-community-worker`（Cloudflare Worker + D1，独立 npm 子目录）。
- 帖子正文用 Markdown（标题 ≤ 80，正文 ≤ 5000），应用内渲染为 `CommunityMarkdownContent`（`marked` GFM + `breaks` + GitHub Alert + highlight.js）。
- 积分体系权威记录为 D1 `points_ledger` 流水表（`SUM(change)` 为累计积分），签到获得正分。
- Worker 现有路由约定：`/api/v1/*` 须过 `X-App-Token` + `X-User-Id` + UA/Referer 校验（`ua-check.ts`）；根路径 `/` 返回 HTML（GitHub 跳转）不经校验。

### 1.3 已确认决策（S1 ~ S4，2026-08-26）

| # | 决策点 | ✅ 已确认方案 | 对应章节 |
|---|--------|---------------|----------|
| S1 | 分享粒度 | **一帖可创建多个分享**（各自独立 `pid` 与访问配额），`pid` 为主键 | §3 |
| S2 | Worker Markdown 渲染 | **引入 `markdown-it`**（零依赖、轻量）＋ GitHub Alert 预处理 + 任务列表轻量支持 | §6 |
| S3 | 分享管理与积分 | 用户端**仅「发起分享」**；分享列表 / 撤销由**管理员界面**控制；**发起分享扣 100 积分**（对接 `points_ledger`） | §4、§5 |
| S4 | 发起权限 | **仅帖子作者本人**可发起分享（Worker 校验归属） | §4.1 |

### 1.4 非目标（本期不做）

| 非目标 | 说明 |
|--------|------|
| 分享评论 / 回复 | 仅帖子 |
| 分享链接限时 / 到期 | 仅按访问次数控制 |
| 撤销分享返还积分 | 只扣不返，积分是成本 |
| 分享数据落本地库 | 权威在 Worker D1（直链由 Worker 出 HTML，本地库里无法权威计数） |
| 用户端撤销自己的分享 | 按 S3 归管理员；如需开放作者撤销，作为后续小迭代（§10 演进） |

---

## 2. 总体架构

```text
┌──────────────────── i-code（Tauri）────────────────────┐
│  前端：详情页分享弹窗 / 管理员分享 Tab                    │
│    └ invokeCommand('community_*')                       │
│  Rust：commands.rs → service.rs → client.rs（reqwest）   │
└─────────────────────────┬──────────────────────────────┘
                          │ HTTPS（/api/v1，App Token + X-User-Id）
                          ▼
┌──────────────────── Cloudflare Worker ─────────────────┐
│  POST/GET/DELETE /api/v1/posts/:id/shares、/api/v1/shares/:pid  用户端点（作者校验 + 扣积分）
│  GET/DELETE   /api/v1/admin/shares、/api/v1/admin/shares/:pid  管理员端点（adminToken）
│  GET          /s/:pid                                 公共直链（免校验，返回 HTML）
│    ├─ 计数原子自增（views < max_views）
│    ├─ 未超限：markdown-it 渲染浅色 HTML
│    └─ 超限：返回超限页 + 5s 后重定向 GitHub
└─────────────────────────┬──────────────────────────────┘
                          ▼
                     D1（share_links + posts + points_ledger）
```

数据流（发起分享）：
`前端分享弹窗 → community_create_share_link → client POST /api/v1/posts/:id/shares → Worker 校验（存在/作者本人/积分≥100/限流）→ batch 写入 points_ledger(-100) + share_links → 返回 { pid, url, maxViews, ... } → 前端展示并可复制`

数据流（访问直链）：
`浏览器 GET https://community-beta.tenma.work/s/{pid} → Worker 顶层路由（免 UA/App Token 校验）→ SELECT share+post+author → 原子 UPDATE views+1 → 未超限渲染 HTML；超限渲染超限页（meta refresh 5s → https://github.com/xucux/i-code）`

---

## 3. Worker D1 迁移

新增迁移文件 `i-code-community-worker/migrations/0011_add_share_links.sql`：

```sql
-- ===== 帖子外链分享（2026-08-26 分享迭代）=====
--   pid        随机 8 位 base62 短码（PRIMARY KEY），对外直链 /s/{pid} 使用，不可枚举
--   post_id    分享的帖子（帖子删除时级联删除分享）
--   user_id    创建者（必须为帖子作者，Worker 校验）
--   max_views  访问配额上限：1 ~ 10000（默认 1000，缺省由插入方归一化）
--   views      已访问次数（每次直链成功访问 +1，原子 UPDATE 限条件防超发）
CREATE TABLE IF NOT EXISTS share_links (
  pid        TEXT PRIMARY KEY,
  post_id    INTEGER NOT NULL REFERENCES posts(post_id),
  user_id    TEXT NOT NULL REFERENCES users(user_id),
  max_views  INTEGER NOT NULL DEFAULT 1000,
  views      INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
-- 帖内分享列表（作者 / 管理员查看）
CREATE INDEX IF NOT EXISTS idx_share_links_post ON share_links (post_id, created_at DESC);
-- 按创建者查询（管理员用户视角，预留）
CREATE INDEX IF NOT EXISTS idx_share_links_user ON share_links (user_id, created_at DESC);
```

要点：

1. **计数原子性**：直链访问时用单条条件更新 `UPDATE share_links SET views = views + 1 WHERE pid = ? AND views < max_views`，SQLite 单语句天然原子，不依赖事务，超限请求不会覆盖写。
2. **帖子删除联动**：现有 `deleteMyPost` / `adminDeletePost` 的 batch 级联（当前含 `reports` + `replies` + `posts`）需**追加 `DELETE FROM share_links WHERE post_id = ?1`**，避免孤儿分享。
3. 迁移文件后需在 Worker 侧 `npm run db:migrate:remote`（与现有 0010 一致，D1 直接执行，无版本表登记逻辑）。

---

## 4. Worker REST API

### 4.1 用户端点（`/api/v1`，需 App Token + X-User-Id，封禁检查同现有，作者校验）

| 方法 | 路径 | 说明 | 校验 / 限流 |
|------|------|------|-------------|
| POST | `/api/v1/posts/:id/shares` | 发起分享 `{ maxViews?: number }`（默认 1000，1~10000） | 帖子存在；**作者本人**（`post.user_id === userId`，否则 403）；积分按档位充足（**S5**：≤1000 次 100 / ≤4000 次 200 / ≤10000 次 500，不足 400「积分不足」）；限流 `share:${userId}` **每 5 分钟 1 次**；成功**按档位扣积分**（§5） |
| GET | `/api/v1/posts/:id/shares` | 该帖已生成的分享列表（作者视角展示 / 复制），游标分页（`createdAt DESC`） | 帖子存在；作者本人；通用读限流 |

> 说明：**用户端不提供撤销**（决策 S3，管理归管理员 §4.2）；如需作者撤销作为后续小迭代。

响应统一 `{ code: 0, message: 'ok', data }`；`ShareLink` 结构见 §4.4。

`GET /posts/:id/shares` 返回结构：

```jsonc
{
  "items": [ { /* ShareLink（见 §4.4） */ } ],
  "nextCursor": "..." | null
}
```

### 4.2 管理员端点（`/api/v1/admin/*`，需 adminToken，沿用 D9 不限流）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/admin/shares?cursor=&limit=&postId=` | 全站分享列表（`postId` 可选过滤），游标分页；含帖子标题与作者摘要（`AdminShareItem`，见 §4.4） |
| DELETE | `/api/v1/admin/shares/:pid` | 撤销任意分享（不返还积分） |

前端管理员界面据此渲染「分享」Tab（§8.3）。

### 4.3 公共直链（`GET /s/:pid`，免校验）

- **路由位置**：与根路径 `/` 同层处理，置于 `fetch` 顶层（`/api/v1` 前缀检查之前），**不执行 `ua-check.ts` / App Token / X-User-Id / 封禁检查**——浏览器直接访问。
- **流程**：

```text
1) SELECT s.pid, s.max_views, s.views, p.post_id, p.title, p.content, p.created_at,
          u.nickname, u.avatar_index, p.section
     FROM share_links s JOIN posts p ON p.post_id = s.post_id
     JOIN users u ON u.user_id = p.user_id
     WHERE s.pid = ?
   └─ 无记录 → 返回 404 浅色 HTML（「分享链接不存在」）
2) 原子计数：UPDATE share_links SET views = views + 1 WHERE pid = ? AND views < max_views
   └─ changes == 0 → 已达配额 → 返回「已超出分享次数」浅色 HTML（§6.4）
3) 未超限 → 返回帖子全文 HTML（§6.3）
```

- **每次直达 404 / 超限页都不再计数**（配额已封顶时恒为超限页）。
- **重定向**：超限页以 `<meta http-equiv="refresh" content="5;url=https://github.com/xucux/i-code">` + 页面文案「将在 5 秒后跳转到 i-code 项目」实现（对齐现有根路径跳转目标，秒数定 **5s**）。
- **缓存头**：`Cache-Control: no-cache`（计数每次实时），`Content-Type: text/html; charset=utf-8`。

### 4.4 ShareLink DTO（Worker / Rust / TS 三端一致）

用户视角 `ShareLink`：

```jsonc
{
  "pid": "a1b2c3D4",          // 8 位 base62 短码
  "postId": 123,
  "maxViews": 1000,           // 配额
  "views": 0,                 // 已访问
  "createdAt": "2026-08-26 12:00:00",
  "url": "https://community-beta.tenma.work/s/a1b2c3D4"   // 组装好的直链（§4.5）
}
```

管理员视角 `AdminShareItem = ShareLink + { postTitle, author: { userId, nickname, avatarIndex } }`。

**pid 生成**：`crypto.getRandomValues` 取 8 位字符，字符集 `0-9A-Za-z`（62 个）；插入 `share_links` 时主键冲突则重试（最多 5 次，仍冲突返回 500）。

### 4.5 直链组装规则

- **url 由 Worker 拼装**：取当前请求的 `url.origin`（即客户端配置 `base_url` 的域名根，去掉 `/api/v1` 后再拼 `/s/{pid}`），与备用域名配置天然一致。
- 直链路径 = 固定 `/s/{pid}`。
- Rust 仅透传 Worker 返回的 `url`，不需要自行拼域名。

---

## 5. 积分对接（points_ledger）

**费用按访问配额档位收取（决策 S5，2026-08-26 追加）：**

| 档位（maxViews） | 费用 | 语义 |
|------------------|------|------|
| `1 ≤ n ≤ 1000`   | **100** 积分 | 默认档，默认 1000 次 |
| `1000 < n ≤ 4000` | **200** 积分 | 中档 |
| `4000 < n ≤ 10000` | **500** 积分 | 高档，上限 10000 |

- **扣分**：发起分享成功时在 `points_ledger` 写一条 `change = -费用, reason = 'share_link'`（沿用现有流水表，累计 = `SUM(change)`，`/users/me` 的 `points` 自动随之变化）。
- **余额校验**：Worker 在扣分前 `SELECT COALESCE(SUM(change), 0) AS points FROM points_ledger WHERE user_id = ?`；`< 费用` → `400「积分不足，发起分享需要 {费用} 积分」`（`code=400`）。
- **写入原子性**：余额校验通过后用 `env.DB.batch([ledger(-费用), share_links INSERT])` 同事务提交。
- **竞态说明**：余额校验与 batch 写入之间非强一致（D1 写非强一致，与现有积分设计一致），极端并发下可能略超扣；属社区低风险场景，文档明示即可，不做悲观锁。
- **费用函数**：Worker `shares.ts` 导出 `shareCostPoints(maxViews)`（与前端弹窗 `SHARE_TIERS` 双写保持一致）。
- **细则**：
  - 撤销分享**不返还**积分（决策 S3）；发起失败（非积分原因）不扣分。
  - 前端弹窗打开时调用 `GET /users/me`（命中现有 6h 资料缓存）展示**当前积分**；费用随输入的 maxViews 实时联动，点击「发起分享」前按当档费用判断余额，不足时禁用按钮并提示。

---

## 6. 直链 HTML 渲染（markdown-it，浅色）

### 6.1 依赖与配置

- Worker `package.json` 新增**运行时依赖** `markdown-it`（零依赖，构建产物 +40KB 级，Worker 包体无压力）。
- 渲染配置：`new MarkdownIt({ html: false, breaks: true, linkify: true })`，语义对齐前端 `communityMarked`（`breaks` 单换行=换行、GFM 表格/删除线为 markdown-it 内置）。
- **XSS 安全**：`html: false` 关闭原始 HTML 透传；标题 / 昵称 / 时间 / 板块等动态值一律 `escapeHtml` 后注入；外部链接 `target="_blank" rel="noopener noreferrer"`。

### 6.2 GFM 能力对齐

| 能力 | 方案 |
|------|------|
| 表格 / 删除线 / 链接 / 引用 | markdown-it 内置（GFM） |
| Git 任务列表 `- [x]` / `- [ ]` | 引入零依赖插件 `markdown-it-task-lists`（renderer 层输出 `<input type="checkbox">`，与 html:false 兼容） |
| GitHub Alert `> [!NOTE|TIP|IMPORTANT|WARNING|CAUTION]` | 预处理为自定义围栏 `\`~~~~icode-alert TYPE\``，由 `fence` 渲染规则输出提示块（浅色版配色，与现有渲染外观贴近） |
| 代码块 | 不做 highlight.js（重）；浅色 `pre` 背景 + 边框 + 等宽字体 + `overflow-x:auto`，行内 `code` 浅灰底；语言名仅作为 class 展示，不高亮 |
| 链接协议 | markdown-it 原生校验会直接拒绝 `javascript:` 等危险协议（实测不产出锚点）；另加链接渲染器白名单（https/mailto/ftp/#/相对路径）做双保险，非白名单协议降级为不可点击 |
| `<br>` 硬换行标记 | 编辑器换行按钮产出字面 `<br>`（`\n<br>\n`）；直链 `html:false` 下先经预处理统一替换为换行符，再由 `breaks:true` 渲染为真 `<br>`；兼容 `<br>` / `<br/>` / `<br />`（大小写不敏感），替换按行进行并**跳过 ` ``` ` 代码围栏**（代码块内 `<br>` 作为普通文本原样展示，与前端 marked 一致）；替换放在 Alert 围栏转换之后，避免打断引用块 `>` 前缀 |

### 6.3 正常分享页结构（浅色，单列卡片）

```html
<section class="post">                 <!-- 白卡片，max-width 720，居中 -->
  <h1 class="title">帖子标题</h1>      <!-- escapeHtml -->
  <div class="meta">
    <span class="avatar">emoji</span>  <!-- avatarIndex → 前端 emoji 映射前端有，Worker 内用本地小型 emoji 预设表（§6.5） -->
    <span>作者昵称</span>
    <span>板块徽标</span>
    <span>yyyy-MM-dd HH:mm</span>
  </div>
  <article class="markdown">{{ markdown-it 输出 }}</article>
  <footer>由 i-code 社区分享 · 剩余访问次数 N</footer>
</section>
```

- 标题：页面 `<title>{post.title} — i-code 社区分享</title>`。
- 底部版权行链接到 `https://github.com/xucux/i-code`。
- 内联 `<style>` 内置于返回的 HTML（浅色：`#fff` 背景 / `#24292f` 正文 / `#0969da` 链接 / `#f6f8fa` 代码块底，GitHub 浅色风格）。

### 6.4 超限 / 404 页结构（同套浅色样式，居中卡片）

- **404**：`「分享链接不存在或已撤销」` + 5s 后重定向 GitHub。
- **超限**：图标 + `「该分享的访问次数已用完」` + `「剩余次数已为 0，5 秒后跳转到 i-code 项目」` + meta refresh。

### 6.5 头像 emoji 映射

应用前端头像为本地 emoji 预设（`src/modules/community/avatars.ts`，约 1200 个）。Worker 内**不复制整表**：直链页仅需作者头像展示，Worker 内内置一份 **30 个常用 emoji 的小表**，按 `avatar_index % 30` 取用（与现有后端「只存 index、零存储」原则一致；与前端展示不完全一致属可接受偏差，文档明示）。

---

## 7. Rust 侧（`src-tauri/src/modules/community/`）

### 7.1 types.rs 新增

```rust
/// 发起分享输入（maxViews 缺省 1000，范围 1~10000）
pub struct ShareLinkInput { #[serde(default, skip_serializing_if="Option::is_none")] pub max_views: Option<i64> } // camelCase

/// 分享链接 DTO（与 Worker §4.4 对齐）
pub struct ShareLink { pid, post_id, max_views, views, created_at, url }

/// 帖内分享列表响应
pub struct ShareLinkListData { items: Vec<ShareLink>, next_cursor: Option<String> }

/// 管理员分享项
pub struct AdminShareItem { pid, post_id, max_views, views, created_at, url, post_title, author: UserBrief }
pub struct AdminShareListData { items: Vec<AdminShareItem>, next_cursor: Option<String> }
```

### 7.2 client.rs 新增

| 函数 | 对应端点 | 说明 |
|------|----------|------|
| `create_share_link(base_url, user_id, post_id, &ShareLinkInput)` | POST `/posts/:id/shares` | 返回 `ShareLink`（含 Worker 拼装的 `url`，透传） |
| `list_post_share_links(base_url, user_id, post_id, cursor, limit)` | GET `/posts/:id/shares` | 返回 `ShareLinkListData` |
| `admin_list_share_links(base_url, admin_token, cursor, limit, post_id)` | GET `/admin/shares` | `AdminShareListData` |
| `admin_revoke_share_link(base_url, admin_token, pid)` | DELETE `/admin/shares/:pid` | `()` |

> url 拼装由 Worker 按请求域根完成（§4.5），Rust 无需本地拼域名。

### 7.3 service.rs / commands.rs / main.rs

- service 校验：`maxViews` 缺省 1000、范围 1~10000（提前拦截，Worker 兜底）；门禁 `require_ready()` 同现有。
- 新增 Command（`snake_case`，全部注册进 `main.rs` `invoke_handler`）：

```text
community_create_share_link(post_id, input)      → ShareLink
community_list_post_share_links(post_id, cursor, limit) → ShareLinkListData
community_admin_get_share_links(admin_token, cursor, limit, post_id) → AdminShareListData
community_admin_revoke_share_link(admin_token, pid) → ()
```

- 列表 limit 复用 `MAX_LIST_LIMIT`（50）校验。

---

## 8. 前端（`src/modules/community/`）

### 8.1 types.ts

新增 `ShareLink`、`ShareLinkInput`、`ShareLinkListData`、`AdminShareItem`、`AdminShareListData`（与 Rust DTO 一一对应）。

### 8.2 hooks（use-community.ts 追加）

```ts
createCommunityShareLink(postId, maxViews)   // → ShareLink
getCommunityPostShareLinks(postId, cursor?)  // → ShareLinkListData
revokeCommunityShareLink(pid)
communityAdminGetShareLinks(adminToken, cursor?, limit?, postId?)
communityAdminRevokeShareLink(adminToken, pid)
```

### 8.3 详情页分享弹窗（新组件 `share-dialog.tsx`，仅作者可见）

- 入口：`post-detail.tsx` 帖头工具栏（返回 / 刷新 / 举报 一行）新增分享图标按钮，`post.author.userId === currentUserId` 才显示。
- 弹窗内容：
  1. **次数设置**：Number 输入（默认 1000，`1~10000`，超出提示）；下方展示**档位收费说明**与**本次按档位费用**（S5：≤1000 次 100 / ≤4000 次 200 / ≤10000 次 500，随输入实时联动），并注明撤销不返还。
  2. **当前积分**：打开时调用 `getCommunityProfile()`（`/users/me`，命中 6h 缓存）；积分 < 本次费用时禁用发起并提示。
  3. 点击「发起分享」：`createCommunityShareLink` → 成功后展示直链（Input 只读可全选）+「复制链接」按钮（成功 toast）；记录当前帖子分享列表刷新。
  4. **本帖已有分享列表**：`getCommunityPostShareLinks` 展示每条 `pid 摘要 / 次数 x / 剩余 y / 创建时间 / 复制按钮`；**不提供撤销**（S3：撤销归管理员）。
- 紧凑布局（900×700 窗口内按现有 Dialog 规范）。

### 8.4 管理员「分享」Tab（`community-admin.tsx` 新增）

- 与现有「用户 / 举报 / 帖子管理 / 站点治理」Tab 并列新增「分享」。
- 能力：全站分享列表（游标分页，按帖子过滤可选）+ 每行 `pid / 帖子标题 / 作者 / 次数 / 剩余 / 状态（正常|已用完）/ 创建时间` + **撤销**（confirm 后 `communityAdminRevokeShareLink`，注明不返还积分）。
- 已用完（`views >= maxViews`）的分享行给「已用完」徽标，便于清理。

### 8.5 i18n

新增 `community.share.*` 与 `community.admin.*`（分享相关）键，**zh-CN / en 双写**：

```text
share.title / share.maxViews / share.maxViewsInvalid / share.costHint(扣 100 积分)
share.insufficientPoints / share.create / share.created / share.copy / share.copied
share.linkList / share.remaining / share.usage / share.statusNormal / share.statusExhausted
admin.tabShares / admin.shares.revoke / admin.shares.revokeConfirm / admin.shares.notRefund
```

---

## 9. 安全与规范

| 项 | 约定 |
|----|------|
| XSS | `markdown-it html:false` + 所有动态值 `escapeHtml`；外部链接 `rel="noopener noreferrer"` |
| 授权 | 用户创建的分享：作者本人（403 兜底）；撤销：管理员（adminToken）|
| 配额防超发 | 原子条件更新（§3），不依赖事务 |
| pid 不可枚举 | 8 位 base62，空间 ≈ 2.2×10¹⁴；主键冲突重试 |
| 直链免校验 | `/s/:pid` 走浏览器，不做 UA/Referer/App Token 校验（对齐 `/` 根路径）；其余 `/api/v1/*` 维持现有校验 |
| 限流 | 发起分享每 5 分钟 1 次/用户；列表走通用读限流；管理员不限（D9）|
| 日志与隐私 | `user_id` / pid 不当敏感日志；不落帖子正文；重定向目标固定 GitHub 主库 |
| 联动 | 帖子删除（用户/管理员）级联删 `share_links`（§3）|
| 积分 | 只扣不返；`points_ledger` 为权威；`/users/me` 的 points 自动反映 |

### 9.1 Worker 代码结构（新增 / 修改文件）

```
i-code-community-worker/
├── migrations/0011_add_share_links.sql          # 新增
├── src/shares.ts                                # 新增：分享 CRUD + 计数 + HTML 渲染（markdown-it + 预处理 + 转义 + 样式）
├── src/index.ts                                 # 修改：注册新 URLPattern、用户/管理员端点路由、fetch 顶层 /s/:pid 分支、
│                                                #       deleteMyPost/adminDeletePost 级联、SHARE_COST_POINTS 常量
├── src/rate-limit.ts                            # 修改：RULES.createShare = { windowSeconds: 300, max: 1 }
└── package.json                                 # 新增依赖 markdown-it
```

---

## 10. 实施计划与验收

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 Worker | `0011_add_share_links.sql`；`shares.ts`（端点 + 计数 + markdown-it 渲染 + 超限/404 页）；`index.ts` 路由与级联；`rate-limit.ts` 规则；`npm run type-check` + `db:migrate:remote` | ✅ 已完成（迁移待线上执行） |
| P1 Rust | types/client/service/commands 四个命令（去掉用户撤销，S3 归管理员）；`main.rs` 注册；`cargo check` | ✅ 已完成 |
| P2 前端 | types.ts / use-community.ts / share-dialog.tsx / post-detail.tsx 按钮 / community-admin.tsx 分享 Tab / i18n 双写；`pnpm type-check` | ✅ 已完成 |
| P3 联调 | `pnpm tauri:dev` 验证：发起→复制→浏览器直链→计数→超限重定向；管理员撤销 | ⏳ 待 Worker 部署后执行 |

验收清单（标注「待部署」的条目依赖 Worker 迁移 / 部署后线上验证）：

- [x] 详情页仅作者可见分享按钮；弹窗可设 1~10000（默认 1000），档位费用随输入联动展示，积分不足禁用
- [x] 发起按档位扣分（S5：≤1000→100 / ≤4000→200 / ≤10000→500），余额不足 400 拦截，重复发起 429 限流（线上生效待部署）
- [x] 直链 `/s/{pid}` 浅色 HTML 渲染标题/作者/正文（表格/任务列表/Alert/代码块），XSS 防护经本地 smoke 验证（`html:false` + 协议白名单）
- [ ] 访问计数原子递增；超限返回超限页，5s 后重定向 GitHub；不存在的 pid 404（待部署）
- [ ] 作者可看本帖分享列表；管理员「分享」Tab 可筛选/撤销；撤销后直链 404 且不返分（待部署）
- [ ] 删除帖子后其全部分享直链失效（待部署）
- [x] 三端 `cargo check` / `pnpm type-check` / Worker `tsc --noEmit` 全部通过

### 10.1 实施完成情况（2026-08-26）

- **代码全部完成**：Worker（`0011_add_share_links.sql` + `shares.ts` + `index.ts` 路由 / 级联 / 常量 + `rate-limit.ts` 规则 + `markdown-it` / `markdown-it-task-lists` 依赖）、Rust（`community_create_share_link` / `community_list_post_share_links` / `community_admin_get_share_links` / `community_admin_revoke_share_link` 四命令已注册）、前端（分享弹窗 / 详情页按钮 / 管理员分享 Tab / i18n zh-CN·en 双写）。
- **档位收费（S5）已落地**：Worker `shareCostPoints(maxViews)` 与前端 `SHARE_TIERS` 双写一致。
- **本地验证**：三端类型检查全部通过；markdown-it 渲染关键路径（表格 / 任务列表 / Alert 围栏 / 代码块 / XSS 转义 / 链接协议过滤）经本地 smoke 脚本验证。
- **变更日志**：功能已写入 `CHANGELOG.md` 0.2.12。
- **待执行（线上）**：
  1. Worker：`npm run db:migrate:remote`（应用 `0011_add_share_links.sql`）→ `npm run deploy`；
  2. 桌面端：`pnpm run tauri:dev` 按 §10 待部署条目完成联调验证。

### 演进方向（后续可选）

- 用户端「撤销自己的分享」（S3 若需放开）；
- 分享配额调整（admin 改 maxViews）；
- 直链统计页（来源 / 图表）。

---

## 11. 决策记录（2026-08-26）

| # | 决策点 | ✅ 已确认方案 | 章节 |
|---|--------|---------------|------|
| S1 | 分享粒度 | 一帖可创建多个分享（pk=pid，各自配额） | §3 |
| S2 | Worker MD 渲染 | 引入 `markdown-it` + Alert 预处理 + 任务列表轻量支持 | §6 |
| S3 | 分享管理与积分 | 用户仅发起分享；管理（列表/撤销）归管理员界面；发起**按档位扣积分**（`points_ledger`，详见 S5，不返还） | §4、§5、§8.3/§8.4 |
| S4 | 发起权限 | 仅帖子作者本人 | §4.1 |
| S5 | 积分档位收费 | 按访问配额档位计费：**≤1000 次 100 积分 / 1000< n ≤4000 次 200 积分 / 4000< n ≤10000 次 500 积分**（`shareCostPoints(maxViews)`，前端双写联动展示） | §5 |