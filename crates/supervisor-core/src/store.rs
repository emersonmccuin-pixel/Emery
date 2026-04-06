use std::path::Path;

use anyhow::{Result, anyhow};
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter, types::Type};
use serde::Serialize;
use serde_json::Value;

use crate::bootstrap::AppPaths;
use crate::git;
use crate::models::{
    AccountDetail, AccountSummary, AccountUpdateRecord, AgentTemplateDetail, AgentTemplateListFilter,
    AgentTemplateSummary, AgentTemplateUpdateRecord, DocumentDetail, DocumentListFilter,
    DocumentSummary, DocumentUpdateRecord, InboxEntryDetail, InboxEntryListFilter,
    InboxEntrySummary, InboxEntryUpdateRecord, MergeQueueEntry, MergeQueueListFilter,
    NewAccountRecord, NewAgentTemplateRecord, NewDocumentRecord, NewInboxEntryRecord,
    NewMergeQueueRecord, NewPlanningAssignmentRecord, NewProjectRecord, NewProjectRootRecord,
    NewSessionRecord, NewSessionSpecRecord, NewWorkItemRecord,
    NewWorkflowReconciliationProposalRecord, NewWorktreeRecord, PlanningAssignmentDetail,
    PlanningAssignmentListFilter, PlanningAssignmentSummary, PlanningAssignmentUpdateRecord,
    ProjectDetail, ProjectRootSummary, ProjectRootUpdateRecord, ProjectSummary, ProjectUpdateRecord,
    SessionArtifactRecord, SessionListFilter, SessionSpecDetail, SessionSpecListFilter,
    SessionSpecSummary, SessionSpecUpdateRecord, SessionSummary, UpdateWorkspaceStateRequest,
    WorkItemDetail, WorkItemListFilter, WorkItemSummary, WorkItemUpdateRecord,
    WorkflowReconciliationProposalDetail, WorkflowReconciliationProposalListFilter,
    WorkflowReconciliationProposalSummary, WorkflowReconciliationProposalUpdateRecord,
    WorkspaceStateRecord, WorktreeDetail, WorktreeListFilter, WorktreeSummary, WorktreeUpdateRecord,
    VaultEntry, VaultEntryRow, VaultAuditEntry, NewVaultEntryRecord, VaultEntryUpdateRecord,
    NewVaultAuditRecord,
};
use crate::schema::{migrate_app_db, migrate_knowledge_db};

