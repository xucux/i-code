# MiMo 免费模型通道参考项目分析

> 分析对象：`参考项目/mimocode2api-main/`（Rust + axum）与 `参考项目/mimo-free-proxy-main/`（Python stdlib 单脚本）  
> 生成时间：2026-08-14  
> 用途：调研小米 MiMo Code 免费通道（`mimo-auto`）的上游协议与特殊请求，供后续（若）在 i-code 内置类似通道时参考。**本文档仅为调研记录，不构成实现承诺。**

---

## 1. 项目背景

两个项目都把小米 **MiMo Code** 官方 CLI 内置的免费模型通道转换成 **OpenAI 兼容 API**：

| 项目 | 语言 | 定位 |
|------|------|------|
| `mimocode2api` | Rust + axum | 完整代理服务：`/v1/chat/completions`、`/v1/models`、`/v1/messages`（Anthropic 兼容）、`/v1/messages/count_tokens`、`/health` |
| `mimo-free-proxy` | Python stdlib | 极轻量单脚本代理：仅 `/v1/chat/completions`、`/v1/models`、`/health` |

两者均**无需小米账号 / Cookie**，匿名换取 JWT 后调用上游；README 均注明「仅供学习研究、非官方用法、上游随时可能变更/失效」。

---

## 2. 上游协议总览

上游 Base URL：`https://api.xiaomimimo.com`，仅两个核心端点：

| 端点 | 用途 | 请求体 |
|------|------|--------|
| `POST /api/free-ai/bootstrap` | 用 client 指纹换取 JWT | `{"client": "<client_id>"}` |
| `POST /api/free-ai/openai/chat` | 核心聊天接口（OpenAI 格式） | 标准 chat completions 体，**模型强制 `mimo-auto`** |

认证流：`client 指纹 → bootstrap 拿 JWT（约 1 小时过期）→ 每次 chat 带 Bearer JWT → 过期前自动刷新`。

限流注意：上游**按源出口 IP 计**（不是按 key / 指纹），同 IP 多指纹无法叠加并发，扩容只能多出口 IP。

---

## 3. 特殊请求细节（关键）

### 3.1 指纹生成与持久化

| 项目 | 生成方式 | 持久化 |
|------|----------|--------|
| `mimo-free-proxy` | `sha256(hostname \| "linux" \| "x64" \| cpu \| user)` | `MIMO_FREE_CLIENT_FILE`（默认 `/opt/mimo-free-proxy/mimo-free-client`，0600） |
| `mimocode2api` | 32 字节随机 hex（64 位小写 hex），读取时校验格式（64 位、全小写 hex） | `MIMOCODE_CLIENT_FILE`（默认 `./.mimocode/client`） |

身份需**保持稳定复用**，换指纹相当于换身份（但限流按 IP 不按指纹）。

### 3.2 Chat 请求特殊 Header

两项目共有的：

```
Authorization: Bearer {jwt}
X-Mimo-Source: mimocode-cli-free     ← 标识来源，必须携带
Content-Type: application/json
```

`mimocode2api` 额外携带（模拟官方 CLI 指纹，建议实现时保留）：

```
User-Agent: mimocode/0.2.0 ai-sdk/provider-utils/4.0.23 runtime/bun/1.3.14
X-Session-Affinity: ses_{...}
Accept: */*
```

- **UA**：`mimocode/{version} ai-sdk/provider-utils/4.0.23 runtime/bun/1.3.14`，模拟官方 CLI 的 bun 运行时 UA
- **`x-session-affinity` 格式**（`src/session.rs`）：`ses_` + 12 位 hex（`now_ms * 4096 + 1` 取反后低 48 位）+ 14 个随机字母数字，每请求新生成
- bootstrap 请求 UA 更简单：`mimocode/{version}`

### 3.3 请求体必须含官方 system prompt 前缀（⚠️ 最重要）

上游对 body 做校验：**messages 首条 system 消息必须以官方 MiMoCode 提示词开头**，否则返回 **403 `Illegal access`**。

