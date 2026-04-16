use crate::db::{touch_project, CreateDocumentInput, DocumentRecord, UpdateDocumentInput};
use crate::error::{AppError, AppResult};
use crate::work_item_store;
use rusqlite::{params, Connection};

pub(crate) fn list_records_by_project(
    connection: &Connection,
    project_id: i64,
) -> Result<Vec<DocumentRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              work_item_id,
              title,
              body,
              created_at,
              updated_at
            FROM documents
            WHERE project_id = ?1
            ORDER BY updated_at DESC, id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare document query: {error}"))?;

    let rows = statement
        .query_map([project_id], |row| {
            Ok(DocumentRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                work_item_id: row.get(2)?,
                title: row.get(3)?,
                body: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|error| format!("failed to load documents: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map documents: {error}"))
}

pub(crate) fn list_records_for_work_item(
    connection: &Connection,
    work_item_id: i64,
) -> Result<Vec<DocumentRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              work_item_id,
              title,
              body,
              created_at,
              updated_at
            FROM documents
            WHERE work_item_id = ?1
            ORDER BY updated_at DESC, id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare work-item document query: {error}"))?;

    let rows = statement
        .query_map([work_item_id], |row| {
            Ok(DocumentRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                work_item_id: row.get(2)?,
                title: row.get(3)?,
                body: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|error| format!("failed to load work-item documents: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map work-item documents: {error}"))
}

pub(crate) fn load_record_by_id(
    connection: &Connection,
    id: i64,
) -> Result<DocumentRecord, String> {
    connection
        .query_row(
            "
            SELECT
              id,
              project_id,
              work_item_id,
              title,
              body,
              created_at,
              updated_at
            FROM documents
            WHERE id = ?1
            ",
            [id],
            |row| {
                Ok(DocumentRecord {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    work_item_id: row.get(2)?,
                    title: row.get(3)?,
                    body: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .map_err(|error| format!("failed to load document: {error}"))
}

pub(crate) fn create_record(
    connection: &Connection,
    input: CreateDocumentInput,
) -> AppResult<DocumentRecord> {
    let title = input.title.trim();
    let body = input.body.trim();

    if title.is_empty() {
        return Err(AppError::invalid_input("document title is required"));
    }

    let work_item_id = validate_work_item_link(connection, input.project_id, input.work_item_id)?;

    connection
        .execute(
            "INSERT INTO documents (project_id, work_item_id, title, body) VALUES (?1, ?2, ?3, ?4)",
            params![input.project_id, work_item_id, title, body],
        )
        .map_err(|error| format!("failed to create document: {error}"))?;

    touch_project(connection, input.project_id)?;
    load_record_by_id(connection, connection.last_insert_rowid()).map_err(Into::into)
}

pub(crate) fn update_record(
    connection: &Connection,
    input: UpdateDocumentInput,
) -> AppResult<DocumentRecord> {
    let title = input.title.trim();
    let body = input.body.trim();

    if title.is_empty() {
        return Err(AppError::invalid_input("document title is required"));
    }

    let existing = load_record_by_id(connection, input.id)?;
    let work_item_id =
        validate_work_item_link(connection, existing.project_id, input.work_item_id)?;

    connection
        .execute(
            "UPDATE documents
             SET work_item_id = ?1,
                 title = ?2,
                 body = ?3,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?4",
            params![work_item_id, title, body, input.id],
        )
        .map_err(|error| format!("failed to update document: {error}"))?;

    touch_project(connection, existing.project_id)?;
    load_record_by_id(connection, input.id).map_err(Into::into)
}

pub(crate) fn delete_record(connection: &Connection, id: i64) -> AppResult<()> {
    let existing = load_record_by_id(connection, id)?;

    connection
        .execute("DELETE FROM documents WHERE id = ?1", [id])
        .map_err(|error| format!("failed to delete document: {error}"))?;

    touch_project(connection, existing.project_id)?;
    Ok(())
}

fn validate_work_item_link(
    connection: &Connection,
    project_id: i64,
    work_item_id: Option<i64>,
) -> Result<Option<i64>, String> {
    let Some(work_item_id) = work_item_id else {
        return Ok(None);
    };

    let work_item = work_item_store::load_record_by_id(connection, work_item_id)?;

    if work_item.project_id != project_id {
        return Err("linked work item must belong to the same project".to_string());
    }

    Ok(Some(work_item_id))
}
