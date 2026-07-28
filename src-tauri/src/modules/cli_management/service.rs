//! # CLI 管理业务服务层
//!
//! 提供 CLI 档案、CLI 供应商绑定、模型映射的业务逻辑。
//!
//! ## 职责
//!
//! - CLI 档案 CRUD 与唯一性校验
//! - CLI 供应商绑定 CRUD，校验路由模式与必填字段
//! - CLI 模型映射 CRUD，校验输入模式与关联关系
//! - 供 workspace 模块查询当前所有 CLI 档案 ID
//!
//! ## 跨模块调用
//!
//! - 依赖 [`ai_gateway`](crate::modules::ai_gateway) 校验 `provider_id` 存在性。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;

use std::collections::HashMap;

use crate::core::id::generate_id;
use crate::error::{IcodeError, IcodeResult};
use crate::modules::ai_gateway::AiGatewayServiceHandle;

use super::repository::CliManagementRepository;
use super::types::{
    ApplyClaudeConfigInput, ApplyClaudeConfigResult, CliConfigFileContent, CliConfigFileInspection,
    CliModelMapping, CliProfile, CliProvider, CliType, CreateCliModelMappingInput,
    CreateCliProfileInput, CreateCliProviderInput, UpdateCliModelMappingInput, UpdateCliProfileInput,
    UpdateCliProviderInput,
};

const MANAGED_CLI_PROFILES: [(&str, &str, &str); 3] = [
    ("claude-code", "Claude CLI", "claude-code"),
    ("codex", "Codex", "codex"),
    ("opencode", "OpenCode", "opencode"),
];

/// CLI 管理服务在 Tauri State 中的句柄
#[derive(Clone)]
pub struct CliManagementServiceHandle {
    inner: Arc<CliManagementService>,
}

impl CliManagementServiceHandle {
    /// 创建 CLI 管理服务句柄
    ///
    /// # 参数
    /// - `ai_gateway_handle`：AI Gateway 服务句柄，用于校验 provider_id 存在性
    pub fn new(ai_gateway_handle: AiGatewayServiceHandle) -> Self {
        Self {
            inner: Arc::new(CliManagementService {
                repo: CliManagementRepository::new(),
                ai_gateway_handle,
            }),
        }
    }

    /// 获取内部 Service 引用
    pub fn service(&self) -> &CliManagementService {
        &self.inner
    }
}

/// CLI 管理服务业务逻辑
pub struct CliManagementService {
    repo: CliManagementRepository,
    ai_gateway_handle: AiGatewayServiceHandle,
}

impl CliManagementService {
    // ===== CLI 档案 =====

    /// 列出所有 CLI 档案
    pub fn list_profiles(&self) -> IcodeResult<Vec<CliProfile>> {
        self.repo.list_profiles()
    }

    /// 幂等创建并返回内置 CLI 档案
    pub fn ensure_default_profiles(&self) -> IcodeResult<Vec<CliProfile>> {
        let mut profiles = Vec::with_capacity(MANAGED_CLI_PROFILES.len());
        for (slug, display_name, cli_type) in MANAGED_CLI_PROFILES {
            if let Some(profile) = self.repo.get_profile_by_slug(slug)? {
                profiles.push(profile);
                continue;
            }

            profiles.push(self.create_profile(CreateCliProfileInput {
                slug: slug.to_string(),
                display_name: display_name.to_string(),
                cli_type: cli_type.to_string(),
                config_file_path: None,
                proxy_json: None,
                is_enabled: true,
            })?);
        }
        Ok(profiles)
    }

    /// 探测 CLI 配置文件位置并验证语法
    ///
    /// 返回值不包含配置正文，避免认证信息越过 Rust 后端边界。
    pub fn inspect_config_file(
        &self,
        cli_type: &str,
        configured_path: Option<&str>,
    ) -> IcodeResult<CliConfigFileInspection> {
        let cli_type_value = CliType::from_str(cli_type)
            .ok_or_else(|| IcodeError::validation(format!("未知的 CLI 类型: {cli_type}")))?;
        let candidates = config_candidates(cli_type_value)?;
        let suggested_path = candidates
            .first()
            .cloned()
            .ok_or_else(|| IcodeError::internal("无法确定 CLI 配置文件路径"))?;
        let configured = configured_path
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_string);
        let resolved_path = configured
            .as_deref()
            .map(expand_home_path)
            .transpose()?
            .unwrap_or_else(|| {
                candidates
                    .iter()
                    .find(|path| path.exists())
                    .cloned()
                    .unwrap_or_else(|| suggested_path.clone())
            });
        let format = config_format(cli_type_value, &resolved_path);

