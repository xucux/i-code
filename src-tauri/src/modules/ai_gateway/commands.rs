//! # AI Gateway 模块 Tauri Command 声明
//!
//! 前端通过 `invoke('gateway_*', payload)` 调用这些命令。
//!
//! ## 命令清单
//!
//! ### 供应商
//! - `gateway_provider_list`：列出所有供应商
//! - `gateway_provider_get`：获取供应商详情
//! - `gateway_provider_create`：创建供应商
//! - `gateway_provider_update`：更新供应商
//! - `gateway_provider_delete`：删除供应商
//!
//! ### 模型配置
//! - `gateway_model_config_create`：创建模型配置
//! - `gateway_model_config_get`：获取模型配置详情
//! - `gateway_model_config_list`：列出所有模型配置
//! - `gateway_model_config_update`：更新模型配置
//! - `gateway_model_config_delete`：删除模型配置
//!
//! ### 网关模型
//! - `gateway_model_create`：创建网关模型（暴露某个模型）
//! - `gateway_model_get`：获取网关模型详情
//! - `gateway_model_list`：列出所有网关模型
//! - `gateway_model_list_by_provider`：列出指定供应商下的网关模型
//! - `gateway_model_delete`：删除网关模型
//!
//! ### 暴露模型
//! - `gateway_exposed_models`：列出所有对外暴露的模型（`/v1/models` 数据源）

use tauri::{Emitter, State};

use crate::error::{IcodeError, IcodeResult};

use super::service::AiGatewayServiceHandle;
use super::seed::{BuiltinModel, BuiltinProvider};
use super::auth::DeviceCodeInfo;
use super::auth::OAuthStartResult;
use super::types::{
    AuthMethod, CreateGatewayAuthKeyInput, CreateGatewayModelInput, CreateModelConfigInput,
    CreateProviderInput, DeviceCodePollResult, ExposedModel, ExportProviderInput, GatewayAuthKey,
    GatewayModel, GatewaySettings, ImportProviderInput, ModelConfig, Provider,
    UpdateGatewayAuthKeyInput, UpdateGatewayModelInput, UpdateGatewaySettingsInput,
    UpdateModelConfigInput, UpdateProviderInput,
};

// ===== 供应商命令 =====

/// 列出所有供应商
#[tauri::command]
pub async fn gateway_provider_list(
    state: State<'_, AiGatewayServiceHandle>,
) -> IcodeResult<Vec<Provider>> {
    state.service().list_providers()
}

/// 获取供应商详情
#[tauri::command]
pub async fn gateway_provider_get(
    state: State<'_, AiGatewayServiceHandle>,
    id: String,
) -> IcodeResult<Provider> {
    state.service().get_provider(&id)
}

/// 查询供应商级附加请求头（供前端编辑表单回填）
///
/// 返回完整行记录；value 可能为 `$SECRET:{uuid}$` 引用，原样返回，
/// 前端编辑后原样回传，转发时由后端统一解密。
#[tauri::command]
pub async fn gateway_provider_extra_headers_list(
    _state: State<'_, AiGatewayServiceHandle>,
    provider_id: String,
) -> IcodeResult<Vec<super::types::ProviderExtraHeader>> {
    super::repository::list_provider_extra_header_rows(&provider_id)
}

/// 创建供应商
///
/// 若 `auth` 中的敏感字段（如 api_key）为明文，Service 层会自动加密并替换为引用。
/// 成功后广播 `provider:changed` 事件（payload: `{ action, providerId }`），
/// 供托盘菜单、供应商列表等监听方自动刷新。
///
/// 业务日志：若 `input.proxy_json` 非空，写入 system 级业务 logger（脱敏后）。
#[tauri::command]
pub async fn gateway_provider_create(
    app_handle: tauri::AppHandle,
    state: State<'_, AiGatewayServiceHandle>,
    input: CreateProviderInput,
) -> IcodeResult<Provider> {
    // 提取代理配置信息用于业务日志（在 input 被消费前）
    let proxy_log = input.proxy_json.as_deref().and_then(parse_proxy_log);
    let provider = state.service().create_provider(input)?;
    // 业务日志：供应商代理配置已设置（脱敏，不含认证信息）
    if let Some(log_str) = proxy_log {
        let msg = format!(
            "供应商代理配置已设置 | provider={} slug={} | {}",
            provider.display_name, provider.slug, log_str
        );
        tracing::info!("{}", msg);
        crate::modules::logger::Log::info(&msg);
    }
    let _ = app_handle.emit(
        "provider:changed",
        ProviderChangedPayload { action: "create", provider_id: provider.id.clone() },
    );
    Ok(provider)
}

