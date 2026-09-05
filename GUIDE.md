# 使用指南

i-code 是一款本地 AI 网关与 CLI 配置管理工具。本指南将带你完成从初始化到实际使用的完整流程：

| 步骤 | 内容 | 状态 |
|------|------|------|
| 1 | 设置密码 | 首次使用必须 |
| 2 | 开启网关并新增授权 API Key | 必须 |
| 3 | 新增供应商 | 必须 |
| 4 | 添加模型 | 必须 |
| 5 | 高级设置（供应商级代理） | 可选 |
| 6 | 验证网关连通性 | 推荐 |
| 7 | 在 CLI 中使用（Claude Code / Codex / OpenCode） | 按需 |
| 8 | 在其他软件中使用（VS Code / Trae / CC Switch） | 按需 |
| 9 | 更多功能 | — |
| 10 | 常见问题 | — |



## 1. 设置密码



安装软件之后，在系统设置中设置密码，该密码用于加密ApiKey、oauthToken等重要信息（同时用于远端备份文件的加密）。

注意事项：

- 密码长度为 **1-20 位**
- **请务必牢记该密码**：**修改密码后，旧密码加密的数据将无法解密**，需要重新填写各供应商的 API Key



![image-20260802231055141](images.assets/Guide.assets/image-20260802231055141.png)





## 2. 开启网关并新增授权ApiKey



### 2.1. 开启网关



在网关设置中，开启网关鉴权、初始化默认密钥，最后打开网关（如果无法开启端口监听，请以管理员身份运行i-code）。

- 网关默认监听 `127.0.0.1:54321`，监听地址与端口可在网关设置中修改（如需局域网内其他设备访问，可将监听地址改为 `0.0.0.0`）
- 开启鉴权后，所有外部请求都必须携带 `Authorization: Bearer <网关密钥>`，否则会返回 401

![image-20260802231346822](images.assets/Guide.assets/image-20260802231346822.png)



### 2.2. 新增授权ApiKey

![image-20260802232424836](images.assets/Guide.assets/image-20260802232424836.png)

除默认密钥外，可按客户端创建多个授权 Key，便于独立分发与停用：

- 支持设置名称、描述、启用/停用状态与有效期
- 创建完成后，支持快速复制

![image-20260802232501961](images.assets/Guide.assets/image-20260802232501961.png)



## 3. 新增供应商



支持从预设中、手动方式新建供应商。内置预设包含常见供应商的基础信息（API 地址、协议、默认模型等），选择预设可减少手动配置



![image-20260802231546639](images.assets/Guide.assets/image-20260802231546639.png)



添加供应商时，先完成基础信息配置，以及供应商的**ApiKey**填写；然后保存

> 除 API Key 外，部分供应商还支持 OAuth 等其他认证方式（如 Google Gemini OAuth、GitHub Copilot 等），可在认证设置中按提示完成授权。API Key 仅以密文保存（AES-GCM 加密），配置文件与日志中不会出现明文。

![image-20260802231712189](images.assets/Guide.assets/image-20260802231712189.png)



## 4. 添加模型

必须先完成第三步骤的保存，在点击编辑供应商，进入模型界面。

模型支持两种添加方式：

- **拉取官方模型列表**：从供应商 API 拉取实时模型列表
- **内置模型库**：从应用内置的常见模型列表中选择

拉取官方模型列表

![image-20260802231809338](images.assets/Guide.assets/image-20260802231809338.png)

从官方模型中勾选，并添加。官方模型较多时，可使用搜索框按模型 ID 过滤

添加完成后，模型对外使用 `{provider_slug}/{model_id}` 格式的路由 ID（如 `openai/gpt-4o`），客户端请求时通过该 ID 由网关路由到真实供应商

![image-20260802232050679](images.assets/Guide.assets/image-20260802232050679.png)



## 5. 高级设置



支持供应商级别的代理设置：当某个供应商需要通过特定代理访问时，可单独为其配置代理，不影响其他供应商与全局代理配置



![image-20260802232133020](images.assets/Guide.assets/image-20260802232133020.png)



## 6. 验证网关

网关启动后，可通过接口验证连通性：

```bash
# 查看网关可用模型列表
curl http://127.0.0.1:54321/v1/models \
  -H "Authorization: Bearer <你的网关密钥>"

# 发起一次对话（OpenAI 兼容格式，model 使用路由 ID）
curl http://127.0.0.1:54321/v1/chat/completions \
  -H "Authorization: Bearer <你的网关密钥>" \
  -H "Content-Type: application/json" \
  -d '{"model": "openai/gpt-4o", "messages": [{"role": "user", "content": "你好"}]}'
```

常用端点一览：

| 端点 | 说明 |
|------|------|
| `GET /health` | 健康检查 |
| `GET /v1/models` | 可用模型列表 |
| `POST /v1/chat/completions` | OpenAI Chat Completions 兼容接口 |
| `POST /v1/messages` | Anthropic Messages 兼容接口 |
| `POST /v1/responses` | OpenAI Responses 兼容接口 |

## 7. 在 CLI 中使用

应用内置 CLI 管理（侧栏 → CLI），为 **Claude Code**、**Codex**、**OpenCode** 维护配置档案，无需手动编辑客户端配置文件：

1. 进入对应客户端 Tab，新增供应商绑定
2. 选择路由模式：
   - **经本地网关**：`base_url` 指向本地网关，模型映射使用 `{provider_slug}/{model_id}` 格式
   - **直连**：填写供应商真实 `base_url`，模型映射使用上游原始模型 ID
