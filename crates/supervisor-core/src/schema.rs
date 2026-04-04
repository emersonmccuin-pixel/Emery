use std::path::Path;

use anyhow::Result;
use rusqlite::{Connection, params};

pub const APP_SCHEMA_VERSION: &str = "0001_initial";
pub const KNOWLEDGE_SCHEMA_VERSION: &str = "0001_initial";

const APP_SCHEMA_SQL: &str = include_str!("migrations/app/0001_initial.sql");
const KNOWLEDGE_SCHEMA_SQL: &str = include_str!("migrations/knowledge/0001_initial.sql");

pub fn migrate_app_db(path: &Path) -> Result<()> {
    migrate(path, APP_SCHEMA_VERSION, APP_SCHEMA_SQL)
}

pub fn migrate_knowledge_db(path: &Path) -> Result<()> {
    migrate(path, KNOWLEDGE_SCHEMA_VERSION, KNOWLEDGE_SCHEMA_SQL)
}

fn migrate(path: &Path, version: &str, sql: &str) -> Result<()> {
    let mut connection = Connection::open(path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;

    let already_applied = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
        [version],
        |row| row.get::<_, i64>(0),
    )?;

    if already_applied == 0 {
        let tx = connection.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at) VALUES(?1, unixepoch())",
            params![version],
        )?;
        tx.commit()?;
    }

    Ok(())
}