#[derive(Debug, Clone)]
pub struct DatabaseSet {
    paths: AppPaths,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatabaseHealth {
    pub available: bool,
    pub schema_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthSnapshot {
    pub app_db: DatabaseHealth,
    pub knowledge_db: DatabaseHealth,
    pub artifact_root_available: bool,
    pub live_session_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BootstrapState {
    pub project_count: i64,
    pub account_count: i64,
    pub live_session_count: i64,
    pub restorable_workspace_count: i64,
    pub interrupted_session_count: i64,
}

impl DatabaseSet {
    pub fn initialize(paths: &AppPaths) -> Result<Self> {
        migrate_app_db(&paths.app_db)?;
        migrate_knowledge_db(&paths.knowledge_db)?;
        Ok(Self {
            paths: paths.clone(),
        })
    }

    pub fn health_snapshot(&self) -> Result<HealthSnapshot> {
        let app_db = database_health(&self.paths.app_db)?;
        let knowledge_db = database_health(&self.paths.knowledge_db)?;
        let app_connection = open_connection(&self.paths.app_db)?;

        Ok(HealthSnapshot {
            live_session_count: count_sessions(&app_connection, "running")?,
            artifact_root_available: self.paths.sessions_dir.is_dir(),
            app_db,
            knowledge_db,
        })
    }

    pub fn bootstrap_state(&self) -> Result<BootstrapState> {
        let connection = open_connection(&self.paths.app_db)?;

        Ok(BootstrapState {
            project_count: count_rows(&connection, "projects")?,
            account_count: count_rows(&connection, "accounts")?,
            live_session_count: count_live_runtime_sessions(&connection)?,
            restorable_workspace_count: count_rows(&connection, "workspace_state")?,
            interrupted_session_count: count_interrupted_sessions(&connection)?,
        })
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn next_project_sort_order(&self) -> Result<i64> {
        let connection = open_connection(&self.paths.app_db)?;
        next_sort_order(
            &connection,
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM projects",
        )
    }

    pub fn project_slug_exists(
        &self,
        slug: &str,
        exclude_project_id: Option<&str>,
    ) -> Result<bool> {
        let connection = open_connection(&self.paths.app_db)?;
        match exclude_project_id {
            Some(project_id) => exists(
                &connection,
                "SELECT 1 FROM projects WHERE slug = ?1 AND id <> ?2",
                params![slug, project_id],
            ),
            None => exists(
                &connection,
                "SELECT 1 FROM projects WHERE slug = ?1",
                params![slug],
            ),
        }
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>> {
        let connection = open_connection(&self.paths.app_db)?;
        let mut statement = connection.prepare(
            "SELECT
                p.id,
                p.name,
                p.slug,
                p.sort_order,
                p.default_account_id,
                p.project_type,
                p.model_defaults_json,
                p.wcp_namespace,
                p.created_at,
                p.updated_at,
                p.archived_at,
                COUNT(DISTINCT pr.id) AS root_count,
                COUNT(DISTINCT s.id) AS live_session_count
             FROM projects p
             LEFT JOIN project_roots pr
               ON pr.project_id = p.id AND pr.archived_at IS NULL
             LEFT JOIN sessions s
               ON s.project_id = p.id
              AND s.status = 'active'
              AND s.runtime_state IN ('starting', 'running', 'stopping')
             GROUP BY
                p.id, p.name, p.slug, p.sort_order, p.default_account_id, p.project_type, p.model_defaults_json, p.wcp_namespace, p.created_at, p.updated_at, p.archived_at
             ORDER BY p.sort_order ASC, p.created_at ASC",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(ProjectSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                slug: row.get(2)?,
                sort_order: row.get(3)?,
                default_account_id: row.get(4)?,
                project_type: row.get(5)?,
                model_defaults_json: row.get(6)?,
                wcp_namespace: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
                archived_at: row.get(10)?,
                root_count: row.get(11)?,
                live_session_count: row.get(12)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_project(&self, project_id: &str) -> Result<Option<ProjectDetail>> {
        let connection = open_connection(&self.paths.app_db)?;
        let project = connection
            .query_row(
                "SELECT id, name, slug, sort_order, default_account_id, settings_json, created_at, updated_at, archived_at, agent_safety_overrides_json, instructions_md, project_type, model_defaults_json, wcp_namespace, dispatch_item_callsign
                 FROM projects
                 WHERE id = ?1",
                [project_id],
                |row| {
                    Ok(ProjectDetail {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        slug: row.get(2)?,
                        sort_order: row.get(3)?,
                        default_account_id: row.get(4)?,
                        settings_json: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                        archived_at: row.get(8)?,
                        agent_safety_overrides_json: row.get(9)?,
                        instructions_md: row.get(10)?,
                        project_type: row.get(11)?,
                        model_defaults_json: row.get(12)?,
                        wcp_namespace: row.get(13)?,
                        dispatch_item_callsign: row.get(14)?,
                        roots: Vec::new(),
                    })
                },
            )
            .optional()?;

        let Some(mut project) = project else {
            return Ok(None);
        };

        project.roots = self.list_project_roots(project_id)?;
        Ok(Some(project))
    }

    pub fn insert_project(&self, record: &NewProjectRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "INSERT INTO projects (
                id,
                name,
                slug,
                sort_order,
                default_account_id,
                project_type,
                model_defaults_json,
                wcp_namespace,
                settings_json,
                instructions_md,
                created_at,
                updated_at,
                archived_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL)",
            params![
                record.id,
                record.name,
                record.slug,
                record.sort_order,
                record.default_account_id,
                record.project_type,
                record.model_defaults_json,
                record.wcp_namespace,
                record.settings_json,
                record.instructions_md,
                record.created_at,
                record.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_project(&self, record: &ProjectUpdateRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        let updated = connection.execute(
            "UPDATE projects
             SET name = ?2,
                 slug = ?3,
                 sort_order = ?4,
                 default_account_id = ?5,
                 project_type = ?6,
                 model_defaults_json = ?7,
                 settings_json = ?8,
                 instructions_md = ?9,
                 wcp_namespace = ?10,
                 dispatch_item_callsign = ?11,
                 updated_at = ?12,
                 agent_safety_overrides_json = ?13
             WHERE id = ?1",
            params![
                record.id,
                record.name,
                record.slug,
                record.sort_order,
                record.default_account_id,
                record.project_type,
                record.model_defaults_json,
                record.settings_json,
                record.instructions_md,
                record.wcp_namespace,
                record.dispatch_item_callsign,
                record.updated_at,
                record.agent_safety_overrides_json,
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!("project {} was not found", record.id));
        }

        Ok(())
    }

    pub fn list_project_roots(&self, project_id: &str) -> Result<Vec<ProjectRootSummary>> {
        let connection = open_connection(&self.paths.app_db)?;
        let mut statement = connection.prepare(
            "SELECT
                id,
                project_id,
                label,
                path,
                git_root_path,
                remote_url,
                root_kind,
                sort_order,
                created_at,
                updated_at,
                archived_at
             FROM project_roots
             WHERE project_id = ?1
             ORDER BY sort_order ASC, created_at ASC",
        )?;

        let rows = statement.query_map([project_id], map_project_root_summary)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_project_root(&self, project_root_id: &str) -> Result<Option<ProjectRootSummary>> {
        let connection = open_connection(&self.paths.app_db)?;
        connection
            .query_row(
                "SELECT
                    id,
                    project_id,
                    label,
                    path,
                    git_root_path,
                    remote_url,
                    root_kind,
                    sort_order,
                    created_at,
                    updated_at,
                    archived_at
                 FROM project_roots
                 WHERE id = ?1",
                [project_root_id],
                map_project_root_summary,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn next_project_root_sort_order(&self, project_id: &str) -> Result<i64> {
        let connection = open_connection(&self.paths.app_db)?;
        Ok(connection.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1
             FROM project_roots
             WHERE project_id = ?1",
            [project_id],
            |row| row.get(0),
        )?)
    }

    pub fn project_root_path_exists(
        &self,
        project_id: &str,
        path: &str,
        exclude_project_root_id: Option<&str>,
    ) -> Result<bool> {
        let connection = open_connection(&self.paths.app_db)?;
        match exclude_project_root_id {
            Some(project_root_id) => exists(
                &connection,
                "SELECT 1 FROM project_roots WHERE project_id = ?1 AND path = ?2 AND id <> ?3",
                params![project_id, path, project_root_id],
            ),
            None => exists(
                &connection,
                "SELECT 1 FROM project_roots WHERE project_id = ?1 AND path = ?2",
                params![project_id, path],
            ),
        }
    }

    pub fn insert_project_root(&self, record: &NewProjectRootRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "INSERT INTO project_roots (
                id,
                project_id,
                label,
                path,
                git_root_path,
                remote_url,
                root_kind,
                sort_order,
                created_at,
                updated_at,
                archived_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL)",
            params![
                record.id,
                record.project_id,
                record.label,
                record.path,
                record.git_root_path,
                record.remote_url,
                record.root_kind,
                record.sort_order,
                record.created_at,
                record.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_project_root(&self, record: &ProjectRootUpdateRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        let updated = connection.execute(
            "UPDATE project_roots
             SET label = ?2,
                 path = ?3,
                 git_root_path = ?4,
                 remote_url = ?5,
                 root_kind = ?6,
                 sort_order = ?7,
                 updated_at = ?8
             WHERE id = ?1",
            params![
                record.id,
                record.label,
                record.path,
                record.git_root_path,
                record.remote_url,
                record.root_kind,
                record.sort_order,
                record.updated_at,
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!("project root {} was not found", record.id));
        }

        Ok(())
    }

    pub fn archive_project(&self, project_id: &str, now: i64) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        let updated = connection.execute(
            "UPDATE projects
             SET archived_at = COALESCE(archived_at, ?2),
                 updated_at = ?2
             WHERE id = ?1",
            params![project_id, now],
        )?;

        if updated == 0 {
            return Err(anyhow!("project {} was not found", project_id));
        }

        Ok(())
    }

    pub fn archive_project_roots_for_project(&self, project_id: &str, now: i64) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "UPDATE project_roots
             SET archived_at = COALESCE(archived_at, ?2),
                 updated_at = ?2
             WHERE project_id = ?1",
            params![project_id, now],
        )?;
        Ok(())
    }

    pub fn delete_project(&self, project_id: &str) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        // Delete child records first to satisfy FK constraints, then the project itself.
        // Order matters: sessions reference worktrees; session_artifacts reference sessions.
        connection.execute(
            "DELETE FROM inbox_entries WHERE project_id = ?1",
            params![project_id],
        )?;
        connection.execute(
            "DELETE FROM merge_queue WHERE project_id = ?1",
            params![project_id],
        )?;
        connection.execute(
            "DELETE FROM planning_assignments WHERE work_item_id IN (SELECT DISTINCT work_item_id FROM sessions WHERE project_id = ?1 AND work_item_id IS NOT NULL)",
            params![project_id],
        )?;
        connection.execute(
            "DELETE FROM workflow_reconciliation_proposals WHERE source_session_id IN (SELECT id FROM sessions WHERE project_id = ?1)",
            params![project_id],
        )?;
        connection.execute(
            "DELETE FROM session_artifacts WHERE session_id IN (SELECT id FROM sessions WHERE project_id = ?1)",
            params![project_id],
        )?;
        connection.execute(
            "DELETE FROM sessions WHERE project_id = ?1",
            params![project_id],
        )?;
        connection.execute(
            "DELETE FROM session_specs WHERE project_id = ?1",
            params![project_id],
        )?;
        connection.execute(
            "DELETE FROM worktrees WHERE project_id = ?1",
            params![project_id],
        )?;
        connection.execute(
            "DELETE FROM agent_templates WHERE project_id = ?1",
            params![project_id],
        )?;
        connection.execute(
            "DELETE FROM project_roots WHERE project_id = ?1",
            params![project_id],
        )?;
        let deleted = connection.execute(
            "DELETE FROM projects WHERE id = ?1",
            params![project_id],
        )?;
        if deleted == 0 {
            return Err(anyhow!("project {} was not found", project_id));
        }
        Ok(())
    }

    pub fn project_has_live_sessions(&self, project_id: &str) -> Result<bool> {
        let connection = open_connection(&self.paths.app_db)?;
        exists(
            &connection,
            "SELECT 1 FROM sessions
             WHERE project_id = ?1
               AND status = 'active'
               AND runtime_state IN ('starting', 'running', 'stopping')",
            params![project_id],
        )
    }

    pub fn project_has_pending_merges(&self, project_id: &str) -> Result<bool> {
        let connection = open_connection(&self.paths.app_db)?;
        exists(
            &connection,
            "SELECT 1 FROM merge_queue
             WHERE project_id = ?1
               AND status IN ('queued', 'ready', 'merging', 'conflict')",
            params![project_id],
        )
    }

    pub fn archive_project_root(&self, project_root_id: &str, now: i64) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        let updated = connection.execute(
            "UPDATE project_roots
             SET archived_at = COALESCE(archived_at, ?2),
                 updated_at = ?2
             WHERE id = ?1",
            params![project_root_id, now],
        )?;

        if updated == 0 {
            return Err(anyhow!("project root {} was not found", project_root_id));
        }

        Ok(())
    }

    pub fn list_accounts(&self) -> Result<Vec<AccountSummary>> {
        let connection = open_connection(&self.paths.app_db)?;
        let mut statement = connection.prepare(
            "SELECT
                id,
                agent_kind,
                label,
                binary_path,
                config_root,
                env_preset_ref,
                is_default,
                status,
                created_at,
                updated_at,
                default_safety_mode,
                default_launch_args_json,
                default_model
             FROM accounts
             ORDER BY is_default DESC, agent_kind ASC, label COLLATE NOCASE ASC, created_at ASC",
        )?;

        let rows = statement.query_map([], map_account_summary)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_account(&self, account_id: &str) -> Result<Option<AccountDetail>> {
        let connection = open_connection(&self.paths.app_db)?;
        let summary = connection
            .query_row(
                "SELECT
                    id,
                    agent_kind,
                    label,
                    binary_path,
                    config_root,
                    env_preset_ref,
                    is_default,
                    status,
                    created_at,
                    updated_at,
                    default_safety_mode,
                    default_launch_args_json,
                    default_model
                 FROM accounts
                 WHERE id = ?1",
                [account_id],
                map_account_summary,
            )
            .optional()?;

        Ok(summary.map(|summary| AccountDetail { summary }))
    }

    pub fn insert_account(&self, record: &NewAccountRecord) -> Result<()> {
        let mut connection = open_connection(&self.paths.app_db)?;
        let tx = connection.transaction()?;

        if record.is_default {
            tx.execute(
                "UPDATE accounts SET is_default = 0 WHERE agent_kind = ?1",
                params![record.agent_kind],
            )?;
        }

        tx.execute(
            "INSERT INTO accounts (
                id,
                agent_kind,
                label,
                binary_path,
                config_root,
                env_preset_ref,
                is_default,
                status,
                default_safety_mode,
                default_launch_args_json,
                default_model,
                created_at,
                updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                record.id,
                record.agent_kind,
                record.label,
                record.binary_path,
                record.config_root,
                record.env_preset_ref,
                bool_to_sqlite(record.is_default),
                record.status,
                record.default_safety_mode,
                record.default_launch_args_json,
                record.default_model,
                record.created_at,
                record.updated_at,
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn update_account(&self, record: &AccountUpdateRecord) -> Result<()> {
        let mut connection = open_connection(&self.paths.app_db)?;
        let tx = connection.transaction()?;

        if record.is_default {
            tx.execute(
                "UPDATE accounts
                 SET is_default = 0
                 WHERE agent_kind = ?1 AND id <> ?2",
                params![record.agent_kind, record.id],
            )?;
        }

        let updated = tx.execute(
            "UPDATE accounts
             SET agent_kind = ?2,
                 label = ?3,
                 binary_path = ?4,
                 config_root = ?5,
                 env_preset_ref = ?6,
                 is_default = ?7,
                 status = ?8,
                 default_safety_mode = ?9,
                 default_launch_args_json = ?10,
                 default_model = ?11,
                 updated_at = ?12
             WHERE id = ?1",
            params![
                record.id,
                record.agent_kind,
                record.label,
                record.binary_path,
                record.config_root,
                record.env_preset_ref,
                bool_to_sqlite(record.is_default),
                record.status,
                record.default_safety_mode,
                record.default_launch_args_json,
                record.default_model,
                record.updated_at,
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!("account {} was not found", record.id));
        }

        tx.commit()?;
        Ok(())
    }

    pub fn env_preset_exists(&self, env_preset_id: &str) -> Result<bool> {
        let connection = open_connection(&self.paths.app_db)?;
        exists(
            &connection,
            "SELECT 1 FROM env_presets WHERE id = ?1",
            [env_preset_id],
        )
    }

    pub fn list_worktrees(&self, filter: &WorktreeListFilter) -> Result<Vec<WorktreeSummary>> {
        let connection = open_connection(&self.paths.app_db)?;
        let mut sql = String::from(
            "SELECT
                w.id,
                w.project_id,
                w.project_root_id,
                w.branch_name,
                w.head_commit,
                w.base_ref,
                w.path,
                w.status,
                w.created_by_session_id,
                w.last_used_at,
                w.created_at,
                w.updated_at,
                w.closed_at,
                COUNT(DISTINCT s.id) AS active_session_count
             FROM worktrees w
             LEFT JOIN sessions s
               ON s.worktree_id = w.id
              AND s.status = 'active'
              AND s.runtime_state IN ('starting', 'running', 'stopping')
             WHERE 1 = 1",
        );
        let mut values = Vec::new();

        if let Some(project_id) = filter.project_id.as_deref() {
            sql.push_str(" AND w.project_id = ?");
            values.push(project_id.to_string());
        }

        if let Some(project_root_id) = filter.project_root_id.as_deref() {
            sql.push_str(" AND w.project_root_id = ?");
            values.push(project_root_id.to_string());
        }

        if let Some(status) = filter.status.as_deref() {
            sql.push_str(" AND w.status = ?");
            values.push(status.to_string());
        }

        sql.push_str(
            " GROUP BY
                w.id,
                w.project_id,
                w.project_root_id,
                w.branch_name,
                w.head_commit,
                w.base_ref,
                w.path,
                w.status,
                w.created_by_session_id,
                w.last_used_at,
                w.created_at,
                w.updated_at,
                w.closed_at
              ORDER BY w.updated_at DESC, w.created_at DESC",
        );

        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            values.push((limit as i64).to_string());
        }

        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), map_worktree_summary)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_worktree(&self, worktree_id: &str) -> Result<Option<WorktreeDetail>> {
        let connection = open_connection(&self.paths.app_db)?;
        let mut statement = connection.prepare(
            "SELECT
                w.id,
                w.project_id,
                w.project_root_id,
                w.branch_name,
                w.head_commit,
                w.base_ref,
                w.path,
                w.status,
                w.created_by_session_id,
                w.last_used_at,
                w.created_at,
                w.updated_at,
                w.closed_at,
                COUNT(DISTINCT s.id) AS active_session_count
             FROM worktrees w
             LEFT JOIN sessions s
               ON s.worktree_id = w.id
              AND s.status = 'active'
              AND s.runtime_state IN ('starting', 'running', 'stopping')
             WHERE w.id = ?1
             GROUP BY
                w.id,
                w.project_id,
                w.project_root_id,
                w.branch_name,
                w.head_commit,
                w.base_ref,
                w.path,
                w.status,
                w.created_by_session_id,
                w.last_used_at,
                w.created_at,
                w.updated_at,
                w.closed_at",
        )?;

        statement
            .query_row([worktree_id], |row| {
                Ok(WorktreeDetail {
                    summary: map_worktree_summary(row)?,
                })
            })
            .optional()
            .map_err(Into::into)
    }

    pub fn worktree_path_exists(
        &self,
        path: &str,
        exclude_worktree_id: Option<&str>,
    ) -> Result<bool> {
        let connection = open_connection(&self.paths.app_db)?;
        match exclude_worktree_id {
            Some(worktree_id) => exists(
                &connection,
                "SELECT 1 FROM worktrees WHERE path = ?1 AND id <> ?2",
                params![path, worktree_id],
            ),
            None => exists(
                &connection,
                "SELECT 1 FROM worktrees WHERE path = ?1",
                params![path],
            ),
        }
    }

    pub fn insert_worktree(&self, record: &NewWorktreeRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "INSERT INTO worktrees (
                id,
                project_id,
                project_root_id,
                branch_name,
                head_commit,
                base_ref,
                path,
                status,
                created_by_session_id,
                last_used_at,
                created_at,
                updated_at,
                closed_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13
             )",
            params![
                record.id,
                record.project_id,
                record.project_root_id,
                record.branch_name,
                record.head_commit,
                record.base_ref,
                record.path,
                record.status,
                record.created_by_session_id,
                record.last_used_at,
                record.created_at,
                record.updated_at,
                record.closed_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_worktree(&self, record: &WorktreeUpdateRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        let updated = connection.execute(
            "UPDATE worktrees
             SET project_root_id = ?2,
                 branch_name = ?3,
                 head_commit = ?4,
                 base_ref = ?5,
                 path = ?6,
                 status = ?7,
                 created_by_session_id = ?8,
                 last_used_at = ?9,
                 updated_at = ?10,
                 closed_at = ?11
             WHERE id = ?1",
            params![
                record.id,
                record.project_root_id,
                record.branch_name,
                record.head_commit,
                record.base_ref,
                record.path,
                record.status,
                record.created_by_session_id,
                record.last_used_at,
                record.updated_at,
                record.closed_at,
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!("worktree {} was not found", record.id));
        }

        Ok(())
    }

    pub fn list_session_specs(
        &self,
        filter: &SessionSpecListFilter,
    ) -> Result<Vec<SessionSpecSummary>> {
        let connection = open_connection(&self.paths.app_db)?;
        let mut sql = String::from(
            "SELECT
                id,
                project_id,
                project_root_id,
                worktree_id,
                work_item_id,
                account_id,
                agent_kind,
                cwd,
                command,
                args_json,
                env_preset_ref,
                origin_mode,
                current_mode,
                title,
                title_policy,
                restore_policy,
                initial_terminal_cols,
                initial_terminal_rows,
                context_bundle_artifact_id,
                created_at,
                updated_at
             FROM session_specs
             WHERE 1 = 1",
        );
        let mut values = Vec::new();

        if let Some(project_id) = filter.project_id.as_deref() {
            sql.push_str(" AND project_id = ?");
            values.push(project_id.to_string());
        }

        if let Some(account_id) = filter.account_id.as_deref() {
            sql.push_str(" AND account_id = ?");
            values.push(account_id.to_string());
        }

        if let Some(worktree_id) = filter.worktree_id.as_deref() {
            sql.push_str(" AND worktree_id = ?");
            values.push(worktree_id.to_string());
        }

        sql.push_str(" ORDER BY updated_at DESC, created_at DESC");
        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            values.push((limit as i64).to_string());
        }

        let mut statement = connection.prepare(&sql)?;
        let rows =
            statement.query_map(params_from_iter(values.iter()), map_session_spec_summary)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_session_spec(&self, session_spec_id: &str) -> Result<Option<SessionSpecDetail>> {
        let connection = open_connection(&self.paths.app_db)?;
        connection
            .query_row(
                "SELECT
                    id,
                    project_id,
                    project_root_id,
                    worktree_id,
                    work_item_id,
                    account_id,
                    agent_kind,
                    cwd,
                    command,
                    args_json,
                    env_preset_ref,
                    origin_mode,
                    current_mode,
                    title,
                    title_policy,
                    restore_policy,
                    initial_terminal_cols,
                    initial_terminal_rows,
                    context_bundle_artifact_id,
                    created_at,
                    updated_at
                 FROM session_specs
                 WHERE id = ?1",
                [session_spec_id],
                |row| {
                    Ok(SessionSpecDetail {
                        summary: map_session_spec_summary(row)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn insert_session_spec(&self, record: &NewSessionSpecRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "INSERT INTO session_specs (
                id,
                project_id,
                project_root_id,
                worktree_id,
                work_item_id,
                account_id,
                agent_kind,
                cwd,
                command,
                args_json,
                env_preset_ref,
                origin_mode,
                current_mode,
                title,
                title_policy,
                restore_policy,
                initial_terminal_cols,
                initial_terminal_rows,
                context_bundle_artifact_id,
                created_at,
                updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21
             )",
            params![
                record.id,
                record.project_id,
                record.project_root_id,
                record.worktree_id,
                record.work_item_id,
                record.account_id,
                record.agent_kind,
                record.cwd,
                record.command,
                record.args_json,
                record.env_preset_ref,
                record.origin_mode,
                record.current_mode,
                record.title,
                record.title_policy,
                record.restore_policy,
                record.initial_terminal_cols,
                record.initial_terminal_rows,
                record.context_bundle_ref,
                record.created_at,
                record.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_session_spec(&self, record: &SessionSpecUpdateRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        let updated = connection.execute(
            "UPDATE session_specs
             SET project_root_id = ?2,
                 worktree_id = ?3,
                 work_item_id = ?4,
                 account_id = ?5,
                 agent_kind = ?6,
                 cwd = ?7,
                 command = ?8,
                 args_json = ?9,
                 env_preset_ref = ?10,
                 origin_mode = ?11,
                 current_mode = ?12,
                 title = ?13,
                 title_policy = ?14,
                 restore_policy = ?15,
                 initial_terminal_cols = ?16,
                 initial_terminal_rows = ?17,
                 context_bundle_artifact_id = ?18,
                 updated_at = ?19
             WHERE id = ?1",
            params![
                record.id,
                record.project_root_id,
                record.worktree_id,
                record.work_item_id,
                record.account_id,
                record.agent_kind,
                record.cwd,
                record.command,
                record.args_json,
                record.env_preset_ref,
                record.origin_mode,
                record.current_mode,
                record.title,
                record.title_policy,
                record.restore_policy,
                record.initial_terminal_cols,
                record.initial_terminal_rows,
                record.context_bundle_ref,
                record.updated_at,
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!("session spec {} was not found", record.id));
        }

        Ok(())
    }

    pub fn list_work_items(&self, filter: &WorkItemListFilter) -> Result<Vec<WorkItemSummary>> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        let mut sql = String::from(
            "SELECT
                w.id,
                w.project_id,
                w.namespace,
                w.parent_id,
                w.root_work_item_id,
                w.callsign,
                w.child_sequence,
                w.title,
                w.description,
                w.acceptance_criteria,
                w.work_item_type,
                w.status,
                w.priority,
                w.created_by,
                w.created_at,
                w.updated_at,
                w.closed_at,
                COUNT(DISTINCT c.id) AS child_count
             FROM work_items w
             LEFT JOIN work_items c
               ON c.parent_id = w.id
             WHERE 1 = 1",
        );
        let mut values = Vec::new();

        if let Some(project_id) = filter.project_id.as_deref() {
            sql.push_str(" AND w.project_id = ?");
            values.push(project_id.to_string());
        }

        if let Some(namespace) = filter.namespace.as_deref() {
            sql.push_str(" AND w.namespace = ?");
            values.push(namespace.to_string());
        }

        if let Some(parent_id) = filter.parent_id.as_deref() {
            sql.push_str(" AND w.parent_id = ?");
            values.push(parent_id.to_string());
        }

        if let Some(root_work_item_id) = filter.root_work_item_id.as_deref() {
            sql.push_str(" AND w.root_work_item_id = ?");
            values.push(root_work_item_id.to_string());
        }

        if let Some(status) = filter.status.as_deref() {
            sql.push_str(" AND w.status = ?");
            values.push(status.to_string());
        }

        if let Some(work_item_type) = filter.work_item_type.as_deref() {
            sql.push_str(" AND w.work_item_type = ?");
            values.push(work_item_type.to_string());
        }

        sql.push_str(
            " GROUP BY
                w.id,
                w.project_id,
                w.namespace,
                w.parent_id,
                w.root_work_item_id,
                w.callsign,
                w.child_sequence,
                w.title,
                w.description,
                w.acceptance_criteria,
                w.work_item_type,
                w.status,
                w.priority,
                w.created_by,
                w.created_at,
                w.updated_at,
                w.closed_at
              ORDER BY w.created_at ASC, w.callsign ASC",
        );

        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            values.push((limit as i64).to_string());
        }

        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), map_work_item_summary)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_work_item(&self, work_item_id: &str) -> Result<Option<WorkItemDetail>> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        let mut statement = connection.prepare(
            "SELECT
                w.id,
                w.project_id,
                w.namespace,
                w.parent_id,
                w.root_work_item_id,
                w.callsign,
                w.child_sequence,
                w.title,
                w.description,
                w.acceptance_criteria,
                w.work_item_type,
                w.status,
                w.priority,
                w.created_by,
                w.created_at,
                w.updated_at,
                w.closed_at,
                COUNT(DISTINCT c.id) AS child_count
             FROM work_items w
             LEFT JOIN work_items c
               ON c.parent_id = w.id
             WHERE w.id = ?1
             GROUP BY
                w.id,
                w.project_id,
                w.namespace,
                w.parent_id,
                w.root_work_item_id,
                w.callsign,
                w.child_sequence,
                w.title,
                w.description,
                w.acceptance_criteria,
                w.work_item_type,
                w.status,
                w.priority,
                w.created_by,
                w.created_at,
                w.updated_at,
                w.closed_at",
        )?;

        statement
            .query_row([work_item_id], |row| {
                Ok(WorkItemDetail {
                    summary: map_work_item_summary(row)?,
                })
            })
            .optional()
            .map_err(Into::into)
    }

    pub fn insert_work_item(&self, record: &NewWorkItemRecord) -> Result<()> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        connection.execute(
            "INSERT INTO work_items (
                id,
                project_id,
                namespace,
                parent_id,
                root_work_item_id,
                callsign,
                child_sequence,
                title,
                description,
                acceptance_criteria,
                work_item_type,
                status,
                priority,
                created_by,
                created_at,
                updated_at,
                closed_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
             )",
            params![
                record.id,
                record.project_id,
                record.namespace,
                record.parent_id,
                record.root_work_item_id,
                record.callsign,
                record.child_sequence,
                record.title,
                record.description,
                record.acceptance_criteria,
                record.work_item_type,
                record.status,
                record.priority,
                record.created_by,
                record.created_at,
                record.updated_at,
                record.closed_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_work_item(&self, record: &WorkItemUpdateRecord) -> Result<()> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        let updated = connection.execute(
            "UPDATE work_items
             SET title = ?2,
                 description = ?3,
                 acceptance_criteria = ?4,
                 work_item_type = ?5,
                 status = ?6,
                 priority = ?7,
                 created_by = ?8,
                 updated_at = ?9,
                 closed_at = ?10
             WHERE id = ?1",
            params![
                record.id,
                record.title,
                record.description,
                record.acceptance_criteria,
                record.work_item_type,
                record.status,
                record.priority,
                record.created_by,
                record.updated_at,
                record.closed_at,
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!("work item {} was not found", record.id));
        }

        Ok(())
    }

    pub fn work_item_exists(&self, work_item_id: &str) -> Result<bool> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        exists(
            &connection,
            "SELECT 1 FROM work_items WHERE id = ?1",
            [work_item_id],
        )
    }

    pub fn ensure_work_item_belongs_to_project(
        &self,
        work_item_id: &str,
        project_id: &str,
    ) -> Result<()> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        if exists(
            &connection,
            "SELECT 1 FROM work_items WHERE id = ?1 AND project_id = ?2",
            params![work_item_id, project_id],
        )? {
            return Ok(());
        }

        Err(anyhow!(
            "work item {} does not belong to project {}",
            work_item_id,
            project_id
        ))
    }

    pub fn next_work_item_child_sequence(&self, parent_id: &str) -> Result<i64> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        Ok(connection.query_row(
            "SELECT COALESCE(MAX(child_sequence), 0) + 1
             FROM work_items
             WHERE parent_id = ?1",
            [parent_id],
            |row| row.get(0),
        )?)
    }

    /// Returns distinct namespace keys from projects.wcp_namespace and
    /// callsign prefixes (the part before the first hyphen) from work_items.
    pub fn list_namespace_suggestions(&self) -> Result<Vec<String>> {
        let connection = open_app_connection_with_knowledge(&self.paths)?;
        let mut statement = connection.prepare(
            "SELECT DISTINCT ns FROM (
                 SELECT wcp_namespace AS ns FROM projects WHERE wcp_namespace IS NOT NULL
                 UNION
                 SELECT SUBSTR(callsign, 1, INSTR(callsign, '-') - 1) AS ns
                 FROM knowledge.work_items
                 WHERE INSTR(callsign, '-') > 0
             )
             WHERE ns IS NOT NULL AND ns != ''
             ORDER BY ns ASC",
        )?;
        let rows = statement.query_map([], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn list_root_work_item_callsigns(&self, project_id: &str) -> Result<Vec<String>> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        let mut statement = connection.prepare(
            "SELECT callsign
             FROM work_items
             WHERE project_id = ?1
               AND parent_id IS NULL
             ORDER BY created_at ASC, callsign ASC",
        )?;
        let rows = statement.query_map([project_id], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn list_root_work_item_callsigns_ns(&self, namespace: &str) -> Result<Vec<String>> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        let mut statement = connection.prepare(
            "SELECT callsign
             FROM work_items
             WHERE namespace = ?1
               AND parent_id IS NULL
             ORDER BY created_at ASC, callsign ASC",
        )?;
        let rows = statement.query_map([namespace], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn list_documents(&self, filter: &DocumentListFilter) -> Result<Vec<DocumentSummary>> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        let mut sql = String::from(
            "SELECT
                id,
                project_id,
                namespace,
                work_item_id,
                session_id,
                doc_type,
                title,
                slug,
                status,
                content_markdown,
                created_at,
                updated_at,
                archived_at
             FROM documents
             WHERE 1 = 1",
        );
        let mut values = Vec::new();

        if let Some(project_id) = filter.project_id.as_deref() {
            sql.push_str(" AND project_id = ?");
            values.push(project_id.to_string());
        }

        if let Some(namespace) = filter.namespace.as_deref() {
            sql.push_str(" AND namespace = ?");
            values.push(namespace.to_string());
        }

        if let Some(work_item_id) = filter.work_item_id.as_deref() {
            sql.push_str(" AND work_item_id = ?");
            values.push(work_item_id.to_string());
        }

        if let Some(session_id) = filter.session_id.as_deref() {
            sql.push_str(" AND session_id = ?");
            values.push(session_id.to_string());
        }

        if let Some(doc_type) = filter.doc_type.as_deref() {
            sql.push_str(" AND doc_type = ?");
            values.push(doc_type.to_string());
        }

        if let Some(status) = filter.status.as_deref() {
            sql.push_str(" AND status = ?");
            values.push(status.to_string());
        }

        sql.push_str(" ORDER BY updated_at DESC, created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            values.push((limit as i64).to_string());
        }

        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), map_document_summary)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_document(&self, document_id: &str) -> Result<Option<DocumentDetail>> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        connection
            .query_row(
                "SELECT
                    id,
                    project_id,
                    namespace,
                    work_item_id,
                    session_id,
                    doc_type,
                    title,
                    slug,
                    status,
                    content_markdown,
                    created_at,
                    updated_at,
                    archived_at
                 FROM documents
                 WHERE id = ?1",
                [document_id],
                |row| {
                    Ok(DocumentDetail {
                        summary: map_document_summary(row)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn insert_document(&self, record: &NewDocumentRecord) -> Result<()> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        connection.execute(
            "INSERT INTO documents (
                id,
                project_id,
                namespace,
                work_item_id,
                session_id,
                doc_type,
                title,
                slug,
                status,
                content_markdown,
                created_at,
                updated_at,
                archived_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13
             )",
            params![
                record.id,
                record.project_id,
                record.namespace,
                record.work_item_id,
                record.session_id,
                record.doc_type,
                record.title,
                record.slug,
                record.status,
                record.content_markdown,
                record.created_at,
                record.updated_at,
                record.archived_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_document(&self, record: &DocumentUpdateRecord) -> Result<()> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        let updated = connection.execute(
            "UPDATE documents
             SET work_item_id = ?2,
                 session_id = ?3,
                 doc_type = ?4,
                 title = ?5,
                 slug = ?6,
                 status = ?7,
                 content_markdown = ?8,
                 updated_at = ?9,
                 archived_at = ?10
             WHERE id = ?1",
            params![
                record.id,
                record.work_item_id,
                record.session_id,
                record.doc_type,
                record.title,
                record.slug,
                record.status,
                record.content_markdown,
                record.updated_at,
                record.archived_at,
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!("document {} was not found", record.id));
        }

        Ok(())
    }

    pub fn document_slug_exists(
        &self,
        project_id: &str,
        slug: &str,
        exclude_document_id: Option<&str>,
    ) -> Result<bool> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        match exclude_document_id {
            Some(document_id) => exists(
                &connection,
                "SELECT 1 FROM documents WHERE project_id = ?1 AND slug = ?2 AND id <> ?3",
                params![project_id, slug, document_id],
            ),
            None => exists(
                &connection,
                "SELECT 1 FROM documents WHERE project_id = ?1 AND slug = ?2",
                params![project_id, slug],
            ),
        }
    }

    pub fn list_work_item_ids_for_project(&self, project_id: &str) -> Result<Vec<String>> {
        let connection = open_connection(&self.paths.knowledge_db)?;
        let mut statement = connection.prepare(
            "SELECT id
             FROM work_items
             WHERE project_id = ?1",
        )?;
        let rows = statement.query_map([project_id], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn list_planning_assignments(
        &self,
        filter: &PlanningAssignmentListFilter,
        scoped_work_item_ids: Option<&[String]>,
    ) -> Result<Vec<PlanningAssignmentSummary>> {
        if matches!(scoped_work_item_ids, Some(ids) if ids.is_empty()) {
            return Ok(Vec::new());
        }

        let connection = open_connection(&self.paths.app_db)?;
        let mut sql = String::from(
            "SELECT
                id,
                work_item_id,
                cadence_type,
                cadence_key,
                created_by,
                created_at,
                updated_at,
                removed_at
             FROM planning_assignments
             WHERE 1 = 1",
        );
        let mut values = Vec::new();

        if let Some(scoped_work_item_ids) = scoped_work_item_ids {
            sql.push_str(" AND work_item_id IN (");
            sql.push_str(&vec!["?"; scoped_work_item_ids.len()].join(", "));
            sql.push(')');
            values.extend(scoped_work_item_ids.iter().cloned());
        }

        if let Some(work_item_id) = filter.work_item_id.as_deref() {
            sql.push_str(" AND work_item_id = ?");
            values.push(work_item_id.to_string());
        }

        if let Some(cadence_type) = filter.cadence_type.as_deref() {
            sql.push_str(" AND cadence_type = ?");
            values.push(cadence_type.to_string());
        }

        if let Some(cadence_key) = filter.cadence_key.as_deref() {
            sql.push_str(" AND cadence_key = ?");
            values.push(cadence_key.to_string());
        }

        if !filter.include_removed {
            sql.push_str(" AND removed_at IS NULL");
        }

        sql.push_str(" ORDER BY updated_at DESC, created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            values.push((limit as i64).to_string());
        }

        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(
            params_from_iter(values.iter()),
            map_planning_assignment_summary,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_planning_assignment(
        &self,
        planning_assignment_id: &str,
    ) -> Result<Option<PlanningAssignmentDetail>> {
        let connection = open_connection(&self.paths.app_db)?;
        connection
            .query_row(
                "SELECT
                    id,
                    work_item_id,
                    cadence_type,
                    cadence_key,
                    created_by,
                    created_at,
                    updated_at,
                    removed_at
                 FROM planning_assignments
                 WHERE id = ?1",
                [planning_assignment_id],
                |row| {
                    Ok(PlanningAssignmentDetail {
                        summary: map_planning_assignment_summary(row)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn insert_planning_assignment(&self, record: &NewPlanningAssignmentRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "INSERT INTO planning_assignments (
                id,
                work_item_id,
                cadence_type,
                cadence_key,
                created_by,
                created_at,
                updated_at,
                removed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                record.id,
                record.work_item_id,
                record.cadence_type,
                record.cadence_key,
                record.created_by,
                record.created_at,
                record.updated_at,
                record.removed_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_planning_assignment(
        &self,
        record: &PlanningAssignmentUpdateRecord,
    ) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        let updated = connection.execute(
            "UPDATE planning_assignments
             SET work_item_id = ?2,
                 cadence_type = ?3,
                 cadence_key = ?4,
                 created_by = ?5,
                 updated_at = ?6
             WHERE id = ?1
               AND removed_at IS NULL",
            params![
                record.id,
                record.work_item_id,
                record.cadence_type,
                record.cadence_key,
                record.created_by,
                record.updated_at,
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!("planning assignment {} was not found", record.id));
        }

        Ok(())
    }

    pub fn soft_delete_planning_assignment(
        &self,
        planning_assignment_id: &str,
        removed_at: i64,
    ) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        let updated = connection.execute(
            "UPDATE planning_assignments
             SET removed_at = COALESCE(removed_at, ?2),
                 updated_at = ?2
             WHERE id = ?1
               AND removed_at IS NULL",
            params![planning_assignment_id, removed_at],
        )?;

        if updated == 0 {
            return Err(anyhow!(
                "planning assignment {} was not found",
                planning_assignment_id
            ));
        }

        Ok(())
    }

    pub fn planning_assignment_exists_active(
        &self,
        work_item_id: &str,
        cadence_type: &str,
        cadence_key: &str,
        exclude_planning_assignment_id: Option<&str>,
    ) -> Result<bool> {
        let connection = open_connection(&self.paths.app_db)?;
        match exclude_planning_assignment_id {
            Some(planning_assignment_id) => exists(
                &connection,
                "SELECT 1
                 FROM planning_assignments
                 WHERE work_item_id = ?1
                   AND cadence_type = ?2
                   AND cadence_key = ?3
                   AND removed_at IS NULL
                   AND id <> ?4",
                params![
                    work_item_id,
                    cadence_type,
                    cadence_key,
                    planning_assignment_id
                ],
            ),
            None => exists(
                &connection,
                "SELECT 1
                 FROM planning_assignments
                 WHERE work_item_id = ?1
                   AND cadence_type = ?2
                   AND cadence_key = ?3
                   AND removed_at IS NULL",
                params![work_item_id, cadence_type, cadence_key],
            ),
        }
    }

    pub fn list_workflow_reconciliation_proposals(
        &self,
        filter: &WorkflowReconciliationProposalListFilter,
        scoped_work_item_ids: Option<&[String]>,
    ) -> Result<Vec<WorkflowReconciliationProposalSummary>> {
        if matches!(scoped_work_item_ids, Some(ids) if ids.is_empty()) {
            return Ok(Vec::new());
        }

        let connection = open_connection(&self.paths.app_db)?;
        let mut sql = String::from(
            "SELECT
                id,
                source_session_id,
                work_item_id,
                target_entity_type,
                target_entity_id,
                proposal_type,
                proposed_change_payload,
                reason,
                confidence,
                status,
                created_at,
                updated_at,
                resolved_at
             FROM workflow_reconciliation_proposals
             WHERE 1 = 1",
        );
        let mut values = Vec::new();

        if let Some(scoped_work_item_ids) = scoped_work_item_ids {
            sql.push_str(" AND work_item_id IN (");
            sql.push_str(&vec!["?"; scoped_work_item_ids.len()].join(", "));
            sql.push(')');
            values.extend(scoped_work_item_ids.iter().cloned());
        }

        if let Some(work_item_id) = filter.work_item_id.as_deref() {
            sql.push_str(" AND work_item_id = ?");
            values.push(work_item_id.to_string());
        }

        if let Some(source_session_id) = filter.source_session_id.as_deref() {
            sql.push_str(" AND source_session_id = ?");
            values.push(source_session_id.to_string());
        }

        if let Some(target_entity_type) = filter.target_entity_type.as_deref() {
            sql.push_str(" AND target_entity_type = ?");
            values.push(target_entity_type.to_string());
        }

        if let Some(proposal_type) = filter.proposal_type.as_deref() {
            sql.push_str(" AND proposal_type = ?");
            values.push(proposal_type.to_string());
        }

        if let Some(status) = filter.status.as_deref() {
            sql.push_str(" AND status = ?");
            values.push(status.to_string());
        }

        sql.push_str(" ORDER BY created_at DESC, id DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            values.push((limit as i64).to_string());
        }

        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(
            params_from_iter(values.iter()),
            map_workflow_reconciliation_proposal_summary,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_workflow_reconciliation_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<Option<WorkflowReconciliationProposalDetail>> {
        let connection = open_connection(&self.paths.app_db)?;
        connection
            .query_row(
                "SELECT
                    id,
                    source_session_id,
                    work_item_id,
                    target_entity_type,
                    target_entity_id,
                    proposal_type,
                    proposed_change_payload,
                    reason,
                    confidence,
                    status,
                    created_at,
                    updated_at,
                    resolved_at
                 FROM workflow_reconciliation_proposals
                 WHERE id = ?1",
                [proposal_id],
                |row| {
                    Ok(WorkflowReconciliationProposalDetail {
                        summary: map_workflow_reconciliation_proposal_summary(row)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn insert_workflow_reconciliation_proposal(
        &self,
        record: &NewWorkflowReconciliationProposalRecord,
    ) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "INSERT INTO workflow_reconciliation_proposals (
                id,
                source_session_id,
                work_item_id,
                target_entity_type,
                target_entity_id,
                proposal_type,
                proposed_change_payload,
                reason,
                confidence,
                status,
                created_at,
                updated_at,
                resolved_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13
             )",
            params![
                record.id,
                record.source_session_id,
                record.work_item_id,
                record.target_entity_type,
                record.target_entity_id,
                record.proposal_type,
                record.proposed_change_payload_json,
                record.reason,
                record.confidence,
                record.status,
                record.created_at,
                record.updated_at,
                record.resolved_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_workflow_reconciliation_proposal(
        &self,
        record: &WorkflowReconciliationProposalUpdateRecord,
    ) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        let updated = connection.execute(
            "UPDATE workflow_reconciliation_proposals
             SET work_item_id = ?2,
                 target_entity_type = ?3,
                 target_entity_id = ?4,
                 proposal_type = ?5,
                 proposed_change_payload = ?6,
                 reason = ?7,
                 confidence = ?8,
                 status = ?9,
                 updated_at = ?10,
                 resolved_at = ?11
             WHERE id = ?1",
            params![
                record.id,
                record.work_item_id,
                record.target_entity_type,
                record.target_entity_id,
                record.proposal_type,
                record.proposed_change_payload_json,
                record.reason,
                record.confidence,
                record.status,
                record.updated_at,
                record.resolved_at,
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!(
                "workflow reconciliation proposal {} was not found",
                record.id
            ));
        }

        Ok(())
    }

    pub fn list_sessions(&self, filter: &SessionListFilter) -> Result<Vec<SessionSummary>> {
        let connection = open_connection(&self.paths.app_db)?;
        let mut sql = String::from(
            "SELECT
                s.id,
                s.session_spec_id,
                s.project_id,
                s.project_root_id,
                s.worktree_id,
                w.branch_name,
                s.work_item_id,
                s.account_id,
                s.agent_kind,
                s.origin_mode,
                s.current_mode,
                s.title,
                s.title_source,
                s.runtime_state,
                s.status,
                s.activity_state,
                s.needs_input_reason,
                s.pty_owner_key,
                s.cwd,
                s.started_at,
                s.ended_at,
                s.last_output_at,
                s.last_attached_at,
                s.created_at,
                s.updated_at,
                s.archived_at,
                s.dispatch_group
             FROM sessions s
             LEFT JOIN worktrees w ON s.worktree_id = w.id
             WHERE 1 = 1",
        );

        let mut values = Vec::new();

        if let Some(project_id) = filter.project_id.as_deref() {
            sql.push_str(" AND s.project_id = ?");
            values.push(project_id.to_string());
        }

        if let Some(status) = filter.status.as_deref() {
            sql.push_str(" AND s.status = ?");
            values.push(status.to_string());
        }

        if let Some(runtime_state) = filter.runtime_state.as_deref() {
            sql.push_str(" AND s.runtime_state = ?");
            values.push(runtime_state.to_string());
        }

        if let Some(work_item_id) = filter.work_item_id.as_deref() {
            sql.push_str(" AND s.work_item_id = ?");
            values.push(work_item_id.to_string());
        }

        sql.push_str(" ORDER BY s.created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            values.push((limit as i64).to_string());
        }

        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), map_session_summary)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn reconcile_interrupted_sessions(&self, now: i64) -> Result<usize> {
        let connection = open_connection(&self.paths.app_db)?;
        let updated = connection.execute(
            "UPDATE sessions
             SET runtime_state = 'interrupted',
                 status = 'interrupted',
                 activity_state = 'idle',
                 ended_at = COALESCE(ended_at, ?1),
                 updated_at = ?1
             WHERE status = 'active'
               AND runtime_state IN ('starting', 'running', 'stopping')",
            params![now],
        )?;
        Ok(updated)
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionSummary>> {
        let connection = open_connection(&self.paths.app_db)?;
        connection
            .query_row(
                "SELECT
                    s.id,
                    s.session_spec_id,
                    s.project_id,
                    s.project_root_id,
                    s.worktree_id,
                    w.branch_name,
                    s.work_item_id,
                    s.account_id,
                    s.agent_kind,
                    s.origin_mode,
                    s.current_mode,
                    s.title,
                    s.title_source,
                    s.runtime_state,
                    s.status,
                    s.activity_state,
                    s.needs_input_reason,
                    s.pty_owner_key,
                    s.cwd,
                    s.started_at,
                    s.ended_at,
                    s.last_output_at,
                    s.last_attached_at,
                    s.created_at,
                    s.updated_at,
                    s.archived_at,
                    s.dispatch_group
                 FROM sessions s
                 LEFT JOIN worktrees w ON s.worktree_id = w.id
                 WHERE s.id = ?1",
                [session_id],
                map_session_summary,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn insert_session(
        &self,
        spec_record: &NewSessionSpecRecord,
        record: &NewSessionRecord,
        artifacts: &[SessionArtifactRecord],
    ) -> Result<()> {
        let mut connection = open_connection(&self.paths.app_db)?;
        let tx = connection.transaction()?;

        tx.execute(
            "INSERT INTO session_specs (
                id,
                project_id,
                project_root_id,
                worktree_id,
                work_item_id,
                account_id,
                agent_kind,
                cwd,
                command,
                args_json,
                env_preset_ref,
                origin_mode,
                current_mode,
                title,
                title_policy,
                restore_policy,
                initial_terminal_cols,
                initial_terminal_rows,
                context_bundle_artifact_id,
                created_at,
                updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21
             )",
            params![
                spec_record.id,
                spec_record.project_id,
                spec_record.project_root_id,
                spec_record.worktree_id,
                spec_record.work_item_id,
                spec_record.account_id,
                spec_record.agent_kind,
                spec_record.cwd,
                spec_record.command,
                spec_record.args_json,
                spec_record.env_preset_ref,
                spec_record.origin_mode,
                spec_record.current_mode,
                spec_record.title,
                spec_record.title_policy,
                spec_record.restore_policy,
                spec_record.initial_terminal_cols,
                spec_record.initial_terminal_rows,
                spec_record.context_bundle_ref,
                spec_record.created_at,
                spec_record.updated_at,
            ],
        )?;

        tx.execute(
            "INSERT INTO sessions (
                id,
                session_spec_id,
                project_id,
                project_root_id,
                worktree_id,
                work_item_id,
                account_id,
                agent_kind,
                origin_mode,
                current_mode,
                title,
                title_source,
                user_prompt_count,
                next_title_refresh_at_prompt_count,
                runtime_state,
                status,
                activity_state,
                needs_input_reason,
                pty_owner_key,
                cwd,
                transcript_primary_artifact_id,
                raw_log_artifact_id,
                started_at,
                ended_at,
                last_output_at,
                last_attached_at,
                created_at,
                updated_at,
                archived_at,
                dispatch_group
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, 1, ?13, ?14, ?15, NULL, ?16, ?17, ?18, ?19, ?20, NULL, NULL, NULL, ?21, ?22, NULL, ?23
             )",
            params![
                record.id,
                record.session_spec_id,
                record.project_id,
                record.project_root_id,
                record.worktree_id,
                record.work_item_id,
                record.account_id,
                record.agent_kind,
                record.origin_mode,
                record.current_mode,
                record.title,
                record.title_source,
                record.runtime_state,
                record.status,
                record.activity_state,
                record.pty_owner_key,
                record.cwd,
                record.transcript_primary_artifact_id,
                record.raw_log_artifact_id,
                record.started_at,
                record.created_at,
                record.updated_at,
                record.dispatch_group,
            ],
        )?;

        for artifact in artifacts {
            tx.execute(
                "INSERT INTO session_artifacts (
                    id,
                    session_id,
                    artifact_class,
                    artifact_type,
                    path,
                    is_durable,
                    is_primary,
                    source,
                    generator_ref,
                    supersedes_artifact_id,
                    created_at
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11
                 )",
                params![
                    artifact.id,
                    artifact.session_id,
                    artifact.artifact_class,
                    artifact.artifact_type,
                    artifact.path,
                    bool_to_sqlite(artifact.is_durable),
                    bool_to_sqlite(artifact.is_primary),
                    artifact.source,
                    artifact.generator_ref,
                    artifact.supersedes_artifact_id,
                    artifact.created_at,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn mark_session_started(&self, session_id: &str, started_at: i64) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "UPDATE sessions
             SET runtime_state = 'running',
                 status = 'active',
                 activity_state = 'working',
                 needs_input_reason = NULL,
                 started_at = COALESCE(started_at, ?2),
                 updated_at = ?2
             WHERE id = ?1",
            params![session_id, started_at],
        )?;
        Ok(())
    }

    pub fn mark_session_stopping(&self, session_id: &str, updated_at: i64) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "UPDATE sessions
             SET runtime_state = 'stopping',
                 status = 'active',
                 activity_state = 'idle',
                 needs_input_reason = NULL,
                 updated_at = ?2
             WHERE id = ?1",
            params![session_id, updated_at],
        )?;
        Ok(())
    }

    pub fn mark_session_failed_to_start(&self, session_id: &str, failed_at: i64) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "UPDATE sessions
             SET runtime_state = 'failed',
                 status = 'failed',
                 activity_state = 'idle',
                 needs_input_reason = NULL,
                 ended_at = COALESCE(ended_at, ?2),
                 updated_at = ?2
             WHERE id = ?1",
            params![session_id, failed_at],
        )?;
        Ok(())
    }

    pub fn record_session_output(&self, session_id: &str, timestamp: i64) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "UPDATE sessions
             SET last_output_at = ?2,
                 activity_state = 'working',
                 needs_input_reason = NULL,
                 updated_at = ?2
             WHERE id = ?1",
            params![session_id, timestamp],
        )?;
        Ok(())
    }

    pub fn record_session_input(&self, session_id: &str, timestamp: i64) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "UPDATE sessions
             SET activity_state = 'working',
                 needs_input_reason = NULL,
                 updated_at = ?2
             WHERE id = ?1",
            params![session_id, timestamp],
        )?;
        Ok(())
    }

    pub fn record_session_attached(&self, session_id: &str, timestamp: i64) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "UPDATE sessions
             SET last_attached_at = ?2,
                 updated_at = ?2
             WHERE id = ?1",
            params![session_id, timestamp],
        )?;
        Ok(())
    }

    pub fn mark_session_finished(
        &self,
        session_id: &str,
        runtime_state: &str,
        status: &str,
        ended_at: i64,
    ) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "UPDATE sessions
             SET runtime_state = ?2,
                 status = ?3,
                 activity_state = 'idle',
                 needs_input_reason = NULL,
                 ended_at = COALESCE(ended_at, ?4),
                 updated_at = ?4
             WHERE id = ?1",
            params![session_id, runtime_state, status, ended_at],
        )?;
        Ok(())
    }

    pub fn update_session_activity(
        &self,
        session_id: &str,
        activity_state: &str,
        needs_input_reason: Option<&str>,
        updated_at: i64,
    ) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "UPDATE sessions
             SET activity_state = ?2,
                 needs_input_reason = ?3,
                 updated_at = ?4
             WHERE id = ?1",
            params![session_id, activity_state, needs_input_reason, updated_at],
        )?;
        Ok(())
    }

    pub fn get_workspace_state(&self, scope: &str) -> Result<Option<WorkspaceStateRecord>> {
        let connection = open_connection(&self.paths.app_db)?;
        connection
            .query_row(
                "SELECT id, scope, payload_json, saved_at
                 FROM workspace_state
                 WHERE scope = ?1
                 ORDER BY saved_at DESC, rowid DESC
                 LIMIT 1",
                [scope],
                map_workspace_state_record,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn save_workspace_state(
        &self,
        request: &UpdateWorkspaceStateRequest,
        id: &str,
        saved_at: i64,
    ) -> Result<()> {
        let mut connection = open_connection(&self.paths.app_db)?;
        let tx = connection.transaction()?;
        tx.execute(
            "DELETE FROM workspace_state WHERE scope = ?1",
            params![request.scope],
        )?;
        tx.execute(
            "INSERT INTO workspace_state (id, scope, payload_json, saved_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                id,
                request.scope,
                serde_json::to_string(&request.payload)?,
                saved_at
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn account_exists(&self, account_id: &str) -> Result<bool> {
        let connection = open_connection(&self.paths.app_db)?;
        exists(
            &connection,
            "SELECT 1 FROM accounts WHERE id = ?1",
            [account_id],
        )
    }

    pub fn session_exists(&self, session_id: &str) -> Result<bool> {
        let connection = open_connection(&self.paths.app_db)?;
        exists(
            &connection,
            "SELECT 1 FROM sessions WHERE id = ?1",
            [session_id],
        )
    }

    pub fn has_active_session_for_worktree(
        &self,
        worktree_id: &str,
        exclude_session_id: Option<&str>,
    ) -> Result<bool> {
        let connection = open_connection(&self.paths.app_db)?;
        match exclude_session_id {
            Some(session_id) => exists(
                &connection,
                "SELECT 1
                 FROM sessions
                 WHERE worktree_id = ?1
                   AND id <> ?2
                   AND status = 'active'
                   AND runtime_state IN ('starting', 'running', 'stopping')",
                params![worktree_id, session_id],
            ),
            None => exists(
                &connection,
                "SELECT 1
                 FROM sessions
                 WHERE worktree_id = ?1
                   AND status = 'active'
                   AND runtime_state IN ('starting', 'running', 'stopping')",
                params![worktree_id],
            ),
        }
    }

    pub fn ensure_project_root_belongs_to_project(
        &self,
        project_root_id: &str,
        project_id: &str,
    ) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        if exists(
            &connection,
            "SELECT 1 FROM project_roots WHERE id = ?1 AND project_id = ?2",
            params![project_root_id, project_id],
        )? {
            return Ok(());
        }

        Err(anyhow!(
            "project root {} does not belong to project {}",
            project_root_id,
            project_id
        ))
    }

    pub fn ensure_worktree_belongs_to_project(
        &self,
        worktree_id: &str,
        project_id: &str,
    ) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        if exists(
            &connection,
            "SELECT 1 FROM worktrees WHERE id = ?1 AND project_id = ?2",
            params![worktree_id, project_id],
        )? {
            return Ok(());
        }

        Err(anyhow!(
            "worktree {} does not belong to project {}",
            worktree_id,
            project_id
        ))
    }

    pub fn ensure_worktree_belongs_to_project_root(
        &self,
        worktree_id: &str,
        project_root_id: &str,
    ) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        if exists(
            &connection,
            "SELECT 1 FROM worktrees WHERE id = ?1 AND project_root_id = ?2",
            params![worktree_id, project_root_id],
        )? {
            return Ok(());
        }

        Err(anyhow!(
            "worktree {} does not belong to project root {}",
            worktree_id,
            project_root_id
        ))
    }

    // --- Merge Queue ---

    pub fn insert_merge_queue_entry(&self, record: &NewMergeQueueRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "INSERT INTO merge_queue (
                id, project_id, session_id, worktree_id, branch_name,
                base_ref, position, status, diff_stat_json,
                conflict_files_json, has_uncommitted_changes, queued_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                record.id,
                record.project_id,
                record.session_id,
                record.worktree_id,
                record.branch_name,
                record.base_ref,
                record.position,
                record.status,
                record.diff_stat_json,
                record.conflict_files_json,
                bool_to_sqlite(record.has_uncommitted_changes),
                record.queued_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_merge_queue(&self, filter: &MergeQueueListFilter) -> Result<Vec<MergeQueueEntry>> {
        let connection = open_app_connection_with_knowledge(&self.paths)?;
        let mut sql = String::from(
            "SELECT
                mq.id, mq.project_id, mq.session_id, mq.worktree_id,
                mq.branch_name, mq.base_ref, mq.position, mq.status,
                mq.diff_stat_json, mq.conflict_files_json,
                mq.has_uncommitted_changes, mq.queued_at, mq.merged_at,
                s.title AS session_title,
                wi.callsign AS work_item_callsign
             FROM merge_queue mq
             LEFT JOIN sessions s ON s.id = mq.session_id
             LEFT JOIN knowledge.work_items wi ON wi.id = s.work_item_id
             WHERE mq.project_id = ?1",
        );
        let mut values: Vec<String> = vec![filter.project_id.clone()];
        if let Some(ref status) = filter.status {
            sql.push_str(" AND mq.status = ?");
            values.push(status.clone());
        }
        sql.push_str(" ORDER BY mq.position ASC");

        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), map_merge_queue_entry)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_merge_queue_entry(&self, id: &str) -> Result<Option<MergeQueueEntry>> {
        let connection = open_app_connection_with_knowledge(&self.paths)?;
        connection
            .query_row(
                "SELECT
                    mq.id, mq.project_id, mq.session_id, mq.worktree_id,
                    mq.branch_name, mq.base_ref, mq.position, mq.status,
                    mq.diff_stat_json, mq.conflict_files_json,
                    mq.has_uncommitted_changes, mq.queued_at, mq.merged_at,
                    s.title AS session_title,
                    wi.callsign AS work_item_callsign
                 FROM merge_queue mq
                 LEFT JOIN sessions s ON s.id = mq.session_id
                 LEFT JOIN knowledge.work_items wi ON wi.id = s.work_item_id
                 WHERE mq.id = ?1",
                [id],
                map_merge_queue_entry,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn update_merge_queue_status(
        &self,
        id: &str,
        status: &str,
        merged_at: Option<i64>,
    ) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "UPDATE merge_queue SET status = ?2, merged_at = ?3 WHERE id = ?1",
            params![id, status, merged_at],
        )?;
        Ok(())
    }

    pub fn update_merge_queue_diff_stat(&self, id: &str, diff_stat_json: &str) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "UPDATE merge_queue SET diff_stat_json = ?2 WHERE id = ?1",
            params![id, diff_stat_json],
        )?;
        Ok(())
    }

    pub fn update_merge_queue_conflict_files(
        &self,
        id: &str,
        conflict_files_json: &str,
    ) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "UPDATE merge_queue SET conflict_files_json = ?2 WHERE id = ?1",
            params![id, conflict_files_json],
        )?;
        Ok(())
    }

    pub fn reorder_merge_queue(&self, project_id: &str, ordered_ids: &[String]) -> Result<()> {
        let mut connection = open_connection(&self.paths.app_db)?;
        let tx = connection.transaction()?;
        for (index, id) in ordered_ids.iter().enumerate() {
            tx.execute(
                "UPDATE merge_queue SET position = ?2 WHERE id = ?1 AND project_id = ?3",
                params![id, index as i64, project_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn next_merge_queue_position(&self, project_id: &str) -> Result<i64> {
        let connection = open_connection(&self.paths.app_db)?;
        let position: i64 = connection.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM merge_queue WHERE project_id = ?1",
            [project_id],
            |row| row.get(0),
        )?;
        Ok(position)
    }

    pub fn update_worktree_status(
        &self,
        worktree_id: &str,
        status: &str,
        updated_at: i64,
    ) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "UPDATE worktrees SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![worktree_id, status, updated_at],
        )?;
        Ok(())
    }

    // --- Inbox ---

    pub fn insert_inbox_entry(&self, record: &NewInboxEntryRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "INSERT INTO inbox_entries (
                id, project_id, session_id, work_item_id, worktree_id,
                entry_type, title, summary, status, branch_name,
                diff_stat_json, metadata_json, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                record.id,
                record.project_id,
                record.session_id,
                record.work_item_id,
                record.worktree_id,
                record.entry_type,
                record.title,
                record.summary,
                record.status,
                record.branch_name,
                record.diff_stat_json,
                record.metadata_json,
                record.created_at,
                record.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_inbox_entry(&self, id: &str) -> Result<Option<InboxEntryDetail>> {
        let connection = open_app_connection_with_knowledge(&self.paths)?;
        connection
            .query_row(
                "SELECT
                    ie.id, ie.project_id, ie.session_id, ie.work_item_id, ie.worktree_id,
                    ie.entry_type, ie.title, ie.summary, ie.status, ie.branch_name,
                    ie.diff_stat_json, ie.metadata_json, ie.read_at, ie.resolved_at,
                    ie.created_at, ie.updated_at,
                    s.title AS session_title,
                    wi.callsign AS work_item_callsign
                 FROM inbox_entries ie
                 LEFT JOIN sessions s ON s.id = ie.session_id
                 LEFT JOIN knowledge.work_items wi ON wi.id = ie.work_item_id
                 WHERE ie.id = ?1",
                [id],
                map_inbox_entry_detail,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_inbox_entries(
        &self,
        filter: &InboxEntryListFilter,
    ) -> Result<Vec<InboxEntrySummary>> {
        let connection = open_app_connection_with_knowledge(&self.paths)?;
        let mut sql = String::from(
            "SELECT
                ie.id, ie.project_id, ie.session_id, ie.work_item_id, ie.worktree_id,
                ie.entry_type, ie.title, ie.summary, ie.status, ie.branch_name,
                ie.diff_stat_json, ie.metadata_json, ie.read_at, ie.resolved_at,
                ie.created_at, ie.updated_at,
                s.title AS session_title,
                wi.callsign AS work_item_callsign
             FROM inbox_entries ie
             LEFT JOIN sessions s ON s.id = ie.session_id
             LEFT JOIN knowledge.work_items wi ON wi.id = ie.work_item_id
             WHERE ie.project_id = ?1",
        );
        let mut values: Vec<String> = vec![filter.project_id.clone()];

        if let Some(ref status) = filter.status {
            sql.push_str(&format!(" AND ie.status = ?{}", values.len() + 1));
            values.push(status.clone());
        }
        if let Some(ref entry_type) = filter.entry_type {
            sql.push_str(&format!(" AND ie.entry_type = ?{}", values.len() + 1));
            values.push(entry_type.clone());
        }
        if filter.unread_only.unwrap_or(false) {
            sql.push_str(" AND ie.read_at IS NULL");
        }

        sql.push_str(" ORDER BY ie.created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut statement = connection.prepare(&sql)?;
        let rows =
            statement.query_map(params_from_iter(values.iter()), map_inbox_entry_summary)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn update_inbox_entry(&self, record: &InboxEntryUpdateRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        let mut parts: Vec<String> = vec!["updated_at = ?2".to_string()];
        let mut idx = 3usize;

        if record.status.is_some() {
            parts.push(format!("status = ?{idx}"));
            idx += 1;
        }
        if record.read_at.is_some() {
            parts.push(format!("read_at = ?{idx}"));
            idx += 1;
        }
        if record.resolved_at.is_some() {
            parts.push(format!("resolved_at = ?{idx}"));
            let _ = idx; // suppress unused warning
        }

        let sql = format!(
            "UPDATE inbox_entries SET {} WHERE id = ?1",
            parts.join(", ")
        );

        // Build params dynamically using rusqlite's raw execute
        let mut raw_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        raw_params.push(Box::new(record.id.clone()));
        raw_params.push(Box::new(record.updated_at));
        if let Some(ref status) = record.status {
            raw_params.push(Box::new(status.clone()));
        }
        if let Some(read_at) = record.read_at {
            raw_params.push(Box::new(read_at));
        }
        if let Some(resolved_at) = record.resolved_at {
            raw_params.push(Box::new(resolved_at));
        }

        let updated = connection.execute(
            &sql,
            rusqlite::params_from_iter(raw_params.iter().map(|p| p.as_ref())),
        )?;

        if updated == 0 {
            return Err(anyhow!("inbox entry {} was not found", record.id));
        }
        Ok(())
    }

    pub fn count_unread_inbox_entries(&self, project_id: &str) -> Result<i64> {
        let connection = open_connection(&self.paths.app_db)?;
        let count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM inbox_entries
             WHERE project_id = ?1 AND read_at IS NULL AND resolved_at IS NULL",
            [project_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // --- Agent Templates ---

    pub fn insert_agent_template(&self, record: &NewAgentTemplateRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "INSERT INTO agent_templates (
                id, project_id, template_key, label, origin_mode, default_model,
                instructions_md, stop_rules_json, sort_order, created_at, updated_at, archived_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL)",
            params![
                record.id,
                record.project_id,
                record.template_key,
                record.label,
                record.origin_mode,
                record.default_model,
                record.instructions_md,
                record.stop_rules_json,
                record.sort_order,
                record.created_at,
                record.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_agent_template(&self, id: &str) -> Result<Option<AgentTemplateDetail>> {
        let connection = open_connection(&self.paths.app_db)?;
        connection
            .query_row(
                "SELECT id, project_id, template_key, label, origin_mode, default_model,
                        instructions_md, stop_rules_json, sort_order, created_at, updated_at, archived_at
                 FROM agent_templates WHERE id = ?1",
                [id],
                map_agent_template_detail,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_agent_templates(
        &self,
        filter: &AgentTemplateListFilter,
    ) -> Result<Vec<AgentTemplateSummary>> {
        let connection = open_connection(&self.paths.app_db)?;
        let include_archived = filter.include_archived.unwrap_or(false);
        let sql = if include_archived {
            "SELECT id, project_id, template_key, label, origin_mode, default_model,
                    instructions_md, stop_rules_json, sort_order, created_at, updated_at, archived_at
             FROM agent_templates WHERE project_id = ?1
             ORDER BY sort_order ASC, created_at ASC"
        } else {
            "SELECT id, project_id, template_key, label, origin_mode, default_model,
                    instructions_md, stop_rules_json, sort_order, created_at, updated_at, archived_at
             FROM agent_templates WHERE project_id = ?1 AND archived_at IS NULL
             ORDER BY sort_order ASC, created_at ASC"
        };
        let mut statement = connection.prepare(sql)?;
        let rows = statement.query_map([&filter.project_id], map_agent_template_summary)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    pub fn update_agent_template(&self, record: &AgentTemplateUpdateRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        let updated = connection.execute(
            "UPDATE agent_templates
             SET template_key = ?2, label = ?3, origin_mode = ?4, default_model = ?5,
                 instructions_md = ?6, stop_rules_json = ?7, sort_order = ?8,
                 updated_at = ?9, archived_at = ?10
             WHERE id = ?1",
            params![
                record.id,
                record.template_key,
                record.label,
                record.origin_mode,
                record.default_model,
                record.instructions_md,
                record.stop_rules_json,
                record.sort_order,
                record.updated_at,
                record.archived_at,
            ],
        )?;
        if updated == 0 {
            return Err(anyhow!("agent_template {} was not found", record.id));
        }
        Ok(())
    }

    pub fn next_agent_template_sort_order(&self, project_id: &str) -> Result<i64> {
        let connection = open_connection(&self.paths.app_db)?;
        Ok(connection.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM agent_templates WHERE project_id = ?1",
            [project_id],
            |row| row.get(0),
        )?)
    }

    // ---------------------------------------------------------------------------
    // Vault store methods
    // ---------------------------------------------------------------------------

    pub fn insert_vault_entry(&self, record: &NewVaultEntryRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "INSERT INTO vault_entries (id, scope, key, encrypted_value, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                record.id,
                record.scope,
                record.key,
                record.encrypted_value,
                record.description,
                record.created_at,
                record.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_vault_entry(&self, id: &str) -> Result<Option<VaultEntryRow>> {
        let connection = open_connection(&self.paths.app_db)?;
        connection
            .query_row(
                "SELECT id, scope, key, encrypted_value, description, created_at, updated_at
                 FROM vault_entries WHERE id = ?1",
                [id],
                map_vault_entry_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn get_vault_entry_by_scope_key(
        &self,
        scope: &str,
        key: &str,
    ) -> Result<Option<VaultEntryRow>> {
        let connection = open_connection(&self.paths.app_db)?;
        connection
            .query_row(
                "SELECT id, scope, key, encrypted_value, description, created_at, updated_at
                 FROM vault_entries WHERE scope = ?1 AND key = ?2",
                params![scope, key],
                map_vault_entry_row,
            )
            .optional()
            .map_err(Into::into)
    }

    /// List vault entries (metadata only, no encrypted_value).
    pub fn list_vault_entries(&self, scope: Option<&str>) -> Result<Vec<VaultEntry>> {
        let connection = open_connection(&self.paths.app_db)?;
        let rows: Vec<VaultEntry> = match scope {
            Some(s) => {
                let mut stmt = connection.prepare(
                    "SELECT id, scope, key, description, created_at, updated_at
                     FROM vault_entries WHERE scope = ?1
                     ORDER BY scope, key",
                )?;
                stmt.query_map([s], map_vault_entry_summary)?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            }
            None => {
                let mut stmt = connection.prepare(
                    "SELECT id, scope, key, description, created_at, updated_at
                     FROM vault_entries
                     ORDER BY scope, key",
                )?;
                stmt.query_map([], map_vault_entry_summary)?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            }
        };
        Ok(rows)
    }

    /// List vault entry rows with encrypted values (used internally by VaultService).
    pub fn list_vault_entry_rows(&self, scope: Option<&str>) -> Result<Vec<VaultEntryRow>> {
        let connection = open_connection(&self.paths.app_db)?;
        let rows: Vec<VaultEntryRow> = match scope {
            Some(s) => {
                let mut stmt = connection.prepare(
                    "SELECT id, scope, key, encrypted_value, description, created_at, updated_at
                     FROM vault_entries WHERE scope = ?1
                     ORDER BY scope, key",
                )?;
                stmt.query_map([s], map_vault_entry_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            }
            None => {
                let mut stmt = connection.prepare(
                    "SELECT id, scope, key, encrypted_value, description, created_at, updated_at
                     FROM vault_entries
                     ORDER BY scope, key",
                )?;
                stmt.query_map([], map_vault_entry_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            }
        };
        Ok(rows)
    }

    pub fn update_vault_entry(&self, record: &VaultEntryUpdateRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;

        let updated = match &record.encrypted_value {
            Some(enc) => connection.execute(
                "UPDATE vault_entries
                 SET encrypted_value = ?2, description = ?3, updated_at = ?4
                 WHERE id = ?1",
                params![record.id, enc, record.description, record.updated_at],
            )?,
            None => connection.execute(
                "UPDATE vault_entries
                 SET description = ?2, updated_at = ?3
                 WHERE id = ?1",
                params![record.id, record.description, record.updated_at],
            )?,
        };

        if updated == 0 {
            return Err(anyhow!("vault entry {} was not found", record.id));
        }
        Ok(())
    }

    pub fn delete_vault_entry(&self, id: &str) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute("DELETE FROM vault_entries WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn insert_vault_audit(&self, record: &NewVaultAuditRecord) -> Result<()> {
        let connection = open_connection(&self.paths.app_db)?;
        connection.execute(
            "INSERT INTO vault_audit_log (id, entry_id, action, actor, details_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                record.id,
                record.entry_id,
                record.action,
                record.actor,
                record.details_json,
                record.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_vault_audit(
        &self,
        entry_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<VaultAuditEntry>> {
        let connection = open_connection(&self.paths.app_db)?;
        let rows: Vec<VaultAuditEntry> = match entry_id {
            Some(eid) => {
                let mut stmt = connection.prepare(
                    "SELECT id, entry_id, action, actor, details_json, created_at
                     FROM vault_audit_log WHERE entry_id = ?1
                     ORDER BY created_at DESC LIMIT ?2",
                )?;
                stmt.query_map(params![eid, limit as i64], map_vault_audit_entry)?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            }
            None => {
                let mut stmt = connection.prepare(
                    "SELECT id, entry_id, action, actor, details_json, created_at
                     FROM vault_audit_log
                     ORDER BY created_at DESC LIMIT ?1",
                )?;
                stmt.query_map(params![limit as i64], map_vault_audit_entry)?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            }
        };
        Ok(rows)
    }
}

fn open_connection(path: &Path) -> Result<Connection> {
    let connection = Connection::open(path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    Ok(connection)
}

/// Open the app DB and attach the knowledge DB under the alias `knowledge`.
/// Use this for queries that need to JOIN across both databases (e.g. merge_queue → work_items).
fn open_app_connection_with_knowledge(paths: &AppPaths) -> Result<Connection> {
    let connection = open_connection(&paths.app_db)?;
    let knowledge_path = paths
        .knowledge_db
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("knowledge_db path is not valid UTF-8"))?;
    connection.execute_batch(&format!(
        "ATTACH DATABASE '{}' AS knowledge;",
        knowledge_path.replace('\'', "''")
    ))?;
    Ok(connection)
}

fn database_health(path: &Path) -> Result<DatabaseHealth> {
    let connection = open_connection(path)?;
    let schema_version = connection.query_row(
        "SELECT version FROM schema_migrations ORDER BY applied_at DESC, version DESC LIMIT 1",
        [],
        |row| row.get(0),
    )?;

    Ok(DatabaseHealth {
        available: true,
        schema_version: Some(schema_version),
    })
}

fn count_rows(connection: &Connection, table: &str) -> Result<i64> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    Ok(connection.query_row(&sql, [], |row| row.get(0))?)
}

fn count_sessions(connection: &Connection, runtime_state: &str) -> Result<i64> {
    Ok(connection.query_row(
        "SELECT COUNT(*) FROM sessions WHERE runtime_state = ?1 AND status = 'active'",
        [runtime_state],
        |row| row.get(0),
    )?)
}

fn count_live_runtime_sessions(connection: &Connection) -> Result<i64> {
    Ok(connection.query_row(
        "SELECT COUNT(*) FROM sessions WHERE status = 'active' AND runtime_state IN ('starting', 'running', 'stopping')",
        [],
        |row| row.get(0),
    )?)
}

fn count_interrupted_sessions(connection: &Connection) -> Result<i64> {
    Ok(connection.query_row(
        "SELECT COUNT(*) FROM sessions WHERE status = 'interrupted' OR runtime_state = 'interrupted'",
        [],
        |row| row.get(0),
    )?)
}

fn bool_to_sqlite(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

fn exists<P>(connection: &Connection, sql: &str, params: P) -> Result<bool>
where
    P: rusqlite::Params,
{
    let found = connection
        .query_row(sql, params, |_row| Ok(1_i64))
        .optional()?
        .is_some();
    Ok(found)
}

fn next_sort_order(connection: &Connection, sql: &str) -> Result<i64> {
    Ok(connection.query_row(sql, [], |row| row.get(0))?)
}

fn map_project_root_summary(row: &Row<'_>) -> rusqlite::Result<ProjectRootSummary> {
    Ok(ProjectRootSummary {
        id: row.get(0)?,
        project_id: row.get(1)?,
        label: row.get(2)?,
        path: row.get(3)?,
        git_root_path: row.get(4)?,
        remote_url: row.get(5)?,
        root_kind: row.get(6)?,
        sort_order: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        archived_at: row.get(10)?,
    })
}

fn map_account_summary(row: &Row<'_>) -> rusqlite::Result<AccountSummary> {
    Ok(AccountSummary {
        id: row.get(0)?,
        agent_kind: row.get(1)?,
        label: row.get(2)?,
        binary_path: row.get(3)?,
        config_root: row.get(4)?,
        env_preset_ref: row.get(5)?,
        is_default: row.get::<_, i64>(6)? != 0,
        status: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        default_safety_mode: row.get(10)?,
        default_launch_args_json: row.get(11)?,
        default_model: row.get(12)?,
    })
}

fn map_worktree_summary(row: &Row<'_>) -> rusqlite::Result<WorktreeSummary> {
    Ok(WorktreeSummary {
        id: row.get(0)?,
        project_id: row.get(1)?,
        project_root_id: row.get(2)?,
        branch_name: row.get(3)?,
        head_commit: row.get(4)?,
        base_ref: row.get(5)?,
        path: row.get(6)?,
        status: row.get(7)?,
        created_by_session_id: row.get(8)?,
        last_used_at: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        closed_at: row.get(12)?,
        active_session_count: row.get(13)?,
    })
}

fn map_session_spec_summary(row: &Row<'_>) -> rusqlite::Result<SessionSpecSummary> {
    Ok(SessionSpecSummary {
        id: row.get(0)?,
        project_id: row.get(1)?,
        project_root_id: row.get(2)?,
        worktree_id: row.get(3)?,
        work_item_id: row.get(4)?,
        account_id: row.get(5)?,
        agent_kind: row.get(6)?,
        cwd: row.get(7)?,
        command: row.get(8)?,
        args: parse_args_json(row.get_ref(9)?.as_str()?)?,
        env_preset_ref: row.get(10)?,
        origin_mode: row.get(11)?,
        current_mode: row.get(12)?,
        title: row.get(13)?,
        title_policy: row.get(14)?,
        restore_policy: row.get(15)?,
        initial_terminal_cols: row.get(16)?,
        initial_terminal_rows: row.get(17)?,
        context_bundle_ref: row.get(18)?,
        created_at: row.get(19)?,
        updated_at: row.get(20)?,
    })
}

fn map_session_summary(row: &Row<'_>) -> rusqlite::Result<SessionSummary> {
    Ok(SessionSummary {
        id: row.get(0)?,
        session_spec_id: row.get(1)?,
        project_id: row.get(2)?,
        project_root_id: row.get(3)?,
        worktree_id: row.get(4)?,
        worktree_branch: row.get(5)?,
        work_item_id: row.get(6)?,
        account_id: row.get(7)?,
        agent_kind: row.get(8)?,
        origin_mode: row.get(9)?,
        current_mode: row.get(10)?,
        title: row.get(11)?,
        title_source: row.get(12)?,
        runtime_state: row.get(13)?,
        status: row.get(14)?,
        activity_state: row.get(15)?,
        needs_input_reason: row.get(16)?,
        pty_owner_key: row.get(17)?,
        cwd: row.get(18)?,
        started_at: row.get(19)?,
        ended_at: row.get(20)?,
        last_output_at: row.get(21)?,
        last_attached_at: row.get(22)?,
        created_at: row.get(23)?,
        updated_at: row.get(24)?,
        archived_at: row.get(25)?,
        dispatch_group: row.get(26)?,
        live: false,
    })
}

fn map_work_item_summary(row: &Row<'_>) -> rusqlite::Result<WorkItemSummary> {
    Ok(WorkItemSummary {
        id: row.get(0)?,
        project_id: row.get(1)?,
        namespace: row.get(2)?,
        parent_id: row.get(3)?,
        root_work_item_id: row.get(4)?,
        callsign: row.get(5)?,
        child_sequence: row.get(6)?,
        title: row.get(7)?,
        description: row.get(8)?,
        acceptance_criteria: row.get(9)?,
        work_item_type: row.get(10)?,
        status: row.get(11)?,
        priority: row.get(12)?,
        created_by: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
        closed_at: row.get(16)?,
        child_count: row.get(17)?,
    })
}

fn map_document_summary(row: &Row<'_>) -> rusqlite::Result<DocumentSummary> {
    Ok(DocumentSummary {
        id: row.get(0)?,
        project_id: row.get(1)?,
        namespace: row.get(2)?,
        work_item_id: row.get(3)?,
        session_id: row.get(4)?,
        doc_type: row.get(5)?,
        title: row.get(6)?,
        slug: row.get(7)?,
        status: row.get(8)?,
        content_markdown: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        archived_at: row.get(12)?,
    })
}

fn map_planning_assignment_summary(row: &Row<'_>) -> rusqlite::Result<PlanningAssignmentSummary> {
    Ok(PlanningAssignmentSummary {
        id: row.get(0)?,
        work_item_id: row.get(1)?,
        cadence_type: row.get(2)?,
        cadence_key: row.get(3)?,
        created_by: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        removed_at: row.get(7)?,
    })
}

fn map_workflow_reconciliation_proposal_summary(
    row: &Row<'_>,
) -> rusqlite::Result<WorkflowReconciliationProposalSummary> {
    Ok(WorkflowReconciliationProposalSummary {
        id: row.get(0)?,
        source_session_id: row.get(1)?,
        work_item_id: row.get(2)?,
        target_entity_type: row.get(3)?,
        target_entity_id: row.get(4)?,
        proposal_type: row.get(5)?,
        proposed_change_payload: parse_json_value(row.get_ref(6)?.as_str()?)?,
        reason: row.get(7)?,
        confidence: row.get(8)?,
        status: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        resolved_at: row.get(12)?,
    })
}

fn map_workspace_state_record(row: &Row<'_>) -> rusqlite::Result<WorkspaceStateRecord> {
    Ok(WorkspaceStateRecord {
        id: row.get(0)?,
        scope: row.get(1)?,
        payload: parse_json_value(row.get_ref(2)?.as_str()?)?,
        saved_at: row.get(3)?,
    })
}

fn parse_args_json(value: &str) -> rusqlite::Result<Vec<String>> {
    serde_json::from_str(value)
        .map_err(|error| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error)))
}

fn parse_json_value(value: &str) -> rusqlite::Result<Value> {
    serde_json::from_str(value)
        .map_err(|error| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error)))
}

fn map_merge_queue_entry(row: &Row<'_>) -> rusqlite::Result<MergeQueueEntry> {
    let diff_stat_json: Option<String> = row.get(8)?;
    let conflict_files_json: Option<String> = row.get(9)?;
    let has_uncommitted: i64 = row.get(10)?;

    let diff_stat = diff_stat_json
        .as_deref()
        .and_then(|s| serde_json::from_str::<git::DiffStat>(s).ok());
    let conflict_files = conflict_files_json
        .as_deref()
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok());

    Ok(MergeQueueEntry {
        id: row.get(0)?,
        project_id: row.get(1)?,
        session_id: row.get(2)?,
        worktree_id: row.get(3)?,
        branch_name: row.get(4)?,
        base_ref: row.get(5)?,
        position: row.get(6)?,
        status: row.get(7)?,
        diff_stat,
        conflict_files,
        has_uncommitted_changes: has_uncommitted != 0,
        queued_at: row.get(11)?,
        merged_at: row.get(12)?,
        session_title: row.get(13)?,
        work_item_callsign: row.get(14)?,
    })
}

fn map_inbox_entry_summary(row: &Row<'_>) -> rusqlite::Result<InboxEntrySummary> {
    Ok(InboxEntrySummary {
        id: row.get(0)?,
        project_id: row.get(1)?,
        session_id: row.get(2)?,
        work_item_id: row.get(3)?,
        worktree_id: row.get(4)?,
        entry_type: row.get(5)?,
        title: row.get(6)?,
        summary: row.get(7)?,
        status: row.get(8)?,
        branch_name: row.get(9)?,
        diff_stat_json: row.get(10)?,
        metadata_json: row.get(11)?,
        read_at: row.get(12)?,
        resolved_at: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
        session_title: row.get(16)?,
        work_item_callsign: row.get(17)?,
    })
}

fn map_inbox_entry_detail(row: &Row<'_>) -> rusqlite::Result<InboxEntryDetail> {
    Ok(InboxEntryDetail {
        summary: map_inbox_entry_summary(row)?,
    })
}

fn map_agent_template_summary(row: &Row<'_>) -> rusqlite::Result<AgentTemplateSummary> {
    Ok(AgentTemplateSummary {
        id: row.get(0)?,
        project_id: row.get(1)?,
        template_key: row.get(2)?,
        label: row.get(3)?,
        origin_mode: row.get(4)?,
        default_model: row.get(5)?,
        instructions_md: row.get(6)?,
        stop_rules_json: row.get(7)?,
        sort_order: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        archived_at: row.get(11)?,
    })
}

fn map_agent_template_detail(row: &Row<'_>) -> rusqlite::Result<AgentTemplateDetail> {
    Ok(AgentTemplateDetail {
        summary: map_agent_template_summary(row)?,
    })
}

fn map_vault_entry_row(row: &Row<'_>) -> rusqlite::Result<VaultEntryRow> {
    Ok(VaultEntryRow {
        id: row.get(0)?,
        scope: row.get(1)?,
        key: row.get(2)?,
        encrypted_value: row.get(3)?,
        description: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn map_vault_entry_summary(row: &Row<'_>) -> rusqlite::Result<VaultEntry> {
    Ok(VaultEntry {
        id: row.get(0)?,
        scope: row.get(1)?,
        key: row.get(2)?,
        description: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn map_vault_audit_entry(row: &Row<'_>) -> rusqlite::Result<VaultAuditEntry> {
    Ok(VaultAuditEntry {
        id: row.get(0)?,
        entry_id: row.get(1)?,
        action: row.get(2)?,
        actor: row.get(3)?,
        details_json: row.get(4)?,
        created_at: row.get(5)?,
    })
}
