use project_commander_lib::db::{
    AppState, CreateDocumentInput, CreateWorkItemInput, DocumentRecord, ProjectRecord,
    ReparentRequest, UpdateDocumentInput, UpdateWorkItemInput, WorkItemRecord,
};
use project_commander_lib::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

const SERVER_NAME: &str = "project-commander";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PROTOCOL_VERSION: &str = "2025-03-26";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> AppResult<()> {
    let db_path = env::var_os("PROJECT_COMMANDER_DB_PATH").ok_or_else(|| {
        AppError::invalid_input(
            "PROJECT_COMMANDER_DB_PATH is required to start the Project Commander MCP server.",
        )
    })?;
    let state = AppState::from_database_path(PathBuf::from(db_path))?;

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    loop {
        let Some(message) = read_message(&mut reader)? else {
            break;
        };

        if let Some(response) = handle_message(&state, message)? {
            write_message(&mut writer, &response)?;
        }
    }

    Ok(())
}

fn handle_message(state: &AppState, message: Value) -> AppResult<Option<Value>> {
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Ok(None);
    };

    let id = message.get("id").cloned();
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));

    match method {
        "initialize" => {
            let protocol_version = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_PROTOCOL_VERSION);

            Ok(Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": protocol_version,
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": SERVER_NAME,
                        "version": SERVER_VERSION
                    },
                    "instructions": "Use Project Commander tools for the active project's work items and documents. Prefer these tools over WCP or unrelated trackers. The server is already bound to the active project."
                }
            })))
        }
        "notifications/initialized" => Ok(None),
        "ping" => Ok(Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {}
        }))),
        "tools/list" => Ok(Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": tool_definitions()
            }
        }))),
        "tools/call" => {
            let response_id = id.ok_or_else(|| AppError::invalid_input("tools/call request missing id"))?;
            let result = handle_tool_call(state, params);

            match result {
                Ok(tool_result) => Ok(Some(json!({
                    "jsonrpc": "2.0",
                    "id": response_id,
                    "result": tool_result
                }))),
                Err(error) => Ok(Some(json!({
                    "jsonrpc": "2.0",
                    "id": response_id,
                    "error": {
                        "code": error.code,
                        "message": error.message
                    }
                }))),
            }
        }
        _ => {
            if let Some(request_id) = id {
                Ok(Some(json!({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "error": {
                        "code": -32601,
                        "message": format!("method not found: {method}")
                    }
                })))
            } else {
                Ok(None)
            }
        }
    }
}