3. 配置模型映射（Claude Code 支持按 Sonnet / Opus / Haiku 等角色分别映射）
4. 点击**应用**，配置将写入客户端对应的配置文件（如 Claude Code 的 `settings.json`、Codex 的 `config.toml`）

## 8. 在其他软件中使用



### 8.1. VS Code (Unify Chat Provider)

在 VS Code 中安装 Unify Chat Provider 插件后：

1. 创建供应商，Base URL 填写本地网关地址（默认 `http://127.0.0.1:54321`），API Key 填写网关授权 Key
2. 拉取官方模型列表并选择需要的模型（列表来自网关的 `/v1/models`，模型 ID 即路由 ID）

VS Code的`ctrl+shift+p`拉起控制面板，找到管理供应商

![管理供应商.png](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadYdqmpUifTQR5BLYrZ4unyLdXxfQSQACiSoAAh062FQy3E0iH-gcLT0E.png)

选择新增供应商，输入下面的名称，协议，地址
```
i-code
OpenAI Chat Completions
http://127.0.0.1:54321/v1
```

![image-20260802232830599](images.assets/Guide.assets/image-20260802232830599.png)

API Key输入
![API Key输入.png](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadYlqmpUojCFJqVuemK02rTZZATXzDQACiyoAAh062FTFFcG-Owd5aj0E.png)

最下面，点击【保存】，然后保存供应商
![保存供应商.png](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadYpqmpUrHVSDwi-_MXRSTrCvPsb0uwACjCoAAh062FQeCCRCeTb1Yj0E.png)

然后重新进入，选择【从官方模型列表添加】

![image-20260802233007955](images.assets/Guide.assets/image-20260802233007955.png)



![image-20260802233106420](images.assets/Guide.assets/image-20260802233106420.png)



### 8.2. Trae 使用i-code代理模型

trae设置，添加模型
![bb41cf25-5fc2-4be2-bf05-de68aafdc48b.png](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadWNqmpMMkasHCqZ_EieqHLs4miCy5wACXyoAAh062FT0UIv_faCK_D0E.png)

按如下添加：
密钥，模型id，地址都可以从i-code中复制
![fd9917e9-68a6-40aa-ba6f-2c5ffaac0eb1.png](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadWVqmpMSCqAisUYowA1l005WdpDRWAACYSoAAh062FQwVk5XFOJ7jT0E.png)

使用自定义模型

![268c757d-dc79-41e0-abfb-efcec5667ec8.png](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadWJqmpMJiJOf8RXiLNiw736_aNw26QACXioAAh062FRE-5MsiAsscj0E.png)

### 8.3. CC Switch 使用i-code代理模型

在cc-switch中新增供应商
![ed7a10d5-7014-4964-a506-aa969d6d2579.png](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadXNqmpPFzBTKEqCrUFDN6ljj1M5ZNAACcSoAAh062FQIXDCJaOUAAaY9BA.png)

支持直接拉取模型，不用自己输入

![72a97e46-0ab4-4877-89bc-b2a72fed7315.png](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadXFqmpO_Y5d82SWH5RVFSHKMbV5z0QACbyoAAh062FTfibS-s2z2oz0E.png)

配置json类似
```json
{
  "env": {
    "ANTHROPIC_AUTH_TOKEN": "sk-icode-xxxxxxxxxxxxxxxxxxxxxxxxxxx",
    "ANTHROPIC_BASE_URL": "http://127.0.0.1:54321",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "cline-free/cline-free/glm-5.2[1M]",
    "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME": "cline-free/cline-free/glm-5.2",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "cline-free/cline-free/glm-5.2[1M]",
    "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME": "cline-free/cline-free/glm-5.2",
    "ANTHROPIC_DEFAULT_FABLE_MODEL": "cline-free/cline-free/glm-5.2[1M]",
    "ANTHROPIC_DEFAULT_FABLE_MODEL_NAME": "cline-free/cline-free/glm-5.2",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "cline-free/cline-free/glm-5.2",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME": "cline-free/cline-free/glm-5.2"
  }
}
```
claude code cli使用

![c7f4cfba-a64d-4870-9bd2-0ad964d046c9.png](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadXJqmpPCQh4_sVtg-_mIuFOVrQklIAACcCoAAh062FT-p1ExRbntFz0E.png)



## 9. 更多功能

| 功能 | 说明 |
|------|------|
| 虚拟供应商 | 以固定别名对外提供模型，底层配置多个候选真实供应商，自动故障转移，客户端无感知 |
| 聊天 | 应用内聊天界面，直接测试网关模型，支持流式输出 |
| 图像生成 | 内置视觉生成工作台与画廊，支持文生图 |
| 额度监控 | 查询供应商余额，支持自定义 Rhai 脚本模板与模板市场 |
| 调用统计 | 记录每次模型调用的用量、耗时等明细 |
| 日志 | 应用内查看网关请求日志，支持按来源/级别/时间筛选与导出 |
| 备份 | 支持本地备份与 WebDAV 远端备份 |

## 10. 常见问题

**Q：网关无法开启端口监听？**

默认端口 `54321` 被占用或权限不足时无法监听。请以管理员身份运行 i-code，或在网关设置中更换端口。

**Q：客户端请求返回 401？**

确认网关设置中已开启鉴权，且请求携带了正确的 `Authorization: Bearer <网关密钥>`；确认所用授权 Key 未被停用或已过期。

**Q：请求提示模型不存在？**

`model` 字段必须使用网关中的路由 ID（格式 `{provider_slug}/{model_id}`），可请求 `GET /v1/models` 查看可用列表。

**Q：忘记密码怎么办？**

密码用于派生加密密钥，不提供找回；修改密码后，旧密码加密的数据将无法解密，需要重新填写各供应商的 API Key。