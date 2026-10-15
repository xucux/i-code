# OpenCode Zen Free 与 Cline Free 反代细节（Cloudflare Worker 指导）

> 依据来源：i-code 网关内置实现（`src-tauri/src/modules/gateway_runtime/client/opencode_free_client.rs`、`header_variable_resolver.rs`、`data/builtin-providers.json`）
> 生成时间：2026-09-26
> 用途：为使用 **Cloudflare Worker 反代 OpenCode Zen 免费通道与 Cline 免费通道**的项目提供协议细节与实现指导。上游校验规则随时可能升级，本文以 i-code 实测通过的规则为基线。

---

## 1. 两条通道概览

| 维度 | OpenCode Zen Free | Cline Free |
|------|-------------------|------------|
| 上游 Base URL | `https://opencode.ai/zen` | `https://api.cline.bot/api/v1` |
| Chat 端点 | `POST {base}/v1/chat/completions` | `POST {base}/chat/completions` |
| 协议 | OpenAI Chat Completions 兼容（**仅流式**） | OpenAI Chat Completions 兼容（流式/非流式均可） |
| 认证 | 无需真实 Key，`Authorization: Bearer public` | 需要真实 Cline 账号 Token，`Authorization: Bearer {token}` |
| 核心校验 | CLI 指纹请求头 + 请求体形状（见 §2） | `x-client-type` 等身份标识头（见 §3） |
| 免费限制 | 免费模型 200K 上下文限制；模型不定时上下架 | 次数与 Token 限制（按账号计） |

反代本质：**把上游对「官方 CLI 客户端」的特征校验，在 Worker 内伪装/补齐**，再以 OpenAI 兼容 API 对外提供服务。

---

## 2. OpenCode Zen Free 细节

### 2.1 上游强制校验项（任一不满足 → `HTTP 403 FreeTierError: OpenCode's free tier can only be used from within OpenCode`）

1. **请求头特征**：`User-Agent` 必须为 `opencode/x.y.z ...` 形态，并携带 CLI 本地计算的会话/请求 ID（见 2.3）；
2. **请求体必须 `"stream": true`**：上游仅接受流式响应；
3. **请求体 `tools` 校验**：数组必须存在、长度 ≥ 2 且包含名为 `bash` 的工具；每个工具必须是 OpenAI 标准嵌套结构（`tools[i].function` 必须为对象），**schema 内容不校验**。

> 历史教训：2025-09 上游升级校验后，仅伪装普通 OpenAI 请求（UUID 形态 ID、缺 tools、非流式）全部 403。ID 乱填或 UUID 形态会被服务端校验**时间戳新鲜度**后拒绝。

### 2.2 请求头清单

| Header | 值 | 说明 |
|--------|----|----|
| `Authorization` | `Bearer public` | 固定值，无需真实 Key |
| `User-Agent` | `opencode/1.18.31 ai-sdk/provider-utils/4.0.40 runtime/bun/1.3.14` | 模拟官方 CLI（bun 运行时）；版本号可随上游更新固化 |
| `x-opencode-client` | `cli` | 固定 |
| `x-opencode-project` | `global` | 固定 |
| `x-opencode-session` | `ses_` + 26 字符（见 2.3） | **当天固定**，跨天更换 |
| `x-opencode-request` | `msg_` + 26 字符（见 2.3） | **每次请求刷新** |

⚠️ 反代时**不要透传客户端的 `User-Agent` / `Authorization`**，必须整体重建请求头。

### 2.3 OpenCode ID 生成算法（关键）

与 opencode CLI 源码 `gen()` 对齐，形如 `0ada1b968001cxm5rgh28JxlDH`（26 字符）：

1. `v = now_ms × 4096 + ctr`（`ctr` 为同一毫秒内自增计数器，跨毫秒重置为 1）；
2. 会话 ID 对 `v` 按位取反（`~v`），请求 ID 不取反；
3. 取**低 48 位**（6 字节），从高到低每字节两位十六进制 → 12 字符时间戳段；
4. 追加 **14 个 base62 随机字符**（`0-9A-Za-z`，源码为 14 随机字节 % 62）。

服务端会解码时间戳段校验新鲜度，因此**必须用真实当前时间**，不能预生成或乱填。

### 2.4 请求体合规化

```text
1. stream 不是 true → 直接拦截（不发上游），返回 OpenAI 标准错误体
   HTTP 400 {"error":{"type":"invalid_request_error",...}}，提示客户端改用流式；
2. tools 缺失或空 → 注入 [bash, read] 两个最小化工具；
3. tools 已有 → 先把扁平格式（顶层 name/description/parameters，无 function 字段）
   规范化为嵌套结构，再检查：缺 bash 则头部插入，总数 < 2 则追加 read。
```

最小化工具模板（schema 内容不校验，`description` 随意但字段必须齐全）：

```json
{
  "type": "function",
  "function": {
    "name": "bash",
    "description": "x",
    "parameters": { "type": "object", "properties": {}, "required": [] },
    "strict": false
  }
}
```

### 2.5 响应与模型列表

