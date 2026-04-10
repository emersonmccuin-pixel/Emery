use crate::db::{DocumentRecord, ProjectRecord, WorkItemRecord};
use crate::session_api::ProjectSessionTarget;
use crate::supervisor_api::{
    CreateProjectDocumentInput, CreateProjectWorkItemInput, ListProjectDocumentsInput,
    ListProjectWorkItemsInput, ProjectDocumentTarget, ProjectWorkItemTarget, SessionBriefOutput,
    UpdateProjectDocumentInput, UpdateProjectWorkItemInput, WorkItemDetailOutput,
};
use reqwest::blocking::Client;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};
use std::time::Duration;

const MCP_SERVER_NAME: &str = "project-commander";
const MCP_SERVER_VERSION: &str = "0.1.0";
const MCP_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

pub fn run_supervisor_mcp_stdio(
    port: u16,
    token: String,
    project_id: i64,
    worktree_id: Option<i64>,
) -> Result<(), String> {
    let client = SupervisorMcpClient::new(port, token, project_id, worktree_id, None)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    loop {
        let Some(message) = read_message(&mut reader)? else {
            break;
        };

        if let Some(response) = handle_message(&client, message)? {
            write_message(&mut writer, &response)?;
        }
    }

    Ok(())
}

pub fn run_supervisor_mcp_stdio_with_session(
    port: u16,
    token: String,
    project_id: i64,
    worktree_id: Option<i64>,
    session_id: Option<i64>,
) -> Result<(), String> {
    let client = SupervisorMcpClient::new(port, token, project_id, worktree_id, session_id)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    loop {
        let Some(message) = read_message(&mut reader)? else {
            break;
        };

        if let Some(response) = handle_message(&client, message)? {
            write_message(&mut writer, &response)?;
        }
    }

    Ok(())
}

struct SupervisorMcpClient {
    client: Client,
    base_url: String,
    token: String,
    project_id: i64,
    worktree_id: Option<i64>,
    session_id: Option<i64>,
}

impl SupervisorMcpClient {
    fn new(
        port: u16,
        token: String,
        project_id: i64,
        worktree_id: Option<i64>,
        session_id: Option<i64>,
    ) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(MCP_REQUEST_TIMEOUT)
            .build()
            .map_err(|error| format!("failed to build Project Commander MCP client: {error}"))?;

