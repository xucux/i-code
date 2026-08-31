# 社区鉴权与账号体系迭代（Auth & Accounts）

> 状态：**已确认（2026-08-31）** —— 待办 D1~D4 已拍板，可按 P1 实施
> 关联：
> - `doc/community.md` —— 社区原设计（身份模型 §3 / 通用约定 §5.1 / 治理 §6）
> - `src/index.ts` / `src/auth.ts` / `src/admin.ts` / `src/rate-limit.ts` —— 本迭代改造对象
> - 主仓 `docs/proposals/community-auth-accounts.md` —— 同一文档的同步副本

---

## 1. 背景与问题

| 项 | 现状 | 问题 |
|----|------|------|
| 身份 | `X-User-Id` = 机器标识加盐 SHA-256（64 hex），懒注册 | 无账号体系；重装系统即身份丢失 |
| 鉴权 | 固定 `X-App-Token` 常量（Worker `i-code-community-app-token-v1` / Rust `-prod`，双端硬编码） | **i-code 开源后固定 key 随源码公开**：任何人可带 key 伪造任意 `X-User-Id` 冒充任意用户（发帖 / 签到 / 打赏 / 点赞），封禁形同虚设 |
| 治理 | 封禁 / 禁言按 `user_id` | 固定 key 泄露后封一个换一个 ID，无法识别 |

## 2. 迭代目标（G1~G4，均已确认）

| 编号 | 目标 |
|------|------|
| G1 | 双登录模式：**匿名登录**（原机器码模式）+ **用户名密码登录** |
| G2 | 匿名用户可「设置用户名（唯一）+ 密码」绑定为账号，之后可跨设备登录同一身份 |
| G3 | 校验规则：用户名 **≥4 位，英文字母开头（可含数字），区分大小写**；密码 **≥8 位** |
| G4 | 简单 **token 鉴权** 替代固定 key；携带旧 key（`X-App-Token`）的请求一律**拦截并提示升级** |

## 3. 总体方案

```
换 token 入口（无需 token）：
  POST /auth/anonymous   ← 匿名：X-User-Id(64hex) 换取 token（限流防刷）
  POST /auth/login       ← 账号：username + password
  POST /auth/register    ← 账号：注册（创建独立身份）

业务接口（原全部 /api/v1/*）：
  X-Auth-Token: <token>  ← Worker 从 sessions 表解析 → user_id（不再信任 X-User-Id）
  旧客户端（带 X-App-Token）→ 统一拦截，返回 426「请升级到最新版本」

管理员接口：
  Bearer adminToken（维持现状），移除 X-App-Token 依赖
```

### 3.1 身份模型

| 模式 | 身份 | 说明 |
|------|------|------|
| anonymous | `user_id` = 机器码哈希（沿用） | 首次进入用 `X-User-Id` 换 token；安全等级等同现状（防呆不防黑） |
| account | `user_id` = 账号绑定的身份 | 注册 / 登录拿 token；登录凭密码，**不可伪造** |
| bind 升级 | 匿名 → 账号 | 匿名用户设置 username + password，`accounts` 关联**当前 user_id**：原帖子 / 回复 / 积分原样保留，此后可在任意设备凭账号登录同一身份 |

> 关键点：所有业务接口由 **token → user_id** 解析身份，彻底移除「客户端自报 `X-User-Id`」，封禁 / 禁言 / 限流才真正有效。

### 3.2 token 方案（✅ 已确认 D1：60 天 + 到期重新登录）

- token = 服务端 `crypto.getRandomValues` 生成 32 hex，存 `sessions` 表；
- **有效期 60 天**（`expires_at = now + 60d`）；到期后请求返回 401「会话已过期，请重新登录」；
- **不做自动续期 / 静默重换**——到期需用户重新登录（匿名：重新匿名进入；账号：重新输入密码）；
- 注销（`/auth/logout`）或封禁时吊销；bind 时吊销原匿名 token（D3）。

## 4. 数据模型（migrations/0014_add_accounts.sql）

