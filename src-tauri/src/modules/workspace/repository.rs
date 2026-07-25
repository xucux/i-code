//! # 工作区模块仓储层
//!
//! 直接操作 `workspaces`、`workspace_cli_configs`、`workspace_prompts`、
//! `workspace_mcp_servers`、`workspace_skills` 五张表，
//! 负责数据库行与 DTO 之间的映射。

use rusqlite::{params, OptionalExtension};

use crate::core::id::generate_id;
use crate::db::{get_db_pool, DbConn};
use crate::error::IcodeResult;

use super::types::{
    CreateWorkspaceInput, CreateWorkspaceMcpServerInput,
    CreateWorkspacePromptInput, CreateWorkspaceSkillInput, UpdateWorkspaceInput,
    UpdateWorkspaceMcpServerInput, UpdateWorkspacePromptInput, UpdateWorkspaceSkillInput,
    Workspace, WorkspaceCliConfig, WorkspaceMcpServer, WorkspacePrompt, WorkspaceSkill,
};

/// 工作区仓储
pub struct WorkspaceRepository;

impl WorkspaceRepository {
    pub fn new() -> Self {
        Self
    }

    // ===== workspaces =====

    /// 列出所有工作区
    pub fn list_workspaces(&self) -> IcodeResult<Vec<Workspace>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, slug, display_name, root_path, is_active, last_applied_at, created_at, updated_at
             FROM workspaces
             ORDER BY is_active DESC, display_name ASC",
        )?;
        let rows = stmt.query_map([], map_workspace_row)?;
        collect_rows(rows)
    }

    /// 根据 ID 获取工作区
    pub fn get_workspace(&self, id: &str) -> IcodeResult<Option<Workspace>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, slug, display_name, root_path, is_active, last_applied_at, created_at, updated_at
             FROM workspaces WHERE id = ?1",
        )?;
        stmt.query_row([id], map_workspace_row).optional().map_err(Into::into)
    }

    /// 根据 slug 获取工作区（用于唯一性校验）
    pub fn get_workspace_by_slug(&self, slug: &str) -> IcodeResult<Option<Workspace>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, slug, display_name, root_path, is_active, last_applied_at, created_at, updated_at
             FROM workspaces WHERE slug = ?1",
        )?;
        stmt.query_row([slug], map_workspace_row).optional().map_err(Into::into)
    }

    /// 获取当前激活的工作区
    pub fn get_active_workspace(&self) -> IcodeResult<Option<Workspace>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, slug, display_name, root_path, is_active, last_applied_at, created_at, updated_at
             FROM workspaces WHERE is_active = 1 LIMIT 1",
        )?;
        stmt.query_row([], map_workspace_row).optional().map_err(Into::into)
    }

    /// 创建工作区
    pub fn create_workspace(
        &self,
        id: &str,
        input: &CreateWorkspaceInput,
        now: &str,
    ) -> IcodeResult<Workspace> {
        let conn = get_conn()?;
        let is_active = if input.is_active { 1 } else { 0 };
        conn.execute(
            "INSERT INTO workspaces (id, slug, display_name, root_path, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, input.slug, input.display_name, input.root_path, is_active, now, now],
        )?;
        Ok(Workspace {
            id: id.to_string(),
            slug: input.slug.clone(),
            display_name: input.display_name.clone(),
            root_path: input.root_path.clone(),
            is_active: input.is_active,
            last_applied_at: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
    }

    /// 更新工作区
    pub fn update_workspace(
        &self,
        id: &str,
        input: &UpdateWorkspaceInput,
        now: &str,
    ) -> IcodeResult<Workspace> {
        let conn = get_conn()?;
        let mut sets = Vec::new();
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(slug) = &input.slug {
            sets.push("slug = ?".to_string());
            args.push(Box::new(slug.clone()));
        }
        if let Some(display_name) = &input.display_name {
            sets.push("display_name = ?".to_string());
            args.push(Box::new(display_name.clone()));
        }
        if let Some(root_path) = &input.root_path {
            sets.push("root_path = ?".to_string());
            args.push(Box::new(root_path.clone()));
        }
        sets.push("updated_at = ?".to_string());
        args.push(Box::new(now.to_string()));

        if sets.is_empty() {
            return self.get_workspace(id)?.ok_or_else(|| crate::error::IcodeError::not_found("工作区", Some(id)));
        }

        let sql = format!("UPDATE workspaces SET {} WHERE id = ?", sets.join(", "));
        args.push(Box::new(id.to_string()));
        let arg_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|a| a.as_ref()).collect();
        conn.execute(&sql, arg_refs.as_slice())?;
        self.get_workspace(id)?.ok_or_else(|| crate::error::IcodeError::not_found("工作区", Some(id)))
    }

    /// 删除工作区（级联删除 workspace_cli_configs 及子配置）
    pub fn delete_workspace(&self, id: &str) -> IcodeResult<()> {
        let conn = get_conn()?;
        conn.execute("DELETE FROM workspaces WHERE id = ?1", [id])?;
        Ok(())
    }

    /// 切换激活工作区（原子性：先全部置 0，再置目标为 1）
    pub fn switch_workspace(&self, id: &str, now: &str) -> IcodeResult<Workspace> {
        let conn = get_conn()?;
        let tx = conn.unchecked_transaction()?;
        tx.execute("UPDATE workspaces SET is_active = 0, updated_at = ?1", [now])?;
        tx.execute(
            "UPDATE workspaces SET is_active = 1, updated_at = ?1 WHERE id = ?2",
            [now, id],
        )?;
        tx.commit()?;
        self.get_workspace(id)?.ok_or_else(|| crate::error::IcodeError::not_found("工作区", Some(id)))
    }

    /// 更新工作区 `last_applied_at`
    pub fn touch_workspace_applied(&self, id: &str, now: &str) -> IcodeResult<()> {
        let conn = get_conn()?;
        conn.execute(
            "UPDATE workspaces SET last_applied_at = ?1, updated_at = ?1 WHERE id = ?2",
            [now, id],
        )?;
        Ok(())
    }

    // ===== workspace_cli_configs =====

    /// 列出某工作区下的所有 CLI 配置头
    pub fn list_cli_configs(&self, workspace_id: &str) -> IcodeResult<Vec<WorkspaceCliConfig>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, cli_profile_id, is_applied, pending_apply, created_at, updated_at
             FROM workspace_cli_configs
             WHERE workspace_id = ?1",
        )?;
        let rows = stmt.query_map([workspace_id], map_cli_config_row)?;
        collect_rows(rows)
    }

    /// 获取 CLI 配置头
    pub fn get_cli_config(&self, id: &str) -> IcodeResult<Option<WorkspaceCliConfig>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, cli_profile_id, is_applied, pending_apply, created_at, updated_at
             FROM workspace_cli_configs WHERE id = ?1",
        )?;
        stmt.query_row([id], map_cli_config_row).optional().map_err(Into::into)
    }

    /// 创建 CLI 配置头
    #[allow(dead_code)]
    pub fn create_cli_config(
        &self,
        id: &str,
        workspace_id: &str,
        cli_profile_id: &str,
        now: &str,
    ) -> IcodeResult<WorkspaceCliConfig> {
        let conn = get_conn()?;
        conn.execute(
            "INSERT INTO workspace_cli_configs (id, workspace_id, cli_profile_id, is_applied, pending_apply, created_at, updated_at)
             VALUES (?1, ?2, ?3, 0, 1, ?4, ?4)",
            params![id, workspace_id, cli_profile_id, now],
        )?;
        Ok(WorkspaceCliConfig {
            id: id.to_string(),
            workspace_id: workspace_id.to_string(),
            cli_profile_id: cli_profile_id.to_string(),
            is_applied: false,
            pending_apply: true,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
    }

    /// 批量创建工作区 CLI 配置头
    pub fn create_cli_configs_batch(
        &self,
        workspace_id: &str,
        cli_profile_ids: &[String],
        now: &str,
    ) -> IcodeResult<Vec<WorkspaceCliConfig>> {
        let conn = get_conn()?;
        let tx = conn.unchecked_transaction()?;
        let mut result = Vec::with_capacity(cli_profile_ids.len());
        for cli_profile_id in cli_profile_ids {
            let id = generate_id();
            tx.execute(
                "INSERT INTO workspace_cli_configs (id, workspace_id, cli_profile_id, is_applied, pending_apply, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 0, 1, ?4, ?4)",
                params![id, workspace_id, cli_profile_id, now],
            )?;
            result.push(WorkspaceCliConfig {
                id: id.clone(),
                workspace_id: workspace_id.to_string(),
                cli_profile_id: cli_profile_id.clone(),
                is_applied: false,
                pending_apply: true,
                created_at: now.to_string(),
                updated_at: now.to_string(),
            });
        }
        tx.commit()?;
        Ok(result)
    }

    /// 将 CLI 配置头标记为已应用
    pub fn mark_cli_config_applied(&self, id: &str, now: &str) -> IcodeResult<()> {
        let conn = get_conn()?;
        conn.execute(
            "UPDATE workspace_cli_configs SET is_applied = 1, pending_apply = 0, updated_at = ?1 WHERE id = ?2",
            [now, id],
        )?;
        Ok(())
    }

    /// 设置 CLI 配置头为待应用状态
    pub fn set_cli_config_pending(&self, id: &str, now: &str) -> IcodeResult<()> {
        let conn = get_conn()?;
        conn.execute(
            "UPDATE workspace_cli_configs SET pending_apply = 1, updated_at = ?1 WHERE id = ?2",
            [now, id],
        )?;
        Ok(())
    }

    /// 为单个 CLI 档案在所有工作区下创建配置头
    ///
    /// 用于 cli_management 创建 CLI 档案后，由 workspace Service 回调创建。
    #[allow(dead_code)]
    pub fn create_cli_config_for_all_workspaces(
        &self,
        cli_profile_id: &str,
        workspace_ids: &[String],
        now: &str,
    ) -> IcodeResult<()> {
        let conn = get_conn()?;
        let tx = conn.unchecked_transaction()?;
        for workspace_id in workspace_ids {
            let id = generate_id();
            tx.execute(
                "INSERT INTO workspace_cli_configs (id, workspace_id, cli_profile_id, is_applied, pending_apply, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 0, 1, ?4, ?4)",
                params![id, workspace_id, cli_profile_id, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// 列出所有工作区 ID
    #[allow(dead_code)]
    pub fn list_workspace_ids(&self) -> IcodeResult<Vec<String>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare("SELECT id FROM workspaces")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        collect_rows(rows)
    }

    // ===== workspace_prompts =====

    pub fn list_prompts(&self, workspace_cli_config_id: &str) -> IcodeResult<Vec<WorkspacePrompt>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_cli_config_id, name, content, sort_order, created_at, updated_at
             FROM workspace_prompts
             WHERE workspace_cli_config_id = ?1
             ORDER BY sort_order ASC, created_at ASC",
        )?;
        let rows = stmt.query_map([workspace_cli_config_id], map_prompt_row)?;
        collect_rows(rows)
    }

    pub fn get_prompt(&self, id: &str) -> IcodeResult<Option<WorkspacePrompt>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_cli_config_id, name, content, sort_order, created_at, updated_at
             FROM workspace_prompts WHERE id = ?1",
        )?;
        stmt.query_row([id], map_prompt_row).optional().map_err(Into::into)
    }

    pub fn create_prompt(
        &self,
        id: &str,
        input: &CreateWorkspacePromptInput,
        now: &str,
    ) -> IcodeResult<WorkspacePrompt> {
        let conn = get_conn()?;
        conn.execute(
            "INSERT INTO workspace_prompts (id, workspace_cli_config_id, name, content, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![id, input.workspace_cli_config_id, input.name, input.content, input.sort_order, now],
        )?;
        Ok(WorkspacePrompt {
            id: id.to_string(),
            workspace_cli_config_id: input.workspace_cli_config_id.clone(),
            name: input.name.clone(),
            content: input.content.clone(),
            sort_order: input.sort_order,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
    }

    pub fn update_prompt(
        &self,
        id: &str,
        input: &UpdateWorkspacePromptInput,
        now: &str,
    ) -> IcodeResult<WorkspacePrompt> {
        let conn = get_conn()?;
        let mut sets = Vec::new();
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(name) = &input.name {
            sets.push("name = ?".to_string());
            args.push(Box::new(name.clone()));
        }
        if let Some(content) = &input.content {
            sets.push("content = ?".to_string());
            args.push(Box::new(content.clone()));
        }
        if let Some(sort_order) = input.sort_order {
            sets.push("sort_order = ?".to_string());
            args.push(Box::new(sort_order));
        }
        sets.push("updated_at = ?".to_string());
        args.push(Box::new(now.to_string()));

        if sets.is_empty() {
            return self.get_prompt(id)?.ok_or_else(|| crate::error::IcodeError::not_found("Prompt", Some(id)));
        }

        let sql = format!("UPDATE workspace_prompts SET {} WHERE id = ?", sets.join(", "));
        args.push(Box::new(id.to_string()));
        let arg_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|a| a.as_ref()).collect();
        conn.execute(&sql, arg_refs.as_slice())?;
        self.get_prompt(id)?.ok_or_else(|| crate::error::IcodeError::not_found("Prompt", Some(id)))
    }

    pub fn delete_prompt(&self, id: &str) -> IcodeResult<()> {
        let conn = get_conn()?;
        conn.execute("DELETE FROM workspace_prompts WHERE id = ?1", [id])?;
        Ok(())
    }

    // ===== workspace_mcp_servers =====

    pub fn list_mcp_servers(
        &self,
        workspace_cli_config_id: &str,
    ) -> IcodeResult<Vec<WorkspaceMcpServer>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_cli_config_id, name, transport, config_json, is_enabled, created_at, updated_at
             FROM workspace_mcp_servers
             WHERE workspace_cli_config_id = ?1
             ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([workspace_cli_config_id], map_mcp_server_row)?;
        collect_rows(rows)
    }

    pub fn get_mcp_server(&self, id: &str) -> IcodeResult<Option<WorkspaceMcpServer>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_cli_config_id, name, transport, config_json, is_enabled, created_at, updated_at
             FROM workspace_mcp_servers WHERE id = ?1",
        )?;
        stmt.query_row([id], map_mcp_server_row).optional().map_err(Into::into)
    }

    pub fn create_mcp_server(
        &self,
        id: &str,
        input: &CreateWorkspaceMcpServerInput,
        now: &str,
    ) -> IcodeResult<WorkspaceMcpServer> {
        let conn = get_conn()?;
        conn.execute(
            "INSERT INTO workspace_mcp_servers (id, workspace_cli_config_id, name, transport, config_json, is_enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![
                id,
                input.workspace_cli_config_id,
                input.name,
                input.transport,
                input.config_json,
                input.is_enabled as i32,
                now,
            ],
        )?;
        Ok(WorkspaceMcpServer {
            id: id.to_string(),
            workspace_cli_config_id: input.workspace_cli_config_id.clone(),
            name: input.name.clone(),
            transport: input.transport.clone(),
            config_json: input.config_json.clone(),
            is_enabled: input.is_enabled,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
    }

    pub fn update_mcp_server(
        &self,
        id: &str,
        input: &UpdateWorkspaceMcpServerInput,
        now: &str,
    ) -> IcodeResult<WorkspaceMcpServer> {
        let conn = get_conn()?;
        let mut sets = Vec::new();
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(name) = &input.name {
            sets.push("name = ?".to_string());
            args.push(Box::new(name.clone()));
        }
        if let Some(transport) = &input.transport {
            sets.push("transport = ?".to_string());
            args.push(Box::new(transport.clone()));
        }
        if let Some(config_json) = &input.config_json {
            sets.push("config_json = ?".to_string());
            args.push(Box::new(config_json.clone()));
        }
        if let Some(is_enabled) = input.is_enabled {
            sets.push("is_enabled = ?".to_string());
            args.push(Box::new(is_enabled as i32));
        }
        sets.push("updated_at = ?".to_string());
        args.push(Box::new(now.to_string()));

        if sets.is_empty() {
            return self.get_mcp_server(id)?.ok_or_else(|| crate::error::IcodeError::not_found("MCP Server", Some(id)));
        }

        let sql = format!("UPDATE workspace_mcp_servers SET {} WHERE id = ?", sets.join(", "));
        args.push(Box::new(id.to_string()));
        let arg_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|a| a.as_ref()).collect();
        conn.execute(&sql, arg_refs.as_slice())?;
        self.get_mcp_server(id)?.ok_or_else(|| crate::error::IcodeError::not_found("MCP Server", Some(id)))
    }

    pub fn delete_mcp_server(&self, id: &str) -> IcodeResult<()> {
        let conn = get_conn()?;
        conn.execute("DELETE FROM workspace_mcp_servers WHERE id = ?1", [id])?;
        Ok(())
    }

    // ===== workspace_skills =====

    pub fn list_skills(&self, workspace_cli_config_id: &str) -> IcodeResult<Vec<WorkspaceSkill>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_cli_config_id, name, source_path, content, is_enabled, created_at, updated_at
             FROM workspace_skills
             WHERE workspace_cli_config_id = ?1
             ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([workspace_cli_config_id], map_skill_row)?;
        collect_rows(rows)
    }

    pub fn get_skill(&self, id: &str) -> IcodeResult<Option<WorkspaceSkill>> {
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_cli_config_id, name, source_path, content, is_enabled, created_at, updated_at
             FROM workspace_skills WHERE id = ?1",
        )?;
        stmt.query_row([id], map_skill_row).optional().map_err(Into::into)
    }

    pub fn create_skill(
        &self,
        id: &str,
        input: &CreateWorkspaceSkillInput,
        now: &str,
    ) -> IcodeResult<WorkspaceSkill> {
        let conn = get_conn()?;
        conn.execute(
            "INSERT INTO workspace_skills (id, workspace_cli_config_id, name, source_path, content, is_enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![
                id,
                input.workspace_cli_config_id,
                input.name,
                input.source_path,
                input.content,
                input.is_enabled as i32,
                now,
            ],
        )?;
        Ok(WorkspaceSkill {
            id: id.to_string(),
            workspace_cli_config_id: input.workspace_cli_config_id.clone(),
            name: input.name.clone(),
            source_path: input.source_path.clone(),
            content: input.content.clone(),
            is_enabled: input.is_enabled,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
    }

    pub fn update_skill(
        &self,
        id: &str,
        input: &UpdateWorkspaceSkillInput,
        now: &str,
    ) -> IcodeResult<WorkspaceSkill> {
        let conn = get_conn()?;
        let mut sets = Vec::new();
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(name) = &input.name {
            sets.push("name = ?".to_string());
            args.push(Box::new(name.clone()));
        }
        if let Some(source_path) = &input.source_path {
            sets.push("source_path = ?".to_string());
            args.push(Box::new(source_path.clone()));
        }
        if let Some(content) = &input.content {
            sets.push("content = ?".to_string());
            args.push(Box::new(content.clone()));
        }
        if let Some(is_enabled) = input.is_enabled {
            sets.push("is_enabled = ?".to_string());
            args.push(Box::new(is_enabled as i32));
        }
        sets.push("updated_at = ?".to_string());
        args.push(Box::new(now.to_string()));

        if sets.is_empty() {
            return self.get_skill(id)?.ok_or_else(|| crate::error::IcodeError::not_found("Skill", Some(id)));
        }

        let sql = format!("UPDATE workspace_skills SET {} WHERE id = ?", sets.join(", "));
        args.push(Box::new(id.to_string()));
        let arg_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|a| a.as_ref()).collect();
        conn.execute(&sql, arg_refs.as_slice())?;
        self.get_skill(id)?.ok_or_else(|| crate::error::IcodeError::not_found("Skill", Some(id)))
    }

    pub fn delete_skill(&self, id: &str) -> IcodeResult<()> {
        let conn = get_conn()?;
        conn.execute("DELETE FROM workspace_skills WHERE id = ?1", [id])?;
        Ok(())
    }
}

