# 社区（Community）功能设计提案

> 状态：**已确认（2026-08-14 定稿）** —— §11 决策已全部拍板，可按 P0 开始实施
> 关联：
> - `docs/development.md` —— 模块架构与 Command 规范
> - `docs/error-handling.md` —— `IcodeError` 前后端转换
> - `docs/proxy.md` —— 全局代理两层配置
> - `docs/proposals/script-template-marketplace.md` —— 外部 HTTP 客户端参考（marketplace 模式）
> - `src/components/layout/app-layout.tsx` —— 侧栏导航
>
> 说明：社区是**独立于 Tauri 应用的第二代码库**（Cloudflare Worker + D1），应用内仅新增前端 UI 模块与 Rust 客户端模块，不改变现有业务架构。

---

## 1. 背景与目标

### 1.1 现状

| 能力 | 状态 |
|------|------|
| 侧栏导航（`NavItem[]` + TanStack Router） | ✅ 已实现 |
| 外部 HTTP 访问（Rust reqwest + 全局代理） | ✅ 已实现（marketplace / balance 等） |
| 全局设置持久化（`settings` 模块） | ✅ 已实现 |
| 机器唯一标识（machine-id / MAC） | ❌ 无 |
| 社区 / 帖子 / 评论能力 | ❌ 无 |
| Worker + D1 代码库 | ❌ 无（需新建） |

### 1.2 目标

| 编号 | 目标 | 说明 |
|------|------|------|
| G1 | 侧栏「社区」Tab | 新增独立路由入口，图标见 §5.1 |
| G2 | 首次使用门禁 | 首次点击高模糊 + 开关，开启后才能使用；关闭可再开 |
| G3 | 设备身份 | 无登录；以机器标识哈希作为唯一 `user_id` |
| G4 | 昵称 + 预设头像 | 仅需昵称即可注册；头像从预设 30 组中选择 |
| G5 | 帖子系统 | 帖子列表 / 详情 / 发帖 |
| G6 | 楼中楼回复 | 评论区支持楼中楼（本期限深度 2 层） |
| G7 | 个人栏 | 昵称修改、签到、我的帖子、我的回复 |
| G8 | 内容治理 | 举报、封禁、限流、字数限制（仅截稿，见 §6） |

### 1.3 非目标（本期不做）

| 非目标 | 说明 |
|--------|------|
| 账号体系（注册 / 登录 / 找回） | 设备即身份；设备重置 = 身份丢失，UI 明示 |
| 图片 / 附件上传 | 文本社区；头像为本地渲染的预设索引 |
| 私信、关注、点赞 / 收藏 | 社交能力二期再议 |
| 实时推送（WebSocket） | 列表轮询刷新即可 |
| 帖子编辑 / 删除 | 仅支持删除自己的帖子 / 回复（二期），本期不做编辑 |
| 多语言社区 | 单一中文社区 |
| 私有 / 邀请制社区 | 公网开放，靠治理手段约束 |

### 1.4 设计原则

1. **两层代码库**：`i-code-community-worker/` 独立 npm + wrangler 项目，与 Tauri 主仓解耦；主仓只消费其 REST API。
2. **网络走 Rust + 全局代理**：前端不直接 `fetch`，一律 `invoke → community client → reqwest（apply_global_proxy）`，与 marketplace 一致。
3. **头像零存储**：使用emoji作为头像预设，后端只存 `avatar_index`（emoji的索引）。
4. **楼中楼限深 2 层**：`帖子 → 顶层评论 → 一层楼中楼`，更深处平铺到第 2 层，防止 UI 爆炸。
5. **身份默认透明**：无登录的公网社区，机器哈希可被逆向伪造，属「防呆不防黑」；靠举报 / 封禁 / 限流兜底。
6. **设备指纹不落日志**：`user_id` 不写入任何日志（含自研 logger 与 tauri-plugin-log）。
7. **紧凑 UI**：900×700 窗口内按 `useAvailableHeight` + `ScrollPage` 规范实现滚动。