fn handle_tool_call(state: &AppState, params: Value) -> Result<Value, McpError> {
    let tool_name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| McpError::invalid_params("tool name is required"))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match tool_name {
        "current_project" => execute_tool(|| {
            let project = resolve_project(state)?;
            Ok(serde_json::to_value(project).expect("project should serialize"))
        }),
        "list_work_items" => execute_tool(|| {
            let args: ListWorkItemsArgs = decode_args(arguments)?;
            let project = resolve_project(state)?;
            let mut work_items = state.list_work_items(project.id)?;

            if let Some(status) = args.status {
                work_items.retain(|item| item.status == status);
            }

            if let Some(item_type) = args.item_type {
                work_items.retain(|item| item.item_type == item_type);
            }

            if args.open_only.unwrap_or(false) {
                work_items.retain(|item| item.status != "done");
            }

            if args.parent_only.unwrap_or(false) {
                work_items.retain(|item| item.parent_work_item_id.is_none());
            }

            let slim: Vec<Value> = work_items.into_iter().map(slim_work_item).collect();
            Ok(serde_json::to_value(slim).expect("work items should serialize"))
        }),
        "get_work_item" => execute_tool(|| {
            let args: GetWorkItemArgs = decode_args(arguments)?;
            let project = resolve_project(state)?;
            let work_item = resolve_work_item(state, args.id, args.call_sign)?;
            ensure_work_item_project(&work_item, &project)?;
            let linked_documents = state
                .list_documents(project.id)?
                .into_iter()
                .filter(|document| document.work_item_id == Some(work_item.id))
                .collect::<Vec<_>>();

            Ok(serde_json::to_value(WorkItemDetailOutput {
                work_item,
                linked_documents,
            })
            .expect("work item detail should serialize"))
        }),
        "create_work_item" => execute_tool(|| {
            let args: CreateWorkItemArgs = decode_args(arguments)?;
            let project = resolve_project(state)?;
            let work_item = state.create_work_item(CreateWorkItemInput {
                project_id: project.id,
                parent_work_item_id: args.parent_work_item_id,
                title: args.title,
                body: args.body.unwrap_or_default(),
                item_type: args.item_type.unwrap_or_else(|| "task".to_string()),
                status: args.status.unwrap_or_else(|| "backlog".to_string()),
            })?;

            Ok(serde_json::to_value(work_item).expect("work item should serialize"))
        }),
        "update_work_item" => execute_tool(|| {
            let args: UpdateWorkItemArgs = decode_args(arguments)?;
            let project = resolve_project(state)?;
            let existing = resolve_work_item(state, args.id, args.call_sign)?;
            ensure_work_item_project(&existing, &project)?;

            let clear_parent = args.clear_parent.unwrap_or(false);
            if args.title.is_none()
                && args.body.is_none()
                && args.item_type.is_none()
                && args.status.is_none()
                && args.parent_work_item_id.is_none()
                && !clear_parent
            {
                return Err(AppError::invalid_input("no changes provided for work item update"));
            }

            let work_item = state.update_work_item(UpdateWorkItemInput {
                id: existing.id,
                title: args.title.unwrap_or(existing.title),
                body: args.body.unwrap_or(existing.body),
                item_type: args.item_type.unwrap_or(existing.item_type),
                status: args.status.unwrap_or(existing.status),
            })?;

            let work_item = if let Some(parent_id) = args.parent_work_item_id {
                state.reparent_work_item(work_item.id, ReparentRequest::SetParent(parent_id))?
            } else if clear_parent {
                state.reparent_work_item(work_item.id, ReparentRequest::Detach)?
            } else {
                work_item
            };

            Ok(serde_json::to_value(work_item).expect("work item should serialize"))
        }),
        "close_work_item" => execute_tool(|| {
            let args: CloseWorkItemArgs = decode_args(arguments)?;
            let project = resolve_project(state)?;
            let existing = resolve_work_item(state, args.id, args.call_sign)?;
            ensure_work_item_project(&existing, &project)?;
            let work_item = state.update_work_item(UpdateWorkItemInput {
                id: existing.id,
                title: existing.title,
                body: existing.body,
                item_type: existing.item_type,
                status: "done".to_string(),
            })?;

            Ok(serde_json::to_value(work_item).expect("work item should serialize"))
        }),
        "list_documents" => execute_tool(|| {
            let args: ListDocumentsArgs = decode_args(arguments)?;
            let project = resolve_project(state)?;
            let mut documents = state.list_documents(project.id)?;

            if let Some(work_item_id) = args.work_item_id {
                documents.retain(|document| document.work_item_id == Some(work_item_id));
            }

            let slim: Vec<Value> = documents.into_iter().map(slim_document).collect();
            Ok(serde_json::to_value(slim).expect("documents should serialize"))
        }),
        "create_document" => execute_tool(|| {
            let args: CreateDocumentArgs = decode_args(arguments)?;
            let project = resolve_project(state)?;
            let document = state.create_document(CreateDocumentInput {
                project_id: project.id,
                work_item_id: args.work_item_id,
                title: args.title,
                body: args.body.unwrap_or_default(),
            })?;

            Ok(serde_json::to_value(document).expect("document should serialize"))
        }),
        "update_document" => execute_tool(|| {
            let args: UpdateDocumentArgs = decode_args(arguments)?;
            let project = resolve_project(state)?;
            let existing = state
                .list_documents(project.id)?
                .into_iter()
                .find(|document| document.id == args.id)
                .ok_or_else(|| {
                    AppError::invalid_input(format!(
                        "document #{} does not belong to the active project",
                        args.id
                    ))
                })?;

            if args.title.is_none()
                && args.body.is_none()
                && args.work_item_id.is_none()
                && !args.clear_work_item
            {
                return Err(AppError::invalid_input("no changes provided for document update"));
            }

            let work_item_id = if args.clear_work_item {
                None
            } else {
                args.work_item_id.or(existing.work_item_id)
            };

            let document = state.update_document(UpdateDocumentInput {
                id: existing.id,
                work_item_id,
                title: args.title.unwrap_or(existing.title),
                body: args.body.unwrap_or(existing.body),
            })?;

            Ok(serde_json::to_value(document).expect("document should serialize"))
        }),
        "delete_document" => execute_tool(|| {
            let args: DeleteDocumentArgs = decode_args(arguments)?;
            let project = resolve_project(state)?;
            let existing = state
                .list_documents(project.id)?
                .into_iter()
                .find(|document| document.id == args.id)
                .ok_or_else(|| {
                    AppError::invalid_input(format!(
                        "document #{} does not belong to the active project",
                        args.id
                    ))
                })?;

            state.delete_document(existing.id)?;

            Ok(json!({
                "deleted": true,
                "id": existing.id,
                "title": existing.title
            }))
        }),
        _ => Err(McpError::method_not_found(format!(
            "unknown tool: {tool_name}"
        ))),
    }
}