- `mimo-free-proxy`：注入完整 `MIMO_GUARD_TEXT`（以 `"You are MiMoCode, an interactive CLI tool that helps users with software engineering tasks.` 开头，含多段 IMPORTANT 说明），必须插在 messages 首位；已存在时以首 80 字符判重
- `mimocode2api`（`src/proxy.rs` `inject_whitelist_system`）：只校验前缀 `WHITELIST_PREFIX`（上述第一句），三种处理分支：
  1. 无 system 消息 → 索引 0 插入前缀
  2. system 为字符串且不匹配前缀 → 改写为 `{prefix}\n{原内容}`
  3. system 为 content 数组 → 在第一个 text block 前插入前缀（已含则跳过）

### 3.4 `max_tokens` 限制

- 上限 **131072**（仅 `mimo-free-proxy` 有钳制逻辑；`mimocode2api` 未钳制）
- `mimo-auto` 是 **reasoning 模型**：`max_tokens` 建议 ≥ 200，太小推理过程会吃光预算导致 `content` 为空

### 3.5 JWT 续期与自愈

- 提前 **5 分钟**（`REFRESH_MARGIN = 300s` / `RENEW_THRESHOLD_MS`）刷新
- 请求遇 `401`（Rust 版）或 `401/403`（Python 版）→ **强制刷新 JWT 并重试一次**
- JWT payload 解析不到 `exp` → 回退 TTL 55 分钟
- 解析 `exp`：base64url 解码 payload（`URL_SAFE_NO_PAD` 优先，失败回退 `URL_SAFE`），`exp` 为秒级需 ×1000

---

## 4. 两项目差异对比

| 维度 | mimo-free-proxy (Python) | mimocode2api (Rust) |
|------|--------------------------|---------------------|
| client 指纹 | 机器特征哈希 | 随机 hex 持久化 |
| UA 伪装 | ❌ urllib 默认 UA | ✅ 模拟官方 CLI UA |
| 会话头 | ❌ 无 | ✅ `x-session-affinity` |
| guard prompt | 完整三段文本，仅判重 | 前缀一句，与已有 system 合并 |
| max_tokens 钳制 | ✅ 131072 | ❌ 无 |
| 401/403 重试 | ✅ 两者都刷 JWT 重试 | ✅ 仅 401 |
| 对外端点 | `/v1/chat/completions`、`/v1/models`、`/health` | 另有 `/v1/messages`（Anthropic→OpenAI 转换）、`/v1/messages/count_tokens`（本地估算 chars/4） |
| 响应透传 | 字节流原样透传（`Connection: close`） | 流式透传上游 bytes_stream；剥 hop-by-hop 头与 `access-control-*`；Anthropic 端点做 SSE 反转换 |
| 配置 | 环境变量 + 启动时生成随机 API KEY | 环境变量（`MIMOCODE_PORT` / `MIMOCODE_MODEL` / `MIMOCODE_CLIENT_FILE`） |

`mimocode2api` 的 Anthropic 转换要点（`src/anthropic.rs`）：
- `system`（字符串或 text block 数组）→ 首条 system 消息
- 消息 content block 转换：`text`→文本、`image`→`image_url`、`tool_use`→`function` tool_calls、`tool_result`→`tool` 消息
- `stop_sequences`→`stop`、`stream=true` 时补 `stream_options.include_usage`
- `count_tokens` 为本地估算（字符数/4），不调上游

---

## 5. 对 i-code 的参考意义（仅记录，待定）

若后续在 i-code 内置 MiMo 免费通道（类比现有 Cline Free 预设），需实现：

1. **上游适配器**：本地 `/v1/chat/completions` 转发到 `/api/free-ai/openai/chat`，模型强制 `mimo-auto`
2. **guard prompt 注入**：转发前注入官方 system prompt 前缀（403 判定依据）
3. **JWT 生命周期**：`client` 指纹 + `jwt` 的持久化与自动续期（建议 Tauri State + DB，而非文件）；401/403 强制刷新重试
4. **模拟指纹头**：`X-Mimo-Source`、伪 UA、`x-session-affinity`
5. **参数钳制与提示**：`max_tokens` 钳 131072；reasoning 模型的 UI 提示（过小 max_tokens 导致空 content）

相关仓库：
- `https://github.com/wx8472235-cell/mimocode2api`
- `https://github.com/xuomen/mimo-free-proxy`
