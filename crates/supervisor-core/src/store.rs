use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;

use crate::bootstrap::AppPaths;
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
            live_session_count: count_sessions(&connection, "running")?,
            restorable_workspace_count: count_rows(&connection, "workspace_state")?,
            interrupted_session_count: count_sessions(&connection, "interrupted")?,
        })
    }
}

fn open_connection(path: &std::path::Path) -> Result<Connection> {
    let connection = Connection::open(path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    Ok(connection)
}

fn database_health(path: &std::path::Path) -> Result<DatabaseHealth> {
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