/// 更新供应商
///
/// 成功后广播 `provider:changed` 事件。
/// 注意：若 `balance_provider_json` 被清空（关闭额度监控），托盘额度子菜单会
/// 通过 `list_balance_snapshots()` 的过滤逻辑自动移除对应项。
///
/// 业务日志：若 `input.proxy_json` 非空，写入 system 级业务 logger（脱敏后）。
#[tauri::command]
pub async fn gateway_provider_update(
    app_handle: tauri::AppHandle,
    state: State<'_, AiGatewayServiceHandle>,
    id: String,
    input: UpdateProviderInput,
) -> IcodeResult<Provider> {
    // 提取代理配置信息用于业务日志（在 input 被消费前）
    let proxy_log = input.proxy_json.as_deref().and_then(parse_proxy_log);
    let provider = state.service().update_provider(&id, input)?;
    // 业务日志：供应商代理配置已更新（脱敏，不含认证信息）
    if let Some(log_str) = proxy_log {
        let msg = format!(
            "供应商代理配置已更新 | provider={} slug={} | {}",
            provider.display_name, provider.slug, log_str
        );
        tracing::info!("{}", msg);
        crate::modules::logger::Log::info(&msg);
    }
    let _ = app_handle.emit(
        "provider:changed",
        ProviderChangedPayload { action: "update", provider_id: provider.id.clone() },
    );
    Ok(provider)
}

/// 删除供应商
///
/// 关联的 gateway_models 会通过外键级联删除。
/// 成功后广播 `provider:changed` 事件（action: "delete"），托盘额度子菜单会
/// 自动移除该供应商的菜单项（快照表也会通过外键级联清理）。
#[tauri::command]
pub async fn gateway_provider_delete(
    app_handle: tauri::AppHandle,
    state: State<'_, AiGatewayServiceHandle>,
    id: String,
) -> IcodeResult<()> {
    state.service().delete_provider(&id)?;
    let _ = app_handle.emit(
        "provider:changed",
        ProviderChangedPayload { action: "delete", provider_id: id },
    );
    Ok(())
}

/// `provider:changed` 事件 payload
///
/// 字段使用 camelCase 序列化，与前端 `FrontendEvents.PROVIDER_CHANGED` 类型对齐。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderChangedPayload {
    /// 变更类型：`create` / `update` / `delete`
    action: &'static str,
    /// 供应商 ID
    provider_id: String,
}

/// 解析供应商代理 JSON 并返回脱敏后的日志描述
///
/// 用于 `gateway_provider_create/update` 在 input 被消费前提取代理信息，
/// 持久化成功后写入业务 logger。解析失败时返回 None（不打日志，避免噪音）。
///
/// 脱敏由 `ProxyConfig::to_log_string` 完成，隐藏 URL 中的 `user:pass@` 部分。
fn parse_proxy_log(json: &str) -> Option<String> {
    serde_json::from_str::<crate::modules::shared::ProxyConfig>(json)
        .ok()
        .map(|p| p.to_log_string())
}

/// 导出供应商配置
///
/// 返回 base64 编码的 JSON 字符串，包含供应商信息、模型列表与配置信息。
/// `include_secrets = true` 时，auth_json 中的敏感字段以明文形式导出；
/// `include_secrets = false` 时，敏感字段会被清空。
#[tauri::command]
pub async fn gateway_provider_export(
    state: State<'_, AiGatewayServiceHandle>,
    input: ExportProviderInput,
) -> IcodeResult<String> {
    state.service().export_provider(input)
}

/// 导入供应商配置
///
/// 接收 base64 编码的 JSON 导出数据，解析后创建新的供应商及其模型。
/// 若 slug 冲突且 `conflict_strategy` 为 `auto_rename`（默认），
/// 会自动生成不冲突的 slug（追加 `-imported` 后缀）。
#[tauri::command]
pub async fn gateway_provider_import(
    state: State<'_, AiGatewayServiceHandle>,
    input: ImportProviderInput,
) -> IcodeResult<Provider> {
    state.service().import_provider(input)
}

// ===== 模型配置命令 =====

/// 创建模型配置
#[tauri::command]
pub async fn gateway_model_config_create(
    state: State<'_, AiGatewayServiceHandle>,
    input: CreateModelConfigInput,
) -> IcodeResult<ModelConfig> {
    state.service().create_model_config(input)
}

/// 获取模型配置详情
#[tauri::command]
pub async fn gateway_model_config_get(
    state: State<'_, AiGatewayServiceHandle>,
    id: String,
) -> IcodeResult<ModelConfig> {
    state.service().get_model_config(&id)
}

/// 列出所有模型配置
#[tauri::command]
pub async fn gateway_model_config_list(
    state: State<'_, AiGatewayServiceHandle>,
) -> IcodeResult<Vec<ModelConfig>> {
    state.service().list_model_configs()
}

/// 更新模型配置
#[tauri::command]
pub async fn gateway_model_config_update(
    state: State<'_, AiGatewayServiceHandle>,
    id: String,
    input: UpdateModelConfigInput,
) -> IcodeResult<ModelConfig> {
    state.service().update_model_config(&id, input)
}

/// 删除模型配置
#[tauri::command]
pub async fn gateway_model_config_delete(
    state: State<'_, AiGatewayServiceHandle>,
    id: String,
) -> IcodeResult<()> {
    state.service().delete_model_config(&id)
}

// ===== 网关模型命令 =====