        if !resolved_path.exists() {
            return Ok(CliConfigFileInspection {
                cli_type: cli_type.to_string(),
                configured_path: configured,
                suggested_path: path_to_string(&suggested_path),
                resolved_path: path_to_string(&resolved_path),
                format,
                exists: false,
                is_file: false,
                readable: false,
                parse_status: "missing".to_string(),
                issue: None,
                client_available: is_client_available(cli_type),
            });
        }

        if !resolved_path.is_file() {
            return Ok(CliConfigFileInspection {
                cli_type: cli_type.to_string(),
                configured_path: configured,
                suggested_path: path_to_string(&suggested_path),
                resolved_path: path_to_string(&resolved_path),
                format,
                exists: true,
                is_file: false,
                readable: false,
                parse_status: "invalid".to_string(),
                issue: Some("not-file".to_string()),
                client_available: is_client_available(cli_type),
            });
        }

        let content = match fs::read_to_string(&resolved_path) {
            Ok(content) => content,
            Err(_) => {
                return Ok(CliConfigFileInspection {
                    cli_type: cli_type.to_string(),
                    configured_path: configured,
                    suggested_path: path_to_string(&suggested_path),
                    resolved_path: path_to_string(&resolved_path),
                    format,
                    exists: true,
                    is_file: true,
                    readable: false,
                    parse_status: "invalid".to_string(),
                    issue: Some("unreadable".to_string()),
                    client_available: is_client_available(cli_type),
                });
            }
        };