fn execute_tool<F>(action: F) -> Result<Value, McpError>
where
    F: FnOnce() -> AppResult<Value>,
{
    match action() {
        Ok(value) => Ok(json!({
            "content": [
                {
                    "type": "text",
                    "text": serde_json::to_string_pretty(&value)
                        .expect("tool result should serialize")
                }
            ],
            "structuredContent": value,
            "isError": false
        })),
        Err(error) => Ok(json!({
            "content": [
                {
                    "type": "text",
                    "text": error.message
                }
            ],
            "isError": true
        })),
    }
}

fn tool_definitions() -> Vec<Value> {
    vec![
        tool_definition(
            "current_project",
            "Return the active Project Commander project bound to this session.",
            json_schema_object(json!({}), vec![]),
            true,
        ),
        tool_definition(
            "list_work_items",
            "List work items for the active project, optionally filtered by status, type, or hierarchy.",
            json_schema_object(
                json!({
                    "status": {
                        "type": "string",
                        "enum": ["backlog", "in_progress", "blocked", "parked", "done"],
                        "description": "Optional status filter."
                    },
                    "itemType": {
                        "type": "string",
                        "enum": ["bug", "task", "feature", "note"],
                        "description": "Optional item type filter."
                    },
                    "parentOnly": {
                        "type": "boolean",
                        "description": "If true, return only top-level work items (no children)."
                    },
                    "openOnly": {
                        "type": "boolean",
                        "description": "If true, exclude work items with status 'done'."
                    }
                }),
                vec![],
            ),
            true,
        ),
        tool_definition(
            "get_work_item",
            "Return one work item plus any linked documents.",
            json_schema_object(
                json!({
                    "id": {
                        "type": "integer",
                        "description": "Work item DB id. Use callSign instead when you have it."
                    },
                    "callSign": {
                        "type": "string",
                        "description": "Call sign (e.g. PJTCMD-56 or PJTCMD-56.01) — preferred over id."
                    }
                }),
                vec![],
            ),
            true,
        ),
        tool_definition(
            "create_work_item",
            "Create a work item in the active project.",
            json_schema_object(
                json!({
                    "title": {
                        "type": "string",
                        "description": "Work item title."
                    },
                    "body": {
                        "type": "string",
                        "description": "Optional work item body."
                    },
                    "itemType": {
                        "type": "string",
                        "enum": ["bug", "task", "feature", "note"],
                        "description": "Work item type."
                    },
                    "status": {
                        "type": "string",
                        "enum": ["backlog", "in_progress", "blocked", "parked", "done"],
                        "description": "Initial work item status."
                    },
                    "parentWorkItemId": {
                        "type": "integer",
                        "description": "Optional parent work item id (creates a child work item)."
                    }
                }),
                vec!["title"],
            ),
            false,
        ),
        tool_definition(
            "update_work_item",
            "Update a work item's title, body, type, status, or parent.",
            json_schema_object(
                json!({
                    "id": {
                        "type": "integer",
                        "description": "Work item DB id. Use callSign instead when you have it."
                    },
                    "callSign": {
                        "type": "string",
                        "description": "Call sign (e.g. PJTCMD-56 or PJTCMD-56.01) — preferred over id."
                    },
                    "title": {
                        "type": "string",
                        "description": "Optional new title."
                    },
                    "body": {
                        "type": "string",
                        "description": "Optional new body."
                    },
                    "itemType": {
                        "type": "string",
                        "enum": ["bug", "task", "feature", "note"],
                        "description": "Optional new work item type."
                    },
                    "status": {
                        "type": "string",
                        "enum": ["backlog", "in_progress", "blocked", "parked", "done"],
                        "description": "Optional new status."
                    },
                    "parentWorkItemId": {
                        "type": "integer",
                        "description": "Optional new parent work item id."
                    },
                    "clearParent": {
                        "type": "boolean",
                        "description": "Set true to detach this work item from its parent (make it top-level)."
                    }
                }),
                vec![],
            ),
            false,
        ),
        tool_definition(
            "close_work_item",
            "Mark a work item done.",
            json_schema_object(
                json!({
                    "id": {
                        "type": "integer",
                        "description": "Work item DB id. Use callSign instead when you have it."
                    },
                    "callSign": {
                        "type": "string",
                        "description": "Call sign (e.g. PJTCMD-56 or PJTCMD-56.01) — preferred over id."
                    }
                }),
                vec![],
            ),
            false,
        ),
        tool_definition(
            "list_documents",
            "List documents for the active project, optionally filtered to a linked work item.",
            json_schema_object(
                json!({
                    "workItemId": {
                        "type": "integer",
                        "description": "Optional work item id filter."
                    }
                }),
                vec![],
            ),
            true,
        ),
        tool_definition(
            "create_document",
            "Create a project document or a document linked to a work item.",
            json_schema_object(
                json!({
                    "title": {
                        "type": "string",
                        "description": "Document title."
                    },
                    "body": {
                        "type": "string",
                        "description": "Optional document body."
                    },
                    "workItemId": {
                        "type": "integer",
                        "description": "Optional linked work item id."
                    }
                }),
                vec!["title"],
            ),
            false,
        ),
        tool_definition(
            "update_document",
            "Update a document's title, body, or linked work item.",
            json_schema_object(
                json!({
                    "id": {
                        "type": "integer",
                        "description": "Document id."
                    },
                    "title": {
                        "type": "string",
                        "description": "Optional new title."
                    },
                    "body": {
                        "type": "string",
                        "description": "Optional new body."
                    },
                    "workItemId": {
                        "type": "integer",
                        "description": "Optional new linked work item id."
                    },
                    "clearWorkItem": {
                        "type": "boolean",
                        "description": "Set true to unlink the document from any work item."
                    }
                }),
                vec!["id"],
            ),
            false,
        ),
        tool_definition(
            "delete_document",
            "Delete a document from the active project.",
            json_schema_object(
                json!({
                    "id": {
                        "type": "integer",
                        "description": "Document id."
                    }
                }),
                vec!["id"],
            ),
            false,
        ),
    ]
}

