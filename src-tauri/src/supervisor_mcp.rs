use crate::db::{DocumentRecord, ProjectRecord, WorkItemRecord, WorktreeRecord};
use crate::error::{AppError, AppResult};
use crate::session_api::ProjectSessionTarget;
use crate::supervisor_api::{
    AckAgentMessagesApiInput, AgentInboxApiInput, AgentMessageListOutput,
    CleanupWorktreeInput, CreateProjectDocumentInput, CreateProjectWorkItemInput,
    LaunchProjectWorktreeAgentInput, ListAgentMessagesApiInput, ListProjectDocumentsInput,
    ListProjectWorkItemsInput, ListProjectWorktreesInput,
    PinWorktreeInput, ProjectDocumentTarget, ProjectWorkItemTarget,
    SendAgentMessageApiInput, SessionBriefOutput, UpdateProjectDocumentInput,
    UpdateProjectWorkItemInput, WorkItemDetailOutput, WorktreeLaunchOutput,
};
use reqwest::blocking::Client;
use serde::de::DeserializeOwned;
use serde::Serialize;
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
) -> AppResult<()> {
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
) -> AppResult<()> {
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
    ) -> AppResult<Self> {
        let client = Client::builder()
            .timeout(MCP_REQUEST_TIMEOUT)
            .build()
            .map_err(|error| AppError::internal(format!("failed to build Project Commander MCP client: {error}")))?;

        Ok(Self {
            client,
            base_url: format!("http://127.0.0.1:{port}"),
            token,
            project_id,
            worktree_id,
            session_id,
        })
    }

    fn current_project(&self) -> AppResult<ProjectRecord> {
        self.post(
            "project/current",
            &ProjectSessionTarget {
                project_id: self.project_id,
                worktree_id: self.worktree_id,
            },
        )
    }

    fn session_brief(&self) -> AppResult<SessionBriefOutput> {
        self.post(
            "project/session-brief",
            &ProjectSessionTarget {
                project_id: self.project_id,
                worktree_id: self.worktree_id,
            },
        )
    }

    fn list_work_items(
        &self,
        status: Option<String>,
        item_type: Option<String>,
        parent_only: bool,
        open_only: bool,
    ) -> AppResult<Vec<WorkItemRecord>> {
        self.post(
            "work-item/list",
            &ListProjectWorkItemsInput {
                project_id: self.project_id,
                status,
                item_type,
                parent_only,
                open_only,
            },
        )
    }

    fn get_work_item(&self, id: i64) -> AppResult<WorkItemDetailOutput> {
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
        parent_work_item_id: Option<i64>,
    ) -> AppResult<WorkItemRecord> {
        self.post(
            "work-item/create",
            &CreateProjectWorkItemInput {
                project_id: self.project_id,
                title,
                body,
                item_type,
                status,
                parent_work_item_id,
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
        parent_work_item_id: Option<i64>,
        clear_parent: bool,
    ) -> AppResult<WorkItemRecord> {
        self.post(
            "work-item/update",
            &UpdateProjectWorkItemInput {
                project_id: self.project_id,
                id,
                title,
                body,
                item_type,
                status,
                parent_work_item_id,
                clear_parent,
            },
        )
    }

    fn close_work_item(&self, id: i64) -> AppResult<WorkItemRecord> {
        self.post(
            "work-item/close",
            &ProjectWorkItemTarget {
                project_id: self.project_id,
                id,
            },
        )
    }

    fn list_documents(&self, work_item_id: Option<i64>) -> AppResult<Vec<DocumentRecord>> {
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
    ) -> AppResult<DocumentRecord> {
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
    ) -> AppResult<DocumentRecord> {
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

    fn delete_document(&self, id: i64) -> AppResult<Value> {
        self.post(
            "document/delete",
            &ProjectDocumentTarget {
                project_id: self.project_id,
                id,
            },
        )
    }

    fn list_worktrees(&self) -> AppResult<Vec<WorktreeRecord>> {
        self.post(
            "worktree/list",
            &ListProjectWorktreesInput {
                project_id: self.project_id,
            },
        )
    }

    fn launch_worktree_agent(
        &self,
        work_item_id: i64,
        launch_profile_id: Option<i64>,
        model: Option<String>,
    ) -> AppResult<WorktreeLaunchOutput> {
        self.post(
            "worktree/launch-agent",
            &LaunchProjectWorktreeAgentInput {
                project_id: self.project_id,
                work_item_id,
                launch_profile_id,
                model,
            },
        )
    }

    fn cleanup_worktree(&self, worktree_id: i64, force: bool) -> AppResult<WorktreeRecord> {
        self.post(
            "worktree/cleanup",
            &CleanupWorktreeInput {
                project_id: self.project_id,
                worktree_id,
                force,
            },
        )
    }

    fn pin_worktree(&self, worktree_id: i64, pinned: bool) -> AppResult<WorktreeRecord> {
        self.post(
            "worktree/pin",
            &PinWorktreeInput {
                project_id: self.project_id,
                worktree_id,
                pinned,
            },
        )
    }

    fn terminate_session(&self, worktree_id: i64) -> AppResult<Value> {
        self.post(
            "session/terminate",
            &ProjectSessionTarget {
                project_id: self.project_id,
                worktree_id: Some(worktree_id),
            },
        )
    }

    fn send_message(
        &self,
        to: String,
        message_type: String,
        body: String,
        context_json: Option<String>,
    ) -> AppResult<Value> {
        self.post(
            "message/send",
            &SendAgentMessageApiInput {
                project_id: self.project_id,
                to_agent: to,
                message_type,
                body,
                context_json,
            },
        )
    }

    fn list_messages(
        &self,
        from_agent: Option<String>,
        to_agent: Option<String>,
        message_type: Option<String>,
        status: Option<String>,
        limit: Option<i64>,
    ) -> AppResult<AgentMessageListOutput> {
        self.post(
            "message/list",
            &ListAgentMessagesApiInput {
                project_id: self.project_id,
                from_agent,
                to_agent,
                message_type,
                status,
                limit,
            },
        )
    }

    fn get_inbox(
        &self,
        unread_only: bool,
        limit: Option<i64>,
    ) -> AppResult<AgentMessageListOutput> {
        self.post(
            "message/inbox",
            &AgentInboxApiInput {
                project_id: self.project_id,
                agent_name: None,
                unread_only,
                limit,
            },
        )
    }

    fn ack_messages(&self, message_ids: Vec<i64>) -> AppResult<Value> {
        self.post(
            "message/ack",
            &AckAgentMessagesApiInput {
                project_id: self.project_id,
                message_ids: Some(message_ids),
                all: false,
            },
        )
    }

    fn post<TRequest, TResponse>(
        &self,
        route: &str,
        payload: &TRequest,
    ) -> AppResult<TResponse>
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
            .map_err(|error| AppError::supervisor(format!("failed to reach Project Commander supervisor: {error}")))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "Project Commander supervisor returned an error".to_string());
            return Err(AppError::from_status(status, body));
        }

        let envelope = response.json::<Value>().map_err(|error| {
            AppError::internal(format!("failed to decode Project Commander supervisor response: {error}"))
        })?;

        let data = envelope
            .get("data")
            .cloned()
            .unwrap_or(Value::Null);

        serde_json::from_value::<TResponse>(data).map_err(|error| {
            AppError::internal(format!("failed to decode Project Commander supervisor response data: {error}"))
        })
    }
}