---

## 2. 总体架构

```text
┌─────────────────────── i-code（Tauri）────────────────────────┐
│  WebView（React）                                              │
│  src/modules/community/{types.ts, ui/}                        │
│    └─ invokeCommand('community_*')                            │
│                                                               │
│  Rust                                                        │
│  src-tauri/src/modules/community/                             │
│    commands.rs   → 参数校验 / 错误转换                        │
│    client.rs     → reqwest 调用 Worker REST API（全局代理）    │
│    types.rs      → DTO（serde camelCase）                     │
└──────────────────────────┬────────────────────────────────────┘
                           │ HTTPS（可配 base_url）
                           ▼
┌───────────────────────────────────────────────────────────────┐
│  Cloudflare Worker（i-code-community-worker/，独立 Git 仓库）    │
│  src/index.ts  → 路由 / 鉴权（身份头）/ 限流 / 治理            │
│  schema.sql    → D1 migrations                                │
└──────────────────────────┬────────────────────────────────────┘
                           ▼
                        Cloudflare D1（SQLite）
```

数据流（发帖）：
`前端表单 → community_create_post → Rust client → POST /api/v1/posts(X-User-Id: hash) → Worker 校验（封禁/限流/字数）→ D1 INSERT → 返回 post_id`

### 2.1 代码库布局

Worker 端是**独立 Git 仓库**（确认 D7）：`i-code-community-worker`（现阶段与主仓 i-code 分开，自有 `package.json` / `wrangler.toml` / migrations，不参与 `tauri:build`，也不共享 pnpm workspace）。

主仓（i-code）内仅新增前端与 Rust 客户端：

```
i-code/                              # Tauri 主仓
├── src/modules/community/          # 前端：types.ts + ui/
├── src/routes/community.tsx        # 主界面（列表 + 个人栏）
├── src/routes/community.post.$id.tsx  # 帖子详情
└── src-tauri/src/modules/community/ # 后端：commands.rs / client.rs / types.rs

i-code-community-worker/            # 独立 Git 仓库（Worker + D1）
├── package.json                    # wrangler 依赖
├── wrangler.toml                   # D1 binding；routes 指向 community-beta.tenma.work
├── src/index.ts                    # Worker 入口 + 路由 + 管理员鉴权
├── src/auth.ts                     # 身份头校验 / 封禁检查
├── src/admin.ts                    # 管理员登录（固定用户名/密码）+ 封禁接口
├── src/rate-limit.ts               # 简易滑动窗口限流（Durable Object 或 DB 计数）
├── src/censor.ts                   # 敏感词过滤（本地常量表）
└── migrations/0001_init.sql        # D1 初始 schema
```

---

## 3. 身份模型

### 3.1 方案对比（**需用户确认**）

| 方案 | 实现 | 优点 | 缺点 |
|------|------|------|------|
| A：机器标识哈希（推荐） | Rust 侧读 `MachineGuid`（Windows）/ 等效标识（macOS、Linux），加盐 SHA-256 → 64 hex `user_id` | 稳定、跨重启不变、不受多网卡影响 | 需新增 `machine-uid` crate；沙箱环境可能读不到需兜底 |
| B：主网卡 MAC 哈希（用户原方案） | 取主网卡 MAC，加盐 SHA-256 | 直观、实现简单 | MAC 不稳定（虚拟网卡 / 随机化 / 多网卡选择），身份可能漂移；隐私敏感度更高 |

**✅ 已确认（D1）：采用方案 A —— 机器标识哈希。**
> Rust 侧读 `MachineGuid`（Windows）/ 等效标识（macOS、Linux），拼应用标识常量加盐 SHA-256 → 64 hex `user_id`。
> 兜底：读取失败时生成本地随机 UUID 并持久化到设置表（`community.user_id_fallback`），保证身份稳定、跨重启不变。

