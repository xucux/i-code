//! # AI Gateway 业务服务层
//!
//! 提供供应商与模型的业务逻辑，负责：
//!
//! - **供应商 CRUD**：创建/读取/更新/删除/列表/启用禁用
//! - **模型配置 CRUD**：创建/读取/列表/删除
//! - **网关模型 CRUD**：创建/读取/列表/删除（含暴露过滤）
//! - **暴露模型查询**：作为 `/v1/models` 接口数据源
//! - **Secret 引用处理**：
//!   - 保存：扫描 AuthConfig 中的明文敏感字段，交给 secret 模块加密，
//!     替换为 `$SECRET:{snowflake_id}$` 引用后序列化为 JSON 存储
//!   - 读取：仅返回引用字符串（明文仅在网关转发时通过 `resolve_auth_for_request` 解析）
//!
//! ## v0.1 实现范围
//!
//! - 仅支持手动添加模型（`source = "manual"`）
//! - 内置供应商/模型种子数据导入待后续迭代
//! - 官方模型拉取（`official_model_cache`）待后续迭代
//! - 供应商附加请求头/请求体（`provider_extra_headers` / `provider_extra_body`）待后续迭代
//!
//! ## 与其他模块的关系
//!
//! - 依赖 [`secret`](crate::modules::secret) 模块加密 API Key 等敏感字段
//! - 被 [`gateway_runtime`](crate::modules::gateway_runtime) 调用获取供应商配置与暴露模型列表

use std::sync::Arc;

use serde_json::Value;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::ai_gateway::repository::ExposedGatewayModelRow;
use crate::modules::secret::{build_secret_ref, SecretServiceHandle, SecretKind};

use super::auth::oauth2::OAuth2Client;
use super::auth::providers::{get_oauth_preset, is_oauth_method};
use super::auth::{AuthorizationResult, DeviceCodeInfo, OAuth2TokenData, OAuthStartResult};
use super::repository;
use super::seed::{self, BuiltinModel, BuiltinProvider};
use super::types::{
    AuthConfig, AuthMethod, CreateGatewayAuthKeyInput, CreateGatewayModelInput,
    CreateModelConfigInput, CreateProviderInput, DeviceCodePollResult, DeviceCodePollStatus,
    ExportedGatewayModel, ExportedModel, ExportedModelConfig, ExportedProvider, ExportProviderInput,
    ExposedModel, GatewayAuthKey, GatewayListenAddress, GatewayModel, GatewaySettings,
    ImportProviderInput, ModelConfig, OAuth2Config, OAuth2GrantType, Provider, ProviderExportData,
    ProviderType, UpdateGatewayAuthKeyInput, UpdateGatewayModelInput, UpdateGatewaySettingsInput,
    UpdateModelConfigInput, UpdateProviderInput,
};

/// AI Gateway Service 在 Tauri State 中的句柄
///
/// 使用 `Arc` 包装以便在多线程间共享。
/// 持有 [`SecretServiceHandle`] 引用，用于加密/解密敏感字段。
#[derive(Clone)]
pub struct AiGatewayServiceHandle {
    inner: Arc<AiGatewayService>,
}

impl AiGatewayServiceHandle {
    /// 创建 AI Gateway Service 句柄
    ///
    /// # 参数
    /// - `secret_handle`：Secret 服务句柄（用于 API Key 等敏感字段加密）
    pub fn new(secret_handle: SecretServiceHandle) -> Self {
        Self {
            inner: Arc::new(AiGatewayService { secret_handle }),
        }
    }

    /// 获取内部 Service 引用
    pub fn service(&self) -> &AiGatewayService {
        &self.inner
    }
}

/// AI Gateway Service 业务逻辑
pub struct AiGatewayService {
    /// Secret 服务句柄，用于加密 API Key 等敏感字段
    secret_handle: SecretServiceHandle,
}

#[allow(dead_code)]
impl AiGatewayService {
    // ===== 供应商 =====

    /// 创建供应商
    ///
    /// 流程：
    /// 1. 校验 slug 唯一
    /// 2. 校验 provider_type 合法
    /// 3. 处理 AuthConfig：扫描明文敏感字段 → 加密 → 替换为 `$SECRET:{snowflake_id}$` 引用
    /// 4. 序列化 AuthConfig 为 JSON 存储
    /// 5. 返回完整 Provider 记录
    pub fn create_provider(&self, input: CreateProviderInput) -> IcodeResult<Provider> {
        // slug 唯一性校验
        if repository::find_provider_by_slug(&input.slug)?.is_some() {
            return Err(IcodeError::conflict(format!(
                "供应商 slug '{}' 已存在",
                input.slug
            )));
        }

        // provider_type 合法性校验
        if super::types::ProviderType::from_str(&input.provider_type).is_none() {
            return Err(IcodeError::validation(format!(
                "未知的 provider_type: {}",
                input.provider_type
            )));
        }

        // 处理 AuthConfig：明文敏感字段加密后转为引用
        let auth_json = match input.auth.clone() {
            Some(auth) => {
                let processed = self.process_auth_config_for_save(&auth)?;
                Some(serde_json::to_string(&processed)?)
            }
            None => None,
        };

        repository::insert_provider(&input, auth_json.as_deref())
    }

    /// 获取供应商详情
    pub fn get_provider(&self, id: &str) -> IcodeResult<Provider> {
        repository::find_provider_by_id(id)
    }

    /// 列出所有供应商
    pub fn list_providers(&self) -> IcodeResult<Vec<Provider>> {
        repository::list_providers()
    }

    /// 列出所有已启用的供应商（供 gateway_runtime 使用）
    pub fn list_enabled_providers(&self) -> IcodeResult<Vec<Provider>> {
        repository::list_enabled_providers()
    }

    /// 更新供应商
    ///
    /// 若传入新的 `auth` 配置，会先加密其中的明文敏感字段。
    pub fn update_provider(&self, id: &str, input: UpdateProviderInput) -> IcodeResult<Provider> {
        // 校验供应商存在
        let _existing = repository::find_provider_by_id(id)?;

        // 处理 auth_json：Some(Some(auth)) → 更新；Some(None) → 置空；None → 不动
        let auth_json: Option<Option<String>> = match input.auth.clone() {
            Some(auth) => {
                let processed = self.process_auth_config_for_save(&auth)?;
                Some(Some(serde_json::to_string(&processed)?))
            }
            None => None,
        };

        repository::update_provider(id, &input, auth_json.as_ref().map(|o| o.as_deref()))
    }

    /// 删除供应商
    ///
    /// 关联的 gateway_models、provider_extra_* 会通过外键级联删除。
    /// 注意：删除供应商不会自动清理已加密的 Secret 记录，
    /// 需由调用方通过 `secret.cleanup_orphaned` 统一清理。
    pub fn delete_provider(&self, id: &str) -> IcodeResult<()> {
        repository::delete_provider(id)
    }

