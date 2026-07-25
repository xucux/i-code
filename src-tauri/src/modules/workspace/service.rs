//! # 工作区业务服务层
//!
//! 提供工作区、Prompts、MCP、Skill 的隔离管理，以及「应用」到 CLI 配置文件的编排逻辑。
//!
//! ## v0.1 实现范围
//!
//! - 工作区 CRUD 与切换
//! - 工作区子配置（Prompts / MCP / Skill）CRUD
//! - 创建 workspace 时自动为所有 CLI 档案生成 `workspace_cli_configs`
//! - 子配置变更时自动将对应 `workspace_cli_configs.pending_apply` 置 1
//! - `workspace_apply` 基础实现：生成统一 JSON 配置并写入 `cli_profiles.config_file_path`
//!
//! ## 待后续迭代
//!
//! - 按不同 CLI 类型（claude-code / codex / gemini-cli）生成对应格式配置文件
//! - 配置文件模板与合并策略
//! - 与 cli_management 的双向同步回调（创建 CLI 档案时自动补充 workspace_cli_configs）
//!
//! ## 跨模块调用
//!
//! - 依赖 [`cli_management`](crate::modules::cli_management) 获取 CLI 档案列表与路径。

use std::sync::Arc;

use chrono::Utc;
use serde_json::json;

use crate::core::id::generate_id;
use crate::error::{IcodeError, IcodeResult};
use crate::modules::cli_management::CliManagementServiceHandle;

use super::repository::WorkspaceRepository;
use super::types::{
    ApplyCliResult, ApplyWorkspaceResult, CreateWorkspaceInput, CreateWorkspaceMcpServerInput,
    CreateWorkspacePromptInput, CreateWorkspaceSkillInput, UpdateWorkspaceInput,
    UpdateWorkspaceMcpServerInput, UpdateWorkspacePromptInput, UpdateWorkspaceSkillInput,
    Workspace, WorkspaceAggregate, WorkspaceCliConfig, WorkspaceCliConfigAggregate,
    WorkspaceMcpServer, WorkspacePreviewResult, WorkspacePrompt, WorkspaceSkill,
};

/// 工作区服务在 Tauri State 中的句柄
#[derive(Clone)]
pub struct WorkspaceServiceHandle {
    inner: Arc<WorkspaceService>,
}

impl WorkspaceServiceHandle {
    /// 创建工作区服务句柄
    ///
    /// # 参数
    /// - `cli_management_handle`：CLI 管理服务句柄，用于读取 CLI 档案信息
    pub fn new(cli_management_handle: CliManagementServiceHandle) -> Self {
        Self {
            inner: Arc::new(WorkspaceService {
                repo: WorkspaceRepository::new(),
                cli_management_handle,
            }),
        }
    }

    /// 获取内部 Service 引用
    pub fn service(&self) -> &WorkspaceService {
        &self.inner
    }
}

/// 工作区服务业务逻辑
pub struct WorkspaceService {
    repo: WorkspaceRepository,
    cli_management_handle: CliManagementServiceHandle,
}

impl WorkspaceService {
    // ===== 工作区 =====

    /// 列出所有工作区
    pub fn list_workspaces(&self) -> IcodeResult<Vec<Workspace>> {
        self.repo.list_workspaces()
    }

    /// 获取工作区详情
    pub fn get_workspace(&self, id: &str) -> IcodeResult<Workspace> {
        self.repo
            .get_workspace(id)?
            .ok_or_else(|| IcodeError::not_found("工作区", Some(id)))
    }

    /// 获取当前激活工作区
    pub fn get_active_workspace(&self) -> IcodeResult<Option<Workspace>> {
        self.repo.get_active_workspace()
    }