        let valid = parse_config_content(cli_type_value, &format, &content);
        Ok(CliConfigFileInspection {
            cli_type: cli_type.to_string(),
            configured_path: configured,
            suggested_path: path_to_string(&suggested_path),
            resolved_path: path_to_string(&resolved_path),
            format,
            exists: true,
            is_file: true,
            readable: true,
            parse_status: if valid { "valid" } else { "invalid" }.to_string(),
            issue: (!valid).then(|| "invalid-syntax".to_string()),
            client_available: is_client_available(cli_type),
        })
    }

    /// 获取 CLI 档案详情
    pub fn get_profile(&self, id: &str) -> IcodeResult<CliProfile> {
        self.repo
            .get_profile(id)?
            .ok_or_else(|| IcodeError::not_found("CLI 档案", Some(id)))
    }

    /// 读取 CLI 配置文件内容
    ///
    /// 仅当文件存在且可读时返回文本，用于前端预览编辑。
    pub fn read_config_file(
        &self,
        cli_type: &str,
        configured_path: Option<&str>,
    ) -> IcodeResult<CliConfigFileContent> {
        let cli_type_value = CliType::from_str(cli_type)
            .ok_or_else(|| IcodeError::validation(format!("未知的 CLI 类型: {cli_type}")))?;
        let resolved_path = resolve_config_path(cli_type_value, configured_path)?;
        let format = config_format(cli_type_value, &resolved_path);
        if !resolved_path.exists() {
            return Err(IcodeError::not_found("配置文件", None));
        }
        if !resolved_path.is_file() {
            return Err(IcodeError::validation("配置路径不是文件"));
        }
        let content = fs::read_to_string(&resolved_path)
            .map_err(|e| IcodeError::internal(format!("无法读取配置文件: {e}")))?;
        Ok(CliConfigFileContent {
            cli_type: cli_type.to_string(),
            resolved_path: path_to_string(&resolved_path),
            format,
            content,
        })
    }

    /// 保存 CLI 配置文件内容
    ///
    /// 写回前校验语法；写入失败时保留原文件不变。
    pub fn save_config_file(
        &self,
        cli_type: &str,
        configured_path: Option<&str>,
        content: &str,
    ) -> IcodeResult<CliConfigFileContent> {
        let cli_type_value = CliType::from_str(cli_type)
            .ok_or_else(|| IcodeError::validation(format!("未知的 CLI 类型: {cli_type}")))?;
        let resolved_path = resolve_config_path(cli_type_value, configured_path)?;
        let format = config_format(cli_type_value, &resolved_path);
        if !parse_config_content(cli_type_value, &format, content) {
            return Err(IcodeError::validation("配置内容语法无效，已拒绝写入"));
        }
        fs::write(&resolved_path, content)
            .map_err(|e| IcodeError::internal(format!("无法写入配置文件: {e}")))?;
        Ok(CliConfigFileContent {
            cli_type: cli_type.to_string(),
            resolved_path: path_to_string(&resolved_path),
            format,
            content: content.to_string(),
        })
    }

    /// 创建 CLI 档案
    ///
    /// 校验：
    /// - slug 全局唯一
    /// - cli_type 合法
    pub fn create_profile(&self, input: CreateCliProfileInput) -> IcodeResult<CliProfile> {
        validate_slug(&input.slug)?;
        if CliType::from_str(&input.cli_type).is_none() {
            return Err(IcodeError::validation(format!(
                "未知的 CLI 类型: {}",
                input.cli_type
            )));
        }
        if self.repo.get_profile_by_slug(&input.slug)?.is_some() {
            return Err(IcodeError::conflict(format!(
                "CLI 档案 slug '{}' 已存在",
                input.slug
            )));
        }
        let id = generate_id();
        let now = now_iso();
        self.repo.create_profile(&id, &input, &now)
    }

    /// 更新 CLI 档案
    ///
    /// 若更新 slug，需保证新 slug 全局唯一。
    pub fn update_profile(&self, id: &str, input: UpdateCliProfileInput) -> IcodeResult<CliProfile> {
        // 先确认存在
        let _ = self.get_profile(id)?;

        if let Some(slug) = &input.slug {
            validate_slug(slug)?;
            if let Some(existing) = self.repo.get_profile_by_slug(slug)? {
                if existing.id != id {
                    return Err(IcodeError::conflict(format!(
                        "CLI 档案 slug '{}' 已存在",
                        slug
                    )));
                }
            }
        }
        if let Some(cli_type) = &input.cli_type {
            if CliType::from_str(cli_type).is_none() {
                return Err(IcodeError::validation(format!(
                    "未知的 CLI 类型: {}",
                    cli_type
                )));
            }
        }
        let now = now_iso();
        self.repo.update_profile(id, &input, &now)
    }

    /// 删除 CLI 档案
    ///
    /// 数据库级联删除关联的 cli_providers 与 cli_model_mappings。
    pub fn delete_profile(&self, id: &str) -> IcodeResult<()> {
        let _ = self.get_profile(id)?;
        self.repo.delete_profile(id)
    }

    /// 获取所有 CLI 档案 ID
    ///
    /// 供 workspace 模块在新建工作区时初始化 `workspace_cli_configs`。
    pub fn list_profile_ids(&self) -> IcodeResult<Vec<String>> {
        self.repo.list_profile_ids()
    }

    // ===== CLI 供应商绑定 =====

    /// 列出某 CLI 档案绑定的供应商
    pub fn list_providers(&self, cli_profile_id: &str) -> IcodeResult<Vec<CliProvider>> {
        let _ = self.get_profile(cli_profile_id)?;
        self.repo.list_providers(cli_profile_id)
    }

    /// 获取 CLI 供应商绑定详情
    pub fn get_provider(&self, id: &str) -> IcodeResult<CliProvider> {
        self.repo
            .get_provider(id)?
            .ok_or_else(|| IcodeError::not_found("CLI 供应商绑定", Some(id)))
    }

    /// 创建 CLI 供应商绑定
    ///
    /// 校验：
    /// - cli_profile_id 存在
    /// - provider_id 若提供，则在 providers 表中存在
    /// - route_mode = 0 时必须提供 direct_base_url
    pub fn create_provider(&self, input: CreateCliProviderInput) -> IcodeResult<CliProvider> {
        let _ = self.get_profile(&input.cli_profile_id)?;
        if let Some(provider_id) = &input.provider_id {
            let _ = self
                .ai_gateway_handle
                .service()
                .get_provider(provider_id)
                .map_err(|_| IcodeError::not_found("AI Gateway 供应商", Some(provider_id)))?;
        }
        if input.route_mode == 0 && input.direct_base_url.is_none() {
            return Err(IcodeError::validation(
                "直连模式（route_mode=0）必须填写 direct_base_url",
            ));
        }
        let id = generate_id();
        let now = now_iso();
        self.repo.create_provider(&id, &input, &now)
    }

    /// 更新 CLI 供应商绑定
    pub fn update_provider(&self, id: &str, input: UpdateCliProviderInput) -> IcodeResult<CliProvider> {
        let existing = self.get_provider(id)?;
        if let Some(provider_id) = &input.provider_id {
            let _ = self
                .ai_gateway_handle
                .service()
                .get_provider(provider_id)
                .map_err(|_| IcodeError::not_found("AI Gateway 供应商", Some(provider_id)))?;
        }
        let route_mode = input.route_mode.unwrap_or(existing.route_mode);
        let direct_base_url = input.direct_base_url.as_ref().or(existing.direct_base_url.as_ref());
        if route_mode == 0 && direct_base_url.is_none() {
            return Err(IcodeError::validation(
                "直连模式（route_mode=0）必须填写 direct_base_url",
            ));
        }
        let now = now_iso();
        self.repo.update_provider(id, &input, &now)
    }

    /// 删除 CLI 供应商绑定
    pub fn delete_provider(&self, id: &str) -> IcodeResult<()> {
        let _ = self.get_provider(id)?;
        self.repo.delete_provider(id)
    }

    // ===== CLI 模型映射 =====

    /// 列出某 CLI 供应商下的模型映射
    pub fn list_model_mappings(&self, cli_provider_id: &str) -> IcodeResult<Vec<CliModelMapping>> {
        let _ = self.get_provider(cli_provider_id)?;
        self.repo.list_model_mappings(cli_provider_id)
    }

    /// 获取模型映射详情
    pub fn get_model_mapping(&self, id: &str) -> IcodeResult<CliModelMapping> {
        self.repo
            .get_model_mapping(id)?
            .ok_or_else(|| IcodeError::not_found("CLI 模型映射", Some(id)))
    }

    /// 创建模型映射
    ///
    /// 校验：
    /// - cli_provider_id 存在
    /// - input_mode 为 `select` 或 `manual`
    /// - select 模式必须提供 gateway_model_id；manual 模式必须提供 raw_model_id
    pub fn create_model_mapping(&self, input: CreateCliModelMappingInput) -> IcodeResult<CliModelMapping> {
        let _ = self.get_provider(&input.cli_provider_id)?;
        validate_mapping_input(&input)?;
        let id = generate_id();
        let now = now_iso();
        self.repo.create_model_mapping(&id, &input, &now)
    }

    /// 更新模型映射
    pub fn update_model_mapping(
        &self,
        id: &str,
        input: UpdateCliModelMappingInput,
    ) -> IcodeResult<CliModelMapping> {
        let _ = self.get_model_mapping(id)?;
        if let Some(input_mode) = &input.input_mode {
            if input_mode != "select" && input_mode != "manual" {
                return Err(IcodeError::validation(
                    "input_mode 必须为 select 或 manual",
                ));
            }
        }
        let now = now_iso();
        self.repo.update_model_mapping(id, &input, &now)
    }

    /// 删除模型映射
    pub fn delete_model_mapping(&self, id: &str) -> IcodeResult<()> {
        let _ = self.get_model_mapping(id)?;
        self.repo.delete_model_mapping(id)
    }

    /// 应用 Claude CLI 配置到实际配置文件
    ///
    /// 根据传入的映射、开关、API Key 生成 Claude Code settings.json，
    /// 写入 cli_profiles.config_file_path 或默认候选路径。
    pub fn apply_claude_config(&self, input: ApplyClaudeConfigInput) -> IcodeResult<ApplyClaudeConfigResult> {
        let provider = self.get_provider(&input.cli_provider_id)?;
        let profile = self.get_profile(&provider.cli_profile_id)?;

        let cli_type = CliType::from_str(&profile.cli_type)
            .ok_or_else(|| IcodeError::validation(format!("未知的 CLI 类型: {}", profile.cli_type)))?;
        if cli_type != CliType::ClaudeCode {
            return Err(IcodeError::validation("该命令仅支持 Claude Code CLI"));
        }

        let resolved_path = resolve_config_path(cli_type, profile.config_file_path.as_deref())?;
        let content = generate_claude_settings_json(&provider, &input)?;

        std::fs::write(&resolved_path, &content)
            .map_err(|e| IcodeError::internal(format!("无法写入 Claude 配置文件: {e}")))?;

        Ok(ApplyClaudeConfigResult {
            cli_provider_id: provider.id,
            resolved_path: path_to_string(&resolved_path),
            content,
        })
    }
}