fn handle_message(client: &SupervisorMcpClient, message: Value) -> AppResult<Option<Value>> {
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
                .ok_or_else(|| AppError::invalid_input("missing tool name"))?;
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
                            "text": error.message
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

fn call_tool(
    client: &SupervisorMcpClient,
    tool_name: &str,
    arguments: Value,
) -> AppResult<Value> {
    match tool_name {
        "current_project" => Ok(serde_json::to_value(client.current_project()?)
            .map_err(|error| AppError::internal(format!("failed to encode current project result: {error}")))?),
        "session_brief" => {
            let brief = client.session_brief()?;
            let mut value = serde_json::to_value(brief)
                .map_err(|error| AppError::internal(format!("failed to encode session brief result: {error}")))?;
            // Strip bodies — keep headers only so the response stays bounded.
            // Callers that need a body should use get_work_item(id) on demand.
            if let Some(items) = value.get_mut("workItems").and_then(Value::as_array_mut) {
                for item in items.iter_mut() {
                    *item = slim_work_item(item);
                }
            }
            if let Some(docs) = value.get_mut("documents").and_then(Value::as_array_mut) {
                for doc in docs.iter_mut() {
                    *doc = slim_document(doc);
                }
            }
            Ok(value)
        }
        "list_work_items" => {
            let status = arguments
                .get("status")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            let item_type = arguments
                .get("itemType")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            let items: Vec<Value> = client
                .list_work_items(
                    status,
                    item_type,
                    arguments
                        .get("parentOnly")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    arguments
                        .get("openOnly")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                )?
                .into_iter()
                .map(|item| {
                    let mut v = serde_json::to_value(item).unwrap_or(Value::Null);
                    if let Some(obj) = v.as_object_mut() {
                        obj.remove("body");
                    }
                    v
                })
                .collect();
            Ok(json!({ "workItems": items }))
        }
        "get_work_item" => {
            let id = read_required_i64(&arguments, "id")?;
            Ok(serde_json::to_value(client.get_work_item(id)?)
                .map_err(|error| AppError::internal(format!("failed to encode work item result: {error}")))?)
        }
        "create_work_item" => Ok(serde_json::to_value(client.create_work_item(
            read_required_string(&arguments, "title")?,
            read_optional_string(&arguments, "body"),
            read_optional_string(&arguments, "itemType"),
            read_optional_string(&arguments, "status"),
            read_optional_i64(&arguments, "parentWorkItemId"),
        )?)
        .map_err(|error| AppError::internal(format!("failed to encode created work item: {error}")))?),
        "update_work_item" => Ok(serde_json::to_value(client.update_work_item(
            read_required_i64(&arguments, "id")?,
            read_optional_string(&arguments, "title"),
            read_optional_string(&arguments, "body"),
            read_optional_string(&arguments, "itemType"),
            read_optional_string(&arguments, "status"),
            read_optional_i64(&arguments, "parentWorkItemId"),
            arguments
                .get("clearParent")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )?)
        .map_err(|error| AppError::internal(format!("failed to encode updated work item: {error}")))?),
        "close_work_item" => Ok(serde_json::to_value(
            client.close_work_item(read_required_i64(&arguments, "id")?)?,
        )
        .map_err(|error| AppError::internal(format!("failed to encode closed work item: {error}")))?),
        "list_documents" => {
            let docs: Vec<Value> = client
                .list_documents(read_optional_i64(&arguments, "workItemId"))?
                .into_iter()
                .map(|doc| {
                    let mut v = serde_json::to_value(doc).unwrap_or(Value::Null);
                    if let Some(obj) = v.as_object_mut() {
                        obj.remove("body");
                    }
                    v
                })
                .collect();
            Ok(json!({ "documents": docs }))
        }
        "create_document" => Ok(serde_json::to_value(client.create_document(
            read_required_string(&arguments, "title")?,
            read_optional_string(&arguments, "body"),
            read_optional_i64(&arguments, "workItemId"),
        )?)
        .map_err(|error| AppError::internal(format!("failed to encode created document: {error}")))?),
        "update_document" => Ok(serde_json::to_value(
            client.update_document(
                read_required_i64(&arguments, "id")?,
                read_optional_string(&arguments, "title"),
                read_optional_string(&arguments, "body"),
                read_optional_i64(&arguments, "workItemId"),
                arguments
                    .get("clearWorkItem")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            )?,
        )
        .map_err(|error| AppError::internal(format!("failed to encode updated document: {error}")))?),
        "delete_document" => Ok(client.delete_document(read_required_i64(&arguments, "id")?)?),
        "list_worktrees" => Ok(json!({
            "worktrees": client.list_worktrees()?
        })),
        "launch_worktree_agent" => {
            let mut value = serde_json::to_value(client.launch_worktree_agent(
                read_required_i64(&arguments, "workItemId")?,
                read_optional_i64(&arguments, "launchProfileId"),
                arguments.get("model").and_then(|v| v.as_str()).map(String::from),
            )?)
            .map_err(|error| AppError::internal(format!("failed to encode launched worktree agent: {error}")))?;
            // Strip session.output — it can be 200k+ chars for long-running sessions.
            // Callers receive output via terminal-output events; they don't need it here.
            if let Some(session) = value.get_mut("session").and_then(Value::as_object_mut) {
                session.remove("output");
                session.remove("outputCursor");
            }
            Ok(value)
        }
        "cleanup_worktree" => Ok(serde_json::to_value(client.cleanup_worktree(
            read_required_i64(&arguments, "worktreeId")?,
            arguments.get("force").and_then(|v| v.as_bool()).unwrap_or(false),
        )?)
        .map_err(|error| AppError::internal(format!("failed to encode cleaned-up worktree: {error}")))?),
        "pin_worktree" => Ok(serde_json::to_value(client.pin_worktree(
            read_required_i64(&arguments, "worktreeId")?,
            arguments
                .get("pinned")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
        )?)
        .map_err(|error| AppError::internal(format!("failed to encode pinned worktree: {error}")))?),
        "terminate_session" => Ok(client.terminate_session(
            read_required_i64(&arguments, "worktreeId")?,
        )?),
        "send_message" => Ok(client.send_message(
            read_required_string(&arguments, "to")?,
            read_required_string(&arguments, "messageType")?,
            read_required_string(&arguments, "body")?,
            read_optional_string(&arguments, "contextJson"),
        )?),
        "list_messages" => {
            let result = client.list_messages(
                read_optional_string(&arguments, "fromAgent"),
                read_optional_string(&arguments, "toAgent"),
                read_optional_string(&arguments, "messageType"),
                read_optional_string(&arguments, "status"),
                read_optional_i64(&arguments, "limit"),
            )?;
            Ok(serde_json::to_value(result)
                .map_err(|error| AppError::internal(format!("failed to encode message list: {error}")))?)
        }
        "get_messages" => {
            let mark_as_read = arguments
                .get("markAsRead")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let limit = read_optional_i64(&arguments, "limit");
            let result = client.get_inbox(!mark_as_read, limit)?;
            let ids: Vec<i64> = result.messages.iter().map(|m| m.id).collect();
            if mark_as_read && !ids.is_empty() {
                client.ack_messages(ids)?;
            }
            Ok(serde_json::to_value(result)
                .map_err(|error| AppError::internal(format!("failed to encode inbox: {error}")))?)
        }
        _ => Err(AppError::invalid_input(format!("unknown tool: {tool_name}"))),
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
            "description": "List work items for the active project, optionally filtered by status, type, or hierarchy.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": ["backlog", "in_progress", "blocked", "parked", "done"],
                        "description": "Optional status filter."
                    },
                    "itemType": {
                        "type": "string",
                        "enum": ["bug", "task", "feature", "note"],
                        "description": "Optional work item type filter."
                    },
                    "parentOnly": {
                        "type": "boolean",
                        "description": "When true, return only top-level work items."
                    },
                    "openOnly": {
                        "type": "boolean",
                        "description": "When true, exclude done work items."
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
                        "enum": ["backlog", "in_progress", "blocked", "parked", "done"],
                        "description": "Initial work item status."
                    },
                    "parentWorkItemId": {
                        "type": "integer",
                        "description": "Optional parent work item id for creating a dotted child item."
                    }
                },
                "required": ["title"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "update_work_item",
            "description": "Update a work item's title, body, type, status, or parent (reparenting).",
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
                        "enum": ["backlog", "in_progress", "blocked", "parked", "done"],
                        "description": "Optional new status."
                    },
                    "parentWorkItemId": {
                        "type": "integer",
                        "description": "Reparent the item under a new parent. Mutually exclusive with clearParent. The item must currently have no children (max 2 levels), and the new parent must be a top-level item in the same project."
                    },
                    "clearParent": {
                        "type": "boolean",
                        "description": "Detach the item from its current parent, making it top-level. Mutually exclusive with parentWorkItemId."
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
        json!({
            "name": "list_worktrees",
            "description": "List supervisor-managed worktrees for the active project with runtime and git state.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "launch_worktree_agent",
            "description": "Ensure a worktree for a work item and launch or reconnect to its Claude session. Optionally specify a model — choose based on task complexity: opus for hard/architectural work, sonnet for standard features/bugs, haiku for simple/mechanical tasks.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workItemId": {
                        "type": "integer",
                        "description": "Work item id to launch in a focused worktree."
                    },
                    "launchProfileId": {
                        "type": "integer",
                        "description": "Optional launch profile override. Defaults to the current session profile or the project default."
                    },
                    "model": {
                        "type": "string",
                        "description": "Optional Claude model override (e.g. 'claude-opus-4-6', 'claude-sonnet-4-6', 'claude-haiku-4-5-20251001'). Pick based on task complexity."
                    }
                },
                "required": ["workItemId"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "cleanup_worktree",
            "description": "Clean up a worktree: remove the git worktree, delete the branch (best-effort), and drop the DB record. Blocked by a live session or pinned worktree. If the work item is in_progress or blocked it is automatically set to parked. If the branch has unmerged commits you must pass force=true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "worktreeId": {
                        "type": "integer",
                        "description": "Worktree id to clean up."
                    },
                    "force": {
                        "type": "boolean",
                        "description": "Pass true to allow cleanup even when the branch has unmerged commits. Defaults to false."
                    }
                },
                "required": ["worktreeId"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "pin_worktree",
            "description": "Pin or unpin a worktree. A pinned worktree is excluded from cleanup eligibility even when its work item is done and its branch is merged.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "worktreeId": {
                        "type": "integer",
                        "description": "Worktree id to pin or unpin."
                    },
                    "pinned": {
                        "type": "boolean",
                        "description": "true to pin (keep), false to unpin (allow cleanup). Defaults to true."
                    }
                },
                "required": ["worktreeId"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "terminate_session",
            "description": "Forcefully terminate a running worktree agent session. Kills the Claude Code process and cleans up the PTY. Use this when an agent is stuck, completed but lingering, or needs to be stopped before worktree cleanup.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "worktreeId": {
                        "type": "integer",
                        "description": "Worktree id whose active session should be terminated."
                    }
                },
                "required": ["worktreeId"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "send_message",
            "description": "Send a message to another agent in the project. The message will be queued in the recipient's inbox.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "to": {
                        "type": "string",
                        "description": "Recipient agent name (e.g. 'dispatcher', 'teammate-42')."
                    },
                    "messageType": {
                        "type": "string",
                        "enum": ["question", "status_update", "request_approval", "complete", "blocked", "info"],
                        "description": "Message type that classifies intent."
                    },
                    "body": {
                        "type": "string",
                        "description": "Message body text."
                    },
                    "contextJson": {
                        "type": "string",
                        "description": "Optional JSON string with additional structured context."
                    }
                },
                "required": ["to", "messageType", "body"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "list_messages",
            "description": "List agent messages in the project, optionally filtered by sender, recipient, type, or status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "fromAgent": {
                        "type": "string",
                        "description": "Optional filter by sender agent name."
                    },
                    "toAgent": {
                        "type": "string",
                        "description": "Optional filter by recipient agent name."
                    },
                    "messageType": {
                        "type": "string",
                        "description": "Optional filter by message type."
                    },
                    "status": {
                        "type": "string",
                        "enum": ["unread", "read"],
                        "description": "Optional filter by read status."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of messages to return."
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
            "name": "get_messages",
            "description": "Check this agent's inbox. By default marks retrieved messages as read.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "markAsRead": {
                        "type": "boolean",
                        "description": "When true (default), mark returned messages as read and acknowledge them."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of messages to return."
                    }
                },
                "required": [],
                "additionalProperties": false
            },
            "_meta": {
                "anthropic/maxResultSizeChars": 200000
            }
        }),
    ]
}