    /// 创建工作区
    ///
    /// 流程：
    /// 1. 校验 slug 全局唯一
    /// 2. 插入 workspaces 记录
    /// 3. 为所有已存在的 CLI 档案创建 `workspace_cli_configs`
    pub fn create_workspace(&self, input: CreateWorkspaceInput) -> IcodeResult<Workspace> {
        validate_slug(&input.slug)?;
        if self.repo.get_workspace_by_slug(&input.slug)?.is_some() {
            return Err(IcodeError::conflict(format!(
                "工作区 slug '{}' 已存在",
                input.slug
            )));
        }

        let id = generate_id();
        let now = now_iso();
        let workspace = self.repo.create_workspace(&id, &input, &now)?;

        // 为所有 CLI 档案创建配置头
        let profile_ids = self.cli_management_handle.service().list_profile_ids()?;
        if !profile_ids.is_empty() {
            self.repo
                .create_cli_configs_batch(&workspace.id, &profile_ids, &now)?;
        }

        // 若设置为激活，则切换
        if input.is_active {
            return self.repo.switch_workspace(&workspace.id, &now);
        }
        Ok(workspace)
    }

    /// 更新工作区
    pub fn update_workspace(&self, id: &str, input: UpdateWorkspaceInput) -> IcodeResult<Workspace> {
        let _ = self.get_workspace(id)?;
        if let Some(slug) = &input.slug {
            validate_slug(slug)?;
            if let Some(existing) = self.repo.get_workspace_by_slug(slug)? {
                if existing.id != id {
                    return Err(IcodeError::conflict(format!(
                        "工作区 slug '{}' 已存在",
                        slug
                    )));
                }
            }
        }
        let now = now_iso();
        self.repo.update_workspace(id, &input, &now)
    }

    /// 删除工作区
    pub fn delete_workspace(&self, id: &str) -> IcodeResult<()> {
        let _ = self.get_workspace(id)?;
        self.repo.delete_workspace(id)
    }

    /// 切换激活工作区
    pub fn switch_workspace(&self, id: &str) -> IcodeResult<Workspace> {
        let _ = self.get_workspace(id)?;
        let now = now_iso();
        self.repo.switch_workspace(id, &now)
    }

    // ===== 工作区 CLI 配置头 =====

    /// 列出某工作区下的 CLI 配置头
    pub fn list_cli_configs(&self, workspace_id: &str) -> IcodeResult<Vec<WorkspaceCliConfig>> {
        let _ = self.get_workspace(workspace_id)?;
        self.repo.list_cli_configs(workspace_id)
    }

    // ===== Prompts =====

    /// 列出某 CLI 配置头下的 Prompts
    pub fn list_prompts(
        &self,
        workspace_cli_config_id: &str,
    ) -> IcodeResult<Vec<WorkspacePrompt>> {
        let _ = self.get_cli_config(workspace_cli_config_id)?;
        self.repo.list_prompts(workspace_cli_config_id)
    }

    /// 获取 Prompt 详情
    pub fn get_prompt(&self, id: &str) -> IcodeResult<WorkspacePrompt> {
        self.repo
            .get_prompt(id)?
            .ok_or_else(|| IcodeError::not_found("Prompt", Some(id)))
    }

    /// 创建 Prompt
    ///
    /// 创建后自动将所属 `workspace_cli_configs.pending_apply` 置 1。
    pub fn create_prompt(&self, input: CreateWorkspacePromptInput) -> IcodeResult<WorkspacePrompt> {
        let config = self.get_cli_config(&input.workspace_cli_config_id)?;
        let id = generate_id();
        let now = now_iso();
        let prompt = self.repo.create_prompt(&id, &input, &now)?;
        self.repo.set_cli_config_pending(&config.id, &now)?;
        Ok(prompt)
    }

    /// 更新 Prompt
    pub fn update_prompt(
        &self,
        id: &str,
        input: UpdateWorkspacePromptInput,
    ) -> IcodeResult<WorkspacePrompt> {
        let existing = self.get_prompt(id)?;
        let now = now_iso();
        let prompt = self.repo.update_prompt(id, &input, &now)?;
        self.repo.set_cli_config_pending(&existing.workspace_cli_config_id, &now)?;
        Ok(prompt)
    }