fn tool_definition(
    name: &str,
    description: &str,
    input_schema: Value,
    large_output: bool,
) -> Value {
    let mut tool = json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    });

    if large_output {
        tool["_meta"] = json!({
            "anthropic/maxResultSizeChars": 200000
        });
    }

    tool
}

fn json_schema_object(properties: Value, required: Vec<&str>) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn decode_args<T>(arguments: Value) -> AppResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(arguments)
        .map_err(|error| AppError::invalid_input(format!("invalid tool arguments: {error}")))
}

fn resolve_project(state: &AppState) -> AppResult<ProjectRecord> {
    if let Ok(project_id) = env::var("PROJECT_COMMANDER_PROJECT_ID") {
        if let Ok(parsed_project_id) = project_id.parse::<i64>() {
            return state.get_project(parsed_project_id);
        }
    }

    if let Some(root_path) = env::var_os("PROJECT_COMMANDER_ROOT_PATH") {
        if let Some(project) = state.find_project_by_path(Path::new(&root_path))? {
            return Ok(project);
        }
    }

    if let Ok(current_dir) = env::current_dir() {
        if let Some(project) = state.find_project_by_path(&current_dir)? {
            return Ok(project);
        }
    }

    Err(AppError::not_found(
        "no active project found. Launch the session from Project Commander before using the MCP server.",
    ))
}

