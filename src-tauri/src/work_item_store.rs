use crate::db::{
    touch_project, CreateWorkItemInput, ReparentRequest, UpdateWorkItemInput, WorkItemRecord,
};
use crate::error::{AppError, AppResult};
use crate::project_store;
use rusqlite::{params, Connection};

struct AssignedWorkItemIdentifier {
    parent_work_item_id: Option<i64>,
    sequence_number: i64,
    child_number: Option<i64>,
    call_sign: String,
}

pub(crate) fn list_in_progress_records(
    connection: &Connection,
) -> Result<Vec<WorkItemRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              parent_work_item_id,
              call_sign,
              title,
              body,
              item_type,
              status,
              sequence_number,
              child_number,
              created_at,
              updated_at
            FROM work_items
            WHERE status = 'in_progress'
            ORDER BY sequence_number ASC, child_number ASC, updated_at DESC, id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare in-progress work item query: {error}"))?;

    let rows = statement
        .query_map([], map_record)
        .map_err(|error| format!("failed to load in-progress work items: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map in-progress work items: {error}"))
}

pub(crate) fn list_records_by_project(
    connection: &Connection,
    project_id: i64,
) -> Result<Vec<WorkItemRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              parent_work_item_id,
              call_sign,
              title,
              body,
              item_type,
              status,
              sequence_number,
              child_number,
              created_at,
              updated_at
            FROM work_items
            WHERE project_id = ?1
            ORDER BY
              CASE status
                WHEN 'in_progress' THEN 0
                WHEN 'blocked' THEN 1
                WHEN 'backlog' THEN 2
                WHEN 'parked' THEN 3
                WHEN 'done' THEN 4
                ELSE 5
              END,
              sequence_number ASC,
              CASE WHEN child_number IS NULL THEN 0 ELSE 1 END ASC,
              child_number ASC,
              updated_at DESC,
              id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare work item query: {error}"))?;

    let rows = statement
        .query_map([project_id], map_record)
        .map_err(|error| format!("failed to load work items: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map work items: {error}"))
}

pub(crate) fn load_record_by_id(
    connection: &Connection,
    id: i64,
) -> Result<WorkItemRecord, String> {
    connection
        .query_row(
            "
            SELECT
              id,
              project_id,
              parent_work_item_id,
              call_sign,
              title,
              body,
              item_type,
              status,
              sequence_number,
              child_number,
              created_at,
              updated_at
            FROM work_items
            WHERE id = ?1
            ",
            [id],
            map_record,
        )
        .map_err(|error| format!("failed to load work item: {error}"))
}

pub(crate) fn load_record_by_call_sign(
    connection: &Connection,
    call_sign: &str,
) -> Result<WorkItemRecord, String> {
    connection
        .query_row(
            "
            SELECT
              id,
              project_id,
              parent_work_item_id,
              call_sign,
              title,
              body,
              item_type,
              status,
              sequence_number,
              child_number,
              created_at,
              updated_at
            FROM work_items
            WHERE call_sign = ?1
            ",
            [call_sign],
            map_record,
        )
        .map_err(|error| format!("failed to load work item by call sign: {error}"))
}

pub(crate) fn create_record(
    connection: &Connection,
    input: CreateWorkItemInput,
) -> AppResult<WorkItemRecord> {
    let title = input.title.trim();
    let body = input.body.trim();
    let item_type = normalize_type(&input.item_type)?;
    let status = normalize_status(&input.status)?;

    if title.is_empty() {
        return Err(AppError::invalid_input("work item title is required"));
    }

    let project = project_store::load_record_by_id(connection, input.project_id)?;
    let project_prefix =
        project_store::ensure_work_item_prefix(connection, project.id, &project.name)?;
    let identifier = assign_next_identifier(
        connection,
        input.project_id,
        &project_prefix,
        input.parent_work_item_id,
    )?;

    connection
        .execute(
            "INSERT INTO work_items (
                project_id,
                parent_work_item_id,
                sequence_number,
                child_number,
                call_sign,
                title,
                body,
                item_type,
                status
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                input.project_id,
                identifier.parent_work_item_id,
                identifier.sequence_number,
                identifier.child_number,
                identifier.call_sign,
                title,
                body,
                item_type,
                status
            ],
        )
        .map_err(|error| format!("failed to create work item: {error}"))?;

    touch_project(connection, input.project_id)?;
    load_record_by_id(connection, connection.last_insert_rowid()).map_err(Into::into)
}