### 3.2 身份规则

| 规则 | 说明 |
|------|------|
| 生成时机 | 首次开启门禁时在 Rust 端生成，前端只收 64 hex 字符串 |
| 传递方式 | 每次请求带 `X-User-Id` 头（Rust client 自动附加），前端不接触原始指纹 |
| 加盐 | 盐 = 应用标识常量 |
| 隐私 | `user_id` / 原始标识禁止写入任何日志；门禁页展示隐私说明 |
| 提示 | 重装系统 / 换机 = 新身份，UI 在门禁与个人栏均有提示 |

---

## 4. D1 数据模型

### 4.1 Schema 初稿（`migrations/0001_init.sql`）

```sql
-- 用户（懒注册：首次发帖 / 签到时 upsert）
CREATE TABLE users (
  user_id      TEXT PRIMARY KEY,              -- 机器哈希（64 hex）
  nickname     TEXT NOT NULL DEFAULT '用户',
  avatar_index INTEGER NOT NULL DEFAULT 0,    -- 0~29 本地预设
  banned       INTEGER NOT NULL DEFAULT 0,    -- 1 = 封禁
  ban_reason   TEXT,
  created_at   TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 帖子
CREATE TABLE posts (
  post_id     INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id     TEXT NOT NULL REFERENCES users(user_id),
  title       TEXT NOT NULL,                  -- ≤ 80 字
  content     TEXT NOT NULL,                  -- ≤ 5000 字
  reply_count INTEGER NOT NULL DEFAULT 0,
  created_at  TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_posts_created ON posts (created_at DESC);
CREATE INDEX idx_posts_user    ON posts (user_id);

-- 回复（楼中楼）：parent_reply_id NULL = 顶层评论
CREATE TABLE replies (
  reply_id        INTEGER PRIMARY KEY AUTOINCREMENT,
  post_id         INTEGER NOT NULL REFERENCES posts(post_id),
  user_id         TEXT NOT NULL REFERENCES users(user_id),
  parent_reply_id INTEGER REFERENCES replies(reply_id),  -- NULL = 顶层
  content         TEXT NOT NULL,                          -- ≤ 1000 字
  created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_replies_post ON replies (post_id, parent_reply_id, reply_id);
CREATE INDEX idx_replies_user ON replies (user_id);

-- 签到（UTC 日期按天唯一）
CREATE TABLE check_ins (
  user_id    TEXT NOT NULL,
  check_date TEXT NOT NULL,                 -- 'YYYY-MM-DD'（UTC）
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (user_id, check_date)
);

-- 举报
CREATE TABLE reports (
  report_id   INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id     TEXT NOT NULL,                -- 举报人
  target_type TEXT NOT NULL,                -- 'post' | 'reply'
  target_id   INTEGER NOT NULL,
  reason      TEXT,
  created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_reports_target ON reports (target_type, target_id);

-- IP 阻拦表（User-Agent / Referer 校验失败，阻拦 48 小时）
CREATE TABLE ip_blocklist (
  ip         TEXT PRIMARY KEY,                -- 来源 IP（CF-Connecting-IP）
  blocked_at TEXT NOT NULL DEFAULT (datetime('now')),
  reason     TEXT                             -- 阻拦原因：'bad_ua' | 'bad_referer'
);
CREATE INDEX idx_ip_blocklist_blocked ON ip_blocklist (blocked_at);
```

### 4.2 楼中楼取数策略

- 详情页返回：帖子 + 顶层评论（`parent_reply_id IS NULL`，分页）+ 每层下**最多 50 条**子回复（按时间升序，超出显示「加载更多」）。
- 子回复的父回复 ID 用于前端分组渲染；`depth > 2` 的回复在 Worker 写入时强制平铺为第 2 层。

---

## 5. Worker REST API

### 5.1 通用约定