        Ok(Self {
            client,
            base_url: format!("http://127.0.0.1:{port}"),
            token,
            project_id,
            worktree_id,
            session_id,
        })
    }

    fn current_project(&self) -> Result<ProjectRecord, String> {
        self.post("project/current", &ProjectSessionTarget {
            project_id: self.project_id,
            worktree_id: self.worktree_id,
        })
    }

    fn session_brief(&self) -> Result<SessionBriefOutput, String> {
        self.post(
            "project/session-brief",
            &ProjectSessionTarget {
                project_id: self.project_id,
                worktree_id: self.worktree_id,
            },
        )
    }

    fn list_work_items(&self, status: Option<String>) -> Result<Vec<WorkItemRecord>, String> {
        self.post(
            "work-item/list",
            &ListProjectWorkItemsInput {
                project_id: self.project_id,
                status,
            },
        )
    }

    fn get_work_item(&self, id: i64) -> Result<WorkItemDetailOutput, String> {
        self.post(
            "work-item/get",
            &ProjectWorkItemTarget {
                project_id: self.project_id,
                id,
            },
        )
    }

    fn create_work_item(
        &self,
        title: String,
        body: Option<String>,
        item_type: Option<String>,
        status: Option<String>,
    ) -> Result<WorkItemRecord, String> {
        self.post(
            "work-item/create",
            &CreateProjectWorkItemInput {
                project_id: self.project_id,
                title,
                body,
                item_type,
                status,
            },
        )
    }

    fn update_work_item(
        &self,
        id: i64,
        title: Option<String>,
        body: Option<String>,
        item_type: Option<String>,
        status: Option<String>,
    ) -> Result<WorkItemRecord, String> {
        self.post(
            "work-item/update",
            &UpdateProjectWorkItemInput {
                project_id: self.project_id,
                id,
                title,
                body,
                item_type,
                status,
            },
        )
    }

    fn close_work_item(&self, id: i64) -> Result<WorkItemRecord, String> {
        self.post(
            "work-item/close",
            &ProjectWorkItemTarget {
                project_id: self.project_id,
                id,
            },
        )
    }

    fn list_documents(&self, work_item_id: Option<i64>) -> Result<Vec<DocumentRecord>, String> {
        self.post(
            "document/list",
            &ListProjectDocumentsInput {
                project_id: self.project_id,
                work_item_id,
            },
        )
    }

    fn create_document(
        &self,
        title: String,
        body: Option<String>,
        work_item_id: Option<i64>,
    ) -> Result<DocumentRecord, String> {
        self.post(
            "document/create",
            &CreateProjectDocumentInput {
                project_id: self.project_id,
                title,
                body,
                work_item_id,
            },
        )
    }

    fn update_document(
        &self,
        id: i64,
        title: Option<String>,
        body: Option<String>,
        work_item_id: Option<i64>,
        clear_work_item: bool,
    ) -> Result<DocumentRecord, String> {
        self.post(
            "document/update",
            &UpdateProjectDocumentInput {
                project_id: self.project_id,
                id,
                title,
                body,
                work_item_id,
                clear_work_item,
            },
        )
    }

    fn delete_document(&self, id: i64) -> Result<Value, String> {
        self.post(
            "document/delete",
            &ProjectDocumentTarget {
                project_id: self.project_id,
                id,
            },
        )
    }

    fn post<TRequest, TResponse>(&self, route: &str, payload: &TRequest) -> Result<TResponse, String>
    where
        TRequest: Serialize,
        TResponse: DeserializeOwned,
    {
        let response = self
            .client
            .post(format!("{}/{}", self.base_url, route))
            .header("x-project-commander-token", &self.token)
            .header("x-project-commander-source", "agent_mcp")
            .header(
                "x-project-commander-session-id",
                self.session_id
                    .map(|session_id| session_id.to_string())
                    .unwrap_or_default(),
            )
            .json(payload)
            .send()
            .map_err(|error| format!("failed to reach Project Commander supervisor: {error}"))?;

        if !response.status().is_success() {
            return Err(
                response
                    .text()
                    .unwrap_or_else(|_| "Project Commander supervisor returned an error".to_string()),
            );
        }

        response
            .json::<TResponse>()
            .map_err(|error| format!("failed to decode Project Commander supervisor response: {error}"))
    }
}

fn handle_message(client: &SupervisorMcpClient, message: Value) -> Result<Option<Value>, String> {
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let id = message.get("id").cloned();
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));

    match method {
        "initialize" => {
            let protocol_version = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2025-03-26");

            Ok(Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": protocol_version,
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": MCP_SERVER_NAME,
                        "version": MCP_SERVER_VERSION
                    },
                    "instructions": "Use Project Commander tools for the active project's work items and documents. These tools are bound to the selected project through the Project Commander supervisor."
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
                "tools": build_tool_definitions()
            }
        }))),
        "tools/call" => {
            let tool_name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing tool name".to_string())?;
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));

            let result = match call_tool(client, tool_name, arguments) {
                Ok(value) => json!({
                    "content": [
                        {
                            "type": "text",
                            "text": serde_json::to_string_pretty(&value)
                                .unwrap_or_else(|_| "null".to_string())
                        }
                    ],
                    "structuredContent": value,
                    "isError": false
                }),
                Err(error) => json!({
                    "content": [
                        {
                            "type": "text",
                            "text": error
                        }
                    ],
                    "isError": true
                }),
            };

            Ok(Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result
            })))
        }
        _ => {
            if id.is_none() {
                return Ok(None);
            }

            Ok(Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("method not found: {method}")
                }
            })))
        }
    }
}