- **响应**：上游恒返回 `text/event-stream`，Worker 必须把 `upstream.body` 以流式原样透传，禁止读取完整 body 后再返回（会触发 CPU 超时并破坏流式体验）；上游 4xx/5xx 错误体原样透传给客户端。
- **模型列表**：`GET https://opencode.ai/zen/v1/models`（带同一套请求头即可）；免费模型不定时上下架，建议动态拉取而非写死。

---

## 3. Cline Free 细节

### 3.1 端点与认证

- Chat：`POST https://api.cline.bot/api/v1/chat/completions`
- 认证：`Authorization: Bearer {cline_token}`（真实 Cline 账号 Token；Worker 建议用 Wrangler Secret 存放，如 `wrangler secret put CLINE_TOKEN`，禁止硬编码/写日志）

### 3.2 请求头（两种预设形态）

**CLI 形态**（最小集，实测可用）：

| Header | 值 |
|--------|----|
| `x-client-type` | `cline-cli` |
| `Authorization` | `Bearer {cline_token}` |

**VSCode 扩展形态**（备选，当 CLI 形态被上游收紧时切换）：

```text
http-referer: https://cline.bot
x-title: Cline
user-agent: Cline/4.1.16
x-core-version: 4.1.16
x-platform-version: 1.106.0
x-client-version: 4.1.16
x-platform: vscode
x-client-type: cline-vscode
```

### 3.3 模型 ID 与限制

- 上游模型 ID 自带**渠道/厂商前缀**，含 `/`，例如：
  `cline-free/kimi-k3`、`z-ai/glm-5.3-flash`、`deepseek/deepseek-v4-flash`
- 免费模型存在**次数与 Token 限制**（按账号计）；上游无公开模型列表接口可用，模型清单需写死或人工维护（i-code 亦为固定预设）。
- 协议为标准 OpenAI Chat 兼容，无请求体形状校验，Worker 只需：改写 `Authorization` 为真实 Token + 注入 `x-client-type` + 透传其余请求体与响应（流式同样建议透传）。

---

## 4. Cloudflare Worker 实现指导

### 4.1 总体架构

```text
外部客户端（OpenAI 兼容）
    → Worker（入站鉴权可选）
    → 识别通道（按路径或模型前缀）
    → OpenCode Free：重建请求头 + ID 生成 + body 合规化（强制 stream/tools）
    → Cline Free：注入 Bearer Token + x-client-type
    → fetch 上游，SSE 字节流原样透传
```

### 4.2 Worker 平台关键注意点

| 事项 | 说明 |
|------|------|
| 设置 `User-Agent` | Workers 的 `fetch` **允许**自定义 `User-Agent`（与浏览器不同），OpenCode 校验依赖它 |
| `Host` 头 | 不可手工设置，也不需要——URL 决定即可 |
| SSE 透传 | `return new Response(upstream.body, { headers })` 即流式转发；等待 I/O 不计 CPU 时间，免费版 10ms CPU 足够 |
| `Date.now()` | 单次调用内时间冻结，仅在 I/O 后推进；ID 生成放在 `fetch` 前、有 await 的位置之后即可保证新鲜 |
| 大整数 | `now_ms × 4096` 超出 32 位安全整数，**必须用 `BigInt`** |
| 会话 ID 缓存 | Worker 无持久状态，用模块级（isolate 级）变量按天缓存即可；isolate 重启导致会话 ID 更换是可接受的（相当于 CLI 重启） |
| 随机数 | 用 `crypto.getRandomValues`，禁止 `Math.random`（伪随机且精度不足无所谓，主要是规范） |
| 错误透传 | 上游 403/429/5xx 原样透传状态码与 body，便于客户端定位；对内错误统一 OpenAI 标准格式 `{error:{message,type,param,code}}` |
| 免费额度 | 100k 请求/天（免费版）；每条上游请求对应一条 Worker 请求，无聚合缓存 |

### 4.3 参考实现骨架（JavaScript）

ID 生成（对应 §2.3 算法）：

```js
const B62 = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
let ctr = 0, lastMs = 0;

function opencodeId(negate) {
  const now = BigInt(Date.now());
  if (lastMs !== now) { lastMs = now; ctr = 0; }
  ctr += 1;
  let v = now * 4096n + BigInt(ctr);
  if (negate) v = ~v;
  let id = (v & 0xFFFFFFFFFFFFn).toString(16).padStart(12, "0"); // 低 48 位
  const bytes = crypto.getRandomValues(new Uint8Array(14));
  for (const b of bytes) id += B62[b % 62];
  return id;
}

// 会话 ID：isolate 级按天缓存
let sessionCache = { day: "", id: "" };
function opencodeSession() {
  const day = new Date().toISOString().slice(0, 10);
  if (sessionCache.day !== day) sessionCache = { day, id: "ses_" + opencodeId(true) };
  return sessionCache.id;
}
function opencodeRequest() { return "msg_" + opencodeId(false); }
```

请求头与 body 合规化：