    /// 删除 Prompt
    pub fn delete_prompt(&self, id: &str) -> IcodeResult<()> {
        let prompt = self.get_prompt(id)?;
        let now = now_iso();
        self.repo.delete_prompt(id)?;
        self.repo.set_cli_config_pending(&prompt.workspace_cli_config_id, &now)?;
        Ok(())
    }

    // ===== MCP Servers =====

    /// 列出某 CLI 配置头下的 MCP Servers
    pub fn list_mcp_servers(
        &self,
        workspace_cli_config_id: &str,
    ) -> IcodeResult<Vec<WorkspaceMcpServer>> {
        let _ = self.get_cli_config(workspace_cli_config_id)?;
        self.repo.list_mcp_servers(workspace_cli_config_id)
    }

    /// 获取 MCP Server 详情
    pub fn get_mcp_server(&self, id: &str) -> IcodeResult<WorkspaceMcpServer> {
        self.repo
            .get_mcp_server(id)?
            .ok_or_else(|| IcodeError::not_found("MCP Server", Some(id)))
    }

    /// 创建 MCP Server
    pub fn create_mcp_server(
        &self,
        input: CreateWorkspaceMcpServerInput,
    ) -> IcodeResult<WorkspaceMcpServer> {
        let config = self.get_cli_config(&input.workspace_cli_config_id)?;
        validate_transport(&input.transport)?;
        let id = generate_id();
        let now = now_iso();
        let server = self.repo.create_mcp_server(&id, &input, &now)?;
        self.repo.set_cli_config_pending(&config.id, &now)?;
        Ok(server)
    }

    /// 更新 MCP Server
    pub fn update_mcp_server(
        &self,
        id: &str,
        input: UpdateWorkspaceMcpServerInput,
    ) -> IcodeResult<WorkspaceMcpServer> {
        let existing = self.get_mcp_server(id)?;
        if let Some(transport) = &input.transport {
            validate_transport(transport)?;
        }
        let now = now_iso();
        let server = self.repo.update_mcp_server(id, &input, &now)?;
        self.repo.set_cli_config_pending(&existing.workspace_cli_config_id, &now)?;
        Ok(server)
    }

    /// 删除 MCP Server
    pub fn delete_mcp_server(&self, id: &str) -> IcodeResult<()> {
        let server = self.get_mcp_server(id)?;
        let now = now_iso();
        self.repo.delete_mcp_server(id)?;
        self.repo.set_cli_config_pending(&server.workspace_cli_config_id, &now)?;
        Ok(())
    }

    // ===== Skills =====

    /// 列出某 CLI 配置头下的 Skills
    pub fn list_skills(
        &self,
        workspace_cli_config_id: &str,
    ) -> IcodeResult<Vec<WorkspaceSkill>> {
        let _ = self.get_cli_config(workspace_cli_config_id)?;
        self.repo.list_skills(workspace_cli_config_id)
    }

    /// 获取 Skill 详情
    pub fn get_skill(&self, id: &str) -> IcodeResult<WorkspaceSkill> {
        self.repo
            .get_skill(id)?
            .ok_or_else(|| IcodeError::not_found("Skill", Some(id)))
    }

    /// 创建 Skill
    pub fn create_skill(&self, input: CreateWorkspaceSkillInput) -> IcodeResult<WorkspaceSkill> {
        let config = self.get_cli_config(&input.workspace_cli_config_id)?;
        let id = generate_id();
        let now = now_iso();
        let skill = self.repo.create_skill(&id, &input, &now)?;
        self.repo.set_cli_config_pending(&config.id, &now)?;
        Ok(skill)
    }

    /// 更新 Skill
    pub fn update_skill(
        &self,
        id: &str,
        input: UpdateWorkspaceSkillInput,
    ) -> IcodeResult<WorkspaceSkill> {
        let existing = self.get_skill(id)?;
        let now = now_iso();
        let skill = self.repo.update_skill(id, &input, &now)?;
        self.repo.set_cli_config_pending(&existing.workspace_cli_config_id, &now)?;
        Ok(skill)
    }