fn call_tool(client: &SupervisorMcpClient, tool_name: &str, arguments: Value) -> Result<Value, String> {
    match tool_name {
        "current_project" => Ok(serde_json::to_value(client.current_project()?)
            .map_err(|error| format!("failed to encode current project result: {error}"))?),
        "session_brief" => Ok(serde_json::to_value(client.session_brief()?)
            .map_err(|error| format!("failed to encode session brief result: {error}"))?),
        "list_work_items" => {
            let status = arguments
                .get("status")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            Ok(json!({
                "workItems": client.list_work_items(status)?
            }))
        }
        "get_work_item" => {
            let id = read_required_i64(&arguments, "id")?;
            Ok(serde_json::to_value(client.get_work_item(id)?)
                .map_err(|error| format!("failed to encode work item result: {error}"))?)
        }
        "create_work_item" => Ok(serde_json::to_value(client.create_work_item(
            read_required_string(&arguments, "title")?,
            read_optional_string(&arguments, "body"),
            read_optional_string(&arguments, "itemType"),
            read_optional_string(&arguments, "status"),
        )?)
        .map_err(|error| format!("failed to encode created work item: {error}"))?),
        "update_work_item" => Ok(serde_json::to_value(client.update_work_item(
            read_required_i64(&arguments, "id")?,
            read_optional_string(&arguments, "title"),
            read_optional_string(&arguments, "body"),
            read_optional_string(&arguments, "itemType"),
            read_optional_string(&arguments, "status"),
        )?)
        .map_err(|error| format!("failed to encode updated work item: {error}"))?),
        "close_work_item" => Ok(serde_json::to_value(client.close_work_item(read_required_i64(
            &arguments, "id",
        )?)?)
        .map_err(|error| format!("failed to encode closed work item: {error}"))?),
        "list_documents" => Ok(json!({
            "documents": client.list_documents(read_optional_i64(&arguments, "workItemId"))?
        })),
        "create_document" => Ok(serde_json::to_value(client.create_document(
            read_required_string(&arguments, "title")?,
            read_optional_string(&arguments, "body"),
            read_optional_i64(&arguments, "workItemId"),
        )?)
        .map_err(|error| format!("failed to encode created document: {error}"))?),
        "update_document" => Ok(serde_json::to_value(client.update_document(
            read_required_i64(&arguments, "id")?,
            read_optional_string(&arguments, "title"),
            read_optional_string(&arguments, "body"),
            read_optional_i64(&arguments, "workItemId"),
            arguments
                .get("clearWorkItem")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )?)
        .map_err(|error| format!("failed to encode updated document: {error}"))?),
        "delete_document" => Ok(client.delete_document(read_required_i64(&arguments, "id")?)?),
        _ => Err(format!("unknown tool: {tool_name}")),
    }
}