```sql
-- 账号表：anonymous 模式可升级绑定；username 全局唯一（BINARY 比较，区分大小写）
CREATE TABLE IF NOT EXISTS accounts (
  username      TEXT PRIMARY KEY,            -- ^[A-Za-z][A-Za-z0-9]{3,31}$（字母开头，区分大小写）
  password_hash TEXT NOT NULL,               -- pbkdf2:{iter}:{salt}:{hash}（PBKDF2-SHA256）
  user_id       TEXT NOT NULL REFERENCES users(user_id),
  created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_accounts_user ON accounts (user_id);

-- 会话表：token → user_id（D1：默认 60 天有效）
CREATE TABLE IF NOT EXISTS sessions (
  token      TEXT PRIMARY KEY,               -- 32 hex
  user_id    TEXT NOT NULL REFERENCES users(user_id),
  mode       TEXT NOT NULL,                  -- 'anonymous' | 'account'
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  expires_at TEXT NOT NULL                   -- UTC ISO（签发时间 + 60 天）
);
CREATE INDEX IF NOT EXISTS idx_sessions_user   ON sessions (user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expiry ON sessions (expires_at);
```

> 用户名唯一性用 SQLite 默认 **BINARY 比较**（`username = ?` 精确匹配），天然区分大小写，无需 `COLLATE NOCASE`。密码只存 PBKDF2 哈希，**明文不落库**。

## 5. Worker API 变更

### 5.1 校验规则（服务端权威，前端 / Rust 同步）

| 字段 | 规则 |
|------|------|
| username | `^[A-Za-z][A-Za-z0-9]{3,31}$`（英文字母开头，≥4 位，区分大小写） |
| password | 长度 ≥8 且 ≤64，无字符集限制 |

### 5.2 新增端点（src/accounts.ts）

| 方法 | 路径 | 请求 | 响应 | 说明 |
|------|------|------|------|------|
| POST | `/auth/anonymous` | 头 `X-User-Id` | `{ token, mode:'anonymous', user }` | 匿名换 token；校验 64 hex + 限流（✅ D2：每 IP 3 次 / 分钟）；不存在则懒注册并返回默认资料 |
| POST | `/auth/register` | `{ username, password }` | `{ token, mode:'account', user }` | 创建**新独立身份**（D4：随机 64 hex user_id，账号与设备解耦，重装不丢）；用户名重复 400 / 规则不合法 400 |
| POST | `/auth/login` | `{ username, password }` | `{ token, mode:'account', user }` | 校验失败 401；**账号登录失败防爆破**：Asia/Shanghai 日历日 + IP 独立计数（`account_login_fail` / `account_ip_ban` 表，不与 admin 混淆），当日失败 10 次 → 永久封 IP（仅 D1 手删恢复） |
| POST | `/auth/bind` | 头 `X-Auth-Token`（匿名态）+ `{ username, password }` | `{ token, mode:'account', user }` | 匿名身份升级账号（D3）：绑定当前 user_id；成功**吊销原匿名 token**，签发 account token；未绑定 username 的匿名 token 无法重复 bind（已有账号的 user 拒绝 400「该身份已绑定账号」） |
| POST | `/auth/logout` | 头 `X-Auth-Token` | `{ ok:true }` | 吊销当前 token（幂等） |

响应 `user` 结构与 `GET /users/me` 的 `user` 一致（ProfileUser）。

### 5.3 业务接口鉴权改造（index.ts 路由层）

用户接口统一替换现有逻辑：

```
旧：appTokenOk(request, env) + getUserId(request) + isBanned
新：getUserIdByToken(request, env)
     → 无 X-Auth-Token / 查无会话 / 已过期 → 401「会话已失效，请重新登录」
     → 会话对应 user 被封 → 403「该账号已被封禁」
```

`/admin/login` 与 `/admin/*`：**移除 `appTokenOk` 强制**；保留 adminToken 校验与现有登录防爆破（D12）。

### 5.4 旧 key 拦截（升级提示）

在 `fetch()` 中 `checkRequest` 之后、路由分发之前统一处理：