fn ensure_work_item_project(item: &WorkItemRecord, project: &ProjectRecord) -> AppResult<()> {
    if item.project_id != project.id {
        return Err(AppError::invalid_input(format!(
            "work item #{} belongs to project #{} instead of the active project #{}",
            item.id, item.project_id, project.id
        )));
    }

    Ok(())
}

fn resolve_work_item(
    state: &AppState,
    id: Option<i64>,
    call_sign: Option<String>,
) -> AppResult<WorkItemRecord> {
    match (id, call_sign) {
        (Some(id), _) => state.get_work_item(id),
        (None, Some(ref cs)) => state.get_work_item_by_call_sign(cs),
        (None, None) => Err(AppError::invalid_input(
            "work item operation requires either 'id' or 'callSign'",
        )),
    }
}

fn read_message(reader: &mut impl BufRead) -> AppResult<Option<Value>> {
    let mut content_length = None;

    loop {
        let mut line = String::new();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|error| AppError::io(format!("failed to read MCP header: {error}")))?;

        if bytes_read == 0 {
            return Ok(None);
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);

        if trimmed.is_empty() {
            break;
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|error| AppError::io(format!("invalid Content-Length header: {error}")))?,
            );
        }
    }

    let content_length =
        content_length.ok_or_else(|| AppError::io("missing Content-Length header"))?;
    let mut payload = vec![0_u8; content_length];
    reader
        .read_exact(&mut payload)
        .map_err(|error| AppError::io(format!("failed to read MCP payload: {error}")))?;

    serde_json::from_slice(&payload)
        .map(Some)
        .map_err(|error| AppError::internal(format!("failed to decode MCP JSON payload: {error}")))
}

fn write_message(writer: &mut impl Write, message: &Value) -> AppResult<()> {
    let payload = serde_json::to_vec(message)
        .map_err(|error| AppError::internal(format!("failed to encode MCP response: {error}")))?;
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());

    writer
        .write_all(header.as_bytes())
        .and_then(|_| writer.write_all(&payload))
        .and_then(|_| writer.flush())
        .map_err(|error| AppError::io(format!("failed to write MCP response: {error}")))
}

#[derive(Debug)]
struct McpError {
    code: i64,
    message: String,
}

impl McpError {
    fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
        }
    }

    fn method_not_found(message: impl Into<String>) -> Self {
        Self {
            code: -32601,
            message: message.into(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkItemDetailOutput {
    work_item: WorkItemRecord,
    linked_documents: Vec<DocumentRecord>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListWorkItemsArgs {
    status: Option<String>,
    item_type: Option<String>,
    parent_only: Option<bool>,
    open_only: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetWorkItemArgs {
    id: Option<i64>,
    call_sign: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWorkItemArgs {
    title: String,
    body: Option<String>,
    item_type: Option<String>,
    status: Option<String>,
    parent_work_item_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateWorkItemArgs {
    id: Option<i64>,
    call_sign: Option<String>,
    title: Option<String>,
    body: Option<String>,
    item_type: Option<String>,
    status: Option<String>,
    parent_work_item_id: Option<i64>,
    clear_parent: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloseWorkItemArgs {
    id: Option<i64>,
    call_sign: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListDocumentsArgs {
    work_item_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateDocumentArgs {
    title: String,
    body: Option<String>,
    work_item_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateDocumentArgs {
    id: i64,
    title: Option<String>,
    body: Option<String>,
    work_item_id: Option<i64>,
    clear_work_item: bool,
}

#[derive(Deserialize)]
struct DeleteDocumentArgs {
    id: i64,
}

/// Strip body and metadata from a work item for list responses.
/// Callers that need the full body should use get_work_item.
fn slim_work_item(item: WorkItemRecord) -> Value {
    json!({
        "id": item.id,
        "callSign": item.call_sign,
        "title": item.title,
        "status": item.status,
        "itemType": item.item_type,
        "parentWorkItemId": item.parent_work_item_id,
        "childNumber": item.child_number,
    })
}

/// Strip body and metadata from a document for list responses.
/// Callers that need the full body should use update_document or get_work_item.
fn slim_document(doc: DocumentRecord) -> Value {
    json!({
        "id": doc.id,
        "title": doc.title,
        "workItemId": doc.work_item_id,
    })
}