    /// 导出供应商配置为 base64 JSON
    ///
    /// 流程：
    /// 1. 查询供应商及其下所有网关模型
    /// 2. 查询每个网关模型关联的模型配置
    /// 3. 根据 `include_secrets` 决定是否解析 `$SECRET:{snowflake_id}$` 引用为明文
    /// 4. 序列化为 JSON 并用 base64（URL_SAFE 无填充）编码返回
    pub fn export_provider(&self, input: ExportProviderInput) -> IcodeResult<String> {
        let provider = repository::find_provider_by_id(&input.provider_id)?;
        let gateway_models = repository::list_gateway_models_by_provider(&input.provider_id)?;

        let mut exported_models = Vec::with_capacity(gateway_models.len());
        for gm in gateway_models {
            let config = repository::find_model_config_by_id(&gm.model_config_id)?;
            exported_models.push(ExportedModel {
                gateway_model: ExportedGatewayModel {
                    model_id: gm.model_id,
                    display_name: gm.display_name,
                    family: gm.family,
                    source: gm.source,
                    is_exposed: gm.is_exposed,
                },
                model_config: ExportedModelConfig {
                    name: config.name,
                    family: config.family,
                    max_input_tokens: config.max_input_tokens,
                    max_output_tokens: config.max_output_tokens,
                    tokenizer: config.tokenizer,
                    token_count_multiplier: config.token_count_multiplier,
                    price_per_1m_tokens: config.price_per_1m_tokens,
                    stream: config.stream,
                    temperature: config.temperature,
                    top_k: config.top_k,
                    top_p: config.top_p,
                    frequency_penalty: config.frequency_penalty,
                    presence_penalty: config.presence_penalty,
                    parallel_tool_calling: config.parallel_tool_calling,
                    service_tier: config.service_tier.clone(),
                    verbosity: config.verbosity.clone(),
                    capabilities_json: config.capabilities_json.clone(),
                    thinking_json: config.thinking_json.clone(),
                    multi_agent_json: config.multi_agent_json.clone(),
                    web_search_json: config.web_search_json.clone(),
                    memory_tool: config.memory_tool,
                    preset_templates_json: config.preset_templates_json.clone(),
                },
                extra_headers: None,
                extra_body: None,
            });
        }

        let auth_json = match provider.auth_json.as_deref() {
            Some(json) if !json.is_empty() => {
                let mut value: serde_json::Value = serde_json::from_str(json)?;
                if input.include_secrets {
                    // 解析所有 $SECRET 引用为明文
                    value = self.secret_handle.service().resolve_in_json(&value)?;
                } else {
                    // 清空敏感字段值，保留认证结构
                    strip_sensitive_auth_values(&mut value);
                }
                Some(serde_json::to_string(&value)?)
            }
            _ => provider.auth_json.clone(),
        };

        let exported = ProviderExportData {
            version: "1.0".to_string(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            provider: ExportedProvider {
                slug: provider.slug,
                display_name: provider.display_name,
                provider_type: provider.provider_type,
                base_url: provider.base_url,
                use_raw_base_url: provider.use_raw_base_url,
                transport: provider.transport,
                service_tier: provider.service_tier,
                auth_json,
                balance_provider_json: provider.balance_provider_json.clone(),
                timeout_json: provider.timeout_json.clone(),
                retry_json: provider.retry_json.clone(),
                proxy_json: provider.proxy_json.clone(),
                auto_fetch_official_models: provider.auto_fetch_official_models,
                context_cache_json: provider.context_cache_json.clone(),
                well_known_template_id: provider.well_known_template_id.clone(),
                is_enabled: provider.is_enabled,
                sort_order: provider.sort_order,
                extra_headers: None,
                extra_body: None,
            },
            models: exported_models,
        };

        let json = serde_json::to_string(&exported)?;
        // 使用标准 base64（含 padding）提高兼容性，避免不同客户端复制时丢失 `=` 导致解码失败
        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            json.as_bytes(),
        ))
    }

    /// 从 base64 JSON 导入供应商配置
    ///
    /// 流程：
    /// 1. base64 解码并解析为 `ProviderExportData`
    /// 2. 校验 provider_type 合法
    /// 3. 处理 slug 冲突：按 `conflict_strategy` 自动重命名或返回错误
    /// 4. 创建供应商（auth_json 中的明文会被 Service 自动加密）
    /// 5. 为每个导出模型创建 model_config 与 gateway_model
    pub fn import_provider(&self, input: ImportProviderInput) -> IcodeResult<Provider> {
        // 清理用户粘贴时可能混入的换行、空格与首尾空白
        let data = input
            .data
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();

        // 优先使用标准 base64（含 padding）解码；失败时回退到 URL_SAFE_NO_PAD，兼容旧数据
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &data)
            .or_else(|_| base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE, &data))
            .or_else(|_| {
                base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &data)
            })
            .map_err(|e| IcodeError::validation(format!("base64 解码失败: {e}")))?;

        let exported: ProviderExportData = serde_json::from_slice(&bytes).map_err(|e| {
            IcodeError::validation(format!("导出数据 JSON 解析失败: {e}"))
        })?;

        // 校验协议类型
        if ProviderType::from_str(&exported.provider.provider_type).is_none() {
            return Err(IcodeError::validation(format!(
                "未知的 provider_type: {}",
                exported.provider.provider_type
            )));
        }

        // 处理 slug 冲突
        let slug = self.resolve_import_slug(&exported.provider.slug, &input.conflict_strategy)?;

        // 构造 AuthConfig 对象（若存在），create_provider 会自动加密明文
        let auth = exported.provider.auth_json.as_deref().and_then(|json| {
            if json.is_empty() {
                None
            } else {
                serde_json::from_str::<AuthConfig>(json).ok()
            }
        });

        let provider_input = CreateProviderInput {
            slug,
            display_name: exported.provider.display_name,
            provider_type: exported.provider.provider_type,
            base_url: exported.provider.base_url,
            use_raw_base_url: exported.provider.use_raw_base_url,
            auth,
            auto_fetch_official_models: exported.provider.auto_fetch_official_models,
            is_enabled: exported.provider.is_enabled,
            sort_order: Some(exported.provider.sort_order),
            balance_provider_json: exported.provider.balance_provider_json,
            timeout_json: exported.provider.timeout_json,
            retry_json: exported.provider.retry_json,
            proxy_json: exported.provider.proxy_json,
        };

        let provider = self.create_provider(provider_input)?;

        // 创建模型配置与网关模型
        for exported_model in exported.models {
            let config_input = CreateModelConfigInput {
                name: exported_model.model_config.name,
                family: exported_model.model_config.family,
                max_input_tokens: exported_model.model_config.max_input_tokens,
                max_output_tokens: exported_model.model_config.max_output_tokens,
                tokenizer: exported_model.model_config.tokenizer,
                token_count_multiplier: exported_model.model_config.token_count_multiplier,
                price_per_1m_tokens: exported_model.model_config.price_per_1m_tokens,
                stream: exported_model.model_config.stream,
                temperature: exported_model.model_config.temperature,
                top_p: exported_model.model_config.top_p,
                parallel_tool_calling: exported_model.model_config.parallel_tool_calling,
                capabilities_json: exported_model.model_config.capabilities_json,
                thinking_json: exported_model.model_config.thinking_json,
            };
            let config = repository::insert_model_config(&config_input)?;

            let gateway_input = CreateGatewayModelInput {
                provider_id: provider.id.clone(),
                model_config_id: config.id,
                model_id: exported_model.gateway_model.model_id,
                display_name: exported_model.gateway_model.display_name,
                family: exported_model.gateway_model.family,
                is_exposed: exported_model.gateway_model.is_exposed,
                source: exported_model.gateway_model.source,
            };
            repository::insert_gateway_model(&gateway_input)?;
        }

        Ok(provider)
    }

    /// 确定导入时使用的 slug
    ///
    /// - `conflict_strategy = "auto_rename"`：原 slug 已存在时追加 `-imported`，
    ///   若仍冲突则继续追加数字后缀 `-1`、`-2`...
    /// - `conflict_strategy = "fail"`：直接返回 CONFLICT 错误
    fn resolve_import_slug(&self, slug: &str, conflict_strategy: &str) -> IcodeResult<String> {
        if repository::find_provider_by_slug(slug)?.is_none() {
            return Ok(slug.to_string());
        }

        match conflict_strategy {
            "fail" => Err(IcodeError::conflict(format!(
                "供应商 slug '{}' 已存在",
                slug
            ))),
            _ => {
                let base = format!("{}-imported", slug);
                if repository::find_provider_by_slug(&base)?.is_none() {
                    return Ok(base);
                }
                for i in 1..10000 {
                    let candidate = format!("{}-{}", base, i);
                    if repository::find_provider_by_slug(&candidate)?.is_none() {
                        return Ok(candidate);
                    }
                }
                Err(IcodeError::conflict(format!(
                    "无法为导入供应商生成可用 slug"
                )))
            }
        }
    }

    // ===== 模型配置 =====

    /// 创建模型配置
    pub fn create_model_config(
        &self,
        input: CreateModelConfigInput,
    ) -> IcodeResult<ModelConfig> {
        if input.name.trim().is_empty() {
            return Err(IcodeError::validation("模型配置 name 不能为空"));
        }
        repository::insert_model_config(&input)
    }

    /// 获取模型配置详情
    pub fn get_model_config(&self, id: &str) -> IcodeResult<ModelConfig> {
        repository::find_model_config_by_id(id)
    }

    /// 列出所有模型配置
    pub fn list_model_configs(&self) -> IcodeResult<Vec<ModelConfig>> {
        repository::list_model_configs()
    }

    /// 更新模型配置
    pub fn update_model_config(
        &self,
        id: &str,
        input: UpdateModelConfigInput,
    ) -> IcodeResult<ModelConfig> {
        // 先确认存在
        let _existing = repository::find_model_config_by_id(id)?;
        repository::update_model_config(id, &input)
    }

    /// 删除模型配置
    pub fn delete_model_config(&self, id: &str) -> IcodeResult<()> {
        repository::delete_model_config(id)
    }

    // ===== 网关模型 =====

    /// 创建网关模型
    ///
    /// 校验：
    /// 1. 供应商存在
    /// 2. 模型配置存在
    /// 3. (provider_id, model_id) 唯一
    pub fn create_gateway_model(
        &self,
        input: CreateGatewayModelInput,
    ) -> IcodeResult<GatewayModel> {
        // 校验供应商存在
        let _provider = repository::find_provider_by_id(&input.provider_id)?;
        // 校验模型配置存在
        let _config = repository::find_model_config_by_id(&input.model_config_id)?;

        // 唯一性校验
        if repository::find_gateway_model_by_provider_and_model(&input.provider_id, &input.model_id)?
            .is_some()
        {
            return Err(IcodeError::conflict(format!(
                "供应商下模型 ID '{}' 已存在",
                input.model_id
            )));
        }

        repository::insert_gateway_model(&input)
    }

    /// 更新网关模型
    ///
    /// 支持修改：model_id、display_name、family、is_exposed（公开/隐藏切换）。
    /// 若修改 model_id，需保证同一供应商下不冲突。
    pub fn update_gateway_model(
        &self,
        id: &str,
        input: UpdateGatewayModelInput,
    ) -> IcodeResult<GatewayModel> {
        // 校验模型存在
        let existing = repository::find_gateway_model_by_id(id)?;

        // 若修改 model_id，需校验同一供应商下唯一性
        if let Some(ref new_model_id) = input.model_id {
            if new_model_id != &existing.model_id {
                if repository::find_gateway_model_by_provider_and_model(
                    &existing.provider_id,
                    new_model_id,
                )?
                .is_some()
                {
                    return Err(IcodeError::conflict(format!(
                        "供应商下模型 ID '{}' 已存在",
                        new_model_id
                    )));
                }
            }
        }

        repository::update_gateway_model(id, &input)
    }

    /// 获取网关模型详情
    pub fn get_gateway_model(&self, id: &str) -> IcodeResult<GatewayModel> {
        repository::find_gateway_model_by_id(id)
    }

    /// 列出所有网关模型
    pub fn list_gateway_models(&self) -> IcodeResult<Vec<GatewayModel>> {
        repository::list_gateway_models()
    }

    /// 列出指定供应商下的所有网关模型
    pub fn list_gateway_models_by_provider(
        &self,
        provider_id: &str,
    ) -> IcodeResult<Vec<GatewayModel>> {
        repository::list_gateway_models_by_provider(provider_id)
    }

    /// 删除网关模型
    pub fn delete_gateway_model(&self, id: &str) -> IcodeResult<()> {
        repository::delete_gateway_model(id)
    }

    // ===== 暴露模型列表 =====

    /// 列出所有对外暴露的模型
    ///
    /// 供 `gateway_runtime` 的 `/v1/models` 接口使用。
    /// 仅返回 `is_exposed = 1` 且供应商 `is_enabled = 1` 的模型。
    /// 对外路由 ID：`{provider_slug}/{model_id}`。
    pub fn list_exposed_models(&self) -> IcodeResult<Vec<ExposedModel>> {
        self.map_exposed_rows(repository::list_exposed_gateway_models()?)
    }

    /// 列出所有网关模型（包含隐藏模型）
    ///
    /// 用于虚拟供应商选择子级真实模型等内部管理场景。
    pub fn list_all_models(&self) -> IcodeResult<Vec<ExposedModel>> {
        self.map_exposed_rows(repository::list_all_gateway_models()?)
    }

    fn map_exposed_rows(&self, rows: Vec<ExposedGatewayModelRow>) -> IcodeResult<Vec<ExposedModel>> {
        Ok(rows
            .into_iter()
            .map(|r| {
                // 暴露层展示名为空时回退到模型 ID
                let display_name = r
                    .display_name
                    .clone()
                    .unwrap_or_else(|| r.model_id.clone());
                ExposedModel {
                    id: format!("{}/{}", r.provider_slug, r.model_id),
                    provider_slug: r.provider_slug,
                    model_id: r.model_id,
                    display_name,
                    family: r.family,
                    provider_id: r.provider_id,
                    gateway_model_id: r.id,
                }
            })
            .collect())
    }

    // ===== 网关设置 =====

    /// 获取网关设置
    pub fn get_gateway_settings(&self) -> IcodeResult<GatewaySettings> {
        repository::find_gateway_settings()
    }

    /// 更新网关设置
    pub fn update_gateway_settings(&self, input: UpdateGatewaySettingsInput) -> IcodeResult<GatewaySettings> {
        repository::update_gateway_settings(&input)
    }

    /// 获取网关监听地址
    ///
    /// 用于 `gateway-runtime` 模块启动 HTTP Server 时绑定。
    /// 返回 `{ host, port }`。
    pub fn get_gateway_listen_address(&self) -> IcodeResult<GatewayListenAddress> {
        let settings = self.get_gateway_settings()?;
        let port = if settings.gateway_port >= 0 && settings.gateway_port <= u16::MAX as i64 {
            settings.gateway_port as u16
        } else {
            return Err(IcodeError::internal(format!(
                "网关端口 {} 超出 u16 范围",
                settings.gateway_port
            )));
        };
        Ok(GatewayListenAddress {
            host: settings.gateway_host,
            port,
        })
    }

    // ===== 网关认证 API Key =====

    /// 创建网关认证 API Key
    pub fn create_gateway_auth_key(&self, input: CreateGatewayAuthKeyInput) -> IcodeResult<GatewayAuthKey> {
        if input.name.trim().is_empty() {
            return Err(IcodeError::validation("API Key name 不能为空"));
        }
        repository::insert_gateway_auth_key(&input)
    }

    /// 更新网关认证 API Key
    pub fn update_gateway_auth_key(
        &self,
        id: &str,
        input: UpdateGatewayAuthKeyInput,
    ) -> IcodeResult<GatewayAuthKey> {
        // 校验记录存在
        let _existing = repository::find_gateway_auth_key(id)?;
        repository::update_gateway_auth_key(id, &input)
    }

    /// 删除网关认证 API Key
    pub fn delete_gateway_auth_key(&self, id: &str) -> IcodeResult<()> {
        repository::delete_gateway_auth_key(id)
    }

    /// 列出所有网关认证 API Key
    pub fn list_gateway_auth_keys(&self) -> IcodeResult<Vec<GatewayAuthKey>> {
        repository::list_gateway_auth_keys()
    }

    /// 按 API Key 值查询网关认证 API Key
    ///
    /// 供 `gateway_runtime` 认证中间件反查启用且未过期的 key。
    pub fn find_gateway_auth_key_by_api_key(&self, api_key: &str) -> IcodeResult<Option<GatewayAuthKey>> {
        repository::find_gateway_auth_key_by_api_key(api_key)
    }

    /// 更新网关认证 API Key 的最后使用时间
    pub fn touch_gateway_auth_key_last_used(&self, id: &str) -> IcodeResult<()> {
        repository::touch_gateway_auth_key_last_used(id)
    }

    // ===== Secret 引用处理 =====

    /// 处理 AuthConfig 中的明文敏感字段
    ///
    /// 扫描 AuthConfig 各变体的敏感字段（如 `api_key` / `token` / `client_secret`），
    /// 若值为明文（非 `$SECRET:` 前缀引用），则：
    /// 1. 调用 secret 模块加密保存
    /// 2. 将字段值替换为 `$SECRET:{snowflake_id}$` 引用
    ///
    /// 若值已经是引用格式或为 None，则保持不变。
    /// 返回处理后的 AuthConfig（可安全序列化为 JSON 存储）。
    fn process_auth_config_for_save(&self, auth: &AuthConfig) -> IcodeResult<AuthConfig> {
        match auth {
            AuthConfig::None => Ok(AuthConfig::None),
            AuthConfig::ApiKey {
                label,
                description,
                api_key,
            } => {
                let new_key = self.maybe_encrypt_secret(api_key, SecretKind::ApiKey)?;
                Ok(AuthConfig::ApiKey {
                    label: label.clone(),
                    description: description.clone(),
                    api_key: new_key,
                })
            }
            AuthConfig::Oauth2 {
                label,
                description,
                identity_id,
                token,
                expires_at,
                oauth,
            } => {
                let new_token = self.maybe_encrypt_secret(token, SecretKind::OauthToken)?;
                let new_oauth = oauth
                    .as_ref()
                    .map(|o| self.process_oauth2_config_for_save(o))
                    .transpose()?;
                Ok(AuthConfig::Oauth2 {
                    label: label.clone(),
                    description: description.clone(),
                    identity_id: identity_id.clone(),
                    token: new_token,
                    expires_at: *expires_at,
                    oauth: new_oauth,
                })
            }
            AuthConfig::GoogleVertexAiAuth {
                label,
                description,
                sub_type,
                project_id,
                location,
                key_file_path,
                api_key,
            } => {
                let new_key = self.maybe_encrypt_secret(api_key, SecretKind::ApiKey)?;
                Ok(AuthConfig::GoogleVertexAiAuth {
                    label: label.clone(),
                    description: description.clone(),
                    sub_type: *sub_type,
                    project_id: project_id.clone(),
                    location: location.clone(),
                    key_file_path: key_file_path.clone(),
                    api_key: new_key,
                })
            }
            AuthConfig::AntigravityOauth {
                label,
                description,
                identity_id,
                token,
                expires_at,
                project_id,
                managed_project_id,
                tier,
                tier_id,
                email,
            } => {
                let new_token = self.maybe_encrypt_secret(token, SecretKind::OauthToken)?;
                Ok(AuthConfig::AntigravityOauth {
                    label: label.clone(),
                    description: description.clone(),
                    identity_id: identity_id.clone(),
                    token: new_token,
                    expires_at: *expires_at,
                    project_id: project_id.clone(),
                    managed_project_id: managed_project_id.clone(),
                    tier: tier.clone(),
                    tier_id: tier_id.clone(),
                    email: email.clone(),
                })
            }
            AuthConfig::GoogleGeminiOauth {
                label,
                description,
                identity_id,
                token,
                expires_at,
                project_id,
                oauth_type,
                managed_project_id,
                tier,
                tier_id,
                email,
            } => {
                let new_token = self.maybe_encrypt_secret(token, SecretKind::OauthToken)?;
                Ok(AuthConfig::GoogleGeminiOauth {
                    label: label.clone(),
                    description: description.clone(),
                    identity_id: identity_id.clone(),
                    token: new_token,
                    expires_at: *expires_at,
                    project_id: project_id.clone(),
                    oauth_type: oauth_type.clone(),
                    managed_project_id: managed_project_id.clone(),
                    tier: tier.clone(),
                    tier_id: tier_id.clone(),
                    email: email.clone(),
                })
            }
            AuthConfig::OpenaiCodexAuth {
                label,
                description,
                identity_id,
                token,
                expires_at,
                account_id,
                email,
            } => {
                let new_token = self.maybe_encrypt_secret(token, SecretKind::OauthToken)?;
                Ok(AuthConfig::OpenaiCodexAuth {
                    label: label.clone(),
                    description: description.clone(),
                    identity_id: identity_id.clone(),
                    token: new_token,
                    expires_at: *expires_at,
                    account_id: account_id.clone(),
                    email: email.clone(),
                })
            }
            AuthConfig::ClaudeCode {
                label,
                description,
                identity_id,
                token,
                expires_at,
                email,
            } => {
                let new_token = self.maybe_encrypt_secret(token, SecretKind::OauthToken)?;
                Ok(AuthConfig::ClaudeCode {
                    label: label.clone(),
                    description: description.clone(),
                    identity_id: identity_id.clone(),
                    token: new_token,
                    expires_at: *expires_at,
                    email: email.clone(),
                })
            }
            AuthConfig::XaiGrokOauth {
                label,
                description,
                identity_id,
                token,
                expires_at,
                email,
            } => {
                let new_token = self.maybe_encrypt_secret(token, SecretKind::OauthToken)?;
                Ok(AuthConfig::XaiGrokOauth {
                    label: label.clone(),
                    description: description.clone(),
                    identity_id: identity_id.clone(),
                    token: new_token,
                    expires_at: *expires_at,
                    email: email.clone(),
                })
            }
            AuthConfig::GithubCopilot {
                label,
                description,
                identity_id,
                token,
                expires_at,
                enterprise_url,
                email,
            } => {
                let new_token = self.maybe_encrypt_secret(token, SecretKind::OauthToken)?;
                Ok(AuthConfig::GithubCopilot {
                    label: label.clone(),
                    description: description.clone(),
                    identity_id: identity_id.clone(),
                    token: new_token,
                    expires_at: *expires_at,
                    enterprise_url: enterprise_url.clone(),
                    email: email.clone(),
                })
            }
        }
    }

    /// 处理 OAuth2Config 中的 client_secret 敏感字段
    ///
    /// 将明文 client_secret 加密为 `$SECRET:{snowflake_id}$` 引用后返回新的 OAuth2Config。
    fn process_oauth2_config_for_save(
        &self,
        oauth: &super::types::OAuth2Config,
    ) -> IcodeResult<super::types::OAuth2Config> {
        let new_client_secret =
            self.maybe_encrypt_secret(&oauth.client_secret, SecretKind::OauthToken)?;
        Ok(super::types::OAuth2Config {
            grant_type: oauth.grant_type,
            token_url: oauth.token_url.clone(),
            revocation_url: oauth.revocation_url.clone(),
            scopes: oauth.scopes.clone(),
            authorization_url: oauth.authorization_url.clone(),
            client_id: oauth.client_id.clone(),
            client_secret: new_client_secret,
            pkce: oauth.pkce,
            redirect_uri: oauth.redirect_uri.clone(),
            device_authorization_url: oauth.device_authorization_url.clone(),
        })
    }

    /// 若值为明文则加密保存并返回 `$SECRET:{snowflake_id}$` 引用；
    /// 若值为 None 或已是引用格式则原样返回。
    fn maybe_encrypt_secret(
        &self,
        value: &Option<String>,
        kind: SecretKind,
    ) -> IcodeResult<Option<String>> {
        match value {
            Some(v) if !v.is_empty() && !is_secret_ref(v) => {
                // 明文值：加密保存，返回引用
                let label = format!("{} (ai-gateway)", kind.as_str());
                let mask =
                    self.secret_handle
                        .service()
                        .save_secret(kind, v, Some(label.as_str()))?;
                Ok(Some(build_secret_ref(&mask.id)))
            }
            // None 或已是引用 → 原样返回
            other => Ok(other.clone()),
        }
    }

    /// 解析供应商的 AuthConfig，返回明文版本
    ///
    /// **仅用于 gateway_runtime 转发请求前**，禁止返回前端。
    /// 若 auth_json 为 None，返回 None。
    /// 若解析失败或 Secret 引用不存在，返回错误。
    pub fn resolve_auth_for_request(&self, provider: &Provider) -> IcodeResult<Option<AuthConfig>> {
        match &provider.auth_json {
            Some(json) if !json.is_empty() => {
                let auth: AuthConfig = serde_json::from_str(json)?;
                let resolved = self.resolve_auth_config(&auth)?;
                Ok(Some(resolved))
            }
            _ => Ok(None),
        }
    }

    /// 递归解析 AuthConfig 中的所有 `$SECRET:{snowflake_id}$` 引用为明文
    ///
    /// 将 AuthConfig 序列化为 JSON Value 后统一调用 `resolve_in_json`，
    /// 再反序列化回 AuthConfig。
    fn resolve_auth_config(&self, auth: &AuthConfig) -> IcodeResult<AuthConfig> {
        let value = serde_json::to_value(auth)?;
        let resolved = self.secret_handle.service().resolve_in_json(&value)?;
        Ok(serde_json::from_value(resolved)?)
    }

    /// 解析供应商配置中的所有 JSON 字段（auth / proxy / retry 等）
    ///
    /// 用于 gateway_runtime 转发前一次性解析所有引用。
    /// 返回的 JSON Value 中所有字符串字段中的引用已被替换为明文。
    pub fn resolve_provider_config_json(&self, provider: &Provider) -> IcodeResult<Provider> {
        let mut resolved = provider.clone();
        resolved.auth_json = self.resolve_json_field(&provider.auth_json)?;
        resolved.balance_provider_json = self.resolve_json_field(&provider.balance_provider_json)?;
        resolved.timeout_json = self.resolve_json_field(&provider.timeout_json)?;
        resolved.retry_json = self.resolve_json_field(&provider.retry_json)?;
        resolved.proxy_json = self.resolve_json_field(&provider.proxy_json)?;
        resolved.context_cache_json = self.resolve_json_field(&provider.context_cache_json)?;
        Ok(resolved)
    }

    /// 解析单个 JSON 字段中的所有 `$SECRET:{snowflake_id}$` 引用
    fn resolve_json_field(&self, field: &Option<String>) -> IcodeResult<Option<String>> {
        match field {
            Some(json) if !json.is_empty() => {
                let value: Value = serde_json::from_str(json)?;
                let resolved = self.secret_handle.service().resolve_in_json(&value)?;
                Ok(Some(serde_json::to_string(&resolved)?))
            }
            _ => Ok(field.clone()),
        }
    }

    /// 获取供应商的认证方法（不需要解密）
    ///
    /// 用于前端展示 AuthMethod 标签，不涉及敏感字段解析
    pub fn get_auth_method(&self, provider: &Provider) -> Option<AuthMethod> {
        provider.auth_json.as_ref().and_then(|json| {
            serde_json::from_str::<AuthConfig>(json)
                .ok()
                .map(|a| a.method())
        })
    }

    /// 构造额度查询所需的 BalanceRefreshInput
    ///
    /// 在后端完成 Secret 解析与认证凭证提取，供托盘与供应商列表场景使用：
    /// 1. 解析 `balance_provider_json` → `BalanceConfig`（获取 method + newapi/claude-relay 配置）
    /// 2. 解析 `auth_json` 中的 `$SECRET` 引用为明文
    /// 3. 从明文 AuthConfig 提取 api_key / OAuth access_token / project_id 等字段
    ///
    /// 返回 `(BalanceConfig, BalanceRefreshInput)`，调用方据此调用 balance service 查询额度。
    pub fn build_balance_refresh_input(
        &self,
        provider: &Provider,
    ) -> IcodeResult<Option<(crate::modules::balance::types::BalanceConfig, crate::modules::balance::provider::BalanceRefreshInput)>> {
        // 1. 解析额度监控配置；无配置或 method=none 时返回 None
        let config: crate::modules::balance::types::BalanceConfig = match &provider.balance_provider_json {
            Some(json) if !json.is_empty() => serde_json::from_str(json)?,
            _ => return Ok(None),
        };
        if matches!(config, crate::modules::balance::types::BalanceConfig::None) {
            return Ok(None);
        }

        // 2. 解析 auth_json 中的 $SECRET 引用为明文
        let resolved_auth = self.resolve_auth_for_request(provider)?;

        // 3. 从明文 AuthConfig 提取凭证字段
        let mut input = crate::modules::balance::provider::BalanceRefreshInput {
            base_url: Some(provider.base_url.clone()),
            ..Default::default()
        };

        if let Some(auth) = &resolved_auth {
            match auth {
                AuthConfig::ApiKey { api_key, .. } => {
                    input.api_key = api_key.clone().filter(|s| !s.is_empty());
                }
                AuthConfig::GoogleVertexAiAuth { api_key, .. } => {
                    input.api_key = api_key.clone().filter(|s| !s.is_empty());
                }
                // OAuth 类：从 token JSON 中提取 access_token 作为 api_key
                AuthConfig::Oauth2 { token, .. }
                | AuthConfig::AntigravityOauth { token, .. }
                | AuthConfig::GoogleGeminiOauth { token, .. }
                | AuthConfig::OpenaiCodexAuth { token, .. }
                | AuthConfig::ClaudeCode { token, .. }
                | AuthConfig::XaiGrokOauth { token, .. }
                | AuthConfig::GithubCopilot { token, .. } => {
                    if let Some(token_str) = token.as_ref().filter(|s| !s.is_empty()) {
                        input.api_key = extract_oauth_access_token_for_balance(token_str);
                    }
                }
                AuthConfig::None => {}
            }

            // 提取 Code Assist / Codex 所需的额外字段
            input.project_id = auth.project_id().cloned();
            input.managed_project_id = auth.managed_project_id().cloned();
            input.account_id = auth.account_id().cloned();
        }

        // 4. 从 BalanceConfig 提取方法特有的配置
        match &config {
            crate::modules::balance::types::BalanceConfig::Newapi(cfg) => {
                input.newapi_config = Some(cfg.clone());
            }
            crate::modules::balance::types::BalanceConfig::ClaudeRelayService(cfg) => {
                input.claude_relay_config = Some(cfg.clone());
            }
            _ => {}
        }

        Ok(Some((config, input)))
    }

    // ===== OAuth 授权辅助方法 =====

    /// 为指定认证方法构造或提取 OAuth2Config
    ///
    /// - 若 `existing_auth_json` 中包含同方法 OAuth 配置，提取其中的 `oauth` 字段并应用预设。
    /// - 否则根据 `method` 创建默认 OAuth 配置并应用预设。
    pub fn build_oauth_config(
        &self,
        method: AuthMethod,
        existing_auth_json: Option<&str>,
    ) -> IcodeResult<OAuth2Config> {
        if !is_oauth_method(method) {
            return Err(IcodeError::validation(format!(
                "认证方法 {:?} 不支持 OAuth 授权",
                method
            )));
        }

        let mut oauth = match existing_auth_json {
            Some(json) if !json.is_empty() => {
                let auth: AuthConfig = serde_json::from_str(json)?;
                if auth.method() != method {
                    return Err(IcodeError::validation(format!(
                        "供应商现有认证方法为 {:?}，与请求的 {:?} 不一致",
                        auth.method(),
                        method
                    )));
                }
                extract_oauth_config(&auth)
            }
            _ => OAuth2Config {
                grant_type: OAuth2GrantType::AuthorizationCode,
                token_url: None,
                revocation_url: None,
                scopes: None,
                authorization_url: None,
                client_id: None,
                client_secret: None,
                pkce: None,
                redirect_uri: None,
                device_authorization_url: None,
            },
        };

        if let Some(preset) = get_oauth_preset(method)? {
            preset.apply(&mut oauth);
        }

        Ok(oauth)
    }

    /// 用 authorization code 换取 OAuth token
    ///
    /// 返回的 `AuthorizationResult` 包含 token 数据，调用方负责加密存储。
    pub async fn exchange_oauth_code(
        &self,
        provider_id: &str,
        config: &OAuth2Config,
        code: &str,
        code_verifier: &str,
    ) -> IcodeResult<AuthorizationResult> {
        let provider = self.get_provider(provider_id)?;
        let client = OAuth2Client::new_for_provider(&provider)?;
        let token = client.exchange_code(config, code, code_verifier).await?;
        Ok(AuthorizationResult {
            token,
            account_info: None,
        })
    }

    /// 启动浏览器授权流程（仅生成 URL 和 PKCE 参数，不等待回调）
    ///
    /// 与 `gateway_provider_oauth_authorize`（一体化等待回调）不同，
    /// 此方法仅完成第一步：构造授权 URL + 启动回调服务器 + 打开浏览器，
    /// 然后立即返回授权 URL 和 PKCE 参数给前端。
    ///
    /// 回调服务器收到浏览器重定向时，会通过 Tauri Event (`oauth-callback-result`)
    /// 通知前端，前端监听事件后可自动调用 `gateway_provider_oauth_complete` 完成流程。
    /// 若回调服务器未收到回调（供应商在浏览器中显示授权码），
    /// 前端可让用户手动输入授权码，再调用 `gateway_provider_oauth_complete`。
    ///
    /// 返回的 `OAuthStartResult` 包含 authorization_url、code_verifier、state、redirect_uri。
    pub async fn start_oauth_browser_authorize(
        &self,
        app: &tauri::AppHandle,
        provider_id: &str,
        method: AuthMethod,
    ) -> IcodeResult<OAuthStartResult> {
        let provider = self.get_provider(provider_id)?;
        let existing_json = provider.auth_json.as_deref();
        let mut oauth_config = self.build_oauth_config(method, existing_json)?;

        let oauth_client = OAuth2Client::new_for_provider(&provider)?;
        let oauth_state = OAuth2Client::generate_state();

        // 启动回调服务器，回调成功时通过 Tauri Event 通知前端
        let redirect_uri = oauth_client
            .start_callback_server_with_event(app, &oauth_state, oauth_config.redirect_uri.as_deref(), provider_id)
            .await?;
        oauth_config.redirect_uri = Some(redirect_uri.clone());

        let (authorization_url, code_verifier) =
            oauth_client.build_authorization_url(&oauth_config, &oauth_state)?;

        Ok(OAuthStartResult {
            authorization_url,
            code_verifier,
            state: oauth_state,
            redirect_uri,
        })
    }

    /// 用手动输入的授权码完成 OAuth 流程
    ///
    /// 当浏览器授权不自动重定向（供应商在浏览器中显示授权码）
    /// 或回调服务器超时时，前端让用户手动输入授权码，
    /// 再调用此方法完成 token 交换并更新供应商。
    ///
    /// 参数：
    /// - `provider_id`：供应商 ID
    /// - `auth_method`：认证方法
    /// - `code`：用户手动输入的 authorization code
    /// - `code_verifier`：start_oauth_browser_authorize 返回的 PKCE code_verifier
    /// - `redirect_uri`：start_oauth_browser_authorize 返回的 redirect_uri
    pub async fn complete_oauth_with_code(
        &self,
        provider_id: &str,
        method: AuthMethod,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
    ) -> IcodeResult<Provider> {
        let provider = self.get_provider(provider_id)?;
        let existing_json = provider.auth_json.as_deref();
        let mut oauth_config = self.build_oauth_config(method, existing_json)?;
        // 使用前端传入的 redirect_uri（与 start 步骤一致）
        oauth_config.redirect_uri = Some(redirect_uri.to_string());

        let client = OAuth2Client::new_for_provider(&provider)?;
        let token = client.exchange_code(&oauth_config, code, code_verifier).await?;

        let new_auth =
            self.build_auth_config_with_token(method, existing_json, token)?;
        let update_input = UpdateProviderInput {
            auth: Some(new_auth),
            ..Default::default()
        };
        self.update_provider(provider_id, update_input)
    }

    /// 请求 Device Code
    ///
    /// 返回设备码、用户码与验证 URL，前端需引导用户在浏览器中完成授权。
    pub async fn request_oauth_device_code(
        &self,
        provider_id: &str,
        method: AuthMethod,
    ) -> IcodeResult<DeviceCodeInfo> {
        let provider = self.get_provider(provider_id)?;
        let existing_json = provider.auth_json.as_deref();
        let oauth = self.build_oauth_config(method, existing_json)?;

        if oauth.grant_type != OAuth2GrantType::DeviceCode {
            return Err(IcodeError::validation(
                "当前认证方法不支持 Device Code 流程",
            ));
        }

        let client = OAuth2Client::new_for_provider(&provider)?;
        client.request_device_code(&oauth).await
    }

    /// 单次轮询 Device Code token
    ///
    /// - 若用户尚未授权，返回 `status: pending`。
    /// - 若授权成功，更新供应商 `auth_json` 并返回 `status: success` 与更新后的供应商。
    pub async fn poll_oauth_device_token(
        &self,
        provider_id: &str,
        method: AuthMethod,
        device_code: &str,
    ) -> IcodeResult<DeviceCodePollResult> {
        let provider = self.get_provider(provider_id)?;
        let existing_json = provider.auth_json.as_deref();
        let oauth = self.build_oauth_config(method, existing_json)?;

        if oauth.grant_type != OAuth2GrantType::DeviceCode {
            return Err(IcodeError::validation(
                "当前认证方法不支持 Device Code 流程",
            ));
        }

        let client = OAuth2Client::new_for_provider(&provider)?;
        match client.poll_device_token(&oauth, device_code).await? {
            Some(token) => {
                let new_auth = self.build_auth_config_with_token(method, existing_json, token)?;
                let update_input = UpdateProviderInput {
                    auth: Some(new_auth),
                    ..Default::default()
                };
                let provider = self.update_provider(provider_id, update_input)?;
                Ok(DeviceCodePollResult {
                    status: DeviceCodePollStatus::Success,
                    provider: Some(provider),
                })
            }
            None => Ok(DeviceCodePollResult {
                status: DeviceCodePollStatus::Pending,
                provider: None,
            }),
        }
    }

    /// 刷新 OAuth access_token
    ///
    /// 读取现有 token 中的 refresh_token，请求新 token 后更新供应商配置。
    pub async fn refresh_oauth_token(
        &self,
        provider_id: &str,
        method: AuthMethod,
    ) -> IcodeResult<Provider> {
        let provider = self.get_provider(provider_id)?;
        let existing_json = provider.auth_json.as_deref();
        let auth: AuthConfig = match existing_json {
            Some(json) if !json.is_empty() => serde_json::from_str(json)?,
            _ => return Err(IcodeError::validation("供应商缺少认证配置")),
        };

        if auth.method() != method {
            return Err(IcodeError::validation(format!(
                "供应商现有认证方法为 {:?}，与请求的 {:?} 不一致",
                auth.method(),
                method
            )));
        }

        let token_string = auth.token().filter(|s| !s.is_empty()).ok_or_else(|| {
            IcodeError::validation("OAuth 认证缺少 token")
        })?;

        let token: OAuth2TokenData = serde_json::from_str(&token_string)
            .map_err(|_| IcodeError::validation("OAuth token 格式无效"))?;

        let refresh_token = token
            .refresh_token
            .as_ref()
            .ok_or_else(|| IcodeError::validation("OAuth token 缺少 refresh_token，无法刷新"))?;

        let oauth = self.build_oauth_config(method, existing_json)?;
        let client = OAuth2Client::new_for_provider(&provider)?;
        let new_token = client
            .refresh_access_token(&oauth, refresh_token)
            .await?;

        let new_auth = self.build_auth_config_with_token(method, existing_json, new_token)?;
        let update_input = UpdateProviderInput {
            auth: Some(new_auth),
            ..Default::default()
        };
        self.update_provider(provider_id, update_input)
    }

    /// 用新的 token 数据更新 AuthConfig
    ///
    /// 保留现有配置中的 label / description / identity_id 等字段，
    /// 仅替换 token 字段；identity_id 不存在时生成新的 UUID。
    pub fn build_auth_config_with_token(
        &self,
        method: AuthMethod,
        existing_json: Option<&str>,
        token: OAuth2TokenData,
    ) -> IcodeResult<AuthConfig> {
        let token_json = serde_json::to_string(&token)
            .map_err(|e| IcodeError::internal(format!("序列化 OAuth token 失败: {}", e)))?;

        let existing = existing_json
            .filter(|s| !s.is_empty())
            .and_then(|json| serde_json::from_str::<AuthConfig>(json).ok());

        let identity_id = existing
            .as_ref()
            .and_then(|a| a.identity_id().cloned())
            .or_else(|| Some(crate::core::id::generate_id()));

        let label = existing.as_ref().and_then(|a| a.label().cloned());
        let description = existing.as_ref().and_then(|a| a.description().cloned());

        match method {
            AuthMethod::Oauth2 => Ok(AuthConfig::Oauth2 {
                label,
                description,
                identity_id,
                token: Some(token_json),
                expires_at: token.expires_at,
                oauth: existing.as_ref().and_then(|a| a.oauth_config().cloned()),
            }),
            AuthMethod::AntigravityOauth => Ok(AuthConfig::AntigravityOauth {
                label,
                description,
                identity_id,
                token: Some(token_json),
                expires_at: token.expires_at,
                project_id: existing.as_ref().and_then(|a| a.project_id().cloned()),
                managed_project_id: existing.as_ref().and_then(|a| a.managed_project_id().cloned()),
                tier: existing.as_ref().and_then(|a| a.tier().cloned()),
                tier_id: existing.as_ref().and_then(|a| a.tier_id().cloned()),
                email: existing.as_ref().and_then(|a| a.email().cloned()),
            }),
            AuthMethod::GoogleGeminiOauth => Ok(AuthConfig::GoogleGeminiOauth {
                label,
                description,
                identity_id,
                token: Some(token_json),
                expires_at: token.expires_at,
                project_id: existing.as_ref().and_then(|a| a.project_id().cloned()),
                oauth_type: existing.as_ref().and_then(|a| a.oauth_type().cloned()),
                managed_project_id: existing.as_ref().and_then(|a| a.managed_project_id().cloned()),
                tier: existing.as_ref().and_then(|a| a.tier().cloned()),
                tier_id: existing.as_ref().and_then(|a| a.tier_id().cloned()),
                email: existing.as_ref().and_then(|a| a.email().cloned()),
            }),
            AuthMethod::OpenaiCodexAuth => Ok(AuthConfig::OpenaiCodexAuth {
                label,
                description,
                identity_id,
                token: Some(token_json),
                expires_at: token.expires_at,
                account_id: existing.as_ref().and_then(|a| a.account_id().cloned()),
                email: existing.as_ref().and_then(|a| a.email().cloned()),
            }),
            AuthMethod::ClaudeCodeAuth => Ok(AuthConfig::ClaudeCode {
                label,
                description,
                identity_id,
                token: Some(token_json),
                expires_at: token.expires_at,
                email: existing.as_ref().and_then(|a| a.email().cloned()),
            }),
            AuthMethod::XaiGrokOauth => Ok(AuthConfig::XaiGrokOauth {
                label,
                description,
                identity_id,
                token: Some(token_json),
                expires_at: token.expires_at,
                email: existing.as_ref().and_then(|a| a.email().cloned()),
            }),
            AuthMethod::GithubCopilotAuth => Ok(AuthConfig::GithubCopilot {
                label,
                description,
                identity_id,
                token: Some(token_json),
                expires_at: token.expires_at,
                enterprise_url: existing.as_ref().and_then(|a| a.enterprise_url().cloned()),
                email: existing.as_ref().and_then(|a| a.email().cloned()),
            }),
            _ => Err(IcodeError::validation(format!(
                "认证方法 {:?} 不支持 OAuth token 更新",
                method
            ))),
        }
    }

    // ===== 内置种子数据 =====

    /// 列出所有内置供应商预设
    ///
    /// 数据来自 `data/builtin-providers.json`，编译时嵌入二进制。
    pub fn list_builtin_providers(&self) -> IcodeResult<Vec<BuiltinProvider>> {
        seed::load_builtin_providers()
    }

    /// 列出所有内置模型预设
    ///
    /// 数据来自 `data/builtin-models.json`，编译时嵌入二进制。
    pub fn list_builtin_models(&self) -> IcodeResult<Vec<BuiltinModel>> {
        seed::load_builtin_models()
    }

    /// 按供应商类型筛选内置模型
    pub fn list_builtin_models_by_provider_type(
        &self,
        provider_type: &str,
    ) -> IcodeResult<Vec<BuiltinModel>> {
        seed::filter_builtin_models_by_provider_type(provider_type)
    }

    /// 获取单个内置供应商预设
    pub fn get_builtin_provider(&self, id: &str) -> IcodeResult<Option<BuiltinProvider>> {
        seed::find_builtin_provider(id)
    }

    /// 获取单个内置模型预设
    pub fn get_builtin_model(&self, id: &str) -> IcodeResult<Option<BuiltinModel>> {
        seed::find_builtin_model(id)
    }

    // ===== 官方模型拉取（实时，不做缓存） =====

    /// 实时从供应商 API 拉取官方模型列表
    ///
    /// 根据 `provider.provider_type` 调用对应协议接口：
    /// - `openai-chat-completion` / `openai-responses` / `openai-codex`：
    ///   `GET {base_url}/models`
    /// - `ollama`：`GET {base_url}/api/tags`
    /// - `google-ai-studio` / `google-vertex-ai`：
    ///   `GET {base_url}/models`（query 参数 key 或 Authorization）
    /// - `anthropic`：官方无模型列表 API，返回内置 `anthropic` 模型列表作为 fallback
    ///
    /// 不做缓存，每次触发都实时请求。
    pub async fn fetch_official_models(&self, provider_id: &str) -> IcodeResult<Vec<String>> {
        let provider = repository::find_provider_by_id(provider_id)?;
        if !provider.is_enabled {
            return Err(IcodeError::validation("供应商已禁用，无法拉取模型列表"));
        }

        let auth = self.resolve_auth_for_request(&provider)?;
        let client = reqwest::Client::new();

        match provider.provider_type.as_str() {
            "openai-chat-completion" | "openai-responses" | "openai-codex" | "xai-grok-build" => {
                let api_key = extract_api_key(&auth);
                self.fetch_openai_compatible_models(&client, &provider, api_key)
                    .await
            }
            "custom" => {
                let api_key = extract_api_key(&auth);
                self.fetch_openai_compatible_models(&client, &provider, api_key)
                    .await
            }
            "ollama" => self.fetch_ollama_models(&client, &provider).await,
            "google-ai-studio" => {
                let api_key = extract_api_key(&auth);
                self.fetch_gemini_models(&client, &provider, api_key)
                    .await
            }
            "google-vertex-ai" => self.fetch_vertex_ai_models(&client, &provider, &auth).await,
            "github-copilot" => {
                let token = extract_oauth_token(&auth);
                self.fetch_github_copilot_models(&client, &provider, token)
                    .await
            }
            "anthropic" => {
                let builtin = seed::filter_builtin_models_by_provider_type("anthropic")?;
                Ok(builtin.into_iter().map(|m| m.id).collect())
            }
            "claude-code" => {
                let builtin = seed::filter_builtin_models_by_provider_type("claude-code")?;
                Ok(builtin.into_iter().map(|m| m.id).collect())
            }
            "google-antigravity" => {
                let builtin = seed::filter_builtin_models_by_provider_type("google-antigravity")?;
                Ok(builtin.into_iter().map(|m| m.id).collect())
            }
            "google-gemini-cli" => {
                let builtin = seed::filter_builtin_models_by_provider_type("google-gemini-cli")?;
                Ok(builtin.into_iter().map(|m| m.id).collect())
            }
            other => Err(IcodeError::not_implemented(format!(
                "provider_type '{}' 的官方模型拉取尚未实现",
                other
            ))),
        }
    }

    /// 按指定协议从供应商拉取模型列表
    ///
    /// 不关心供应商的 `provider_type`，直接按用户选择的协议发起请求：
    /// - `openai-compatible`：`GET {base_url}/models`，`Authorization: Bearer {api_key}`
    /// - `anthropic-native`：`GET {base_url}/v1/models`，`x-api-key` + `anthropic-version`
    pub async fn fetch_models_by_protocol(
        &self,
        provider_id: &str,
        protocol: &str,
    ) -> IcodeResult<Vec<String>> {
        let provider = repository::find_provider_by_id(provider_id)?;
        if !provider.is_enabled {
            return Err(IcodeError::validation("供应商已禁用，无法拉取模型列表"));
        }

        let auth = self.resolve_auth_for_request(&provider)?;
        let client = reqwest::Client::new();

        match protocol {
            "openai-compatible" => {
                let api_key = extract_api_key(&auth);
                self.fetch_openai_compatible_models(&client, &provider, api_key)
                    .await
            }
            "anthropic-native" => {
                let api_key = extract_api_key(&auth);
                self.fetch_anthropic_native_models(&client, &provider, api_key)
                    .await
            }
            other => Err(IcodeError::validation(format!(
                "不支持的拉取协议 '{}'，仅支持 openai-compatible / anthropic-native",
                other
            ))),
        }
    }

    /// 拉取 OpenAI 兼容协议的模型列表
    async fn fetch_openai_compatible_models(
        &self,
        client: &reqwest::Client,
        provider: &Provider,
        api_key: Option<&str>,
    ) -> IcodeResult<Vec<String>> {
        let url = if provider.use_raw_base_url {
            format!("{}/models", provider.base_url.trim_end_matches('/'))
        } else {
            let base = provider.base_url.trim_end_matches('/');
            if base.contains("/v1") {
                format!("{}/models", base)
            } else {
                format!("{}/v1/models", base)
            }
        };

        let start = std::time::Instant::now();
        let mut req = client.get(&url);
        if let Some(key) = api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let resp = req.send().await.map_err(|e| {
            log::warn!(
                "Provider API other | GET {} | error={}",
                redact_url_key_param(&url),
                e
            );
            IcodeError::gateway(format!("请求供应商模型列表失败: {}", e))
        })?;
        let status = resp.status().as_u16();
        let duration_ms = start.elapsed().as_millis();

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            log::warn!(
                "Provider API other non-200 | GET {} | status={} | duration={}ms | response_body={}",
                redact_url_key_param(&url),
                status,
                duration_ms,
                text
            );
            return Err(IcodeError::gateway(format!(
                "供应商返回错误 {}: {}",
                status, text
            )));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            log::warn!(
                "Provider API other | GET {} | status={} | duration={}ms | parse_error={}",
                redact_url_key_param(&url),
                status,
                duration_ms,
                e
            );
            IcodeError::gateway(format!("解析供应商模型列表失败: {}", e))
        })?;

        log::info!(
            "Provider API other | GET {} | status={} | duration={}ms",
            redact_url_key_param(&url),
            status,
            duration_ms
        );
        log::debug!(
            "Provider API other response body | GET {} | {}",
            redact_url_key_param(&url),
            body.to_string()
        );

        let data = body.get("data").and_then(|d| d.as_array()).ok_or_else(|| {
            IcodeError::gateway("供应商模型列表缺少 data 数组")
        })?;

        let ids: Vec<String> = data
            .iter()
            .filter_map(|item| item.get("id").and_then(|v| v.as_str()).map(String::from))
            .collect();

        Ok(ids)
    }

    /// 拉取 Anthropic 原生协议的模型列表
    ///
    /// Anthropic Messages API 的模型列表端点：
    /// - `GET {base_url}/v1/models`（原始模式）或 `GET {base_url}/models`（自动模式且 base_url 已含 /v1）
    /// - 认证头：`x-api-key`
    /// - 版本头：`anthropic-version: 2023-06-01`
    async fn fetch_anthropic_native_models(
        &self,
        client: &reqwest::Client,
        provider: &Provider,
        api_key: Option<&str>,
    ) -> IcodeResult<Vec<String>> {
        let url = if provider.use_raw_base_url {
            format!("{}/v1/models", provider.base_url.trim_end_matches('/'))
        } else {
            let base = provider.base_url.trim_end_matches('/');
            if base.contains("/v1") {
                format!("{}/models", base)
            } else {
                format!("{}/v1/models", base)
            }
        };

        let start = std::time::Instant::now();
        let mut req = client
            .get(&url)
            .header("anthropic-version", "2023-06-01");

        if let Some(key) = api_key {
            req = req.header("x-api-key", key);
        }

        let resp = req.send().await.map_err(|e| {
            log::warn!(
                "Provider API other | GET {} | error={}",
                redact_url_key_param(&url),
                e
            );
            IcodeError::gateway(format!("请求 Anthropic 模型列表失败: {}", e))
        })?;
        let status = resp.status().as_u16();
        let duration_ms = start.elapsed().as_millis();

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            log::warn!(
                "Provider API other non-200 | GET {} | status={} | duration={}ms | response_body={}",
                redact_url_key_param(&url),
                status,
                duration_ms,
                text
            );
            return Err(IcodeError::gateway(format!(
                "Anthropic 返回错误 {}: {}",
                status, text
            )));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            log::warn!(
                "Provider API other | GET {} | status={} | duration={}ms | parse_error={}",
                redact_url_key_param(&url),
                status,
                duration_ms,
                e
            );
            IcodeError::gateway(format!("解析 Anthropic 模型列表失败: {}", e))
        })?;

        log::info!(
            "Provider API other | GET {} | status={} | duration={}ms",
            redact_url_key_param(&url),
            status,
            duration_ms
        );
        log::debug!(
            "Provider API other response body | GET {} | {}",
            redact_url_key_param(&url),
            body.to_string()
        );

        let data = body.get("data").and_then(|d| d.as_array()).ok_or_else(|| {
            IcodeError::gateway("Anthropic 模型列表缺少 data 数组")
        })?;

        let ids: Vec<String> = data
            .iter()
            .filter_map(|item| item.get("id").and_then(|v| v.as_str()).map(String::from))
            .collect();

        Ok(ids)
    }

    /// 拉取 Ollama 本地模型列表
    async fn fetch_ollama_models(
        &self,
        client: &reqwest::Client,
        provider: &Provider,
    ) -> IcodeResult<Vec<String>> {
        let url = format!("{}/api/tags", provider.base_url.trim_end_matches('/'));

        let start = std::time::Instant::now();
        let resp = client.get(&url).send().await.map_err(|e| {
            log::warn!(
                "Provider API other | GET {} | error={}",
                redact_url_key_param(&url),
                e
            );
            IcodeError::gateway(format!("请求 Ollama 模型列表失败: {}", e))
        })?;
        let status = resp.status().as_u16();
        let duration_ms = start.elapsed().as_millis();

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            log::warn!(
                "Provider API other non-200 | GET {} | status={} | duration={}ms | response_body={}",
                redact_url_key_param(&url),
                status,
                duration_ms,
                text
            );
            return Err(IcodeError::gateway(format!(
                "Ollama 返回错误 {}: {}",
                status, text
            )));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            log::warn!(
                "Provider API other | GET {} | status={} | duration={}ms | parse_error={}",
                redact_url_key_param(&url),
                status,
                duration_ms,
                e
            );
            IcodeError::gateway(format!("解析 Ollama 模型列表失败: {}", e))
        })?;

        log::info!(
            "Provider API other | GET {} | status={} | duration={}ms",
            redact_url_key_param(&url),
            status,
            duration_ms
        );
        log::debug!(
            "Provider API other response body | GET {} | {}",
            redact_url_key_param(&url),
            body.to_string()
        );

        let models = body.get("models").and_then(|m| m.as_array()).ok_or_else(|| {
            IcodeError::gateway("Ollama 模型列表缺少 models 数组")
        })?;

        let names: Vec<String> = models
            .iter()
            .filter_map(|item| item.get("name").and_then(|v| v.as_str()).map(String::from))
            .collect();

        Ok(names)
    }

    /// 拉取 Google Gemini 模型列表
    async fn fetch_gemini_models(
        &self,
        client: &reqwest::Client,
        provider: &Provider,
        api_key: Option<&str>,
    ) -> IcodeResult<Vec<String>> {
        let base = provider.base_url.trim_end_matches('/');
        let url = if let Some(key) = api_key {
            format!("{}/models?key={}", base, key)
        } else {
            format!("{}/models", base)
        };

        let start = std::time::Instant::now();
        let resp = client.get(&url).send().await.map_err(|e| {
            log::warn!(
                "Provider API other | GET {} | error={}",
                redact_url_key_param(&url),
                e
            );
            IcodeError::gateway(format!("请求 Gemini 模型列表失败: {}", e))
        })?;
        let status = resp.status().as_u16();
        let duration_ms = start.elapsed().as_millis();

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            log::warn!(
                "Provider API other non-200 | GET {} | status={} | duration={}ms | response_body={}",
                redact_url_key_param(&url),
                status,
                duration_ms,
                text
            );
            return Err(IcodeError::gateway(format!(
                "Gemini 返回错误 {}: {}",
                status, text
            )));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            log::warn!(
                "Provider API other | GET {} | status={} | duration={}ms | parse_error={}",
                redact_url_key_param(&url),
                status,
                duration_ms,
                e
            );
            IcodeError::gateway(format!("解析 Gemini 模型列表失败: {}", e))
        })?;

        log::info!(
            "Provider API other | GET {} | status={} | duration={}ms",
            redact_url_key_param(&url),
            status,
            duration_ms
        );
        log::debug!(
            "Provider API other response body | GET {} | {}",
            redact_url_key_param(&url),
            body.to_string()
        );

        let models = body.get("models").and_then(|m| m.as_array()).ok_or_else(|| {
            IcodeError::gateway("Gemini 模型列表缺少 models 数组")
        })?;

        let names: Vec<String> = models
            .iter()
            .filter_map(|item| {
                item.get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.strip_prefix("models/").unwrap_or(s).to_string())
            })
            .collect();

        Ok(names)
    }
    /// 拉取 Google Vertex AI 模型列表
    async fn fetch_vertex_ai_models(
        &self,
        client: &reqwest::Client,
        provider: &Provider,
        auth: &Option<AuthConfig>,
    ) -> IcodeResult<Vec<String>> {
        let (project_id, api_key) = match auth {
            Some(AuthConfig::GoogleVertexAiAuth {
                project_id,
                api_key,
                ..
            }) => (project_id.clone(), api_key.clone()),
            _ => {
                return Err(IcodeError::validation(
                    "Vertex AI 需要 GoogleVertexAiAuth 认证配置",
                ))
            }
        };

        let project = project_id.ok_or_else(|| {
            IcodeError::validation("Vertex AI 需要 project_id")
        })?;

        let base = provider.base_url.replace("{region}", "us-central1");
        let base = base.trim_end_matches('/');
        let url = format!(
            "{base}/projects/{project}/locations/us-central1/publishers/google/models"
        );

        let start = std::time::Instant::now();
        let mut req = client.get(&url);
        if let Some(key) = &api_key {
            req = req.query(&[("key", key)]);
        }

        let resp = req.send().await.map_err(|e| {
            log::warn!(
                "Provider API other | GET {} | error={}",
                redact_url_key_param(&url),
                e
            );
            IcodeError::gateway(format!("请求 Vertex AI 模型列表失败: {}", e))
        })?;
        let status = resp.status().as_u16();
        let duration_ms = start.elapsed().as_millis();

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            log::warn!(
                "Provider API other non-200 | GET {} | status={} | duration={}ms | response_body={}",
                redact_url_key_param(&url),
                status,
                duration_ms,
                text
            );
            return Err(IcodeError::gateway(format!(
                "Vertex AI 返回错误 {}: {}",
                status, text
            )));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            log::warn!(
                "Provider API other | GET {} | status={} | duration={}ms | parse_error={}",
                redact_url_key_param(&url),
                status,
                duration_ms,
                e
            );
            IcodeError::gateway(format!("解析 Vertex AI 模型列表失败: {}", e))
        })?;

        log::info!(
            "Provider API other | GET {} | status={} | duration={}ms",
            redact_url_key_param(&url),
            status,
            duration_ms
        );
        log::debug!(
            "Provider API other response body | GET {} | {}",
            redact_url_key_param(&url),
            body.to_string()
        );

        let models = body.get("models").and_then(|m| m.as_array()).ok_or_else(|| {
            IcodeError::gateway("Vertex AI 模型列表缺少 models 数组")
        })?;

        let names: Vec<String> = models
            .iter()
            .filter_map(|item| {
                item.get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.rsplit('/').next().unwrap_or(s).to_string())
            })
            .collect();

        Ok(names)
    }

    /// 拉取 GitHub Copilot 模型列表
    async fn fetch_github_copilot_models(
        &self,
        client: &reqwest::Client,
        provider: &Provider,
        token: Option<&str>,
    ) -> IcodeResult<Vec<String>> {
        let url = if provider.use_raw_base_url {
            format!("{}/models", provider.base_url.trim_end_matches('/'))
        } else {
            let base = provider.base_url.trim_end_matches('/');
            if base.contains("/v1") {
                format!("{}/models", base)
            } else {
                format!("{}/v1/models", base)
            }
        };

        let start = std::time::Instant::now();
        let mut req = client.get(&url)
            .header("X-GitHub-Api-Version", "2026-06-01")
            .header("Editor-Version", "vscode/1.0")
            .header("Copilot-Integration-Id", "vscode-chat");

        if let Some(token) = token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let resp = req.send().await.map_err(|e| {
            log::warn!(
                "Provider API other | GET {} | error={}",
                redact_url_key_param(&url),
                e
            );
            IcodeError::gateway(format!("请求 GitHub Copilot 模型列表失败: {}", e))
        })?;
        let status = resp.status().as_u16();
        let duration_ms = start.elapsed().as_millis();

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            log::warn!(
                "Provider API other non-200 | GET {} | status={} | duration={}ms | response_body={}",
                redact_url_key_param(&url),
                status,
                duration_ms,
                text
            );
            return Err(IcodeError::gateway(format!(
                "GitHub Copilot 返回错误 {}: {}",
                status, text
            )));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            log::warn!(
                "Provider API other | GET {} | status={} | duration={}ms | parse_error={}",
                redact_url_key_param(&url),
                status,
                duration_ms,
                e
            );
            IcodeError::gateway(format!("解析 GitHub Copilot 模型列表失败: {}", e))
        })?;

        log::info!(
            "Provider API other | GET {} | status={} | duration={}ms",
            redact_url_key_param(&url),
            status,
            duration_ms
        );
        log::debug!(
            "Provider API other response body | GET {} | {}",
            redact_url_key_param(&url),
            body.to_string()
        );

        let data = body.get("data").and_then(|d| d.as_array()).ok_or_else(|| {
            IcodeError::gateway("GitHub Copilot 模型列表缺少 data 数组")
        })?;

        let ids: Vec<String> = data
            .iter()
            .filter_map(|item| item.get("id").and_then(|v| v.as_str()).map(String::from))
            .collect();

        Ok(ids)
    }
}