| 项 | 约定 |
|----|------|
| 基础路径 | `https://community-beta.tenma.work/api/v1`（✅ 已确认 D2 自定义域名；`base_url` 仍保留在应用设置中便于后续切换） |
| 身份头 | `X-User-Id: <64hex>`（全部接口必带，Worker 校验格式） |
| 认证头 | `X-App-Token: <worker 下发>（Rust client 内置常量，防止外部脚本直接刷接口） |
| 请求头检查 | `User-Agent` 须为 `i-code/X.Y.Z`（匹配 `i-code/\d+\.\d+\.\d+`）；`Referer` 须为 `https://community-beta.tenma.work/` 或 `https://i-code-community-worker.xuux.workers.dev`。不满足的请求来源 IP 进入 48 小时阻拦表，见 §6 |
| 响应体 | `{ code: 0, message: 'ok', data: {...} }`；非 0 为业务错误，附可读 message |
| 时间 | 服务器 UTC，客户端本地化展示 |
| 错误 | 封禁 → 403；超字数 / 命中敏感词 → 400（附原因）；限流 → 429；请求头不合法 → 403（附原因）；IP 被阻拦 → 403（含剩余阻拦秒数） |

### 5.2 端点清单

| 方法 | 路径 | 说明 | 节流建议 |
|------|------|------|----------|
| GET | `/posts?cursor=&limit=` | 帖子列表（游标分页，`reply_count` 实时） | 通用限流 |
| POST | `/posts` | 发帖 `{ title, content }` | 每 5 分钟 1 帖 |
| GET | `/posts/:id` | 详情 + 顶层评论分页 + 楼中楼 | 通用限流 |
| POST | `/posts/:id/replies` | 回复 `{ content, parentReplyId? }` | 每 30 秒 1 条 |
| GET | `/users/me` | 资料 + 签到统计（总天数 / 连续天数）+ 发帖数 | 通用限流 |
| PUT | `/users/me` | 改昵称 / 头像 `{ nickname?, avatarIndex? }` | 每 5 分钟 1 次 |
| POST | `/users/me/check-in` | 签到（同 UTC 日重复 → 409 已签到） | 天然每天 1 次 |
| GET | `/users/me/posts?cursor=` | 我的帖子 | 通用限流 |
| GET | `/users/me/replies?cursor=` | 我的回复（含所在帖子标题） | 通用限流 |
| POST | `/reports` | 举报 `{ targetType, targetId, reason? }` | 每 10 分钟 3 次 |

**管理员端点**（✅ 已确认 D3，登录后发放短期 Admin Token，见 §5.3）：

| 方法 | 路径 | 说明 | 鉴权 |
|------|------|------|------|
| POST | `/admin/login` | `{ username, password }` → 校验 Worker 内固定凭据，返回 `adminToken` | 无（本身即登录） |
| GET | `/admin/users` | 用户列表（昵称 / 头像 / 封禁状态 / 发帖回复数） | Admin Token |
| POST | `/admin/users/:id/ban` | 封禁 `{ reason? }`（`users.banned=1`） | Admin Token |
| POST | `/admin/users/:id/unban` | 解封 | Admin Token |
| GET | `/admin/reports` | 举报列表（待处理优先） | Admin Token |
| POST | `/admin/reports/:id/resolve` | 处理举报（忽略 / 处置） | Admin Token |

### 5.3 管理员身份

- Worker 内保存**固定管理员用户名 / 密码**（`.dev.vars` / KV 密文，仅 Dashboard 可改），应用内**不存**、Rust client **不内置**管理员凭据（避免随包分发泄露）。
- 管理员页面每次使用需重新登录，凭据仅在 Worker 侧比对，客户端只持有短期 `adminToken`。
- 普通用户接口不感知管理员；封禁 / 解封仅管理员端点可操作。

---

## 6. 安全与治理

