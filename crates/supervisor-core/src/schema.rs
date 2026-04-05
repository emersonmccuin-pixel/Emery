use std::path::Path;

use anyhow::Result;
use rusqlite::{Connection, params};

const APP_MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_initial",
        include_str!("migrations/app/0001_initial.sql"),
    ),
    (
        "0002_session_spec_fields",
        include_str!("migrations/app/0002_session_spec_fields.sql"),
    ),
    (
        "0003_planning_assignment_constraints",
        include_str!("migrations/app/0003_planning_assignment_constraints.sql"),
    ),
    (
        "0004_workflow_reconciliation_shape",
        include_str!("migrations/app/0004_workflow_reconciliation_shape.sql"),
    ),
    (
        "0005_merge_queue",
        include_str!("migrations/app/0005_merge_queue.sql"),
    ),
];
const KNOWLEDGE_MIGRATIONS: &[(&str, &str)] = &[(
    "0001_initial",
    include_str!("migrations/knowledge/0001_initial.sql"),
)];

pub fn migrate_app_db(path: &Path) -> Result<()> {
    migrate(path, APP_MIGRATIONS)
}

pub fn migrate_knowledge_db(path: &Path) -> Result<()> {
    migrate(path, KNOWLEDGE_MIGRATIONS)
}

fn migrate(path: &Path, migrations: &[(&str, &str)]) -> Result<()> {
    let mut connection = Connection::open(path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;

    for (version, sql) in migrations {
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
    }

    Ok(())
}