// ===== 私有辅助函数 =====

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn validate_slug(slug: &str) -> IcodeResult<()> {
    if slug.is_empty() {
        return Err(IcodeError::validation("slug 不能为空"));
    }
    if slug.contains('/') || slug.contains(' ') {
        return Err(IcodeError::validation("slug 不能包含空格或斜杠"));
    }
    Ok(())
}

fn validate_mapping_input(input: &CreateCliModelMappingInput) -> IcodeResult<()> {
    if input.input_mode != "select" && input.input_mode != "manual" {
        return Err(IcodeError::validation("input_mode 必须为 select 或 manual"));
    }
    if input.input_mode == "select" && input.gateway_model_id.is_none() {
        return Err(IcodeError::validation(
            "select 模式必须提供 gateway_model_id",
        ));
    }
    if input.input_mode == "manual" && input.raw_model_id.is_none() {
        return Err(IcodeError::validation(
            "manual 模式必须提供 raw_model_id",
        ));
    }
    Ok(())
}

/// 生成 Claude Code settings.json 内容
///
/// 角色环境变量映射与前端 `claude-cli-panel.tsx` 的 `generateSettingsJson` 保持一致。
fn generate_claude_settings_json(
    provider: &CliProvider,
    input: &ApplyClaudeConfigInput,
) -> IcodeResult<String> {
    let mut env: HashMap<String, String> = HashMap::new();

    // 基础 URL：网关模式用网关地址，直连模式用 direct_base_url
    let base_url = if provider.route_mode == 1 {
        provider
            .gateway_base_url
            .clone()
            .unwrap_or_else(|| "http://127.0.0.1:54321".to_string())
    } else {
        provider.direct_base_url.clone().unwrap_or_default()
    };
    env.insert("ANTHROPIC_BASE_URL".to_string(), base_url);

    // 角色 → 环境变量键映射
    let role_env_map: HashMap<&str, (&str, &str)> = [
        ("Sonnet", ("ANTHROPIC_DEFAULT_SONNET_MODEL", "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME")),
        ("Opus", ("ANTHROPIC_DEFAULT_OPUS_MODEL", "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME")),
        ("Fable", ("ANTHROPIC_DEFAULT_FABLE_MODEL", "ANTHROPIC_DEFAULT_FABLE_MODEL_NAME")),
        ("Haiku", ("ANTHROPIC_DEFAULT_HAIKU_MODEL", "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME")),
    ]
    .into_iter()
    .collect();

    // 遍历映射生成模型环境变量
    for item in &input.mappings {
        let Some((model_key, name_key)) = role_env_map.get(item.role.as_str()).copied() else {
            continue;
        };
        if item.actual_model.is_empty() {
            continue;
        }

        let model_name = item.actual_model.replace("[1M]", "");
        let model_value = if item.supports_1m {
            format!("{}[1M]", model_name)
        } else {
            model_name.clone()
        };

        env.insert(model_key.to_string(), model_value);
        env.insert(name_key.to_string(), model_name);
    }

    // 兜底模型：显式 fallback > 第一个映射的实际模型
    let fallback = {
        let trimmed = input.fallback_model.trim().replace("[1M]", "");
        if trimmed.is_empty() {
            input
                .mappings
                .first()
                .map(|m| m.actual_model.replace("[1M]", ""))
                .unwrap_or_default()
        } else {
            trimmed
        }
    };
    if !fallback.is_empty() {
        env.insert("ANTHROPIC_MODEL".to_string(), fallback);
    }

    // Auth Token
    env.insert(
        "ANTHROPIC_AUTH_TOKEN".to_string(),
        if input.api_key.trim().is_empty() {
            "sk-daeafweeeeeeeeeeeeeeee".to_string()
        } else {
            input.api_key.trim().to_string()
        },
    );

    // 开关联动
    if input.switches.agent_teams {
        env.insert("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS".to_string(), "1".to_string());
    }
    if input.switches.tool_search {
        env.insert("ENABLE_TOOL_SEARCH".to_string(), "true".to_string());
    }
    if input.switches.max_effort {
        env.insert("CLAUDE_CODE_EFFORT_LEVEL".to_string(), "max".to_string());
    }
    if input.switches.disable_autoupdater {
        env.insert("DISABLE_AUTOUPDATER".to_string(), "1".to_string());
    }

    let settings = serde_json::json!({
        "env": env,
        "includeCoAuthoredBy": input.switches.hide_co_author,
        "model": "haiku",
    });

    serde_json::to_string_pretty(&settings)
        .map_err(|e| IcodeError::internal(format!("生成 Claude 配置 JSON 失败: {e}")))
}