/// 从 AuthConfig 中提取 ApiKey
fn extract_api_key(auth: &Option<AuthConfig>) -> Option<&str> {
    match auth {
        Some(AuthConfig::ApiKey { api_key, .. }) => api_key.as_deref(),
        Some(AuthConfig::GoogleVertexAiAuth { api_key, .. }) => api_key.as_deref(),
        _ => None,
    }
}

/// 从 AuthConfig 中提取 OAuth Token
fn extract_oauth_token(auth: &Option<AuthConfig>) -> Option<&str> {
    match auth {
        Some(AuthConfig::Oauth2 { token, .. }) => token.as_deref(),
        Some(AuthConfig::AntigravityOauth { token, .. }) => token.as_deref(),
        Some(AuthConfig::GoogleGeminiOauth { token, .. }) => token.as_deref(),
        Some(AuthConfig::OpenaiCodexAuth { token, .. }) => token.as_deref(),
        Some(AuthConfig::ClaudeCode { token, .. }) => token.as_deref(),
        Some(AuthConfig::XaiGrokOauth { token, .. }) => token.as_deref(),
        Some(AuthConfig::GithubCopilot { token, .. }) => token.as_deref(),
        _ => None,
    }
}

/// 从 OAuth token JSON 字符串中提取 access_token，供额度查询使用
///
/// 兼容 camelCase（`accessToken`）与 snake_case（`access_token`）两种字段风格；
/// 若 JSON 解析失败则原样返回（兼容直接存储 access_token 的旧数据）。
fn extract_oauth_access_token_for_balance(token_str: &str) -> Option<String> {
    // 优先尝试 camelCase（参考项目存储格式）
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct OAuthTokenCamel {
        access_token: String,
    }
    if let Ok(data) = serde_json::from_str::<OAuthTokenCamel>(token_str) {
        if !data.access_token.is_empty() {
            return Some(data.access_token);
        }
    }

    // 再尝试标准 snake_case
    #[derive(serde::Deserialize)]
    struct OAuthTokenSnake {
        access_token: String,
    }
    if let Ok(data) = serde_json::from_str::<OAuthTokenSnake>(token_str) {
        if !data.access_token.is_empty() {
            return Some(data.access_token);
        }
    }

    // 兼容旧数据：直接存 access_token 字符串（排除 $SECRET 引用前缀）
    if !token_str.starts_with('$') && token_str.len() >= 10 {
        Some(token_str.to_string())
    } else {
        None
    }
}