| 项 | 方案 |
|----|------|
| 身份伪造 | 边界说明：「机器哈希可被逆向，社区为信噪比治理而非实名」。二期可选 HMAC 请求签名（`X-App-Token` 参与），仍属防呆不防黑 |
| 请求头校验与 IP 阻拦 | 每次请求（`/api/v1/*`）检查 `User-Agent`（须匹配 `i-code/\d+\.\d+\.\d+`）与 `Referer`（须为 `https://community-beta.tenma.work/` 或 `https://i-code-community-worker.xuux.workers.dev`）。不满足的请求来源 IP 记入 `ip_blocklist` 表，**阻拦 48 小时**（自动清理过期条目）。阻拦期间返回 403 + 剩余阻拦秒数。根路径 `/` 返回 HTML 页面（GitHub 跳转），不执行上述检查 |
| 封禁 | `users.banned=1`：其读写接口一律 403。✅ 已确认 D3：由**管理员页面**（登录固定凭据）执行封禁 / 解封（§5.3），替代手工 Dashboard |
| 举报 | ✅ 已确认 D3：任意用户可举报帖子 / 回复（§5.2 `/reports`），管理员在 `/admin/reports` 处理 |
| 限流 | Cloudflare Rate Limiting（按 IP，公共接口 60 次 / 10 分钟；写接口更严）+ Worker 内业务节流（§5.2） |
| 内容过滤 | ✅ 已确认 D3：敏感词用 **Worker 内本地常量表**（`src/censor.ts`），发布 / 回复校验命中返回 400 不落库 |
| 防刷 | 签到按 `(user_id, UTC date)` 主键兜底；发帖频率限流 |
| 字数 | 标题 ≤ 80，正文 ≤ 5000，回复 ≤ 1000（前后端都校验） |
| CORS | 无需开启（仅 Rust 客户端访问；保守起见 Worker 可不发 CORS 头） |

---

## 7. Rust 侧设计（`src-tauri/src/modules/community/`）

### 7.1 命令清单（全部注册于 `main.rs`）

| 命令 | 用途 |
|------|------|
| `community_get_posts` | 帖子列表（cursor / limit） |
| `community_get_post` | 帖子详情 + 评论 |
| `community_create_post` | 发帖 |
| `community_create_reply` | 回复 / 楼中楼 |
| `community_get_profile` | 我的资料 + 签到统计 |
| `community_update_profile` | 改昵称 / 头像 |
| `community_check_in` | 签到 |
| `community_get_my_posts` / `community_get_my_replies` | 我的帖子 / 回复 |
| `community_report` | 举报 |
| `community_get_local_state` / `community_set_enabled` | 门禁开关与本地状态读写（不进 Worker） |

管理员（✅ D3，仅管理员页面调用；登录凭据由用户手动输入，见 §5.3）：

| 命令 | 用途 |
|------|------|
| `community_admin_login` | 管理员登录（用户输入固定用户名/密码，Worker 校验，返回 adminToken） |
| `community_admin_get_users` / `community_admin_ban` / `community_admin_unban` | 用户列表 / 封禁 / 解封 |
| `community_admin_get_reports` / `community_admin_resolve_report` | 举报列表 / 处理 |

参数：标量 snake_case；复杂对象用 DTO + `#[serde(rename_all = "camelCase")]`。错误统一 `IcodeError`（`TIMEOUT` / `BAD_GATEWAY` / 业务错误透传 `{code, message}`，禁暴露 SQL / 堆栈）。

### 7.2 client.rs 要点

- `base_url` 读取自 `app_settings.community.base_url`，`http://` 兜底 `https://`；
- 超时 15s（读）/ 10s（写），复用 `shared::apply_global_proxy`；
- `X-User-Id` 从本地状态注入；`X-App-Token` 编译期常量；
- 响应先验 `code`，非 0 转 `IcodeError::gateway`，`429` → 转「操作过于频繁」toast 文案。

