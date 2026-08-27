# Kiro IDE 免费通道参考项目分析

> 分析对象：
> - [`Quorinex/Kiro-Go`](https://github.com/Quorinex/Kiro-Go)（Go，main 分支，调研版本 `f8f6071`）：Kiro 账号 → OpenAI/Anthropic 兼容 API 的代理服务
> - [`hj01857655/kiro-account-manager`](https://github.com/hj01857655/kiro-account-manager)（Tauri 2 + Rust，public 分支，调研版本 `c5c4776`）：Kiro 账号管理器（新版 kiro.dev 后端）
> - [`chaogei/Kiro-account-manager`](https://github.com/chaogei/Kiro-account-manager)（Electron + React + TS，main 分支，调研版本 v1.7.0）：同上功能的 Electron 移植（旧版 AWS 原生后端）
>
> 生成时间：2026-08-26  
> 用途：调研 Kiro IDE 的 OAuth2.0 授权、模型调用、模型列表与余额接口，供在 i-code 新增 Kiro 供应商渠道时参考。  
> **本次范围（用户明确）：i-code 仅需支持 Google / GitHub 社交登录**，AWS Builder ID / 企业 SSO / 微软 Entra / API Key 仅作背景记录。  
> **本文档仅为调研记录，不构成实现承诺；上游协议随时可能变更/失效，落地前建议抓包复核。**

---

## 1. 项目背景

**Kiro（Kiro IDE）** 是 AWS CodeWhisperer 生态的衍生 IDE，账号体系复用 AWS SSO（Builder ID / IAM Identity Center 企业 SSO / 微软 Entra ID），也支持 Google / GitHub Social 登录与本司自签发的 `ksk_` API Key。官方客户端直连的模型接口是 **Amazon CodeWhisperer Streaming 协议**（AWS JSON 1.0 + AWS 二进制 EventStream），并非标准 OpenAI/Anthropic 协议。

| 项目 | 语言/形态 | 定位 | 上游后端 |
|------|-----------|------|----------|
| `Kiro-Go` | Go / 服务端代理 | `/v1/chat/completions`、`/v1/messages`、`/v1/responses`；多账号轮询；自动刷新 | 混合（AWS 原生 + `runtime.kiro.dev`） |
| `hj01857655/kiro-account-manager` | Rust + Tauri 2 / 桌面 | 账号登录/切换/自动换号、配额监控、Kiro2API 网关 | **新版 `management./runtime.kiro.dev`** |
| `chaogei/Kiro-account-manager` | Electron + TS / 桌面 | 同 hj（fork 移植），侧重点在 IDE 切号/机器码 | 旧版 AWS 原生 `codewhisperer./q.` 端点 |

三个项目本质一致：**用 OAuth/API Key 拿到上游凭证 → 按 CodeWhisperer Streaming 协议调用 `generateAssistantResponse` → 上游二进制 EventStream → 转成 OpenAI/Anthropic SSE 给客户端**；REST 类数据（模型列表、余额）走 `getUsageLimits` / `ListAvailableModels`。

> ⚠️ **关键差异（已三方验证）**：模型/余额的**上游端点域名**在不同实现间不一致——
> - **chaogei / Kiro-Go（旧）**：`codewhisperer.{region}.amazonaws.com`、`q.{region}.amazonaws.com`
> - **hj（新）**：`management.{region}.kiro.dev`、`runtime.{region}.kiro.dev`
>
> 唯独 **Social（Google/GitHub）登录/刷新链路三方完全一致**（都打在 Kiro Auth Service `https://prod.us-east-1.auth.desktop.kiro.dev`）。i-code 只做 Social 登录时，**登录链路可直接照搬；模型/余额端点建议沿用新版 `runtime./management.kiro.dev`（hj，即下文 §4-§6「推荐」列）**。

---

## 2. 上游协议总览

按用途分四基座，`{region}` 默认 `us-east-1`：

| 基座 | 域名 | 协议 | 用途 |
|------|------|------|------|
| Kiro Auth Service | `https://prod.us-east-1.auth.desktop.kiro.dev` | OAuth2 + JSON | Google/GitHub Social 授权码、token 换发与刷新（三方一致） |
| AWS SSO OIDC | `https://oidc.{region}.amazonaws.com` | OIDC 设备流 / 授权码+PKCE | Builder ID、IAM Identity Center（企业 SSO），**本次不做** |
| Kiro 管理面（推荐） | `https://management.{region}.kiro.dev` | REST（Bearer + AWS JSON 1.0） | `getUsageLimits` 余额、`ListAvailableModels` 模型列表 |
| Kiro 运行时（推荐） | `https://runtime.{region}.kiro.dev` | AWS EventStream（二进制） | `generateAssistantResponse` 模型生成 |

另有两套**旧版 AWS 原生后端**（chaogei / Kiro-Go 使用，仅供参考）：`codewhisperer.{region}.amazonaws.com`、`q.{region}.amazonaws.com`，端点路径/头与新版有出入（见 §7 对比）。

---

## 3. OAuth2.0 授权与 Token（Social 高通量）

**本次只做 Social（Google / GitHub）。** 授权流 = 标准 **授权码 + PKCE**（S256），经 Kiro Auth Service。全程**无 client_id / scope**——Kiro 服务端内置了客户端，客户端只需要 idp/redirect_uri/code_challenge/state。

### 3.1 授权启动（打开浏览器）

```
GET https://prod.us-east-1.auth.desktop.kiro.dev/login
  ?idp=Github|Google                  # 两端取值：Github / Google
  &redirect_uri=kiro%3A%2F%2Fkiro.kiroAgent%2Fauthenticate-success
  &code_challenge={S256}
  &code_challenge_method=S256
  &state={random}
```

- PKCE：`codeVerifier` = 随机字节 base64url（chaogei 64 字节 / hj 32 字节均可），`codeChallenge = sha256(codeVerifier)` base64url（URL_SAFE_NO_PAD）
- `redirect_uri` 固定为 deep link **`kiro://kiro.kiroAgent/authenticate-success`**，回调形如 `kiro://kiro.kiroAgent/authenticate-success?code=...&state=...`
- state 需会话内保留，回调时校验

### 3.2 桌面端如何捕获回调（三端做法，i-code 参考）

| 框架 | 做法 | 代码位置 |
|------|------|----------|
| Electron（chaogei） | `app.setAsDefaultProtocolClient('kiro')` 注册 `kiro://` scheme；macOS `open-url` 事件 / Win-Linux 单实例锁 `second-instance` 传命令行参数解析 | `src/main/index.ts:2385/7252-7275` |
| Tauri（hj） | `tauri-plugin-deep-link` 注册 scheme `kiro`，`onOpenUrl` 监听；Windows/Linux 走第二实例 | `core/deep_link_handler.rs`、`core/protocol_registry.rs` |
| Go 服务端（Kiro-Go） | 不支持交互式 Social，仅导入 refresh token | — |

i-code 是 Tauri 2，可仿 hj 用 `tauri-plugin-deep-link` 注册 `kiro://`；若不想占用该 scheme，可退化为「用户手动回填 token / 导入 refresh token」（见 §8）。

### 3.3 code 换 token

```
POST https://prod.us-east-1.auth.desktop.kiro.dev/oauth/token
Content-Type: application/json
User-Agent: KiroIDE-0.6.18-{machineId}     # 建议两处都带；chaogei 该请求未带、hj 带
Body: {"code":"...","code_verifier":"...","redirect_uri":"kiro://kiro.kiroAgent/authenticate-success"}
```

响应（camelCase）：

```jsonc
{
  "accessToken": "...",
  "refreshToken": "...",
  "profileArn": "arn:aws:codewhisperer:us-east-1:699475941385:profile/EHGA3GRVQMUK",
  "expiresIn": 3600,          // 秒
  "idToken": "...", "tokenType": "Bearer"
}
```

### 3.4 Token 刷新（不走 AWS OIDC）

```
POST https://prod.us-east-1.auth.desktop.kiro.dev/refreshToken
Content-Type: application/json
User-Agent: KiroIDE-0.6.18-{machineId}
Body: {"refreshToken":"..."}
```

响应：`{ accessToken, refreshToken, expiresIn }`（`refreshToken` 服务端可能轮换；未回传时沿用旧值）。
错误分类（hj `kiro_auth_client.rs`）：HTTP **401** → `AUTH_ERROR:*`（登出信号，refresh token 失效）；**403**（CloudFront 拦截）→ `UPSTREAM_BLOCKED:*`。建议 i-code 同样区分。

### 3.5 Social 固定 profileArn（关键常量，无需自愈）

Social 账号不依赖 AWS SSO，因此使用**写死的固定 ARN**（三方一致）：

```
arn:aws:codewhisperer:us-east-1:699475941385:profile/EHGA3GRVQMUK    # KIRO_SOCIAL_PROFILE_ARN
```

与对比项（仅记录）：BuilderId 占位 `arn:aws:codewhisperer:us-east-1:638616132270:profile/AAAACCCCXXXX`。

### 3.6 写盘 token 文件格式（可选对齐）

Kiro IDE 真实读写 `~/.aws/sso/cache/kiro-auth-token.json`（mode `0o600`），Social 字段与顺序（`kiroAuthSync.ts:129-187`、hj `kiro_auth_client.rs`一致）：

```jsonc
{ "accessToken": "...", "refreshToken": "...",
  "profileArn": "arn:aws:codewhisperer:us-east-1:699475941385:profile/EHGA3GRVQMUK",
  "expiresAt": "2026-09-01T12:00:00.000Z",   // ISO（IdC/BuilderId 用去 Z 的格式，Social 带 Z）
  "authMethod": "social", "provider": "Github" }
```

若 i-code 仅做网关转发（不切 Kiro IDE 账号），此文件非必需；若要与官方 IDE 互认或让 CLI 使用同一凭证，则按此格式写盘。

### 3.7 其余途径（本次不做，仅记录）

| 途径 | 授权流 | redirect_uri | 刷新 |
|------|--------|--------------|------|
| AWS Builder ID | OIDC **设备流**（`oidc.{region}.amazonaws.com`，动态注册 client + device_authorization + 轮询 token） | 无（浏览器输码） | AWS OIDC `/token` |
| IAM Identity Center（企业 SSO） | OIDC 授权码 + PKCE（https://oidc.{region}.amazonaws.com`，redirectUris `http://127.0.0.1/oauth/callback` 不带端口） | 本地回环 | AWS OIDC `/token`（`grantType=refresh_token`） |
| 微软 Entra（external_idp） | 微软 OAuth2 授权码 + PKCE（两步：Kiro 门户 → 微软，回环 `http://localhost:3128`） | 本地回环 | 微软 token endpoint（表单，不走 AWS OIDC） |
| Kiro API Key（`ksk_...`） | 无（Key 即 Bearer，`ksk_key|region` 可指定 region，默认 us-east-1） | — | 不支持 |

---

## 4. 模型调用 `generateAssistantResponse`

> 端点域名有新旧两套（见 §1 提示）。本节约定以**新版 runtime（hj）**为准，旧版写法在 §7 对比。

### 4.1 端点与请求头

```
POST https://runtime.{region}.kiro.dev/generateAssistantResponse      # 新版（hj）
   或  POST https://runtime.{region}.kiro.dev/  + X-Amz-Target        # Kiro-Go 的 API Key 写法

Authorization: Bearer <accessToken>
Accept: application/vnd.amazon.eventstream
content-type: application/json
user-agent: aws-sdk-js/1.0.39 ua/2.1 os/{os}#{rel} lang/js md/nodejs#{ver} api/codewhispererstreaming#1.0.39 m/N KiroIDE-{ver}-{machineId}
x-amz-user-agent: aws-sdk-js/1.0.39 KiroIDE-{ver}-{machineId}
x-amzn-codewhisperer-optout: true
x-amzn-kiro-agent-mode: vibe            # 即 agent 模式：vibe / spec
x-amzn-kiro-profile-arn: <profileArn>   # 新版放 header（hj）；旧版(chaogei)放 body payload.profileArn
amz-sdk-invocation-id: <uuid>
amz-sdk-request: attempt=1; max=3
```

> **不带 `tokentype` / `TokenType` 头**：新版已移除（会导致部分接口 403）；旧版 chaogei 仅 external_idp 账号带 `TokenType: EXTERNAL_IDP`，Social 不带。

### 4.2 请求体（KiroPayload）

```jsonc
{
  "conversationState": {
    "chatTriggerType": "MANUAL",
    "conversationId": "<uuid>",            // 会话 ID，见 4.4
    "agentContinuationId": "<同上>",
    "agentTaskType": "vibe",
    "currentMessage": {
      "userInputMessage": {
        "content": "用户消息",
        "modelId": "claude-sonnet-4.5",    // 已映射的上游模型 ID
        "origin": "AI_EDITOR",             // API Key/CLI 用 KIRO_CLI
        "images": [{"format":"png","source":{"type":"bytes","data":"<base64>"}}],
        "userInputMessageContext": {       // 工具（可选）
          "tools": [{"toolSpecification":{"name":..,"description":..,"inputSchema":{"json":{..}}}}],
          "toolResults": [{"toolUseId":..,"content":[{"text":..}],"status":"SUCCESS"}]
        }
      }
    },
    "history": [                           // 多轮历史：user/assistant 交替，首尾必须 user，最长 30 条
      {"userInputMessage": {...}},
      {"assistantResponseMessage": {"content":..,"modelId":..,"toolUses":[...]}}
    ]
  },
  "profileArn": "arn:aws:codewhisperer:...",        // 新版(hj)可选；旧版(chaogei)必带 Social ARN
  "inferenceConfig": {"maxTokens":0,"temperature":0,"topP":0},   // 可选
  "additionalModelRequestFields": {"thinking": {...}}            // 可选，thinking 参数透传
}
```

- **system prompt 注入**（hj converter）：内联进第一条 user 消息 content，包 `--- SYSTEM PROMPT --- ... --- END SYSTEM PROMPT ---`；thinking 在其前插入 `<thinking_mode>enabled</thinking_mode>` 等标记
- **Origin**：`AI_EDITOR`（IDE/HTTP 网关）或 `KIRO_CLI`（API Key/CLI）
- 历史需规范：user 开头/结尾、user/assistant 交替、toolUse↔toolResult 成对、空消息清理

### 4.3 流式响应：AWS 二进制 EventStream（非文本 SSE）

上游回包 `application/vnd.amazon.eventstream` 二进制帧：Prelude 12 字节（totalLength + headersLength + preludeCRC）→ headers 区（`:message-type` / `:event-type` / `:content-type`）→ payload JSON → messageCRC。

| `:event-type` | payload 字段 | 语义 |
|---|---|---|
| `assistantResponseEvent` | `content`（string） | 模型出口文本增量（最终答案） |
| `reasoningContentEvent` | `text`、`signature`、`redactedContent` | 思考增量/签名/脱敏内容 |
| `toolUseEvent` | `toolUseId`/`name`/`input`/`stop` | 工具调用（stop=true 结束） |
| `messageMetadataEvent` / `metadataEvent` | `stopReason`、`tokenUsage{uncachedInputTokens, cacheReadInputTokens, cacheWriteInputTokens, outputTokens, totalTokens}` | 停止原因与 token 用量 |
| `contextUsageEvent` | `contextUsagePercentage`、`breakdown{conversation, mcpTools, steeringFiles}` | 上下文占用% 与拆解 |
| `meteringEvent` | `usage` | 计费消耗 |
| `codeReferenceEvent` / `citationEvent` / `supplementaryWebLinksEvent` | `references[]` 等 | 代码引用/引文/网页链接 |
| `invalidStateEvent` / `interactionComponentsEvent` 等 | — | 异常/交互组件 |

> **SSE 透传约束（对齐 AGENTS.md §10.1）**：客户端侧是否走 OpenAI/Anthropic SSE 由网关自行组装；到上游侧一定是二进制帧，**不能**把 EventStream 当文本 SSE 原样透传，必须先解码。

### 4.4 会话 ID 维护

- **Kiro-Go**：`uuid5(modelId + "\n" + systemPrompt + "\n" + 首条用户消息锚点)` 确定性生成——同会话同锚点得到同一 ID；空/`.`/`begin conversation` 等合成锚点则每次新生成
- **hj**：取 OpenAI Responses `previousResponseId`（Anthropic 无则每轮新 uuid）；响应 `assistantResponseEvent.conversationId` / `metadataEvent.conversationId` 回填下轮；`agentContinuationId = conversationId`
- **chaogei**：sessionHint 会话缓存（2h TTL）→ history fingerprint → 新 UUID
- 多轮历史显式用 `conversationState.history[]` 携带，不依赖服务端会话

### 4.5 模型名 → 上游 modelId 映射

客户端模型名先剥 `-thinking` 后缀并标记思考模式，再归一化（`-` → `.`、去日期后缀），最后对照映射表：

| 外部别名 | 上游 modelId |
|---|---|
| `auto` / `default` | `auto` |
| `opus` | `claude-opus-4.7`（Kiro-Go 另有 claude-3-5-opus→claude-sonnet-4.5 兜底） |
| `sonnet` / `claude-3-5-sonnet` / `gpt-4o` 等 | `claude-sonnet-4.5`（Free 最高可用兜底） |
| `haiku` | `claude-haiku-4.5` |
| `claude-sonnet-5` | `claude-sonnet-5` |
| `deepseek-3-2` | `deepseek-3.2` |
| `qwen3-coder` | `qwen3-coder-next` |
| `gpt-5.6-*` / `glm-5` / `minimax-m2-5` | 原样透传（Kiro 原生模型） |
| 未知 | `claude-sonnet-4.5`（降级兜底） |

> 映射表随上游频繁变化，均以「可用模型列表动态校验 + 降级兜底」为准；thinking 能力可从 `ListAvailableModels` 的 `additionalModelRequestFieldsSchema` 探测（chaogei v1.7.5）。

---

## 5. 模型列表 `ListAvailableModels`

**推荐（新版，hj）**：

```
POST https://management.{region}.kiro.dev/
Content-Type: application/x-amz-json-1.0
X-Amz-Target: KiroControlPlaneBearerService.ListAvailableModels
User-Agent: api/kirocontrolplanebearer#1.0.0 ...
Body: {"origin":"AI_EDITOR","profileArn":"<Social ARN 可选>"}
```

**旧版（参考）**：`GET https://q.{region}.amazonaws.com/ListAvailableModels?origin=AI_EDITOR&maxResults=50&profileArn=...&nextToken=...`（chaogei `fetchKiroModels`，分页直到无 nextToken）；Kiro-Go 用 `GET https://codewhisperer.{region}.amazonaws.com/ListAvailableModels`。

配套 `ListAvailableProfiles`：同一基座空 body `{}`，用于缺 profileArn 时取首个 arn（Social 不需要，用固定 ARN）。

响应结构（第 5 节任一入口同构）：

```jsonc
{
  "models": [
    {
      "modelId": "claude-sonnet-4.5",        // 上游实际模型 ID
      "modelName": "", "description": "",
      "provider": "...", "status": "...",
      "rateMultiplier": 1.0, "rateUnit": "...",
      "supportedInputTypes": ["TEXT","IMAGE"],
      "tokenLimits": {"maxInputTokens": 200000, "maxOutputTokens": 8192},
      "promptCaching": {"supportsPromptCaching": true, "maximumCacheCheckpointsPerRequest": 3, "minimumTokensPerCacheCheckpoint": 1024},
      "additionalModelRequestFieldsSchema": {...},   // 用于探测 thinking/effort 支持
      "availableOrigins": ["AI_EDITOR", "KIRO_CLI"]
    }
  ],
  "nextToken": "...", "defaultModel": {...}
}
```

缓存建议 30 分钟（hj `AVAILABLE_MODELS_CACHE_TTL_SECONDS=30*60`；chaogei 5min），归一化：默认模型置顶、`defaultModel` 补齐、排空非法 modelId。

---

## 6. 余额 / 配额 `getUsageLimits`

**推荐（新版，hj）**：

```
GET https://management.{region}.kiro.dev/getUsageLimits?isEmailRequired=true&origin=AI_EDITOR&resourceType=AGENTIC_REQUEST
```

- **不带 profileArn**（企业账号带了会 400；Social 同样建议不带，`kiro_client.rs`）
- 头：`Authorization: Bearer <access>`、management 型 UA（`api/codewhispererruntime#1.0.0`）、`amz-sdk-invocation-id`、`amz-sdk-request`、`accept: application/json`

**旧版（参考）**：`GET https://q.{region}.amazonaws.com/getUsageLimits?origin=AI_EDITOR&resourceType=AGENTIC_REQUEST&isEmailRequired=true&profileArn=...`（chaogei 带 ARN，主端点 403 → 回退另一区域 q 端点）；Kiro-Go 另有 `q.` 的 **overage 扩展**读取 `overageConfiguration`。

region 回退链：账号 region → 企业回退 `us-east-1, eu-central-1`；`AUTH_ERROR:` / `BANNED:` 立即返回不换区。

### 6.1 响应结构与字段含义

```jsonc
{
  "subscriptionInfo": {
    "subscriptionName": "", "subscriptionTitle": "KIRO PRO+",
    "subscriptionType": "", "status": "",
    "overageCapability": "OVERAGE_CAPABLE|OVERAGE_INCAPABLE",  // 有无资格开超额
    "upgradeCapability": "", "subscriptionManagementTarget": ""
  },
  "overageConfiguration": { "overageStatus": "ENABLED|DISABLED|UNKNOWN" },
  "usageBreakdownList": [{
    "resourceType": "AGENTIC_REQUEST",
    "displayName": "",
    "currentUsage": 12.5,                // 优先 *WithPrecision 后缀字段
    "usageLimit": 100,
    "overageCap": 0, "currentOverages": 0, "overageRate": 0.0, "overageCharges": 0,
    "currency": "USD", "unit": "units",
    "freeTrialInfo": {"freeTrialStatus":"ACTIVE|EXPIRED","currentUsage":0,"usageLimit":0,"freeTrialExpiry":0},
    "bonuses": [{"bonusCode":"","displayName":"","status":"ACTIVE","expiresAt":0,"currentUsage":0,"usageLimit":0}]
  }],
  "nextDateReset": 1735689600,        // epoch 秒 → /1000 转 ISO
  "limits": 0, "totalUsage": 0, "daysUntilReset": 0,
  "userInfo": {"userId": "...", "email": "..."}
}
```

判定口径（hj `core/usage.rs`）：
- **有效额度** `effective_limit = limit + overage_cap`（仅当 `overageStatus==ENABLED`）
- **剩余可用** = (主额度 + 试用(仅 ACTIVE) + 奖励(仅 ACTIVE 未过期) + 超额) − 已用
- `is_in_overage`：ENABLED 且 `current > limit && current < limit+cap`
- `is_usage_capped`：剩余 ≤ 0；账号状态：403 `TemporarilySuspended`/`suspended`/423 → `BANNED`；401 → `AUTH_ERROR`

---

## 7. 三个参考项目差异对比

| 维度 | Kiro-Go (Go) | hj (Tauri) | chaogei (Electron) |
|------|-------------|------------|--------------------|
| Social 登录 | 不支持交互式（仅导入 refresh token） | ✅ 浏览器 + `kiro://` deep link | ✅ 浏览器 + `kiro://` deep link |
| Social 登录链路 | — | ✅ 与 chaogei **完全一致** | ✅ 与 hj **完全一致** |
| 模型调用端点 | `runtime.{region}.kiro.dev/`（API Key）+ 旧版 AWS 端点 | `runtime.{region}.kiro.dev/generateAssistantResponse` | 旧版 AWS `codewhisperer./q.` …`generateAssistantResponse` |
| profileArn 位置 | body | **header `x-amzn-kiro-profile-arn`**（新版） | body `payload.profileArn` |
| TokenType 头 | 小写 `tokentype: API_KEY`（API Key） | **已移除** | external_idp 才带 `TokenType: EXTERNAL_IDP` |
| 模型列表 | `GET codewhisperer./ListAvailableModels` | `POST management./` + `KiroControlPlaneBearerService` | `GET q./ListAvailableModels` |
| 余额 | `GET codewhisperer./getUsageLimits` + q. overage 扩展 | `GET management./getUsageLimits`（**不带 ARN**） | `GET q./getUsageLimits`（带 ARN，403 换区） |
| Social 固定 ARN | — | `…699475941385:profile/EHGA3GRVQMUK` | 同左 |
| 流式 | 事件解码 → OpenAI/Anthropic SSE | EventStream → Anthropic/OpenAI SSE | EventStream → OpenAI/Anthropic SSE |
| token 刷新 | 统一 `auth/oidc.go` | 60s 定时器 + 网关 3min 阈值回刷 | 主动刷新 + 5min 后台轮询 |

共同点：依赖 **UA 伪装**（`aws-sdk-js` 版本 + `KiroIDE-{ver}-{machineId}`）、**machineId**（UA 后缀 + 账号持久化 + 确定性派生）、**region 派生链**（profileArn → account.region → 默认 us-east-1）、**SSE 客户端格式**（`chat.completion.chunk` / Anthropic `content_block_delta`）。

---

## 8. 对 i-code 的参考意义（仅 Google / GitHub 社交登录，待定）

### 认证（Social 是本次唯一范围）

1. **OAuth 流**：走标准 `OAuth2Auth`（`method: 'oauth2'`，`grantType: 'authorization_code'` + PKCE），映射为 Kiro Auth Service 专属端点（**不能用通用 `/authorize`+`/token` 标准路径**，需后端子类型或 provider 特判）：
   - `authorizationUrl = https://prod.us-east-1.auth.desktop.kiro.dev/login`（query `idp&redirect_uri&code_challenge&code_challenge_method&state`）
   - `tokenUrl = https://prod.us-east-1.auth.desktop.kiro.dev/oauth/token`（body `code&code_verifier&redirect_uri`，非表单）
   - `redirectUri = kiro://kiro.kiroAgent/authenticate-success`
   - 建议新增专用 auth 常量（如内置预设的 `defaultAuth`），或扩展现有 OAuth 机制支持自定义授权/换 token 端点
2. **deep link 捕获**：Tauri 2 用 `tauri-plugin-deep-link` 注册 `kiro://`（对齐 hj `protocol_registry.rs`）；macOS `open-url`、Win/Linux 第二实例冷启动传参。若不想占用该 scheme，备选：仅支持「粘贴 refresh token / 导入 json 凭证」（对齐 Kiro-Go）。
3. **token 生命周期**：复用现有 `OAuth2TokenData`（accessToken/refreshToken/expiresAt）；`expiresIn(秒)` → `expiresAt(Unix 秒)`。刷新走 `/refreshToken`，提前 10 分钟后台刷新 + 网关侧 401 回刷一次；401→auth 失效、403→限流提示。
4. **Social 固定 ARN**：`arn:aws:codewhisperer:us-east-1:699475941385:profile/EHGA3GRVQMUK` 一并写入 auth/token 数据（后续模型/余额请求用）。
5. （可选）与官方 IDE 互认时，按 §3.6 格式写 `~/.aws/sso/cache/kiro-auth-token.json`（mode 0600），本次网关场景可不做。

### 模型 / 余额 / 网关

6. **模型列表**：推荐新版 `POST management./` + `KiroControlPlaneBearerService.ListAvailableModels`；返回 `modelId` 直接落 `official` 来源 gateway_models，`autoFetchOfficialModels` 可开；30 分钟缓存语义与现有 `official_model_cache` 一致。
7. **模型调用**：网关新增 CodeWhisperer 适配器（`gateway-runtime`）：
   - 上游 `runtime.{region}.kiro.dev/generateAssistantResponse` + **AWS EventStream 二进制解码**（Prelude/Headers/CRC，现有一切文本 SSE 上游都没有）
   - `conversationId` 策略：Anthropic 入口无 previous_response_id → 建议 Kiro-Go 式 uuid5 确定性派生
   - system prompt/tools 内联转 `userInputMessage`；thinking 映射 `reasoningContentEvent`；`origin` 用 `AI_EDITOR`
8. **余额**：新版 `GET management./getUsageLimits`（不带 ARN），响应与现有 `balance` 模块监控模型天然对应（额度/已用/重置日/状态）；overage 判定可复刻到 balance provider。
9. **内置预设**：`claude-sonnet-4.5` / `claude-opus-4.7` / `claude-haiku-4.5` 作内置模型 + `builtin_providers` 预设；`-thinking` 后缀与现有 thinking 配置对接。
10. **风险提示**：上游为 AWS/非官方衍生通道，UA/machineId 被风控时可能 403；**三参考项目的端点域名/请求头/列表结构不一致，落地前用官方 Kiro IDE 抓一次真实请求复核**（参考 `docs/log-framework.md` 网关日志抓取机制）。

相关仓库：
- `https://github.com/Quorinex/Kiro-Go`
- `https://github.com/hj01857655/kiro-account-manager`
- `https://github.com/chaogei/Kiro-account-manager`
- 官方入口：`https://app.kiro.dev`、`https://prod.us-east-1.auth.desktop.kiro.dev`（Social 登录）、`https://runtime.{region}.kiro.dev`、`https://management.{region}.kiro.dev`