```js
const OPENCODE_HEADERS = {
  "Authorization": "Bearer public",
  "User-Agent": "opencode/1.18.31 ai-sdk/provider-utils/4.0.40 runtime/bun/1.3.14",
  "x-opencode-client": "cli",
  "x-opencode-project": "global",
};

function minimalTool(name) {
  return { type: "function", function: { name, description: "x",
    parameters: { type: "object", properties: {}, required: [] }, strict: false } };
}

function prepareBody(body) {
  if (body.stream !== true) {
    return { error: { message: "此通道仅支持流式请求（stream 必须为 true）",
      type: "invalid_request_error", param: "stream", code: "bad_request" } };
  }
  body.stream = true;
  if (!Array.isArray(body.tools) || body.tools.length === 0) {
    body.tools = [minimalTool("bash"), minimalTool("read")];
  } else {
    // 扁平格式 → 嵌套结构
    for (const t of body.tools) {
      if (!t.function && t.name) {
        t.function = { name: t.name, description: t.description, parameters: t.parameters };
        delete t.name; delete t.description; delete t.parameters;
      }
    }
    const hasBash = body.tools.some(t => t.function?.name === "bash");
    if (!hasBash) body.tools.unshift(minimalTool("bash"));
    if (body.tools.length < 2) body.tools.push(minimalTool("read"));
  }
  return null;
}
```

Worker 入口：

```js
export default {
  async fetch(request) {
    const url = new URL(request.url);

    if (url.pathname === "/v1/models") {
      // OpenCode 免费模型动态拉取（Cline 无列表接口，返回固定清单）
      return fetch("https://opencode.ai/zen/v1/models", {
        headers: { ...OPENCODE_HEADERS,
          "x-opencode-session": opencodeSession(),
          "x-opencode-request": opencodeRequest() },
      });
    }

    if (url.pathname !== "/v1/chat/completions") {
      return json(404, { error: { message: "Not Found", type: "invalid_request_error",
        param: null, code: "not_found" } });
    }

    const body = await request.json();
    const err = prepareBody(body);
    if (err) return json(400, err);

    const upstream = await fetch("https://opencode.ai/zen/v1/chat/completions", {
      method: "POST",
      headers: { ...OPENCODE_HEADERS, "Content-Type": "application/json",
        "x-opencode-session": opencodeSession(),
        "x-opencode-request": opencodeRequest() },
      body: JSON.stringify(body),
    });

    // SSE 原样透传（含 4xx/5xx 错误体）
    const headers = new Headers(upstream.headers);
    headers.set("Content-Type", upstream.headers.get("content-type") || "text/event-stream");
    headers.set("Cache-Control", "no-cache");
    return new Response(upstream.body, { status: upstream.status, headers });
  },
};
```

Cline Free 变体（替换上游 URL 与头构造）：

```js
async function forwardCline(body, env) {
  return fetch("https://api.cline.bot/api/v1/chat/completions", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `Bearer ${env.CLINE_TOKEN}`,   // wrangler secret put CLINE_TOKEN
      "x-client-type": "cline-cli",
    },
    body: JSON.stringify(body),
  });
}
```

### 4.4 排查表

| 现象 | 可能原因 |
|------|----------|
| OpenCode 403 `FreeTierError` | UA 不符 / session、request ID 缺失或 UUID 形态 / `stream` 非 true / tools 缺失、无 bash、非嵌套结构 |
| OpenCode 403（ID 看似正确） | ID 时间戳过期（预生成、时钟漂移）——用真实 `Date.now()` 现算 |
| OpenCode 400 StreamRequired | 客户端发非流式请求，Worker 未拦截提示 |
| Cline 401/403 | Token 失效或未注入 `x-client-type` |
| Cline 免费额度异常消耗 | 会话维度重复建立——同一上游会话尽量复用（OpenCode 同理，session ID 按天固定） |
| SSE 卡顿/一次性吐出 | Worker 中读了 `upstream.text()` 再返回，必须直接透传 `upstream.body` |

---

## 5. 与 i-code 内置实现对照

| 本文规则 | i-code 实现位置 |
|----------|----------------|
| OpenCode 三项校验 + body 合规化 | `src-tauri/src/modules/gateway_runtime/client/opencode_free_client.rs` |
| ID 生成算法与模板变量 | `src-tauri/src/modules/gateway_runtime/header_variable_resolver.rs` |
| 请求头注入顺序（extra 头最后、可覆盖） | `src-tauri/src/modules/gateway_runtime/client/openai_chat_client.rs` |
| 供应商预设与默认模型 | `src-tauri/data/builtin-providers.json`（`opencode-zen-free` / `cline-free` / `cline-vscode`） |
| URL 拼接（`/v1` 自动补全） | `src-tauri/src/modules/gateway_runtime/client/mod.rs` `build_upstream_url` |

## 6. 风险声明

- 上游校验规则**随时可能升级**（OpenCode 已有 2025-09 前科：UUID ID 全面失效）；UA 中的版本号建议定期对照官方 CLI 最新版更新。
- 本反代用法属非官方用途，仅供学习研究，免费通道存在限流、下架与封禁风险。
- Cline Token 属敏感凭证：仅存 Worker Secret，不落日志、不返回给客户端。