### 7.3 本地状态存储

存入 `app_settings` JSON 新增 `community` 对象（沿用 settings 模块，不建新表）：

```jsonc
{
  "enabled": false,            // 门禁开关
  "baseUrl": "https://...",    // Worker 地址（可自定义域名）
  "userId": null,              // 64 hex；null = 未生成
  "nickname": null,            // 本地缓存，启动时以 /users/me 为准
  "avatarIndex": null
}
```

---

## 8. 前端设计（`src/modules/community/` + `src/routes/`）

### 8.1 路由与侧栏

```
/community            主界面：左帖子列表 + 右个人栏
/community/posts/:id  帖子详情（评论区楼中楼）
/community/admin      管理员页面（✅ D3，登录固定凭据后可封禁/解封/处理举报）
```

侧栏（`app-layout.tsx` `mainNavItems`）新增：

```ts
{ to: '/community', icon: 'fa-solid fa-users', labelKey: 'nav.community' }
```

✅ **D8：未开启门禁时侧栏不做任何角标 / 红点 / 提示**——仅于点击进入后展示模糊门禁页。

i18n 键：`nav.community` + `community.*`（zh-CN / en 同步）。

### 8.2 门禁页（首次访问）

```
┌──────────────────────────────────────────────┐
│            （整页 backdrop-blur-lg）           │
│  社区公测中：匿名设备身份、昵称与预设头像      │
│  [ 我已了解，开启社区 ]（Switch 确认）          │
│  关闭后仍可再次开启；身份与本机绑定无法找回    │
└──────────────────────────────────────────────┘
```

- 状态：`community.enabled === false` 时整页模糊 + 居中卡片 + Switch（✅ D8：侧栏无任何提示，仅点进来见本页）；
- 开启后若 `userId` 为空 → Rust 生成；若 `nickname` 为空 → 引导弹层（昵称输入 + 30 头像九宫格预览）；
- 个人栏提供「关闭社区」入口（回模糊态，可再开）。

### 8.3 主界面布局（900×700 紧凑）

| 区域 | 内容 | 滚动 |
|------|------|------|
| 左列（~62%） | 帖子列表：标题 + 简介 + 昵称/头像 + 回复数 + 时间；顶部工具条（刷新 / 发帖按钮） | `ScrollPage`（复用 `useAvailableHeight`） |
| 右列（~38%） | 个人卡：头像 + 昵称 + 编辑（弹层）；签到按钮（✅ D5：纯计数 + 连续天数，今日已签置灰）；入口：发帖 / 我的帖子 / 我的回复 / 关闭社区 | 随内容 |

### 8.4 帖子详情页

```
帖子头：标题 + 作者（头像/昵称/时间）+ 正文
工具栏：刷新 / 返回
评论区：
  └─ 顶层评论（分页 20/页）
      └─ 楼中楼（缩进 + 左竖线，最多 50 / 加载更多）
底部：回复输入框（回复顶层或某人 → 楼中楼）
```

- 楼中楼深度 2 层：UI 通过 `parentReplyId` 分组渲染，深度 ≥2 平铺到第 2 层；
- 评论作者旁显示「楼主」Badge（作者 == 发帖人）；
- 个人栏与详情页共用 `use-community-*` hooks（内部 `invokeCommand` + toast，不散落 invoke）。

### 8.5 头像预设（✅ D6：本地 emoji 预设索引，零存储）

- 使用 emoji 作为头像，**后端只存 `avatar_index`（0~29）**，客户端本地渲染对应 emoji；
- 零上传、零 CDN / R2、零审查负担；选择器为 5×6 网格缩略图。
- 详见 §7.3 本地状态 `avatarIndex` 与 D1 `users.avatar_index`。

---

## 9. 与现有规范的对齐

