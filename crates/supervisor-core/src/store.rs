use std::path::Path;

use anyhow::{Result, anyhow};
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};
use serde::Serialize;

use crate::bootstrap::AppPaths;
use crate::models::{
    NewSessionRecord, ProjectDetail, ProjectRootSummary, ProjectSummary, SessionArtifactRecord,
    SessionListFilter, SessionSummary,
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

    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>> {
        let connection = open_connection(&self.paths.app_db)?;
        let mut statement = connection.prepare(
            "SELECT
                p.id,
                p.name,
                p.slug,
                p.sort_order,
                p.default_account_id,
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
                p.id, p.name, p.slug, p.sort_order, p.default_account_id, p.created_at, p.updated_at, p.archived_at
             ORDER BY p.sort_order ASC, p.created_at ASC",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(ProjectSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                slug: row.get(2)?,
                sort_order: row.get(3)?,
                default_account_id: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                archived_at: row.get(7)?,
                root_count: row.get(8)?,
                live_session_count: row.get(9)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_project(&self, project_id: &str) -> Result<Option<ProjectDetail>> {
        let connection = open_connection(&self.paths.app_db)?;
        let project = connection
            .query_row(
                "SELECT id, name, slug, sort_order, default_account_id, settings_json, created_at, updated_at, archived_at
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

    pub fn list_sessions(&self, filter: &SessionListFilter) -> Result<Vec<SessionSummary>> {
        let connection = open_connection(&self.paths.app_db)?;
        let mut sql = String::from(
            "SELECT
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
                runtime_state,
                status,
                activity_state,
                needs_input_reason,
                pty_owner_key,
                cwd,
                started_at,
                ended_at,
                last_output_at,
                last_attached_at,
                created_at,
                updated_at,
                archived_at
             FROM sessions
             WHERE 1 = 1",
        );

        let mut values = Vec::new();

        if let Some(project_id) = filter.project_id.as_deref() {
            sql.push_str(" AND project_id = ?");
            values.push(project_id.to_string());
        }

        if let Some(status) = filter.status.as_deref() {
            sql.push_str(" AND status = ?");
            values.push(status.to_string());
        }

        if let Some(runtime_state) = filter.runtime_state.as_deref() {
            sql.push_str(" AND runtime_state = ?");
            values.push(runtime_state.to_string());
        }

        sql.push_str(" ORDER BY created_at DESC");

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
                    runtime_state,
                    status,
                    activity_state,
                    needs_input_reason,
                    pty_owner_key,
                    cwd,
                    started_at,
                    ended_at,
                    last_output_at,
                    last_attached_at,
                    created_at,
                    updated_at,
                    archived_at
                 FROM sessions
                 WHERE id = ?1",
                [session_id],
                map_session_summary,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn insert_session(
        &self,
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
                title_policy,
                restore_policy,
                initial_terminal_cols,
                initial_terminal_rows,
                context_bundle_artifact_id,
                created_at,
                updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, NULL, ?17, ?18
             )",
            params![
                record.session_spec_id,
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
                record.title_policy,
                record.restore_policy,
                record.initial_terminal_cols,
                record.initial_terminal_rows,
                record.created_at,
                record.updated_at,
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
                archived_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, 1, ?13, ?14, ?15, NULL, ?16, ?17, ?18, ?19, ?20, NULL, NULL, NULL, ?21, ?22, NULL
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
                 ended_at = COALESCE(ended_at, ?4),
                 updated_at = ?4
             WHERE id = ?1",
            params![session_id, runtime_state, status, ended_at],
        )?;
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

    fn list_project_roots(&self, project_id: &str) -> Result<Vec<ProjectRootSummary>> {
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

        let rows = statement.query_map([project_id], |row| {
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
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }
}

fn open_connection(path: &Path) -> Result<Connection> {
    let connection = Connection::open(path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
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

fn map_session_summary(row: &Row<'_>) -> rusqlite::Result<SessionSummary> {
    Ok(SessionSummary {
        id: row.get(0)?,
        session_spec_id: row.get(1)?,
        project_id: row.get(2)?,
        project_root_id: row.get(3)?,
        worktree_id: row.get(4)?,
        work_item_id: row.get(5)?,
        account_id: row.get(6)?,
        agent_kind: row.get(7)?,
        origin_mode: row.get(8)?,
        current_mode: row.get(9)?,
        title: row.get(10)?,
        title_source: row.get(11)?,
        runtime_state: row.get(12)?,
        status: row.get(13)?,
        activity_state: row.get(14)?,
        needs_input_reason: row.get(15)?,
        pty_owner_key: row.get(16)?,
        cwd: row.get(17)?,
        started_at: row.get(18)?,
        ended_at: row.get(19)?,
        last_output_at: row.get(20)?,
        last_attached_at: row.get(21)?,
        created_at: row.get(22)?,
        updated_at: row.get(23)?,
        archived_at: row.get(24)?,
        live: false,
    })
}