    /// 删除 Skill
    pub fn delete_skill(&self, id: &str) -> IcodeResult<()> {
        let skill = self.get_skill(id)?;
        let now = now_iso();
        self.repo.delete_skill(id)?;
        self.repo.set_cli_config_pending(&skill.workspace_cli_config_id, &now)?;
        Ok(())
    }

    // ===== 应用工作区 =====

    /// 将当前工作区配置应用到 CLI 实际配置文件
    ///
    /// v0.1 实现方案：
    /// - 读取该工作区下所有 `workspace_cli_configs`
    /// - 对每个配置头，读取 prompts / mcp_servers / skills
    /// - 生成统一 JSON 结构写入 `cli_profiles.config_file_path`
    /// - 更新 `workspace_cli_configs` 为已应用状态
    ///
    /// ## 后续可迭代方案
    ///
    /// 方案 A（当前）：统一 JSON，所有 CLI 共用同一结构，由 CLI 自行读取。
    /// 方案 B：按 CLI 类型生成对应格式（如 Claude Code 的 JSON、Codex 的 YAML）。
    /// 方案 C：通过模板引擎渲染配置文件，支持用户自定义模板。
    pub fn apply_workspace(&self, id: &str) -> IcodeResult<ApplyWorkspaceResult> {
        let workspace = self.get_workspace(id)?;
        let configs = self.repo.list_cli_configs(id)?;
        let now = now_iso();
        let mut details = Vec::new();
        let mut applied_count = 0;
        let mut failed_count = 0;

        for config in configs {
            let cli_profile_id = config.cli_profile_id.clone();
            let result = self.apply_single_cli_config(&config, &now);
            match result {
                Ok(_) => {
                    self.repo.mark_cli_config_applied(&config.id, &now)?;
                    applied_count += 1;
                    details.push(ApplyCliResult {
                        cli_profile_id,
                        success: true,
                        error: None,
                    });
                }
                Err(e) => {
                    failed_count += 1;
                    details.push(ApplyCliResult {
                        cli_profile_id,
                        success: false,
                        error: Some(e.message),
                    });
                }
            }
        }

        self.repo.touch_workspace_applied(&workspace.id, &now)?;

        Ok(ApplyWorkspaceResult {
            workspace_id: workspace.id,
            applied_count,
            failed_count,
            details,
        })
    }

    /// 聚合查询工作区下所有 CLI 配置及其子配置
    ///
    /// 一次查询完成 config -> profile -> prompts / mcp_servers / skills 的关联，
    /// 供前端新工作区页面一次性加载完整数据。
    pub fn aggregate_workspace(&self, id: &str) -> IcodeResult<WorkspaceAggregate> {
        let workspace = self.get_workspace(id)?;
        let configs = self.repo.list_cli_configs(id)?;
        let mut cli_configs = Vec::with_capacity(configs.len());

        for config in configs {
            let profile = self
                .cli_management_handle
                .service()
                .get_profile(&config.cli_profile_id)
                .map_err(|e| {
                    // CLI 档案缺失不应导致整个聚合失败，降级为占位档案
                    IcodeError::internal(format!(
                        "无法加载 CLI 档案 {}: {}",
                        config.cli_profile_id, e.message
                    ))
                })?;
            let prompts = self.repo.list_prompts(&config.id)?;
            let mcp_servers = self.repo.list_mcp_servers(&config.id)?;
            let skills = self.repo.list_skills(&config.id)?;
            cli_configs.push(WorkspaceCliConfigAggregate {
                config,
                profile,
                prompts,
                mcp_servers,
                skills,
            });
        }

        Ok(WorkspaceAggregate {
            workspace,
            cli_configs,
        })
    }