```ts
// 旧客户端必带 X-App-Token（固定常量）；新客户端不带
if (request.headers.has('X-App-Token')) {
  return fail(426, '社区已升级鉴权方式，请将 i-code 升级到最新版本');
}
```

- `/s/{pid}` 直链与根路径**不拦截**（浏览器访问，无该头）；
- `/auth/*` 换 token 入口同样统一拦截（新客户端不再携带 `X-App-Token`，正常调用不受影响；命中只可能是旧客户端）。
- 实际部署时若曾用 `wrangler secret put APP_TOKEN` 写入过生产 secret，需 `wrangler secret delete APP_TOKEN` 后再部署，避免残留头。痕迹清理见 wrangler.toml 注释。

## 6. Rust 侧改动（主仓 `src-tauri/src/modules/community/`）

### 6.1 本地状态扩展（types.rs `CommunityLocalState`）

```rust
pub auth_token: Option<String>,   // Worker 签发；null = 未登录 / 被登出 / 到期清除
pub auth_mode: Option<String>,    // 'anonymous' | 'account' | null
pub username: Option<String>,     // 账号模式用户名（展示用）
```

### 6.2 client.rs

- **删除** `APP_TOKEN` 常量与 `X-App-Token` 头（固定 key 下线）；
- `send()` 新增 `auth_token: Option<&str>` 参数注入 `X-Auth-Token`（用户接口必带，管理员接口依旧不带 `user_id` / token）；
- `X-User-Id` 仅保留给 `auth_anonymous`（独立方法，不走通用 send）；
- 新增：`auth_anonymous` / `auth_register` / `auth_login` / `auth_bind` / `auth_logout`；
- `401` → `IcodeError::unauthorized`，由 service 层处理（见 §6.4）；
- UA / Referer / 代理 / 超时逻辑不变。

### 6.3 service.rs / commands.rs

| 新命令 | 职责 |
|--------|------|
| `community_auth_anonymous` | 读取本地 `user_id` → 调 Worker → 保存 auth_token / auth_mode 到本地状态 |
| `community_auth_login` | username + password → 存 token / username |
| `community_auth_register` | username + password（Worker 校验，前端透传错误文案） |
| `community_auth_bind` | 匿名态设置账号（成功后本地 mode → account，存 username） |
| `community_auth_logout` | 吊销 token 并清空本地登录态 |

- `require_ready()` 改为要求 `auth_token`（而非仅 `user_id`）；
- `set_enabled(true)` 流程：生成本机 `user_id` → **自动调用 `/auth/anonymous` 换取 token**（老用户升级补换 token —— 维持 anonymous 模式，**不自动迁移为账户**）；
- 401 处理（D1：不自动续期）：本地登录态清除 + 抛 `unauthorized`，前端捕获后回到登录卡。

### 6.4 401 语义（D1 已确认：不做自动重换）

| 模式 | 401 后行为 |
|------|-----------|
| anonymous | 清除本地 token，前端回到登录卡（用户可再次「匿名进入」重新换 token） |
| account | 清除本地 token / username，前端弹出登录卡提示「登录已过期，请重新登录」 |

## 7. 前端改动（`src/routes/community` + `src/modules/community/ui`）

### 7.1 门禁页 → 登录 / 进入页（community-gate.tsx 改造）

```
┌────────────────────────────────────────────┐
│  社区 · 选择登录方式                        │
│  [ 身份说明：匿名=本机绑定，账号=跨设备 ]    │
│  ┌──────────────────────────────────────┐  │
│  │ 匿名进入（本机身份，无需密码）        │  │
│  └──────────────────────────────────────┘  │
│  ── 或使用账号 ──                          │
│  用户名   [________]（≥4 位字母/数字）      │
│  密码     [________]（≥8 位）               │
│  [ 登录 ]         [ 注册新账号 ]            │
└────────────────────────────────────────────┘
```

- 「匿名进入」→ `community_auth_anonymous` 成功后进入；
- 登录 / 注册失败（401 / 400）toast 展示 Worker 原因；
- 本地已存在 token → 直接进入不重复换；
- `enabled` 开关语义保留（登录 / 注册成功后同样置 enabled=true）。