pub(crate) fn update_record(
    connection: &Connection,
    input: UpdateWorkItemInput,
) -> AppResult<WorkItemRecord> {
    let title = input.title.trim();
    let body = input.body.trim();
    let item_type = normalize_type(&input.item_type)?;
    let status = normalize_status(&input.status)?;

    if title.is_empty() {
        return Err(AppError::invalid_input("work item title is required"));
    }

    let existing = load_record_by_id(connection, input.id)?;

    connection
        .execute(
            "UPDATE work_items
             SET title = ?1,
                 body = ?2,
                 item_type = ?3,
                 status = ?4,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?5",
            params![title, body, item_type, status, input.id],
        )
        .map_err(|error| format!("failed to update work item: {error}"))?;

    touch_project(connection, existing.project_id)?;
    load_record_by_id(connection, input.id).map_err(Into::into)
}

pub(crate) fn reparent_record(
    connection: &Connection,
    id: i64,
    request: ReparentRequest,
) -> AppResult<WorkItemRecord> {
    let existing = load_record_by_id(connection, id)?;

    let target_parent: Option<i64> = match request {
        ReparentRequest::Detach => None,
        ReparentRequest::SetParent(parent_id) => {
            if parent_id == existing.id {
                return Err(AppError::invalid_input(
                    "work item cannot be its own parent",
                ));
            }
            Some(parent_id)
        }
    };

    if target_parent == existing.parent_work_item_id {
        return Ok(existing);
    }

    if target_parent.is_some() {
        let child_count = connection
            .query_row(
                "SELECT COUNT(*) FROM work_items WHERE parent_work_item_id = ?1",
                [existing.id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| format!("failed to inspect child work items: {error}"))?;
        if child_count > 0 {
            return Err(AppError::invalid_input(
                "cannot reparent a work item that has child work items",
            ));
        }
    }

    let project = project_store::load_record_by_id(connection, existing.project_id)?;
    let project_prefix =
        project_store::ensure_work_item_prefix(connection, project.id, &project.name)?;
    let identifier = assign_next_identifier(
        connection,
        existing.project_id,
        &project_prefix,
        target_parent,
    )?;

    connection
        .execute(
            "UPDATE work_items
             SET parent_work_item_id = ?1,
                 sequence_number = ?2,
                 child_number = ?3,
                 call_sign = ?4,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?5",
            params![
                identifier.parent_work_item_id,
                identifier.sequence_number,
                identifier.child_number,
                identifier.call_sign,
                existing.id,
            ],
        )
        .map_err(|error| format!("failed to reparent work item: {error}"))?;

    touch_project(connection, existing.project_id)?;
    load_record_by_id(connection, existing.id).map_err(Into::into)
}

pub(crate) fn delete_record(connection: &mut Connection, id: i64) -> AppResult<()> {
    let existing = load_record_by_id(connection, id)?;
    let child_count = connection
        .query_row(
            "SELECT COUNT(*) FROM work_items WHERE parent_work_item_id = ?1",
            [id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("failed to inspect child work items: {error}"))?;

    if child_count > 0 {
        return Err(AppError::invalid_input(
            "cannot delete a parent work item while child work items still exist",
        ));
    }

    let tx = connection
        .transaction()
        .map_err(|error| format!("failed to start delete transaction: {error}"))?;
    tx.execute(
        "DELETE FROM work_item_vectors WHERE work_item_id = ?1",
        [id],
    )
    .map_err(|error| format!("failed to delete work item vector row: {error}"))?;
    tx.execute("DELETE FROM work_items WHERE id = ?1", [id])
        .map_err(|error| format!("failed to delete work item: {error}"))?;
    tx.commit()
        .map_err(|error| format!("failed to commit work item delete: {error}"))?;

    touch_project(connection, existing.project_id)?;
    Ok(())
}

pub(crate) fn reconcile_identifiers(connection: &Connection) -> Result<(), String> {
    let mut project_statement = connection
        .prepare(
            "
            SELECT id, name, COALESCE(work_item_prefix, '')
            FROM projects
            ORDER BY id ASC
            ",
        )
        .map_err(|error| format!("failed to prepare project work item reconcile query: {error}"))?;
    let projects = project_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("failed to load project work item reconcile rows: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode project work item reconcile rows: {error}"))?;

    for (project_id, project_name, current_prefix) in projects {
        let project_prefix = if current_prefix.trim().is_empty() {
            project_store::ensure_work_item_prefix(connection, project_id, &project_name)?
        } else {
            current_prefix
        };
        let mut statement = connection
            .prepare(
                "
                SELECT id, parent_work_item_id
                FROM work_items
                WHERE project_id = ?1
                  AND (sequence_number IS NULL OR sequence_number != 0 OR parent_work_item_id IS NOT NULL)
                ORDER BY sequence_number ASC, child_number ASC, id ASC
                ",
            )
            .map_err(|error| format!("failed to prepare work item reconcile query: {error}"))?;
        let rows = statement
            .query_map([project_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?))
            })
            .map_err(|error| format!("failed to load work item reconcile rows: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to decode work item reconcile rows: {error}"))?;

        let mut parent_sequence_number = 0_i64;

        for (work_item_id, _) in rows.iter().copied().filter(|(_, parent)| parent.is_none()) {
            parent_sequence_number += 1;
            let call_sign = format!("{project_prefix}-{parent_sequence_number}");
            connection
                .execute(
                    "
                    UPDATE work_items
                    SET sequence_number = ?1,
                        child_number = NULL,
                        call_sign = ?2
                    WHERE id = ?3
                    ",
                    params![parent_sequence_number, call_sign, work_item_id],
                )
                .map_err(|error| {
                    format!("failed to reconcile top-level work item identifiers: {error}")
                })?;

            let mut child_statement = connection
                .prepare(
                    "
                    SELECT id
                    FROM work_items
                    WHERE parent_work_item_id = ?1
                    ORDER BY child_number ASC, id ASC
                    ",
                )
                .map_err(|error| {
                    format!("failed to prepare child work item reconcile query: {error}")
                })?;
            let child_ids = child_statement
                .query_map([work_item_id], |row| row.get::<_, i64>(0))
                .map_err(|error| format!("failed to load child work item reconcile rows: {error}"))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| {
                    format!("failed to decode child work item reconcile rows: {error}")
                })?;

            for (index, child_id) in child_ids.into_iter().enumerate() {
                let child_number = i64::try_from(index + 1)
                    .map_err(|_| "child work item numbering overflowed".to_string())?;
                let child_call_sign = format!("{call_sign}.{child_number:02}");
                connection
                    .execute(
                        "
                        UPDATE work_items
                        SET sequence_number = ?1,
                            child_number = ?2,
                            call_sign = ?3
                        WHERE id = ?4
                        ",
                        params![
                            parent_sequence_number,
                            child_number,
                            child_call_sign,
                            child_id
                        ],
                    )
                    .map_err(|error| {
                        format!("failed to reconcile child work item identifiers: {error}")
                    })?;
            }
        }
    }

    Ok(())
}