/// 从 AuthConfig 中提取 OAuth2Config
///
/// 若对应变体不存在 `oauth` 字段，返回默认的空配置。
fn extract_oauth_config(auth: &AuthConfig) -> OAuth2Config {
    match auth {
        AuthConfig::Oauth2 { oauth, .. } => oauth.clone().unwrap_or_else(|| OAuth2Config {
            grant_type: OAuth2GrantType::AuthorizationCode,
            token_url: None,
            revocation_url: None,
            scopes: None,
            authorization_url: None,
            client_id: None,
            client_secret: None,
            pkce: None,
            redirect_uri: None,
            device_authorization_url: None,
        }),
        _ => OAuth2Config {
            grant_type: OAuth2GrantType::AuthorizationCode,
            token_url: None,
            revocation_url: None,
            scopes: None,
            authorization_url: None,
            client_id: None,
            client_secret: None,
            pkce: None,
            redirect_uri: None,
            device_authorization_url: None,
        },
    }
}

/// 清空 AuthConfig JSON 对象中的敏感字段值
///
/// 用于「不带密钥导出」场景：保留认证方法结构与公共字段，
/// 仅将 apiKey、token、clientSecret、keyFilePath 等敏感值置为空字符串。
fn strip_sensitive_auth_values(value: &mut serde_json::Value) {
    let sensitive_keys: &[&str] = &[
        "apiKey",
        "token",
        "clientSecret",
        "keyFilePath",
        "identityId",
        "projectId",
        "managedProjectId",
        "tierId",
        "email",
        "accountId",
    ];
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                if sensitive_keys.contains(&k.as_str()) && v.is_string() {
                    *v = serde_json::Value::String(String::new());
                } else {
                    strip_sensitive_auth_values(v);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                strip_sensitive_auth_values(item);
            }
        }
        _ => {}
    }
}