    /// 预览单个 CLI 配置头将要生成的配置文件内容
    ///
    /// 只读生成，不写入文件系统，供前端预览弹窗使用。
    pub fn preview_cli_config(
        &self,
        workspace_cli_config_id: &str,
    ) -> IcodeResult<WorkspacePreviewResult> {
        let config = self.get_cli_config(workspace_cli_config_id)?;
        let cli_profile = self
            .cli_management_handle
            .service()
            .get_profile(&config.cli_profile_id)?;
        let now = now_iso();
        let content = self.generate_single_cli_config_content(&config, &now)?;
        Ok(WorkspacePreviewResult {
            workspace_cli_config_id: config.id,
            cli_profile_id: cli_profile.id,
            cli_type: cli_profile.cli_type,
            content,
        })
    }

    /// 应用单个 CLI 配置头
    ///
    /// 只将指定 CLI 配置头的 prompts / mcp_servers / skills 写入对应 CLI 配置文件，
    /// 与 `workspace_apply`（应用整个工作区）形成互补。
    pub fn apply_cli_config(&self, workspace_cli_config_id: &str) -> IcodeResult<ApplyCliResult> {
        let config = self.get_cli_config(workspace_cli_config_id)?;
        let cli_profile_id = config.cli_profile_id.clone();
        let now = now_iso();
        match self.apply_single_cli_config(&config, &now) {
            Ok(_) => {
                self.repo.mark_cli_config_applied(&config.id, &now)?;
                Ok(ApplyCliResult {
                    cli_profile_id,
                    success: true,
                    error: None,
                })
            }
            Err(e) => Ok(ApplyCliResult {
                cli_profile_id,
                success: false,
                error: Some(e.message),
            }),
        }
    }

    /// 为单个 CLI 配置头生成并写入配置文件
    fn apply_single_cli_config(
        &self,
        config: &WorkspaceCliConfig,
        now: &str,
    ) -> IcodeResult<()> {
        let cli_profile = self
            .cli_management_handle
            .service()
            .get_profile(&config.cli_profile_id)?;

        let config_path = cli_profile
            .config_file_path
            .ok_or_else(|| IcodeError::validation("CLI 档案未设置 config_file_path"))?;

        let content = self.generate_single_cli_config_content(config, now)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    /// 生成单个 CLI 配置头的配置文件内容（统一 JSON 格式）
    fn generate_single_cli_config_content(
        &self,
        config: &WorkspaceCliConfig,
        now: &str,
    ) -> IcodeResult<String> {
        let prompts = self.repo.list_prompts(&config.id)?;
        let mcp_servers = self.repo.list_mcp_servers(&config.id)?;
        let skills = self.repo.list_skills(&config.id)?;

        // v0.1 统一 JSON 格式；后续可按 cli_type 生成不同格式
        let output = json!({
            "generated_at": now,
            "workspace_id": config.workspace_id,
            "cli_profile_id": config.cli_profile_id,
            "prompts": prompts.iter().map(|p| {
                json!({
                    "name": p.name,
                    "content": p.content,
                    "sort_order": p.sort_order,
                })
            }).collect::<Vec<_>>(),
            "mcp_servers": mcp_servers.iter().filter(|s| s.is_enabled).map(|s| {
                json!({
                    "name": s.name,
                    "transport": s.transport,
                    "config": serde_json::from_str::<serde_json::Value>(&s.config_json).unwrap_or(serde_json::Value::Null),
                })
            }).collect::<Vec<_>>(),
            "skills": skills.iter().filter(|s| s.is_enabled).map(|s| {
                json!({
                    "name": s.name,
                    "source_path": s.source_path,
                    "content": s.content,
                })
            }).collect::<Vec<_>>(),
        });

        Ok(serde_json::to_string_pretty(&output)?)
    }

    fn get_cli_config(&self, id: &str) -> IcodeResult<WorkspaceCliConfig> {
        self.repo
            .get_cli_config(id)?
            .ok_or_else(|| IcodeError::not_found("工作区 CLI 配置头", Some(id)))
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

fn validate_transport(transport: &str) -> IcodeResult<()> {
    match transport {
        "stdio" | "sse" | "http" => Ok(()),
        _ => Err(IcodeError::validation(format!(
            "未知的 MCP 传输方式: {}，仅支持 stdio / sse / http",
            transport
        ))),
    }
}
