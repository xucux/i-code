# Usage Guide

i-code is a local AI gateway and CLI configuration manager. This guide walks you through the complete workflow from initialization to daily use:

| Step | Content | Status |
|------|---------|--------|
| 1 | Set password | Required on first use |
| 2 | Start the gateway and add authorized API keys | Required |
| 3 | Add a provider | Required |
| 4 | Add models | Required |
| 5 | Advanced settings (provider-level proxy) | Optional |
| 6 | Verify the gateway | Recommended |
| 7 | Use with CLIs (Claude Code / Codex / OpenCode) | As needed |
| 8 | Use with other software (VS Code / Trae / CC Switch) | As needed |
| 9 | More features | — |
| 10 | FAQ | — |



## 1. Set Password



After installing the software, set a password in the system settings. This password is used to encrypt important information such as API keys and OAuth tokens (it also encrypts remote backup files).

Notes:

- The password must be **1-20 characters** long
- **Keep it safe**: **after changing the password, data encrypted with the old password can no longer be decrypted**, and you will need to re-enter provider API keys



![image-20260802231055141](images.assets/Guide.assets/image-20260802231055141.png)





## 2. Start the Gateway and Add Authorized API Keys



### 2.1. Start the Gateway



In the gateway settings, enable gateway authentication and initialize the default key, then start the gateway (if the port cannot be bound, run i-code as administrator).

- By default the gateway listens on `127.0.0.1:54321`; the host and port can be changed in gateway settings (use `0.0.0.0` to allow access from other devices on the LAN)
- When authentication is enabled, all external requests must carry `Authorization: Bearer <gateway key>`, otherwise a 401 is returned

![image-20260802231346822](images.assets/Guide.assets/image-20260802231346822.png)



### 2.2. Add Authorized API Keys

![image-20260802232424836](images.assets/Guide.assets/image-20260802232424836.png)

Besides the default key, you can create multiple authorized keys for different clients, so they can be distributed and revoked independently:

- Each key supports a name, description, enable/disable state, and an expiration date
- After creation, you can quickly copy the key.

![image-20260802232501961](images.assets/Guide.assets/image-20260802232501961.png)



## 3. Add a Provider



You can create a provider from presets or manually. Built-in presets contain the basic information of common providers (API base URL, protocol, default models, etc.), which reduces manual configuration.



![image-20260802231546639](images.assets/Guide.assets/image-20260802231546639.png)



When adding a provider, first complete the basic information and fill in the provider's **API key**, then save.

> Besides API keys, some providers support other authentication methods such as OAuth (e.g. Google Gemini OAuth, GitHub Copilot). Complete the authorization as prompted in the authentication settings. API keys are only stored encrypted (AES-GCM); they never appear in plain text in configuration files or logs.

![image-20260802231712189](images.assets/Guide.assets/image-20260802231712189.png)



## 4. Add Models

You must first save step 3, then click "Edit Provider" to enter the models screen.

Models can be added in two ways:

- **Fetch the official model list**: fetch the live model list from the provider API
- **Built-in model library**: pick from the common models bundled with the app

Fetch the official model list.

![image-20260802231809338](images.assets/Guide.assets/image-20260802231809338.png)

Select models from the official list and add them. When there are many models, use the search box to filter by model ID.

Once added, each model is exposed with a route ID in the `{provider_slug}/{model_id}` format (e.g. `openai/gpt-4o`); clients request models by this ID and the gateway routes them to the real provider.

![image-20260802232050679](images.assets/Guide.assets/image-20260802232050679.png)



## 5. Advanced Settings



Provider-level proxy settings are supported: when a specific provider must be accessed through a particular proxy, configure the proxy for that provider only, without affecting other providers or the global proxy.



![image-20260802232133020](images.assets/Guide.assets/image-20260802232133020.png)



## 6. Verify the Gateway

After the gateway is started, verify connectivity through its API:

```bash
# List models available through the gateway
curl http://127.0.0.1:54321/v1/models \
  -H "Authorization: Bearer <your gateway key>"

# Send a chat request (OpenAI-compatible format, model uses the route ID)
curl http://127.0.0.1:54321/v1/chat/completions \
  -H "Authorization: Bearer <your gateway key>" \
  -H "Content-Type: application/json" \
  -d '{"model": "openai/gpt-4o", "messages": [{"role": "user", "content": "hello"}]}'
```

Common endpoints:

| Endpoint | Description |
|----------|-------------|
| `GET /health` | Health check |
| `GET /v1/models` | Available model list |
| `POST /v1/chat/completions` | OpenAI Chat Completions compatible API |
| `POST /v1/messages` | Anthropic Messages compatible API |
| `POST /v1/responses` | OpenAI Responses compatible API |

## 7. Use with CLIs