### 7.2 个人栏（community-profile-panel.tsx）

- 登录模式徽章：`匿名` / `账号 · {username}`；
- 匿名用户：资料卡新增「**绑定用户名密码**」按钮 → 弹窗（用户名 + 密码 + 确认密码，本地校验规则提示）→ `community_auth_bind`；
- 账号用户：新增「退出登录」→ `community_auth_logout` 回到登录卡；
- 其余（资料 / 签到 / 我的内容 / 排行）不变。

### 7.3 其他

- `types.ts`：`CommunityLocalState` 同步增字段；新增 `AuthResult`、`AuthBindInput` 等 DTO；
- i18n：新增 `community.auth.*`（zh-CN / en）——登录卡、规则提示（"用户名至少 4 位，仅字母和数字"、"密码至少 8 位"）、升级提示、会话过期提示等。

## 8. 兼容与迁移

| 场景 | 处理 |
|------|------|
| 老用户升级（已有 enabled=true + userId，无 token） | 进入社区前**自动调 `auth_anonymous` 换 token**；身份 / 数据不变；**保持匿名模式，不自动迁移为账号**（用户主动 bind 才升级，见 D3） |
| 老客户端（旧版本 i-code） | 带 `X-App-Token` → Worker 返回 426「请升级到最新版本」 |
| 已绑定账号的用户后续行为 | 任意设备 `login` 后身份 = 同一 `user_id`，数据同源 |
| 管理员 | 登录接口行为不变（移除 `X-App-Token` 前置，仍受 D12 防爆破保护） |

## 9. 安全边界

| 项 | 说明 |
|----|------|
| 匿名模式 | 仍「防呆不防黑」（机器码可逆向，换 token 无需密码）；受每 IP 3 次 / 分钟限流约束。匿名 token 只是把固定 key 换为不可批量重放的会话凭证 |
| 账号模式 | 真实防伪：伪造他人身份需知其密码；PBKDF2 + 随机盐防彩虹表；登录失败按 IP 防爆破 |
| token 泄露 | 60 天有效期内可冒充对应身份；到期失效，泄露面可控（D1） |
| 固定 key 结局 | 新旧两常量（Worker + Rust）全部移除，仅保留 426 拦截识别旧头 |
| 隐私 | `user_id` / token **禁止写入任何日志**（含 Worker console 与 Rust 日志），延续原设计 |

## 10. 决策记录（2026-08-31 全部拍板）

| # | 决策点 | ✅ 已确认方案 |
|---|--------|---------------|
| D1 | token 有效期 | **60 天**有效；到期返回 401，**需重新登录**（匿名重进 / 账号重输密码），不做自动续期 |
| D2 | `/auth/anonymous` 防刷 | **每 IP 3 次 / 分钟**（`rate-limit.ts` RULES 新增 `authAnonymous`，按 CF-Connecting-IP 计数） |
| D3 | bind 升级语义 | **吊销匿名 token**，签发 account token；同一 user_id 已有账号时拒绝重复 bind |
| D4 | 注册身份归属 | **注册 = 新建独立身份**（随机 64 hex user_id，账号与设备解耦，重装不丢） |
| D5 | 老匿名用户迁移 | 老用户升级后**保持匿名模式，不自动迁移为账号**；bind 为主动可选操作（G2 保留） |

## 11. 分期计划

| 阶段 | 内容 |
|------|------|
| P1 Worker | migration 0014 + `src/accounts.ts`（PBKDF2 / 注册 / 登录 / bind / anonymous / logout / 登录防爆破）+ `auth.ts` / `rate-limit.ts` / `index.ts` 改造（token 鉴权 + 426 拦截 + `/auth` 路由） |
| P2 Rust | local state 扩展 + client auth_*（删固定 key）+ service 401 处理 + 全部业务调用改带 token + 新 commands 注册 |
| P3 前端 | 登录 / 注册 / 绑定 UI + 个人栏模式徽章 / 退出 + i18n + 类型同步 |
| 联调 | cargo check / pnpm type-check / wrangler typecheck + `db:migrate:remote` 部署 |