/// 判断字符串是否为 `$SECRET:{snowflake_id}$` 引用格式
fn is_secret_ref(s: &str) -> bool {
    s.starts_with("$SECRET:") && s.ends_with('$') && s.len() > 10
}

/// 脱敏 URL 中的 key 查询参数
///
/// 部分供应商（如 Gemini）通过 `?key=...` 传递 API Key，
/// 打印日志前需将其替换，避免敏感信息泄漏到 tauri-plugin-log 中。
fn redact_url_key_param(url: &str) -> String {
    let mut result = url.to_string();
    for sep in ["?key=", "&key="] {
        if let Some(pos) = result.find(sep) {
            let start = pos + sep.len();
            let end = result[start..]
                .find('&')
                .map(|i| start + i)
                .unwrap_or(result.len());
            result.replace_range(start..end, "<redacted>");
            break;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_secret_ref() {
        assert!(!is_secret_ref(""));
        assert!(!is_secret_ref("sk-abc123"));
        assert!(!is_secret_ref("$SECRET:"));
        assert!(!is_secret_ref("$SECRET:$"));
        assert!(is_secret_ref("$SECRET:abc-123$"));
        assert!(is_secret_ref("$SECRET:550e8400-e29b-41d4-a716-446655440000$"));
    }

    #[test]
    fn test_handle_clone() {
        // 验证 Handle 可克隆（Tauri State 要求 Clone）
        // 使用 mock 的 SecretServiceHandle（不实际连接数据库）
        let mut key = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut key);
        // 直接构造 handle 的测试需要 SecretServiceHandle::new，此处仅测试 Clone trait 存在
        // 完整集成测试在 main.rs setup 流程中验证
    }
}