/// 创建网关模型
#[tauri::command]
pub async fn gateway_model_create(
    state: State<'_, AiGatewayServiceHandle>,
    input: CreateGatewayModelInput,
) -> IcodeResult<GatewayModel> {
    state.service().create_gateway_model(input)
}

/// 获取网关模型详情
#[tauri::command]
pub async fn gateway_model_get(
    state: State<'_, AiGatewayServiceHandle>,
    id: String,
) -> IcodeResult<GatewayModel> {
    state.service().get_gateway_model(&id)
}

/// 列出所有网关模型
#[tauri::command]
pub async fn gateway_model_list(
    state: State<'_, AiGatewayServiceHandle>,
) -> IcodeResult<Vec<GatewayModel>> {
    state.service().list_gateway_models()
}

/// 列出指定供应商下的所有网关模型
#[tauri::command]
pub async fn gateway_model_list_by_provider(
    state: State<'_, AiGatewayServiceHandle>,
    provider_id: String,
) -> IcodeResult<Vec<GatewayModel>> {
    state.service().list_gateway_models_by_provider(&provider_id)
}

/// 删除网关模型
#[tauri::command]
pub async fn gateway_model_delete(
    state: State<'_, AiGatewayServiceHandle>,
    id: String,
) -> IcodeResult<()> {
    state.service().delete_gateway_model(&id)
}

/// 更新网关模型（如切换公开/隐藏状态，修改展示名等）
#[tauri::command]
pub async fn gateway_model_update(
    state: State<'_, AiGatewayServiceHandle>,
    id: String,
    input: UpdateGatewayModelInput,
) -> IcodeResult<GatewayModel> {
    state.service().update_gateway_model(&id, input)
}

// ===== 暴露模型命令 =====

/// 列出所有对外暴露的模型
///
/// 用于 `/v1/models` 接口数据源，也可供前端展示「可用模型列表」。
/// 仅返回 `is_exposed = 1` 且供应商 `is_enabled = 1` 的模型。
#[tauri::command]
pub async fn gateway_exposed_models(
    state: State<'_, AiGatewayServiceHandle>,
) -> IcodeResult<Vec<ExposedModel>> {
    state.service().list_exposed_models()
}

/// 列出所有网关模型（包含隐藏模型）
///
/// 用于虚拟供应商选择子级真实模型等内部管理场景，不限制 `is_exposed`。
#[tauri::command]
pub async fn gateway_all_models(
    state: State<'_, AiGatewayServiceHandle>,
) -> IcodeResult<Vec<ExposedModel>> {
    state.service().list_all_models()
}

// ===== 内置种子数据命令 =====

/// 列出所有内置供应商预设
#[tauri::command]
pub async fn gateway_builtin_providers_list(
    state: State<'_, AiGatewayServiceHandle>,
) -> IcodeResult<Vec<BuiltinProvider>> {
    state.service().list_builtin_providers()
}

/// 列出所有内置模型预设
#[tauri::command]
pub async fn gateway_builtin_models_list(
    state: State<'_, AiGatewayServiceHandle>,
) -> IcodeResult<Vec<BuiltinModel>> {
    state.service().list_builtin_models()
}

/// 按供应商类型筛选内置模型
#[tauri::command]
pub async fn gateway_builtin_models_by_provider_type(
    state: State<'_, AiGatewayServiceHandle>,
    provider_type: String,
) -> IcodeResult<Vec<BuiltinModel>> {
    state.service().list_builtin_models_by_provider_type(&provider_type)
}

/// 列出内置视觉生成供应商预设（仅来自 builtin-providers-vision.json，不含在通用列表中）
#[tauri::command]
pub async fn gateway_builtin_media_providers_list(
    state: State<'_, AiGatewayServiceHandle>,
) -> IcodeResult<Vec<BuiltinProvider>> {
    state.service().list_builtin_media_providers()
}

/// 列出内置视觉生成模型预设（仅来自 builtin-models-vision.json，不含在通用列表中）
#[tauri::command]
pub async fn gateway_builtin_media_models_list(
    state: State<'_, AiGatewayServiceHandle>,
) -> IcodeResult<Vec<BuiltinModel>> {
    state.service().list_builtin_media_models()
}

/// 实时从供应商 API 拉取官方模型列表
///
/// 不做缓存，每次触发都实时请求。
#[tauri::command]
pub async fn gateway_fetch_official_models(
    state: State<'_, AiGatewayServiceHandle>,
    provider_id: String,
) -> IcodeResult<Vec<String>> {
    state.service().fetch_official_models(&provider_id).await
}

/// 按指定协议从供应商实时拉取模型列表
///
/// 支持两种协议：
/// - `openai-compatible`：OpenAI 兼容 `/models` 接口
/// - `anthropic-native`：Anthropic 原生 `/v1/models` 接口
#[tauri::command]
pub async fn gateway_fetch_models_by_protocol(
    state: State<'_, AiGatewayServiceHandle>,
    provider_id: String,
    protocol: String,
) -> IcodeResult<Vec<String>> {
    state.service().fetch_models_by_protocol(&provider_id, &protocol).await
}