fn config_candidates(cli_type: CliType) -> IcodeResult<Vec<PathBuf>> {
    let home = dirs::home_dir().ok_or_else(|| IcodeError::internal("无法获取用户主目录"))?;
    let candidates = match cli_type {
        CliType::ClaudeCode => vec![
            home.join(".claude").join("settings.json"),
            home.join(".claude.json"),
        ],
        CliType::Codex => vec![home.join(".codex").join("config.toml")],
        CliType::OpenCode => vec![
            home.join(".config").join("opencode").join("opencode.json"),
            home.join(".config").join("opencode").join("opencode.jsonc"),
        ],
        _ => {
            return Err(IcodeError::validation(
                "当前客户端暂不支持配置文件探测",
            ));
        }
    };
    Ok(candidates)
}

fn expand_home_path(path: &str) -> IcodeResult<PathBuf> {
    if path == "~" {
        return dirs::home_dir().ok_or_else(|| IcodeError::internal("无法获取用户主目录"));
    }
    if let Some(relative) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        let home = dirs::home_dir().ok_or_else(|| IcodeError::internal("无法获取用户主目录"))?;
        return Ok(home.join(relative));
    }
    Ok(PathBuf::from(path))
}

fn config_format(cli_type: CliType, path: &Path) -> String {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("toml") => "toml",
        Some("jsonc") => "jsonc",
        Some("json") => "json",
        _ if cli_type == CliType::Codex => "toml",
        _ => "json",
    }
    .to_string()
}

