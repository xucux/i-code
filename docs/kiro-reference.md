# Kiro IDE 免费通道参考项目分析

> 分析对象：
> - [`Quorinex/Kiro-Go`](https://github.com/Quorinex/Kiro-Go)（Go + GoTUI，main 分支，调研版本 `f8f6071`）：把 Kiro 账号转成 OpenAI / Anthropic 兼容 API 的代理服务
> - [`hj01857655/kiro-account-manager`](https://github.com/hj01857655/kiro-account-manager)（Tauri 2 + Rust，public 分支，调研版本 `c5c4776`）：Kiro 账号管理器，含配额监控与内置 Kiro2API 网关
>
> 生成时间：2026-08-26  
> 用途：调研 Kiro IDE 的 OAuth2.0 授权、模型调用、模型列表与余额接口，供在 i-code 新增 Kiro 供应商渠道时参考。**本文档仅为调研记录，不构成实现承诺；上游协议随时可能变更/失效，落地前建议抓包复核。**

---

## 1. 项目背景

**Kiro（Kiro IDE）** 是 AWS CodeWhisperer 生态的衍生 IDE，账号体系复用 AWS SSO（Builder ID / IAM Identity Center 企业 SSO / 微软 Entra ID），也支持 Google / GitHub Social 登录与本司自签发的 `ksk_` API Key。官方客户端直连的模型接口是 **Amazon CodeWhisperer Streaming 协议**（AWS JSON 1.0 + AWS 二进制 EventStream），并非标准 OpenAI/Anthropic 协议。

| 项目 | 语言 | 定位 |
|------|------|------|
| `Kiro-Go` | Go | 完整代理：`/v1/chat/completions`、`/v1/messages`、`/v1/responses`；多账号轮询负载均衡；全自动 token 刷新；SSE 流式；Web 管理面板 |
| `kiro-account-manager` | Rust + Tauri 2 | 桌面应用：Kiro 账号登录/切换/自动换号、余额（配额）监控、随附 Kiro2API 网关（Anthropic/OpenAI → CodeWhisperer 转发） |

两者实现的本质一致：**用 OAuth/API Key 拿到上游凭证 → 按 CodeWhisperer Streaming 协议调用 `generateAssistantResponse` → 上游二进制 EventStream → 转成 OpenAI/Anthropic SSE 给客户端**；REST 类数据（模型列表、余额）走 `getUsageLimits` / `ListAvailableModels`。

---

## 2. 上游协议总览

Kiro 上游按用途分四个基座，域名里 `{region}` 默认 `us-east-1`（其余 region 改写域名）：

| 基座 | 域名 | 协议 | 用途 |
|------|------|------|------|
| Kiro Auth Service | `https://prod.us-east-1.auth.desktop.kiro.dev` | OAuth2 + JSON | Google/GitHub Social 授权码、token 换发与刷新 |
| AWS SSO OIDC | `https://oidc.{region}.amazonaws.com` | OIDC 设备流 / 授权码+PKCE | Builder ID、IAM Identity Center（企业 SSO） |
| Kiro 管理面 | `https://management.{region}.kiro.dev` | REST（Bearer + AWS JSON 1.0） | `getUsageLimits` 余额、`ListAvailableModels` 模型列表（control plane） |
| Kiro 运行时 | `https://runtime.{region}.kiro.dev` | AWS EventStream（二进制） | `generateAssistantResponse` 模型生成（流式文本/推理/工具） |

> 注意：`Kiro-Go` 对 OAuth 账号另支持 AWS 原生端点 `codewhisperer.us-east-1.amazonaws.com` / `q.{region}.amazonaws.com`（Kiro IDE 真身所在），对 API Key 账号则固定走 `runtime.{region}.kiro.dev`。`kiro-account-manager` 全部收敛到 `management./runtime.{region}.kiro.dev`。两套实现**端点路径略有出入（`/` vs `/generateAssistantResponse`）**，落地时以官方 IDE 抓包为准。

---

## 3. OAuth2.0 授权与 Token

共 4 类账号途径 + 1 种 API Key：

| 途径 | 授权流 | redirect_uri | 刷新 |
|------|--------|--------------|------|
| Social（Google/GitHub） | 授权码 + PKCE，经 Kiro Auth Service | `kiro://kiro.kiroAgent/authenticate-success`（deep link） | Kiro Auth Service `/refreshToken` |
| AWS Builder ID | OIDC **设备授权流**（device_code） | 无（浏览器输码） | AWS OIDC `/token` |
| AWS IAM Identity Center（企业 SSO） | OIDC 授权码 + PKCE | `http://127.0.0.1/oauth/callback`（本地回环） | AWS OIDC `/token` |
| 微软 Entra ID / Azure AD（external_idp） | 微软 OAuth2 授权码 + PKCE（两步：Kiro 门户 → 微软） | `http://localhost:3128`（本地回环固定端口） | 微软 token endpoint（表单 refresh_token，不走 AWS OIDC） |
| Kiro API Key（`ksk_...`） | 无（Key 即 Bearer） | — | 不支持 |

下文仅展开与 i-code 集成最相关的三种。

### 3.1 Social（Google / GitHub）— 最贴近标准 OAuth2 授权码

（`kiro-account-manager` `clients/kiro_auth_client.rs` + `core/deep_link_handler.rs`）

1. 浏览器打开：
   ```
   GET https://prod.us-east-1.auth.desktop.kiro.dev/login?idp={Google|Github}&redirect_uri=kiro://kiro.kiroAgent/authenticate-success&code_challenge={S256}&code_challenge_method=S256&state={uuid}
   ```
   - PKCE code_verifier 32 字节随机，SHA256 得 code_challenge（URL_SAFE_NO_PAD）
   - 回调地址是 Kiro IDE 注册的 `kiro://` deep link：`kiro://kiro.kiroAgent/authenticate-success?code=...&state=...`
2. code 换 token：
   ```
   POST https://prod.us-east-1.auth.desktop.kiro.dev/oauth/token
   Body: {"code":"...","code_verifier":"...","redirect_uri":"kiro://kiro.kiroAgent/authenticate-success"}
   UA:   KiroIDE-0.6.18-{machineId}
   ```
   响应：`{accessToken, refreshToken, profileArn?, expiresIn, idToken?, tokenType?}`
3. 刷新（不走 AWS OIDC）：
   ```
   POST https://prod.us-east-1.auth.desktop.kiro.dev/refreshToken
   Body: {"refreshToken":"..."}
   响应：{accessToken, refreshToken, expiresIn, profileArn}
   ```
   401 → `AUTH_ERROR:`；CloudFront 403 → `UPSTREAM_BLOCKED:`

> ⚠️ redirect_uri 是 `kiro://` scheme。非 Kiro 应用要复用它，需自行注册该 URL scheme（`Kiro-Go` 干脆不支持交互式 Social 登录，仅导入 refresh token）。

### 3.2 AWS Builder ID — 设备授权流（适合无回调场景）

（`Kiro-Go` `auth/builderid.go`，base = `https://oidc.{region}.amazonaws.com`）

1. 动态注册客户端：`POST /client/register` `{clientName:"Kiro", clientType:"public", scopes:[...5 个 codewhisperer scope], grantTypes:["urn:ietf:params:oauth:grant-type:device_code","refresh_token"], issuerUrl:"https://view.awsapps.com/start"}` → 返回 `clientId/clientSecret`
2. `POST /device_authorization`：`{clientId, clientSecret, startUrl}` → `{deviceCode, userCode, verificationUri, verificationUriComplete, interval, expiresIn}`
3. 用户浏览器打开 verificationUri 输码授权
4. 轮询 `POST /token`：`{clientId, clientSecret, grantType:"urn:...:device_code", deviceCode}` → 200 `{accessToken, refreshToken, expiresIn}`；400 处理 `authorization_pending / slow_down(+5s) / expired_token / access_denied`

### 3.3 AWS IAM Identity Center（企业 SSO）— 授权码 + PKCE，与 i-code 现有 OAuth 回调机制最契合

（`kiro-account-manager` `auth/providers/idc.rs` + `clients/aws_sso_client.rs`）

1. 动态注册：`POST /client/register`，`clientName`="Kiro IDE"、`clientType`="public"、`grantTypes:["authorization_code","refresh_token"]`、`redirectUris:["http://127.0.0.1/oauth/callback"]`（**不带端口**，否则拿不到 refresh_token grant）、`issuerUrl`=startUrl（Builder ID=`https://view.awsapps.com/start`）
2. 本机起临时回调服务（RFC8252 回环任意端口），拼 authorize URL：
   ```
   https://oidc.{region}.amazonaws.com/authorize?response_type=code&client_id=..&redirect_uri=http://127.0.0.1:{port}/oauth/callback&scopes=<逗号连接>&state=..&code_challenge=..&code_challenge_method=S256
   ```
   scopes = `codewhisperer:completions,analysis,conversations,transformations,taskassist`
3. code 换 token：
   ```
   POST https://oidc.{region}.amazonaws.com/token
   {"clientId":..,"clientSecret":..,"grantType":"authorization_code","code":..,"codeVerifier":..,"redirectUri":..}
   → {accessToken, refreshToken, idToken?, tokenType?, expiresIn, aws_sso_app_session_id?}
   ```
4. 刷新（走 AWS OIDC）：
   ```
   POST /token  {"clientId":..,"clientSecret":..,"grantType":"refresh_token","refreshToken":..}
   → {accessToken, refreshToken, expiresIn, profileArn}
   ```

### 3.4 Kiro API Key（`ksk_...`）— 最简通道

- 无 OAuth；Key 直接当 `Authorization: Bearer <ksk_...>` 使用；`ksk_xxx|region` 写法可显式指定 region（默认 `us-east-1`）
- `Kiro-Go` 中 API Key 账号走 `runtime.{region}.kiro.dev`（即 3.x 中的 CLI runtime），重定向/刷新一概不需要；余额与模型列表同样可用（见 §5/§6）
- machineId 若未配置，由 key 确定性派生：`sha256("KiroAPIKey/" + apiKey)` 第 64 位 hex（`Kiro-Go` `config/config.go`）

---

## 4. 模型调用 `generateAssistantResponse`

### 4.1 端点与请求头

两者实现对运行时端点写法不一致（落地需核实）：

- `Kiro-Go`（API Key/CLI runtime）：`POST https://runtime.{region}.kiro.dev/`＋头 `X-Amz-Target: AmazonCodeWhispererStreamingService.GenerateAssistantResponse`
- `kiro-account-manager`：`POST https://runtime.{region}.kiro.dev/generateAssistantResponse`，无 X-Amz-Target

公共请求头（``Kiro-Go` proxy/kiro_headers.go` / `kiro-account-manager` `gateway/proxy.rs`）：

```
Authorization: Bearer <accessToken|ksk_...>
tokentype: API_KEY             # 仅 API Key 账号（小写）；external_idp 用 EXTERNAL_IDP；其余不设
Accept: application/vnd.amazon.eventstream
Content-Type: application/json (或 application/x-amz-json-1.0)
User-Agent: aws-sdk-js/1.0.39 ua/2.1 os/{os}#{rel} lang/js md/nodejs#{ver} api/codewhispererstreaming#1.0.39 m/N KiroIDE-{ver}-{machineId}
x-amz-user-agent: aws-sdk-js/1.0.39 KiroIDE-{ver}-{machineId}
x-amzn-codewhisperer-optout: true
x-amzn-kiro-agent-mode: vibe
x-amzn-kiro-profile-arn: <profileArn>          # 有 profileArn 时
amz-sdk-invocation-id: <uuid>
amz-sdk-request: attempt=1; max=3
```

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
  "profileArn": "arn:aws:codewhisperer:<region>:<12位账号>:profile/<id>",
  "inferenceConfig": {"maxTokens":0,"temperature":0,"topP":0}   // 可选
}
```

- **system prompt 注入方式**（`kiro-account-manager` converter）：内联塞进第一条 user 消息 content，包在 `--- SYSTEM PROMPT --- ... --- END SYSTEM PROMPT ---` 内；thinking 模式在其前插入 `<thinking_mode>enabled</thinking_mode>` 等标记
- **Origin 取值**：`AI_EDITOR`（IDE/AAI）或 `KIRO_CLI`（API Key/CLI 运行时）
- 历史裁剪规则需维护 user 开头/结尾、user/assistant 交替、工具调用成对等 7 条约束

### 4.3 流式响应：AWS 二进制 EventStream（非文本 SSE）

上游回包为 `application/vnd.amazon.eventstream` 二进制帧：Prelude 12 字节（totalLength + headersLength + preludeCRC）→ headers 区（`:message-type` / `:event-type` / `:content-type`）→ payload JSON → messageCRC。

事件类型与 payload 字段：

| `:event-type` | payload 字段 | 语义 |
|---|---|---|
| `assistantResponseEvent` | `content`（string） | 模型出口文本增量（最终答案） |
| `reasoningContentEvent` | `text`（string）、`signature` | 思考增量与签名 |
| `toolUseEvent` + `toolUseId/name/input` | — | 工具调用 |
| `metadataEvent` | `stopReason` / `tokenUsage{uncachedInputTokens, cacheReadInputTokens, cacheWriteInputTokens, outputTokens, totalTokens}` | 停止原因与 token 用量 |
| `contextUsageEvent` | `contextUsagePercentage` | 上下文占用% |
| `meteringEvent` | `unit(s), usage` | 计费消耗 |
| `codeReferenceEvent` / 引用类事件 | `references[]` | 代码引用/引文 |

> **SSE 透传约束（对齐 AGENTS.md §10.1）**：客户端侧是否走 OpenAI/Anthropic SSE 由网关自行组装；到上游侧一定是二进制帧，**不能**把 EventStream 当文本 SSE 原样透传，必须先解码。

### 4.4 会话 ID 维护

- `conversationId` 策略：
  - `Kiro-Go`：`uuid5(modelId + "\n" + systemPrompt + "\n" + 首条用户消息锚点)` **确定性生成**——同会话同锚点得到同一 ID，多轮复用；空/`.`/`begin conversation` 等合成锚点则每次新生成
  - `kiro-account-manager`：取客户端 `previousResponseId`（OpenAI Responses），Anthropic 无此概念则每轮新生成 uuid；响应中 `assistantResponseEvent.conversationId` / `metadataEvent.conversationId` 返回供下轮回填；`agentContinuationId = conversationId`
- 多轮历史显式通过 `conversationState.history[]` 携带，不依赖服务端会话存储

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

> 映射表随 Kiro 上游频繁变化，两侧实现均以「可用模型列表动态校验 + 降级兜底」为准。

---

## 5. 模型列表 `ListAvailableModels`

| 来源 | 端点 | 说明 |
|---|---|---|
| `kiro-account-manager`（control plane） | `POST https://management.{region}.kiro.dev/`，头 `Content-Type: application/x-amz-json-1.0`、`X-Amz-Target: KiroControlPlaneBearerService.ListAvailableModels`，body `{"origin":"AI_EDITOR","profileArn":"..."}` | UA `api/kirocontrolplanebearer#1.0.0` |
| `Kiro-Go`（AWS REST） | `GET https://codewhisperer.us-east-1.amazonaws.com/ListAvailableModels?origin=AI_EDITOR&maxResults=50&profileArn=...` | OAuth 账号 |

配套 `ListAvailableProfiles`：同一基座，空 body `{}`，用于在账号缺少 profileArn 时取首个可用 arn。

响应结构（`AvailableModel` / `ModelInfo`）：

```jsonc
{
  "models": [
    {
      "modelId": "claude-sonnet-4.5",        // 上游实际模型 ID
      "modelName": "", "description": "",
      "provider": "...",
      "capabilities": ["AGENTIC_REQUEST", ...],
      "contextWindow": 200000,
      "rateMultiplier": 1.0,
      "promptCaching": {"supportsPromptCaching": true, "maximumCacheCheckpointsPerRequest": 3, "minimumTokensPerCacheCheckpoint": 1024},
      "supportedInputTypes": ["TEXT","IMAGE"],
      "tokenLimits": {"maxInputTokens": 200000, "maxOutputTokens": 8192},
      "isDefault": false
    }
  ],
  "nextToken": "...", "defaultModel": {...}
}
```

两实现均对结果做了 30 分钟缓存（`AVAILABLE_MODELS_CACHE_TTL_SECONDS`），并归一化：默认模型置顶、`defaultModel` 补齐、排空非法 modelId。

---

## 6. 余额 / 配额 `getUsageLimits`

### 6.1 端点与请求头

- `kiro-account-manager`：`GET https://management.{region}.kiro.dev/getUsageLimits?isEmailRequired=true&origin=AI_EDITOR&resourceType=AGENTIC_REQUEST`
- `Kiro-Go`：`GET https://codewhisperer.us-east-1.amazonaws.com/getUsageLimits?origin=AI_EDITOR&resourceType=AGENTIC_REQUEST&isEmailRequired=true&profileArn=...`（API Key 账号不带 profileArn）；超额（overage）状态另走 `https://q.us-east-1.amazonaws.com/getUsageLimits`

请求头：`Authorization: Bearer <access>`、UA 为 management 型（`api/codewhispererruntime#1.0.0`）、`amz-sdk-invocation-id`、`amz-sdk-request`、`accept: application/json`。

region 处理：账号 region 优先 → 企业账号回退 `us-east-1, eu-central-1`；`AUTH_ERROR:` / `BANNED:` 立即返回不换区。

### 6.2 响应结构与字段含义

```jsonc
{
  "subscriptionInfo": {
    "subscriptionName": "", "subscriptionTitle": "KIRO PRO+",
    "subscriptionType": "", "status": "",
    "overageCapability": "OVERAGE_CAPABLE|OVERAGE_INCAPABLE",  // 有无资格开超额
    "upgradeCapability": ""
  },
  "overageConfiguration": { "overageStatus": "ENABLED|DISABLED|UNKNOWN" },
  "usageBreakdownList": [{
    "resourceType": "AGENTIC_REQUEST",
    "currentUsage": 12.5,                // 优先 *WithPrecision 后缀字段
    "usageLimit": 100,
    "overageCap": 0, "currentOverages": 0, "overageRate": 0.0, "overageCharges": 0,
    "displayName": "", "unit": "units", "currency": "USD",
    "nextDateReset": 0,                   // epoch 秒：配额重置日
    "freeTrialInfo": {"freeTrialStatus":"ACTIVE|EXPIRED","currentUsage":0,"usageLimit":0,"freeTrialExpiry":0},
    "bonuses": [{"status":"ACTIVE","expiresAt":0,"currentUsage":0,"usageLimit":0,"displayName":""}]
  }],
  "nextDateReset": 0, "limits": 0, "totalUsage": 0, "daysUntilReset": 0,
  "userInfo": {"userId": "...", "email": "..."}
}
```

判定口径（`kiro-account-manager` `core/usage.rs`）：
- **有效额度** `effective_limit = limit + overage_cap`（仅当 `overageStatus==ENABLED`）
- **剩余可用** = (主额度 + 试用(仅 ACTIVE) + 奖励(仅 ACTIVE 未过期) + 超额) − 已用**
- `is_in_overage`：ENABLED 且 `current > limit && current < limit+cap`
- `is_usage_capped`：剩余 ≤ 0（封顶状态）
- 账号状态判定：403 `TemporarilySuspended`/`suspended`/423 → `BANNED`；401 → `AUTH_ERROR`

---

## 7. 两项目差异对比

| 维度 | Kiro-Go | kiro-account-manager |
|------|---------|----------------------|
| 形态 | 服务端代理（Docker/自建） | 桌面应用（Tauri 2） |
| Social 登录 | 不支持交互式（仅导入 refresh token） | ✅ 浏览器 + `kiro://` deep link |
| Builder ID | OIDC 设备流 | 授权码 + PKCE（localhost 回调） |
| 企业 SSO | 授权码 + PKCE | 授权码 + PKCE |
| API Key | ✅ `runtime.{region}.kiro.dev` + `X-Amz-Target` | 仅 Kiro2API 客户端鉴权用（`Authorization`/`x-api-key`） |
| 模型调用端点 | `runtime.{region}.kiro.dev/`（或 AWS 原生 `q.`/`codewhisperer.`） | `runtime.{region}.kiro.dev/generateAssistantResponse` |
| 模型列表 | `GET /ListAvailableModels`（AWS REST） | `POST /` + `KiroControlPlaneBearerService.ListAvailableModels` |
| 余额 | `GET /getUsageLimits`（AWS REST + q overage 扩展） | `GET /getUsageLimits`（management.kiro.dev） |
| 流式 | 事件解码 → OpenAI/Anthropic SSE | EventStream 解码 → Anthropic `message_start/content_block_*` / OpenAI chunk |
| token 刷新 | 统一 `auth/oidc.go`（API Key 除外） | 60s 后台定时器 + 网关侧过期 3 分钟阈值回刷 |
| 模型映射 | `translator.go` 别名 + `-thinking` 后缀 | `converter.rs` 全表映射 + 可用集校验降级 |

共同点：都依赖 **User-Agent 伪装**（`aws-sdk-js` 版本号 + `KiroIDE-{ver}-{machineId}`）、**machineId**（UA 后缀 + 账号持久化）、**profileArn**（OAuth 账号必传，API Key 不传）、**region 派生链**（profileArn → account.region → 默认 us-east-1）。

---

## 8. 对 i-code 的参考意义（仅记录，待定）

若在 i-code 新增 Kiro 供应商渠道，按 §2 的四基座映射到现有能力：

1. **认证**：优先复用通用 `OAuth2Auth`（`method: 'oauth2'`，支持 `authorization_code` / `device_code` + PKCE + redirectUri）：
   - Builder ID → `grantType: device_code`（与现有 Device Code 轮询组件契合）
   - 企业 SSO → `authorization_code` + PKCE，redirect_uri 用本地回环（i-code 已有 `CallbackServerInfo` 回调服务器机制）
   - Social（Google/GitHub）→ 需注册 `kiro://` URL scheme 或规避（Kiro-Go 方案为直接导入 refresh token）
   - API Key 通道 → 现有 `api-key` 认证即可覆盖，成本最低，可先做
   - token 字段结构与现有 `OAuth2TokenData`（accessToken / refreshToken / expiresAt）兼容；过期刷新仿 `kiro-account-manager`：提前 10 分钟后台定时刷新 + 请求遇 401/403 回刷重试一次
2. **模型列表**：`ListAvailableModels`（control plane 或 AWS REST 任一）返回的 `modelId` 直接落 `official` 来源的 gateway_models；`autoFetchOfficialModels` 可开启；30 分钟缓存语义与现有 `official_model_cache` 一致
3. **模型调用**：网关需新增 CodeWhisperer 适配器（`gateway-runtime`）：
   - 上游 `generateAssistantResponse` + AWS EventStream **二进制解码**（Prelude/Headers/CRC）——这是与现有一切文本 SSE 上游最大的差异点
   - `conversationId` 确定性派生（仿 Kiro-Go uuid5）或回填策略（仿 kiro-account-manager）
   - system prompt/tools 需要内联转成 `userInputMessage` 携带；思考模式映射为 `reasoningContentEvent`
   - `origin`：HTTP 网关场景用 `AI_EDITOR`，CLI 场景用 `KIRO_CLI`（API Key 固定 `KIRO_CLI`）
4. **余额**：`getUsageLimits` 响应与 i-code 现有 `balance` 模块的监控模型天然对应（额度/已用/重置日/状态），region 回退链与 overage 判定可复刻到 balance provider 脚本或内置 provider
5. **模型预设**：`claude-sonnet-4.5` / `claude-opus-4.7` / `claude-haiku-4.5` 等可作为内置模型 + `builtin_providers` 预设；`-thinking` 后缀语义与现有 thinking 配置对接
6. **风险提示**：上游为 AWS/非官方衍生通道，协议变更频繁、UA/machineId 被风控时可能 403；`tokentype`、端点路径、列表结构三处两参考项目不一致，**实现前建议用官方 Kiro IDE 抓一次真实请求为准**（参考 `docs/log-framework.md` 中网关日志抓取 SSE/来自流式请求正文的机制）

相关仓库：
- `https://github.com/Quorinex/Kiro-Go`
- `https://github.com/hj01857655/kiro-account-manager`
- 官方入口：`https://app.kiro.dev`（Social 登录/微软门户）、`https://runtime.{region}.kiro.dev`、`https://management.{region}.kiro.dev`