// ===== 网关设置命令 =====

/// 获取网关设置
#[tauri::command]
pub async fn gateway_settings_get(
    state: State<'_, AiGatewayServiceHandle>,
) -> IcodeResult<GatewaySettings> {
    state.service().get_gateway_settings()
}

/// 更新网关设置
#[tauri::command]
pub async fn gateway_settings_update(
    state: State<'_, AiGatewayServiceHandle>,
    input: UpdateGatewaySettingsInput,
) -> IcodeResult<GatewaySettings> {
    state.service().update_gateway_settings(input)
}

// ===== 网关认证 API Key 命令 =====

/// 创建网关认证 API Key
#[tauri::command]
pub async fn gateway_auth_key_create(
    state: State<'_, AiGatewayServiceHandle>,
    input: CreateGatewayAuthKeyInput,
) -> IcodeResult<GatewayAuthKey> {
    state.service().create_gateway_auth_key(input)
}

/// 更新网关认证 API Key
#[tauri::command]
pub async fn gateway_auth_key_update(
    state: State<'_, AiGatewayServiceHandle>,
    id: String,
    input: UpdateGatewayAuthKeyInput,
) -> IcodeResult<GatewayAuthKey> {
    state.service().update_gateway_auth_key(&id, input)
}

/// 删除网关认证 API Key
#[tauri::command]
pub async fn gateway_auth_key_delete(
    state: State<'_, AiGatewayServiceHandle>,
    id: String,
) -> IcodeResult<()> {
    state.service().delete_gateway_auth_key(&id)
}

/// 列出所有网关认证 API Key
#[tauri::command]
pub async fn gateway_auth_key_list(
    state: State<'_, AiGatewayServiceHandle>,
) -> IcodeResult<Vec<GatewayAuthKey>> {
    state.service().list_gateway_auth_keys()
}

// ===== 供应商网络检测命令 =====

use std::time::{Duration, Instant};

/// 单个供应商网络检测结果
///
/// 返回给前端展示，同时写入自研 logger（source=system）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingProviderResult {
    pub provider_id: String,
    pub display_name: String,
    pub slug: String,
    pub base_url: String,
    pub success: bool,
    pub status_code: Option<u16>,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

/// 网络检测完成事件 payload
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingDonePayload {
    pub mode: String,
    pub total: usize,
    pub success: usize,
    pub failed: usize,
}

/// 检测所有供应商 URL 的网络连通性
///
/// 支持三种模式：
/// - `direct`：直连网络检测（强制 `no_proxy`，忽略所有代理设置）
/// - `proxy`：代理网络检测（按全局代理设置发起请求；若全局代理未启用则回退到直连）
/// - `config`：按供应商配置代理检测（按各供应商 `proxy_json` 配置发起请求；
///   未配置或 `global` 类型回退到全局代理，`direct` 类型直连，`socks`/`http` 类型使用各自代理）
///
/// 事件推送流程：
/// 1. 前端调用后，立即返回供应商数量（前端据此打开弹窗并展示待检测列表）
/// 2. 每个供应商检测完成后，通过 `provider:ping-result` 事件推送单条结果
/// 3. 全部完成后，通过 `provider:ping-done` 事件推送汇总
///
/// 同时每个结果写入自研 logger（source=system），
/// 便于在「日志」页面按 system 来源筛选查看。
/// 任何 HTTP 响应（含 4xx/5xx）均视为网络可达；仅网络错误（超时、DNS 失败、连接拒绝）视为失败。
#[tauri::command]
pub async fn gateway_provider_ping(
    app_handle: tauri::AppHandle,
    state: State<'_, AiGatewayServiceHandle>,
    mode: String,
) -> IcodeResult<usize> {
    use crate::modules::logger::Log;

    let providers = state.service().list_providers()?;
    let total = providers.len();

    let mode_label = match mode.as_str() {
        "direct" => "直连",
        "proxy" => "代理",
        "config" => "按供应商配置",
        other => return Err(IcodeError::validation(format!("不支持的网络检测模式: {}", other))),
    };

    Log::info(&format!(
        "[网络检测] 开始检测 {} 个供应商连通性 | 模式: {}",
        total, mode_label
    ));

    // 逐个检测并逐条推送事件 + 写 logger
    let mut success = 0usize;
    let mut failed = 0usize;
    for provider in &providers {
        let result = ping_single_provider(provider, &mode).await;

        // 写入业务 logger（system 来源）
        let status = if result.success {
            format!("✓ ({}ms)", result.latency_ms.unwrap_or(0))
        } else {
            "✗".to_string()
        };
        let code = result
            .status_code
            .map(|c| format!(" [{}]", c))
            .unwrap_or_default();
        let err = result
            .error
            .as_deref()
            .map(|e| format!(" | err={}", e))
            .unwrap_or_default();
        let msg = format!(
            "[网络检测-{}] {} ({}) | url={} | {}{}{}",
            mode_label, result.display_name, result.slug, result.base_url, status, code, err
        );
        if result.success {
            Log::info(&msg);
            success += 1;
        } else {
            Log::warn(&msg);
            failed += 1;
        }

        // 推送单条结果事件
        let _ = app_handle.emit("provider:ping-result", &result);
    }

    Log::info(&format!(
        "[网络检测] 完成 | 模式: {} | 成功: {} | 失败: {}",
        mode_label, success, failed
    ));

    // 推送完成汇总事件
    let _ = app_handle.emit(
        "provider:ping-done",
        PingDonePayload {
            mode: mode.clone(),
            total,
            success,
            failed,
        },
    );

    Ok(total)
}