fn map_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkItemRecord> {
    Ok(WorkItemRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        parent_work_item_id: row.get(2)?,
        call_sign: row.get(3)?,
        sequence_number: row.get(8)?,
        child_number: row.get(9)?,
        title: row.get(4)?,
        body: row.get(5)?,
        item_type: row.get(6)?,
        status: row.get(7)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn assign_next_identifier(
    connection: &Connection,
    project_id: i64,
    project_prefix: &str,
    parent_work_item_id: Option<i64>,
) -> Result<AssignedWorkItemIdentifier, String> {
    let Some(parent_work_item_id) = parent_work_item_id else {
        let sequence_number = connection
            .query_row(
                "
                SELECT COALESCE(MAX(sequence_number), 0) + 1
                FROM work_items
                WHERE project_id = ?1 AND parent_work_item_id IS NULL
                ",
                [project_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| format!("failed to assign work item call sign: {error}"))?;

        return Ok(AssignedWorkItemIdentifier {
            parent_work_item_id: None,
            sequence_number,
            child_number: None,
            call_sign: format!("{project_prefix}-{sequence_number}"),
        });
    };

    let parent = load_record_by_id(connection, parent_work_item_id)?;

    if parent.project_id != project_id {
        return Err("parent work item must belong to the same project".to_string());
    }

    if parent.parent_work_item_id.is_some() {
        return Err("child work items cannot own child work items".to_string());
    }

    let child_number = connection
        .query_row(
            "
            SELECT COALESCE(MAX(child_number), 0) + 1
            FROM work_items
            WHERE parent_work_item_id = ?1
            ",
            [parent_work_item_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("failed to assign child work item call sign: {error}"))?;

    Ok(AssignedWorkItemIdentifier {
        parent_work_item_id: Some(parent_work_item_id),
        sequence_number: parent.sequence_number,
        child_number: Some(child_number),
        call_sign: format!("{}.{child_number:02}", parent.call_sign),
    })
}

fn normalize_type(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_lowercase();

    match normalized.as_str() {
        "bug" | "task" | "feature" | "note" => Ok(normalized),
        _ => Err("work item type must be bug, task, feature, or note".to_string()),
    }
}

fn normalize_status(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_lowercase();

    match normalized.as_str() {
        "backlog" | "in_progress" | "blocked" | "parked" | "done" => Ok(normalized),
        _ => Err(
            "work item status must be backlog, in_progress, blocked, parked, or done".to_string(),
        ),
    }
}