| 规范 | 落点 |
|------|------|
| 模块同名对应 | `src/modules/community/` ↔ `src-tauri/src/modules/community/` |
| 命令 snake_case + 注册 | §7.1 清单，`main.rs` `invoke_handler` |
| 异常体系 | `IcodeError`；前端 `toIcodeError` + toast，禁止 `String(e)` |
| 敏感数据 | 设备指纹 / `user_id` 不进日志；导出配置不含 |
| i18n | 全部 UI 串 zh-CN / en 双写 |
| 滚动布局 | `useAvailableHeight` + `ScrollPage`，禁止 `h-full` 猜测 |
| 日志 | 社区模块自身用 `log::info!`（dev 调试）；**业务日志不记录用户内容**（帖子正文含隐私，仅记录「发帖成功」等无内容事件）|

---

## 10. 分期计划与工作量

| 阶段 | 内容 | 估时 |
|------|------|------|
| P0 骨架 | 侧栏 + 路由 + 门禁模糊页 + 开关状态 + 机器 ID 生成 + 昵称/头像设置弹层 + i18n | 2~3 天 |
| P1 核心 | `i-code-community-worker/`（schema + 全部端点 + 限流/过滤 + 管理员接口）+ Rust client + 帖子列表/详情 + 楼中楼 + 右侧个人栏 + 签到 | 5~8 天 |
| P2 治理优化 | 管理员页面打磨（举报处理 / 封禁 / 解封）+ 高频操作节流细化 + 帖子删除 + 自定义域名切换收尾 | 2~3 天 |

**合计：MVP（P0+P1）约 7~11 个工作日；含 P2 约 10~14 个工作日。**

风险项：

| 风险 | 影响 | 缓解 |
|------|------|------|
| 社区域名（`community-beta.tenma.work`）直连受限 / 国内网络波动 | 社区不可用 | 复用全局代理（默认开启）；`base_url` 保留可配以便切换备用域名 |
| 机器标识读取失败（沙箱/虚拟化） | 身份漂移 | 本地 UUID 兜底（§3.1） |
| 恶意刷量与灌水 | 社区质量劣化 | CF 限流 + 业务节流 + 封禁表 |
| 楼中楼递归取数性能 | 大帖卡顿 | 深度限 2、分页、`reply_count` 预聚合 |

---

## 11. 决策记录（已被确认，2026-08-14）

> 下列决策均已拍板，正文已按此定稿，不再视为「待确认」。

| # | 决策点 | ✅ 已确认方案 | 对应章节 |
|---|--------|---------------|----------|
| 1 | 身份方案 | 方案 A：机器标识哈希（`MachineGuid` 加盐 SHA-256 → 64 hex） | §3.1 |
| 2 | 社区域名 | `community-beta.tenma.work`（自定义域名，非 `*.workers.dev`） | §5.1 |
| 3 | 内容治理 | 含举报 + 封禁 + 敏感词（Worker 本地常量表），并新增**管理员页面**（Worker 固定用户名/密码登录） | §5.2/§5.3、§6、§8 |
| 4 | 楼中楼深度 | 限 2 层 | §4.2、§8.4 |
| 5 | 签到奖励 | 纯计数 + 连续天数（无积分/等级体系） | §8.3 |
| 6 | 头像形态 | 本地 emoji 预设索引（后端仅存 `avatar_index`，零存储） | §8.5 |
| 7 | 社区代码存放 | 独立 Git 仓库，当前目录名 `i-code-community-worker` | §2.1 |
| 8 | 匿名可视性 | 未开启门禁时**侧栏无任何红点/角标提示**（仅点入后展示模糊门禁页） | §8.1、§8.2 |

**开工前剩余待办（非决策项，属实施细节）：**

- Worker 仓库初始化（`i-code-community-worker`）与 D1 migration 落库；
- 管理员固定凭据的安全存放方式（`.dev.vars` / KV 密文）；
- `community-beta.tenma.work` 的 Cloudflare 路由 / TLS 绑定配置。