The app has a built-in CLI manager (sidebar → CLI) that maintains configuration profiles for **Claude Code**, **Codex**, and **OpenCode**, so you don't have to edit client config files by hand:

1. Go to the client tab and add a provider binding
2. Choose the routing mode:
   - **Via local gateway**: `base_url` points to the local gateway, and model mappings use the `{provider_slug}/{model_id}` format
   - **Direct connection**: fill in the provider's real `base_url`, and model mappings use the upstream original model IDs
3. Configure model mappings (Claude Code supports per-role mappings for Sonnet / Opus / Haiku, etc.)
4. Click **Apply** to write the configuration into the client's config file (e.g. `settings.json` for Claude Code, `config.toml` for Codex)

## 8. Use with Other Software



### 8.1. VS Code (Unify Chat Provider)

After installing the Unify Chat Provider extension in VS Code:

1. Create a provider with the Base URL set to the local gateway (default `http://127.0.0.1:54321`) and the API key set to a gateway key
2. Fetch the official model list and pick the models you need (the list comes from the gateway's `/v1/models`; model IDs are route IDs)

Press `ctrl+shift+p` in VS Code to open the command palette, then find "Manage Providers"

![Manage Providers](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadYdqmpUifTQR5BLYrZ4unyLdXxfQSQACiSoAAh062FQy3E0iH-gcLT0E.png)

Choose "Add Provider" and enter the following name, protocol, and address:

```
i-code
OpenAI Chat Completions
http://127.0.0.1:54321/v1
```

![image-20260802232830599](images.assets/Guide.assets/image-20260802232830599.png)

Enter the API key

![API Key input](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadYlqmpUojCFJqVuemK02rTZZATXzDQACiyoAAh062FTFFcG-Owd5aj0E.png)

At the bottom, click 【Save】 to save the provider

![Save provider](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadYpqmpUrHVSDwi-_MXRSTrCvPsb0uwACjCoAAh062FQeCCRCeTb1Yj0E.png)

Then re-enter and choose 【Add from official model list】

![image-20260802233007955](images.assets/Guide.assets/image-20260802233007955.png)



![image-20260802233106420](images.assets/Guide.assets/image-20260802233106420.png)



### 8.2. Use i-code Proxied Models in Trae

In Trae settings, add a model

![Trae add model](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadWNqmpMMkasHCqZ_EieqHLs4miCy5wACXyoAAh062FT0UIv_faCK_D0E.png)

Add it as follows:

The key, model ID, and address can all be copied from i-code

![Trae provider settings](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadWVqmpMSCqAisUYowA1l005WdpDRWAACYSoAAh062FQwVk5XFOJ7jT0E.png)

Use the custom model

![Use custom model](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadWJqmpMJiJOf8RXiLNiw736_aNw26QACXioAAh062FRE-5MsiAsscj0E.png)

### 8.3. Use i-code Proxied Models in CC Switch

Add a provider in cc-switch

![Add provider in cc-switch](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadXNqmpPFzBTKEqCrUFDN6ljj1M5ZNAACcSoAAh062FQIXDCJaOUAAaY9BA.png)

Models can be fetched directly — no need to type them yourself

![Fetch models in cc-switch](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadXFqmpO_Y5d82SWH5RVFSHKMbV5z0QACbyoAAh062FTfibS-s2z2oz0E.png)

The configuration JSON looks like:

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

Use with the Claude Code CLI

![Use with Claude Code CLI](images.assets/GUIDE.assets/BQACAgUAAyEGAASHRsPbAAEadXJqmpPCQh4_sVtg-_mIuFOVrQklIAACcCoAAh062FT-p1ExRbntFz0E.png)

## 9. More Features

| Feature | Description |
|---------|-------------|
| Virtual providers | Expose models under a fixed alias with multiple underlying candidate providers and automatic failover, invisible to clients |
| Chat | Built-in chat UI to test gateway models directly, with streaming support |
| Image generation | Built-in vision generation workbench and gallery |
| Balance monitoring | Query provider balances; supports custom Rhai script templates and the template marketplace |
| Call statistics | Records usage, latency, and other details for every model call |
| Logs | View gateway request logs in-app, with filtering and export |
| Backup | Local backup and WebDAV remote backup |

## 10. FAQ

**Q: The gateway fails to bind the port?**

The default port `54321` may be occupied, or permissions are insufficient. Run i-code as administrator, or change the port in gateway settings.

**Q: Client requests return 401?**

Make sure gateway authentication is enabled and the request carries a valid `Authorization: Bearer <gateway key>`; also confirm the key is not disabled or expired.

**Q: Requests report model not found?**

The `model` field must use a route ID in the `{provider_slug}/{model_id}` format. Request `GET /v1/models` to see the available list.

**Q: What if I forget the password?**

The password derives the encryption key and cannot be recovered. After changing it, data encrypted with the old password can no longer be decrypted, and provider API keys must be re-entered.