/// 检测单个供应商 URL 连通性
///
/// 先尝试 HEAD 请求；若返回网络错误（非 HTTP 响应），降级为 GET 重试一次。
/// 任意 HTTP 响应（含 405/404/401）均视为网络可达。
async fn ping_single_provider(provider: &super::types::Provider, mode: &str) -> PingProviderResult {
    use crate::modules::shared::apply_global_proxy;

    let base = provider.base_url.trim();
    let make_result = |success: bool,
                       status_code: Option<u16>,
                       latency_ms: Option<u64>,
                       error: Option<String>| PingProviderResult {
        provider_id: provider.id.clone(),
        display_name: provider.display_name.clone(),
        slug: provider.slug.clone(),
        base_url: provider.base_url.clone(),
        success,
        status_code,
        latency_ms,
        error,
    };

    if base.is_empty() {
        return make_result(false, None, None, Some("baseUrl 为空".to_string()));
    }

    // 构造 HTTP 客户端：direct 强制直连；proxy 按全局代理设置；config 按供应商配置代理
    let mut builder = reqwest::Client::builder()
        .user_agent(concat!("i-code-gateway/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none());

    match mode {
        "direct" => {
            builder = builder.no_proxy();
        }
        "proxy" => {
            builder = apply_global_proxy(builder);
        }
        "config" => {
            // 按供应商自身 proxy_json 配置应用代理
            builder = match crate::modules::shared::apply_provider_proxy(
                builder,
                provider.proxy_json.as_deref(),
            ) {
                Ok(b) => b,
                Err(e) => {
                    return make_result(
                        false,
                        None,
                        None,
                        Some(format!("应用供应商代理配置失败: {}", e)),
                    );
                }
            };
        }
        _ => {
            builder = builder.no_proxy();
        }
    }

    let client = match builder.build() {
        Ok(c) => c,
        Err(e) => {
            return make_result(
                false,
                None,
                None,
                Some(format!("构造 HTTP 客户端失败: {}", e)),
            );
        }
    };

    let start = Instant::now();
    // 先尝试 HEAD；网络错误时降级 GET 重试
    let resp_result = client.head(base).send().await;
    let resp_result = if resp_result.is_err() {
        client.get(base).send().await
    } else {
        resp_result
    };
    let latency_ms = start.elapsed().as_millis() as u64;

    match resp_result {
        Ok(resp) => make_result(true, Some(resp.status().as_u16()), Some(latency_ms), None),
        Err(e) => make_result(false, None, Some(latency_ms), Some(format!("{}", e))),
    }
}

// ===== OAuth 授权命令 =====

use crate::modules::ai_gateway::auth::oauth2::OAuth2Client;

/// 触发 OAuth 浏览器授权流程
///
/// 流程：
/// 1. 根据 `authMethod` 获取/构造 OAuth2Config 并应用供应商预设
/// 2. 启动临时 `127.0.0.1:0` 回调服务器
/// 3. 生成授权 URL（含 PKCE）并通过系统浏览器打开
/// 4. 等待浏览器回调，拿到 authorization code
/// 5. 用 code 换 token
/// 6. 更新供应商 `auth_json`，将 token 序列化后由 Service 加密存储
///
/// # 参数
/// - `provider_id`：供应商 ID
/// - `auth_method`：认证方法（如 `claude-code`、`openai-codex` 等）
///
/// # 返回
/// 更新后的供应商对象。
#[tauri::command]
pub async fn gateway_provider_oauth_authorize(
    _app: tauri::AppHandle,
    state: State<'_, AiGatewayServiceHandle>,
    provider_id: String,
    auth_method: AuthMethod,
) -> IcodeResult<Provider> {
    // 1. 获取供应商现有认证配置
    let provider = state.service().get_provider(&provider_id)?;
    let existing_json = provider.auth_json.as_deref();

    // 2. 构造/提取 OAuth2Config 并应用供应商预设
    let mut oauth_config = state.service().build_oauth_config(auth_method, existing_json)?;

    // 3. 生成统一的 OAuth state，并启动本地回调服务器
    //    固定 redirect_uri 的供应商（如 Claude Code / OpenAI Codex / xAI）必须绑定到预设端口；
    //    其他供应商使用动态端口。实际 redirect_uri 必须写回 oauth_config 供后续换 token 使用。
    let oauth_client = OAuth2Client::new();
    let oauth_state = OAuth2Client::generate_state();
    tracing::info!(
        "OAuth 授权流程开始: provider_id={}, auth_method={:?}, provider_name={}",
        provider_id,
        auth_method,
        provider.display_name
    );
    let (redirect_uri, rx) = oauth_client
        .start_callback_server(&oauth_state, oauth_config.redirect_uri.as_deref(), &provider_id, &provider.display_name)
        .await?;
    oauth_config.redirect_uri = Some(redirect_uri);

    // 4. 生成授权 URL（使用同一 state 与 redirect_uri）
    let (auth_url, code_verifier) =
        oauth_client.build_authorization_url(&oauth_config, &oauth_state)?;

    // 5. 打开系统浏览器
    tauri_plugin_opener::open_url(&auth_url, None::<&str>)
        .map_err(|e| IcodeError::internal(format!("打开浏览器失败: {}", e)))?;

    // 6. 等待回调（120 秒超时，与前端倒计时保持一致）
    let query = tokio::time::timeout(std::time::Duration::from_secs(120), rx)
        .await
        .map_err(|_| IcodeError::validation("OAuth 授权超时"))?
        .map_err(|_| IcodeError::internal("OAuth 回调通道已关闭"))??;

    let code = query
        .code
        .ok_or_else(|| IcodeError::validation("OAuth 回调缺少 code 参数"))?;

    // 7. 用 code 换 token
    let result = state
        .service()
        .exchange_oauth_code(&provider_id, &oauth_config, &code, &code_verifier)
        .await?;

    // 8. 构造新的 AuthConfig，保留现有非敏感字段并写入 token
    let new_auth = state
        .service()
        .build_auth_config_with_token(auth_method, existing_json, result.token)?;

    // 9. 更新供应商
    let update_input = UpdateProviderInput {
        auth: Some(new_auth),
        ..Default::default()
    };
    let updated = state.service().update_provider(&provider_id, update_input)?;
    tracing::info!("OAuth 授权流程完成: provider_id={}", provider_id);
    Ok(updated)
}

/// 启动 OAuth 浏览器授权（不等待回调，立即返回授权 URL 和 PKCE 参数）
///
/// 与 `gateway_provider_oauth_authorize`（一体化等待回调）不同，
/// 此命令仅完成第一步：构造授权 URL + 启动回调服务器 + 打开浏览器，
/// 然后立即返回 PKCE 参数给前端。
///
/// 适用于以下场景：
/// - 某些 OAuth 供应商（如 xAI/Grok）在浏览器授权完成后不自动重定向回本地服务器，
///   而是显示授权码提示用户复制到客户端
/// - 前端需要更灵活地处理授权流程，例如手动输入授权码作为备选
///
/// 返回的 `OAuthStartResult` 包含：
/// - `authorization_url`：浏览器授权 URL（前端需调用 `open()` 打开）
/// - `code_verifier`：PKCE code_verifier（用于后续手动换 token）
/// - `state`：OAuth state（用于验证回调合法性）
/// - `redirect_uri`：回调服务器实际监听的 redirect_uri
#[tauri::command]
pub async fn gateway_provider_oauth_start(
    app: tauri::AppHandle,
    state: State<'_, AiGatewayServiceHandle>,
    provider_id: String,
    auth_method: AuthMethod,
) -> IcodeResult<OAuthStartResult> {
    let result = state
        .service()
        .start_oauth_browser_authorize(&app, &provider_id, auth_method)
        .await?;

    // 打开系统浏览器
    tauri_plugin_opener::open_url(&result.authorization_url, None::<&str>)
        .map_err(|e| IcodeError::internal(format!("打开浏览器失败: {}", e)))?;

    Ok(result)
}

/// 手动完成 OAuth 授权（使用用户输入的授权码换取 token）
///
/// 当浏览器授权流程不自动重定向回本地服务器时（某些供应商在浏览器中显示授权码），
/// 用户可手动复制授权码并在前端输入，然后调用此命令完成 token 交换与供应商更新。
///
/// 参数：
/// - `provider_id`：供应商 ID
/// - `auth_method`：认证方法
/// - `code`：用户手动输入的 authorization code
/// - `code_verifier`：`gateway_provider_oauth_start` 返回的 PKCE code_verifier
/// - `redirect_uri`：`gateway_provider_oauth_start` 返回的 redirect_uri
#[tauri::command]
pub async fn gateway_provider_oauth_complete(
    state: State<'_, AiGatewayServiceHandle>,
    provider_id: String,
    auth_method: AuthMethod,
    code: String,
    code_verifier: String,
    redirect_uri: String,
) -> IcodeResult<Provider> {
    state
        .service()
        .complete_oauth_with_code(&provider_id, auth_method, &code, &code_verifier, &redirect_uri)
        .await
}

/// 列出当前活跃的 OAuth 回调服务器
///
/// 返回内存注册表中所有正在监听的回调服务器信息，包括端口、供应商、固定/动态标识等。
/// 数据仅存在于内存中，不持久化。
#[tauri::command]
pub async fn gateway_oauth_callback_list() -> IcodeResult<Vec<crate::modules::ai_gateway::auth::CallbackServerInfo>> {
    Ok(crate::modules::ai_gateway::auth::global_registry().list())
}

/// 强制关闭指定的 OAuth 回调服务器
///
/// 通过发送 graceful shutdown 信号关闭回调服务器并释放端口。
/// 关闭后从注册表中移除。
///
/// # 参数
/// - `id`：回调服务器条目 ID（由 `gateway_oauth_callback_list` 返回）
///
/// # 返回
/// `true` 表示找到并关闭了服务器，`false` 表示未找到（可能已自动关闭）。
#[tauri::command]
pub async fn gateway_oauth_callback_close(id: String) -> IcodeResult<bool> {
    let closed = crate::modules::ai_gateway::auth::global_registry().force_close(&id);
    if closed {
        tracing::info!("强制关闭 OAuth 回调服务器: id={}", id);
    }
    Ok(closed)
}

/// 清空供应商的 OAuth token（保留端点配置等非敏感字段）
///
/// 用于「重新授权」场景：用户勾选「删除历史认证信息」后，先调用此命令清空旧 token，
/// 再发起授权流程。仅清空 token/expires_at，保留 method、OAuth 端点配置、
/// project_id、email 等非敏感字段，避免用户重新填写端点。
///
/// # 参数
/// - `provider_id`：供应商 ID
///
/// # 返回
/// 更新后的供应商对象（auth_json 中 token 已清空）。
#[tauri::command]
pub async fn gateway_provider_clear_oauth_token(
    state: State<'_, AiGatewayServiceHandle>,
    provider_id: String,
) -> IcodeResult<Provider> {
    state.service().clear_oauth_token(&provider_id)
}

/// 请求 OAuth Device Code
///
/// 用于 GitHub Copilot 等 device_code 流程。返回设备码、用户码与验证 URL，
/// 前端需引导用户在浏览器中访问验证 URL 并输入用户码。
#[tauri::command]
pub async fn gateway_provider_oauth_device_code(
    state: State<'_, AiGatewayServiceHandle>,
    provider_id: String,
    auth_method: AuthMethod,
) -> IcodeResult<DeviceCodeInfo> {
    state
        .service()
        .request_oauth_device_code(&provider_id, auth_method)
        .await
}

/// 单次轮询 OAuth Device Code token
///
/// - 若用户尚未完成授权，返回 `status: pending`，前端应按 interval 继续轮询。
/// - 若授权成功，后端自动更新供应商 `auth_json` 并返回 `status: success` 与更新后的供应商。
#[tauri::command]
pub async fn gateway_provider_oauth_poll_device_token(
    state: State<'_, AiGatewayServiceHandle>,
    provider_id: String,
    auth_method: AuthMethod,
    device_code: String,
) -> IcodeResult<DeviceCodePollResult> {
    state
        .service()
        .poll_oauth_device_token(&provider_id, auth_method, &device_code)
        .await
}

/// 刷新 OAuth access_token
///
/// 读取现有 token 中的 refresh_token，请求新 token 后更新供应商配置。
/// 适用于支持 refresh_token 的 OAuth 供应商（如 Google、xAI、OpenAI Codex 等）。
#[tauri::command]
pub async fn gateway_provider_oauth_refresh_token(
    state: State<'_, AiGatewayServiceHandle>,
    provider_id: String,
    auth_method: AuthMethod,
) -> IcodeResult<Provider> {
    state
        .service()
        .refresh_oauth_token(&provider_id, auth_method)
        .await
}

/// 解密供应商的认证配置并返回明文 JSON 字符串
///
/// 用于前端「查看 token」眼睛按钮：在后端解析所有 `$SECRET:{uuid}$` 引用，
/// 返回格式化的 JSON 字符串供弹窗展示。明文仅在本次返回中传输，前端不缓存。
#[tauri::command]
pub async fn gateway_provider_decrypt_token(
    state: State<'_, AiGatewayServiceHandle>,
    provider_id: String,
) -> IcodeResult<String> {
    state.service().decrypt_provider_token(&provider_id)
}

// ===== 额度查询命令 =====
//
// 编排层：ai_gateway 依赖 balance 模块，在此完成「加载供应商 → 解析 Secret →
// 查询额度 → 持久化快照」的完整流程，供前端供应商列表与系统托盘使用。

use crate::modules::balance::repository as balance_repository;
use crate::modules::balance::service::BalanceServiceHandle;
use crate::modules::balance::types::BalanceRefreshResult;

/// 刷新指定供应商的额度快照
///
/// 后端完成全部流程：
/// 1. 加载供应商配置与认证信息
/// 2. 解析 `$SECRET` 引用为明文
/// 3. 调用 balance service 查询额度
/// 4. 将快照持久化到 `provider_balance_snapshots` 表
/// 5. 广播 `balance:snapshot-updated` 事件
///
/// # 参数
/// - `provider_id`：供应商 ID
///
/// # 返回
/// 额度刷新结果（含快照与可能的警告信息）。
#[tauri::command]
pub async fn balance_refresh_provider(
    app_handle: tauri::AppHandle,
    state: State<'_, AiGatewayServiceHandle>,
    balance_state: State<'_, BalanceServiceHandle>,
    provider_id: String,
) -> IcodeResult<BalanceRefreshResult> {
    // 1. 加载供应商
    let provider = state.service().get_provider(&provider_id)?;

    // 2. 构造查询参数（解析 Secret + 提取凭证）
    let (config, input) = match state.service().build_balance_refresh_input(&provider)? {
        Some(v) => v,
        None => {
            // 未配置额度监控，返回空快照并持久化（便于列表统一处理）
            let empty = BalanceRefreshResult {
                cli_provider_id: provider_id.clone(),
                snapshot: crate::modules::balance::types::BalanceSnapshot {
                    updated_at: chrono::Utc::now().timestamp_millis(),
                    items: vec![],
                },
                warnings: vec![],
            };
            balance_repository::upsert_balance_snapshot(&provider_id, &empty.snapshot)?;
            let _ = app_handle.emit("balance:snapshot-updated", &empty);
            return Ok(empty);
        }
    };

    // 3. 查询额度
    let method = config.method().as_str();
    let result = match balance_state
        .service()
        .query_balance(&provider_id, &config, &input)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            // 双日志：开发追踪 + 应用内日志页
            let msg = format!(
                "额度刷新失败 | provider={} slug={} method={} | {}",
                provider.display_name, provider.slug, method, e.message
            );
            tracing::warn!("{}", msg);
            crate::modules::logger::Log::warn(&msg);
            return Err(e);
        }
    };

    // 4. 持久化快照
    balance_repository::upsert_balance_snapshot(&provider_id, &result.snapshot)?;

    // 5. 打印额度调用结果（双日志，不写 Secret）
    let summary = format_balance_snapshot_summary(&result.snapshot);
    let ok_msg = format!(
        "额度刷新成功 | provider={} slug={} method={} | items={} | {}",
        provider.display_name,
        provider.slug,
        method,
        result.snapshot.items.len(),
        summary
    );
    tracing::info!("{}", ok_msg);
    crate::modules::logger::Log::info(&ok_msg);
    if !result.warnings.is_empty() {
        let warn_msg = format!(
            "额度刷新警告 | provider={} | {}",
            provider.slug,
            result.warnings.join("; ")
        );
        tracing::warn!("{}", warn_msg);
        crate::modules::logger::Log::warn(&warn_msg);
    }

    // 6. 广播事件
    let _ = app_handle.emit("balance:snapshot-updated", &result);

    Ok(result)
}