fn build_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "current_project",
            "description": "Return the active Project Commander project bound to this session.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            },
            "_meta": {
                "anthropic/maxResultSizeChars": 200000
            }
        }),
        json!({
            "name": "session_brief",
            "description": "Return the active project plus all work items and documents for a quick briefing.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            },
            "_meta": {
                "anthropic/maxResultSizeChars": 200000
            }
        }),
        json!({
            "name": "list_work_items",
            "description": "List work items for the active project, optionally filtered by status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": ["backlog", "in_progress", "blocked", "done"],
                        "description": "Optional status filter."
                    }
                },
                "required": [],
                "additionalProperties": false
            },
            "_meta": {
                "anthropic/maxResultSizeChars": 200000
            }
        }),
        json!({
            "name": "get_work_item",
            "description": "Return one work item plus any linked documents.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {
                        "type": "integer",
                        "description": "Work item id."
                    }
                },
                "required": ["id"],
                "additionalProperties": false
            },
            "_meta": {
                "anthropic/maxResultSizeChars": 200000
            }
        }),
        json!({
            "name": "create_work_item",
            "description": "Create a work item in the active project.",
            "inputSchema": {
                "type": "object",
                "properties": {
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
                        "enum": ["backlog", "in_progress", "blocked", "done"],
                        "description": "Initial work item status."
                    }
                },
                "required": ["title"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "update_work_item",
            "description": "Update a work item's title, body, type, or status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {
                        "type": "integer",
                        "description": "Work item id."
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
                        "enum": ["backlog", "in_progress", "blocked", "done"],
                        "description": "Optional new status."
                    }
                },
                "required": ["id"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "close_work_item",
            "description": "Mark a work item done.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {
                        "type": "integer",
                        "description": "Work item id."
                    }
                },
                "required": ["id"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "list_documents",
            "description": "List documents for the active project, optionally filtered to a linked work item.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workItemId": {
                        "type": "integer",
                        "description": "Optional work item id filter."
                    }
                },
                "required": [],
                "additionalProperties": false
            },
            "_meta": {
                "anthropic/maxResultSizeChars": 200000
            }
        }),
        json!({
            "name": "create_document",
            "description": "Create a project document or a document linked to a work item.",
            "inputSchema": {
                "type": "object",
                "properties": {
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
                },
                "required": ["title"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "update_document",
            "description": "Update a document's title, body, or linked work item.",
            "inputSchema": {
                "type": "object",
                "properties": {
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
                },
                "required": ["id"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "delete_document",
            "description": "Delete a document from the active project.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {
                        "type": "integer",
                        "description": "Document id."
                    }
                },
                "required": ["id"],
                "additionalProperties": false
            }
        }),
    ]
}

fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>, String> {
    let mut first_line = String::new();

    loop {
        first_line.clear();
        let bytes_read = reader
            .read_line(&mut first_line)
            .map_err(|error| format!("failed to read Project Commander MCP input: {error}"))?;

        if bytes_read == 0 {
            return Ok(None);
        }

        if !first_line.trim().is_empty() {
            break;
        }
    }

    let trimmed = first_line.trim_end_matches(['\r', '\n']);

    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return serde_json::from_str(trimmed)
            .map(Some)
            .map_err(|error| format!("failed to decode Project Commander MCP JSON line: {error}"));
    }

    let mut content_length = None;

    if let Some(value) = trimmed.strip_prefix("Content-Length:") {
        content_length = Some(
            value
                .trim()
                .parse::<usize>()
                .map_err(|error| format!("invalid Content-Length header: {error}"))?,
        );
    }

    loop {
        let mut line = String::new();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|error| format!("failed to read Project Commander MCP header: {error}"))?;

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
                    .map_err(|error| format!("invalid Content-Length header: {error}"))?,
            );
        }
    }

    let content_length =
        content_length.ok_or_else(|| "missing Content-Length header".to_string())?;
    let mut payload = vec![0_u8; content_length];
    reader
        .read_exact(&mut payload)
        .map_err(|error| format!("failed to read Project Commander MCP payload: {error}"))?;

    serde_json::from_slice(&payload)
        .map(Some)
        .map_err(|error| format!("failed to decode Project Commander MCP payload: {error}"))
}

fn write_message(writer: &mut impl Write, message: &Value) -> Result<(), String> {
    let raw = serde_json::to_string(message)
        .map_err(|error| format!("failed to encode Project Commander MCP response: {error}"))?;

    writer
        .write_all(raw.as_bytes())
        .and_then(|_| writer.write_all(b"\n"))
        .and_then(|_| writer.flush())
        .map_err(|error| format!("failed to write Project Commander MCP response: {error}"))
}

fn read_required_string(arguments: &Value, key: &str) -> Result<String, String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("missing required string field: {key}"))
}

fn read_optional_string(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn read_required_i64(arguments: &Value, key: &str) -> Result<i64, String> {
    arguments
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("missing required integer field: {key}"))
}

fn read_optional_i64(arguments: &Value, key: &str) -> Option<i64> {
    arguments.get(key).and_then(Value::as_i64)
}