fn parse_config_content(cli_type: CliType, format: &str, content: &str) -> bool {
    if content.trim().is_empty() {
        return true;
    }
    match format {
        "toml" => toml::from_str::<toml::Value>(content).is_ok(),
        "jsonc" => json5::from_str::<serde_json::Value>(content).is_ok(),
        "json" if cli_type == CliType::OpenCode => {
            json5::from_str::<serde_json::Value>(content).is_ok()
        }
        _ => serde_json::from_str::<serde_json::Value>(content).is_ok(),
    }
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// 解析 CLI 配置文件最终路径（统一规则）
fn resolve_config_path(cli_type: CliType, configured_path: Option<&str>) -> IcodeResult<PathBuf> {
    let candidates = config_candidates(cli_type)?;
    let suggested_path = candidates
        .first()
        .cloned()
        .ok_or_else(|| IcodeError::internal("无法确定 CLI 配置文件路径"))?;
    let configured = configured_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_string);
    Ok(configured
        .as_deref()
        .map(expand_home_path)
        .transpose()?
        .unwrap_or_else(|| {
            candidates
                .iter()
                .find(|path| path.exists())
                .cloned()
                .unwrap_or_else(|| suggested_path.clone())
        }))
}

/// 检查客户端 CLI 二进制是否在 PATH 中可用
pub fn is_client_available(cli_type: &str) -> bool {
    let binary_name = match cli_type {
        "claude-code" => "claude",
        "codex" => "codex",
        "opencode" => "opencode",
        "gemini-cli" => "gemini",
        "cursor-agent" => "cursor-agent",
        _ => return false,
    };
    which::which(binary_name).is_ok()
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn parses_supported_config_formats() {
        assert!(parse_config_content(CliType::ClaudeCode, "json", r#"{"env":{}}"#));
        assert!(parse_config_content(CliType::Codex, "toml", "model = \"gpt-5\""));
        assert!(parse_config_content(
            CliType::OpenCode,
            "jsonc",
            "{ // comment\n \"provider\": {}\n}",
        ));
        assert!(!parse_config_content(CliType::Codex, "toml", "model = ["));
    }
}