/// 将快照压缩为一行摘要，供日志使用（不含密钥）
fn format_balance_snapshot_summary(
    snapshot: &crate::modules::balance::types::BalanceSnapshot,
) -> String {
    use crate::modules::balance::types::{BalanceDirection, BalanceMetricType};

    let mut parts: Vec<String> = Vec::new();
    for item in &snapshot.items {
        match item.metric_type {
            BalanceMetricType::Amount => {
                let dir = item
                    .direction
                    .map(|d| match d {
                        BalanceDirection::Remaining => "remaining",
                        BalanceDirection::Used => "used",
                        BalanceDirection::Limit => "limit",
                    })
                    .unwrap_or("amount");
                let val = item
                    .value
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".into());
                let cur = item.currency_symbol.as_deref().unwrap_or("");
                parts.push(format!(
                    "{}({}):{}{}",
                    item.id, dir, cur, val
                ));
            }
            BalanceMetricType::Percent => {
                let val = item
                    .value
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".into());
                parts.push(format!("{}:{}%", item.id, val));
            }
            BalanceMetricType::Token => {
                let used = item
                    .used
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".into());
                parts.push(format!("{}:tokens_used={}", item.id, used));
            }
            BalanceMetricType::Integer => {
                let val = item
                    .value
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".into());
                parts.push(format!("{}:{}", item.id, val));
            }
            BalanceMetricType::Status => {
                let val = item
                    .value
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".into());
                parts.push(format!("{}:status={}", item.id, val));
            }
            BalanceMetricType::Time => {
                let val = item
                    .value
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".into());
                parts.push(format!("{}:{}", item.id, val));
            }
        }
    }
    if parts.is_empty() {
        "empty".into()
    } else {
        parts.join(", ")
    }
}

/// 列出所有供应商的额度快照
///
/// 关联 `providers` 表与 `provider_balance_snapshots` 表，返回每个已配置额度监控
/// 且存在快照的供应商信息（含展示名、slug、监控方法、快照内容、更新时间）。
///
/// 供前端供应商列表与系统托盘使用。
#[tauri::command]
pub async fn balance_list_snapshots() -> IcodeResult<Vec<balance_repository::ProviderBalanceSnapshotRow>> {
    balance_repository::list_balance_snapshots()
}

/// 删除指定供应商的额度快照
///
/// 用于供应商删除或额度监控关闭后清理过期数据。
#[tauri::command]
pub async fn balance_delete_snapshot(provider_id: String) -> IcodeResult<()> {
    balance_repository::delete_balance_snapshot(&provider_id)
}