impl Default for WorkspaceRepository {
    fn default() -> Self {
        Self::new()
    }
}

// ===== 私有辅助函数 =====

fn get_conn() -> IcodeResult<DbConn> {
    Ok(get_db_pool()?.get()?)
}

fn map_workspace_row(row: &rusqlite::Row) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: row.get("id")?,
        slug: row.get("slug")?,
        display_name: row.get("display_name")?,
        root_path: row.get("root_path")?,
        is_active: row.get::<_, i32>("is_active")? != 0,
        last_applied_at: row.get("last_applied_at")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_cli_config_row(row: &rusqlite::Row) -> rusqlite::Result<WorkspaceCliConfig> {
    Ok(WorkspaceCliConfig {
        id: row.get("id")?,
        workspace_id: row.get("workspace_id")?,
        cli_profile_id: row.get("cli_profile_id")?,
        is_applied: row.get::<_, i32>("is_applied")? != 0,
        pending_apply: row.get::<_, i32>("pending_apply")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_prompt_row(row: &rusqlite::Row) -> rusqlite::Result<WorkspacePrompt> {
    Ok(WorkspacePrompt {
        id: row.get("id")?,
        workspace_cli_config_id: row.get("workspace_cli_config_id")?,
        name: row.get("name")?,
        content: row.get("content")?,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_mcp_server_row(row: &rusqlite::Row) -> rusqlite::Result<WorkspaceMcpServer> {
    Ok(WorkspaceMcpServer {
        id: row.get("id")?,
        workspace_cli_config_id: row.get("workspace_cli_config_id")?,
        name: row.get("name")?,
        transport: row.get("transport")?,
        config_json: row.get("config_json")?,
        is_enabled: row.get::<_, i32>("is_enabled")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_skill_row(row: &rusqlite::Row) -> rusqlite::Result<WorkspaceSkill> {
    Ok(WorkspaceSkill {
        id: row.get("id")?,
        workspace_cli_config_id: row.get("workspace_cli_config_id")?,
        name: row.get("name")?,
        source_path: row.get("source_path")?,
        content: row.get("content")?,
        is_enabled: row.get::<_, i32>("is_enabled")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn collect_rows<T>(rows: impl Iterator<Item = rusqlite::Result<T>>) -> IcodeResult<Vec<T>> {
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}