fn read_message(reader: &mut impl BufRead) -> AppResult<Option<Value>> {
    let mut first_line = String::new();

    loop {
        first_line.clear();
        let bytes_read = reader
            .read_line(&mut first_line)
            .map_err(|error| AppError::io(format!("failed to read Project Commander MCP input: {error}")))?;

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
            .map_err(|error| AppError::internal(format!("failed to decode Project Commander MCP JSON line: {error}")));
    }

    let mut content_length = None;

    if let Some(value) = trimmed.strip_prefix("Content-Length:") {
        content_length = Some(
            value
                .trim()
                .parse::<usize>()
                .map_err(|error| AppError::io(format!("invalid Content-Length header: {error}")))?,
        );
    }

    loop {
        let mut line = String::new();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|error| AppError::io(format!("failed to read Project Commander MCP header: {error}")))?;

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
        .map_err(|error| AppError::io(format!("failed to read Project Commander MCP payload: {error}")))?;

    serde_json::from_slice(&payload)
        .map(Some)
        .map_err(|error| AppError::internal(format!("failed to decode Project Commander MCP payload: {error}")))
}

fn write_message(writer: &mut impl Write, message: &Value) -> AppResult<()> {
    let raw = serde_json::to_string(message)
        .map_err(|error| AppError::internal(format!("failed to encode Project Commander MCP response: {error}")))?;

    writer
        .write_all(raw.as_bytes())
        .and_then(|_| writer.write_all(b"\n"))
        .and_then(|_| writer.flush())
        .map_err(|error| AppError::io(format!("failed to write Project Commander MCP response: {error}")))
}

fn read_required_string(arguments: &Value, key: &str) -> AppResult<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::invalid_input(format!("missing required string field: {key}")))
}

fn read_optional_string(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn read_required_i64(arguments: &Value, key: &str) -> AppResult<i64> {
    arguments
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| AppError::invalid_input(format!("missing required integer field: {key}")))
}

fn read_optional_i64(arguments: &Value, key: &str) -> Option<i64> {
    arguments.get(key).and_then(Value::as_i64)
}

/// Return a work item header for list/brief contexts — strips body and metadata.
/// Callers that need the full body should use get_work_item(id).
fn slim_work_item(item: &Value) -> Value {
    json!({
        "id": item["id"],
        "callSign": item["callSign"],
        "title": item["title"],
        "status": item["status"],
        "itemType": item["itemType"],
        "parentWorkItemId": item["parentWorkItemId"],
        "childNumber": item["childNumber"],
    })
}

/// Return a document header for list/brief contexts — strips body and metadata.
/// Callers that need the full body should use get_work_item(id) or update_document.
fn slim_document(doc: &Value) -> Value {
    json!({
        "id": doc["id"],
        "title": doc["title"],
    })
}
