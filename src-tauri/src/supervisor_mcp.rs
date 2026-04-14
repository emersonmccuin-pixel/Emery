use crate::db::{
    DocumentRecord, ProjectRecord, SessionEventRecord, WorkItemRecord, WorktreeRecord,
};
use crate::error::{AppError, AppErrorCode, AppResult};
use crate::session_api::ProjectSessionTarget;
use crate::supervisor_api::{
    AckAgentMessagesApiInput, AgentInboxApiInput, AgentMessageListOutput, CleanupWorktreeInput,
    CreateProjectDocumentInput, CreateProjectWorkItemInput, HeartbeatSessionInput,
    LaunchProjectWorktreeAgentInput, ListAgentMessagesApiInput, ListProjectDocumentsInput,
    ListProjectWorkItemsInput, ListProjectWorktreesInput, MarkSessionWorkerDoneInput,
    PinWorktreeInput, ProjectCallSignTarget, ProjectDocumentTarget, ProjectWorkItemTarget,
    PublishSessionWorkerStatusInput, ReconcileProjectTrackerInput, SendAgentMessageApiInput,
    UpdateProjectDocumentInput, UpdateProjectWorkItemInput, WaitAgentMessagesApiInput,
    WaitAgentMessagesOutput, WorkItemDetailOutput, WorktreeLaunchOutput,
};
use crate::vault::{
    ExecuteVaultCliIntegrationInput, ExecuteVaultCliIntegrationOutput,
    ExecuteVaultHttpIntegrationInput, ExecuteVaultHttpIntegrationOutput,
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

pub fn handle_supervisor_mcp_message(
    port: u16,
    token: String,
    project_id: i64,
    worktree_id: Option<i64>,
    session_id: Option<i64>,
    message: Value,
) -> AppResult<Option<Value>> {
    let client = SupervisorMcpClient::new(port, token, project_id, worktree_id, session_id)?;
    handle_message(&client, message)
}

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
            .map_err(|error| {
                AppError::internal(format!(
                    "failed to build Project Commander MCP client: {error}"
                ))
            })?;

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

    fn get_work_item_by_call_sign(&self, call_sign: &str) -> AppResult<WorkItemDetailOutput> {
        self.post(
            "work-item/get-by-call-sign",
            &ProjectCallSignTarget {
                project_id: self.project_id,
                call_sign: call_sign.to_owned(),
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
        execution_mode: Option<String>,
    ) -> AppResult<WorktreeLaunchOutput> {
        self.post(
            "worktree/launch-agent",
            &LaunchProjectWorktreeAgentInput {
                project_id: self.project_id,
                work_item_id,
                launch_profile_id,
                model,
                execution_mode,
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
        thread_id: Option<String>,
        reply_to_message_id: Option<i64>,
    ) -> AppResult<Value> {
        self.post(
            "message/send",
            &SendAgentMessageApiInput {
                project_id: self.project_id,
                to_agent: to,
                thread_id,
                reply_to_message_id,
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
        thread_id: Option<String>,
        reply_to_message_id: Option<i64>,
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
                thread_id,
                reply_to_message_id,
                message_type,
                status,
                limit,
            },
        )
    }

    fn get_inbox(
        &self,
        unread_only: bool,
        from_agent: Option<String>,
        thread_id: Option<String>,
        reply_to_message_id: Option<i64>,
        message_type: Option<String>,
        limit: Option<i64>,
    ) -> AppResult<AgentMessageListOutput> {
        self.post(
            "message/inbox",
            &AgentInboxApiInput {
                project_id: self.project_id,
                agent_name: None,
                unread_only,
                from_agent,
                thread_id,
                reply_to_message_id,
                message_type,
                limit,
            },
        )
    }

    fn wait_for_messages(
        &self,
        from_agent: Option<String>,
        thread_id: Option<String>,
        reply_to_message_id: Option<i64>,
        message_type: Option<String>,
        limit: Option<i64>,
        timeout_ms: Option<u64>,
    ) -> AppResult<WaitAgentMessagesOutput> {
        self.post(
            "message/wait",
            &WaitAgentMessagesApiInput {
                project_id: self.project_id,
                agent_name: None,
                from_agent,
                thread_id,
                reply_to_message_id,
                message_type,
                limit,
                timeout_ms,
            },
        )
    }

    fn publish_status(
        &self,
        state: String,
        detail: Option<String>,
        thread_id: Option<String>,
        provider_session_id: Option<String>,
        context_json: Option<Value>,
    ) -> AppResult<SessionEventRecord> {
        self.post(
            "session/status",
            &PublishSessionWorkerStatusInput {
                state,
                detail,
                thread_id,
                provider_session_id,
                context_json,
            },
        )
    }

    fn heartbeat(&self, detail: Option<String>, context_json: Option<Value>) -> AppResult<Value> {
        self.post(
            "session/heartbeat",
            &HeartbeatSessionInput {
                detail,
                context_json,
            },
        )
    }

    fn mark_done(
        &self,
        summary: String,
        thread_id: Option<String>,
        provider_session_id: Option<String>,
        context_json: Option<Value>,
    ) -> AppResult<SessionEventRecord> {
        self.post(
            "session/mark-done",
            &MarkSessionWorkerDoneInput {
                summary,
                thread_id,
                provider_session_id,
                context_json,
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

    fn reconcile_inbox(&self) -> AppResult<Value> {
        self.post(
            "message/reconcile-stale",
            &AckAgentMessagesApiInput {
                project_id: self.project_id,
                message_ids: None,
                all: false,
            },
        )
    }

    fn reconcile_tracker(&self) -> AppResult<WorkItemRecord> {
        self.post(
            "tracker/reconcile",
            &ReconcileProjectTrackerInput {
                project_id: self.project_id,
            },
        )
    }

    fn call_http_integration(
        &self,
        integration_id: i64,
        method: String,
        path: String,
        query: Option<Value>,
        headers: Option<Value>,
        body: Option<String>,
        json_body: Option<Value>,
    ) -> AppResult<ExecuteVaultHttpIntegrationOutput> {
        let query = query
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| {
                AppError::invalid_input(format!(
                    "call_http_integration query must be an object of string pairs: {error}"
                ))
            })?
            .unwrap_or_default();
        let headers = headers
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| {
                AppError::invalid_input(format!(
                    "call_http_integration headers must be an object of string pairs: {error}"
                ))
            })?
            .unwrap_or_default();

        self.post(
            "integration/http",
            &ExecuteVaultHttpIntegrationInput {
                integration_id,
                method,
                path,
                query,
                headers,
                body,
                json_body,
            },
        )
    }

    fn call_cli_integration(
        &self,
        integration_id: i64,
        args: Option<Value>,
        cwd: Option<String>,
        stdin: Option<String>,
    ) -> AppResult<ExecuteVaultCliIntegrationOutput> {
        let args = args
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| {
                AppError::invalid_input(format!(
                    "call_cli_integration args must be an array of strings: {error}"
                ))
            })?
            .unwrap_or_default();

        self.post(
            "integration/cli",
            &ExecuteVaultCliIntegrationInput {
                integration_id,
                args,
                cwd,
                stdin,
            },
        )
    }

    fn post<TRequest, TResponse>(&self, route: &str, payload: &TRequest) -> AppResult<TResponse>
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
                "x-project-commander-project-id",
                self.project_id.to_string(),
            )
            .header(
                "x-project-commander-session-id",
                self.session_id
                    .map(|session_id| session_id.to_string())
                    .unwrap_or_default(),
            )
            .json(payload)
            .send()
            .map_err(|error| {
                AppError::supervisor(format!(
                    "failed to reach Project Commander supervisor: {error}"
                ))
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "Project Commander supervisor returned an error".to_string());
            return Err(AppError::from_status(status, body));
        }

        let envelope = response.json::<Value>().map_err(|error| {
            AppError::internal(format!(
                "failed to decode Project Commander supervisor response: {error}"
            ))
        })?;

        let data = envelope.get("data").cloned().unwrap_or(Value::Null);

        serde_json::from_value::<TResponse>(data).map_err(|error| {
            AppError::internal(format!(
                "failed to decode Project Commander supervisor response data: {error}"
            ))
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

fn call_tool(client: &SupervisorMcpClient, tool_name: &str, arguments: Value) -> AppResult<Value> {
    match tool_name {
        "current_project" => Ok(serde_json::to_value(client.current_project()?).map_err(
            |error| AppError::internal(format!("failed to encode current project result: {error}")),
        )?),
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
            let id = read_optional_i64(&arguments, "id");
            let call_sign = read_optional_string(&arguments, "callSign");
            let detail = match (id, call_sign) {
                (Some(id), _) => client.get_work_item(id)?,
                (None, Some(ref cs)) => client.get_work_item_by_call_sign(cs)?,
                (None, None) => {
                    return Err(AppError::invalid_input(
                        "get_work_item requires either 'id' or 'callSign'",
                    ))
                }
            };
            Ok(serde_json::to_value(detail).map_err(|error| {
                AppError::internal(format!("failed to encode work item result: {error}"))
            })?)
        }
        "create_work_item" => Ok(serde_json::to_value(client.create_work_item(
            read_required_string(&arguments, "title")?,
            read_optional_string(&arguments, "body"),
            read_optional_string(&arguments, "itemType"),
            read_optional_string(&arguments, "status"),
            read_optional_i64(&arguments, "parentWorkItemId"),
        )?)
        .map_err(|error| {
            AppError::internal(format!("failed to encode created work item: {error}"))
        })?),
        "update_work_item" => {
            let id = read_optional_i64(&arguments, "id");
            let call_sign = read_optional_string(&arguments, "callSign");
            let resolved_id = match (id, call_sign) {
                (Some(id), _) => id,
                (None, Some(ref cs)) => client.get_work_item_by_call_sign(cs)?.work_item.id,
                (None, None) => {
                    return Err(AppError::invalid_input(
                        "update_work_item requires either 'id' or 'callSign'",
                    ))
                }
            };
            Ok(serde_json::to_value(
                client.update_work_item(
                    resolved_id,
                    read_optional_string(&arguments, "title"),
                    read_optional_string(&arguments, "body"),
                    read_optional_string(&arguments, "itemType"),
                    read_optional_string(&arguments, "status"),
                    read_optional_i64(&arguments, "parentWorkItemId"),
                    arguments
                        .get("clearParent")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                )?,
            )
            .map_err(|error| {
                AppError::internal(format!("failed to encode updated work item: {error}"))
            })?)
        }
        "close_work_item" => {
            let id = read_optional_i64(&arguments, "id");
            let call_sign = read_optional_string(&arguments, "callSign");
            let resolved_id = match (id, call_sign) {
                (Some(id), _) => id,
                (None, Some(ref cs)) => client.get_work_item_by_call_sign(cs)?.work_item.id,
                (None, None) => {
                    return Err(AppError::invalid_input(
                        "close_work_item requires either 'id' or 'callSign'",
                    ))
                }
            };
            Ok(
                serde_json::to_value(client.close_work_item(resolved_id)?).map_err(|error| {
                    AppError::internal(format!("failed to encode closed work item: {error}"))
                })?,
            )
        }
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
        .map_err(|error| {
            AppError::internal(format!("failed to encode created document: {error}"))
        })?),
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
        .map_err(|error| {
            AppError::internal(format!("failed to encode updated document: {error}"))
        })?),
        "delete_document" => Ok(client.delete_document(read_required_i64(&arguments, "id")?)?),
        "list_worktrees" => Ok(json!({
            "worktrees": client.list_worktrees()?
        })),
        "launch_worktree_agent" => {
            let mut value = serde_json::to_value(
                client.launch_worktree_agent(
                    read_required_i64(&arguments, "workItemId")?,
                    read_optional_i64(&arguments, "launchProfileId"),
                    arguments
                        .get("model")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    arguments
                        .get("executionMode")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                )?,
            )
            .map_err(|error| {
                AppError::internal(format!("failed to encode launched worktree agent: {error}"))
            })?;
            // Strip session.output — it can be 200k+ chars for long-running sessions.
            // Callers receive output via terminal-output events; they don't need it here.
            if let Some(session) = value.get_mut("session").and_then(Value::as_object_mut) {
                session.remove("output");
                session.remove("outputCursor");
            }
            Ok(value)
        }
        "cleanup_worktree" => Ok(serde_json::to_value(
            client.cleanup_worktree(
                read_required_i64(&arguments, "worktreeId")?,
                arguments
                    .get("force")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            )?,
        )
        .map_err(|error| {
            AppError::internal(format!("failed to encode cleaned-up worktree: {error}"))
        })?),
        "pin_worktree" => Ok(serde_json::to_value(
            client.pin_worktree(
                read_required_i64(&arguments, "worktreeId")?,
                arguments
                    .get("pinned")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
            )?,
        )
        .map_err(|error| {
            AppError::internal(format!("failed to encode pinned worktree: {error}"))
        })?),
        "terminate_session" => {
            match client.terminate_session(read_required_i64(&arguments, "worktreeId")?) {
                Ok(_) => Ok(json!({ "status": "terminated", "message": "Session terminated" })),
                Err(error) if error.code == AppErrorCode::NotFound => {
                    Ok(json!({ "status": "already_exited", "message": "Session already exited" }))
                }
                Err(error) => Err(error),
            }
        }
        "send_message" => Ok(client.send_message(
            read_required_string(&arguments, "to")?,
            read_required_string(&arguments, "messageType")?,
            read_required_string(&arguments, "body")?,
            read_optional_string(&arguments, "contextJson"),
            read_optional_string(&arguments, "threadId"),
            read_optional_i64(&arguments, "replyToMessageId"),
        )?),
        "publish_status" => Ok(serde_json::to_value(client.publish_status(
            read_required_string(&arguments, "state")?,
            read_optional_string(&arguments, "detail"),
            read_optional_string(&arguments, "threadId"),
            read_optional_string(&arguments, "providerSessionId"),
            arguments.get("contextJson").cloned(),
        )?)
        .map_err(|error| {
            AppError::internal(format!("failed to encode worker status event: {error}"))
        })?),
        "heartbeat" => Ok(client.heartbeat(
            read_optional_string(&arguments, "detail"),
            arguments.get("contextJson").cloned(),
        )?),
        "mark_done" => Ok(serde_json::to_value(client.mark_done(
            read_required_string(&arguments, "summary")?,
            read_optional_string(&arguments, "threadId"),
            read_optional_string(&arguments, "providerSessionId"),
            arguments.get("contextJson").cloned(),
        )?)
        .map_err(|error| {
            AppError::internal(format!("failed to encode worker done event: {error}"))
        })?),
        "list_messages" => {
            let result = client.list_messages(
                read_optional_string(&arguments, "fromAgent"),
                read_optional_string(&arguments, "toAgent"),
                read_optional_string(&arguments, "threadId"),
                read_optional_i64(&arguments, "replyToMessageId"),
                read_optional_string(&arguments, "messageType"),
                read_optional_string(&arguments, "status"),
                read_optional_i64(&arguments, "limit").or(Some(50)),
            )?;
            Ok(serde_json::to_value(result).map_err(|error| {
                AppError::internal(format!("failed to encode message list: {error}"))
            })?)
        }
        "get_messages" => {
            let mark_as_read = arguments
                .get("markAsRead")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let unread_only = arguments
                .get("unreadOnly")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let from_agent = read_optional_string(&arguments, "fromAgent");
            let thread_id = read_optional_string(&arguments, "threadId");
            let reply_to_message_id = read_optional_i64(&arguments, "replyToMessageId");
            let message_type = read_optional_string(&arguments, "messageType");
            let limit = read_optional_i64(&arguments, "limit").or(Some(20));
            let result = client.get_inbox(
                unread_only,
                from_agent,
                thread_id,
                reply_to_message_id,
                message_type,
                limit,
            )?;
            let ids: Vec<i64> = result.messages.iter().map(|m| m.id).collect();
            if mark_as_read && !ids.is_empty() {
                client.ack_messages(ids)?;
            }
            let mut value = serde_json::to_value(result)
                .map_err(|error| AppError::internal(format!("failed to encode inbox: {error}")))?;
            strip_inbox_response_fields(&mut value);
            Ok(value)
        }
        "wait_for_messages" => {
            let mark_as_read = arguments
                .get("markAsRead")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let from_agent = read_optional_string(&arguments, "fromAgent");
            let thread_id = read_optional_string(&arguments, "threadId");
            let reply_to_message_id = read_optional_i64(&arguments, "replyToMessageId");
            let message_type = read_optional_string(&arguments, "messageType");
            let limit = read_optional_i64(&arguments, "limit").or(Some(20));
            let timeout_ms = arguments
                .get("timeoutMs")
                .and_then(Value::as_u64)
                .or(Some(30_000));
            let result = client.wait_for_messages(
                from_agent,
                thread_id,
                reply_to_message_id,
                message_type,
                limit,
                timeout_ms,
            )?;
            let ids: Vec<i64> = result.messages.iter().map(|m| m.id).collect();
            if mark_as_read && !ids.is_empty() {
                client.ack_messages(ids)?;
            }
            let mut value = serde_json::to_value(result).map_err(|error| {
                AppError::internal(format!("failed to encode waited inbox: {error}"))
            })?;
            strip_wait_response_fields(&mut value);
            Ok(value)
        }
        "reconcile_inbox" => Ok(client.reconcile_inbox()?),
        "reconcile_tracker" => Ok(serde_json::to_value(client.reconcile_tracker()?).map_err(
            |error| {
                AppError::internal(format!(
                    "failed to encode reconcile tracker result: {error}"
                ))
            },
        )?),
        "call_http_integration" => Ok(serde_json::to_value(client.call_http_integration(
            read_required_i64(&arguments, "integrationId")?,
            read_required_string(&arguments, "method")?,
            read_required_string(&arguments, "path")?,
            arguments.get("query").cloned(),
            arguments.get("headers").cloned(),
            read_optional_string(&arguments, "body"),
            arguments.get("jsonBody").cloned(),
        )?)
        .map_err(|error| {
            AppError::internal(format!(
                "failed to encode brokered integration response: {error}"
            ))
        })?),
        "call_cli_integration" => Ok(serde_json::to_value(client.call_cli_integration(
            read_required_i64(&arguments, "integrationId")?,
            arguments.get("args").cloned(),
            read_optional_string(&arguments, "cwd"),
            read_optional_string(&arguments, "stdin"),
        )?)
        .map_err(|error| {
            AppError::internal(format!(
                "failed to encode CLI integration response: {error}"
            ))
        })?),
        _ => Err(AppError::invalid_input(format!(
            "unknown tool: {tool_name}"
        ))),
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
                        "description": "Work item DB id. Use callSign instead when you have it."
                    },
                    "callSign": {
                        "type": "string",
                        "description": "Call sign (e.g. PJTCMD-56 or PJTCMD-56.01) — preferred over id."
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
                        "description": "Reparent the item under a new parent. Mutually exclusive with clearParent. The item must currently have no children (max 2 levels), and the new parent must be a top-level item in the same project."
                    },
                    "clearParent": {
                        "type": "boolean",
                        "description": "Detach the item from its current parent, making it top-level. Mutually exclusive with parentWorkItemId."
                    }
                },
                "required": [],
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
                        "description": "Work item DB id. Use callSign instead when you have it."
                    },
                    "callSign": {
                        "type": "string",
                        "description": "Call sign (e.g. PJTCMD-56 or PJTCMD-56.01) — preferred over id."
                    }
                },
                "required": [],
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
            "description": "Ensure a worktree for a work item and launch or reconnect to its SDK-backed worker session. Optionally specify a provider-specific model override. For Claude worker profiles use Claude model ids such as opus/sonnet/haiku; for Codex worker profiles use OpenAI model ids such as gpt-5.4 or gpt-5.4-mini.",
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
                        "description": "Optional provider-specific model override for the selected worker profile."
                    },
                    "executionMode": {
                        "type": "string",
                        "enum": ["plan", "build", "plan_and_build"],
                        "description": "Controls the agent's execution strategy. 'plan': agent writes a plan and requests approval before writing code. 'build': agent implements immediately (default). 'plan_and_build': agent plans then implements without waiting for approval."
                    }
                },
                "required": ["workItemId"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "cleanup_worktree",
            "description": "Clean up a worktree: remove the git worktree, delete the branch (best-effort), and drop the DB record. Blocked by a live session or pinned worktree. If the work item is in_progress or blocked it is automatically set to parked. If the branch has unmerged commits you must pass force=true. Also blocked if the worktree has uncommitted staged changes; commit or stash them first, or pass force=true to discard.",
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
            "description": "Forcefully terminate a running worktree agent session and clean up the PTY host. Use this when an agent is stuck, completed but lingering, or needs to be stopped before worktree cleanup.",
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
            "description": "Send a broker message to another agent in the project. Preserve threadId and set replyToMessageId when replying inside an existing conversation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "to": {
                        "type": "string",
                        "description": "Recipient agent name (e.g. 'dispatcher', 'AGENT-42')."
                    },
                    "messageType": {
                        "type": "string",
                        "enum": ["question", "blocked", "complete", "options", "status_update", "request_approval", "handoff", "directive"],
                        "description": "Message type that classifies intent."
                    },
                    "body": {
                        "type": "string",
                        "description": "Message body text."
                    },
                    "contextJson": {
                        "type": "string",
                        "description": "Optional JSON string with additional structured context."
                    },
                    "threadId": {
                        "type": "string",
                        "description": "Optional thread identifier. Reuse the incoming threadId when replying in an existing conversation."
                    },
                    "replyToMessageId": {
                        "type": "integer",
                        "description": "Optional message id this reply answers. Set this when responding to a specific broker message."
                    }
                },
                "required": ["to", "messageType", "body"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "publish_status",
            "description": "Publish the current worker-host lifecycle state for this session. Use this to surface ready/busy/idle/blocked/failed transitions without relying on terminal text.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "state": {
                        "type": "string",
                        "enum": ["launching", "ready", "busy", "waiting_for_tool", "waiting_for_permission", "idle", "blocked", "completed", "failed", "shell_fallback"],
                        "description": "Structured worker state to record for this session."
                    },
                    "detail": {
                        "type": "string",
                        "description": "Optional short human-readable detail for the state transition."
                    },
                    "threadId": {
                        "type": "string",
                        "description": "Optional provider or broker thread id associated with the current turn."
                    },
                    "providerSessionId": {
                        "type": "string",
                        "description": "Optional provider-native session/thread id to persist when it changes at runtime."
                    },
                    "contextJson": {
                        "description": "Optional structured JSON context to persist with the lifecycle event."
                    }
                },
                "required": ["state"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "heartbeat",
            "description": "Refresh the current session heartbeat without emitting terminal output. Use this during long waits or long-running tool turns.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "detail": {
                        "type": "string",
                        "description": "Optional short reason for the heartbeat."
                    },
                    "contextJson": {
                        "description": "Optional structured JSON context for future diagnostics."
                    }
                },
                "required": [],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "mark_done",
            "description": "Record that the current worker turn reached a completed state with a short summary. This does not terminate the session; it logs directive-level completion.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "summary": {
                        "type": "string",
                        "description": "Short summary of what just completed."
                    },
                    "threadId": {
                        "type": "string",
                        "description": "Optional provider or broker thread id associated with the completed turn."
                    },
                    "providerSessionId": {
                        "type": "string",
                        "description": "Optional provider-native session/thread id to persist when it changes at runtime."
                    },
                    "contextJson": {
                        "description": "Optional structured JSON context to persist with the completion event."
                    }
                },
                "required": ["summary"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "list_messages",
            "description": "Query message history for review or audit. Never marks messages as read. Use get_messages for routine inbox checks.",
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
                    "threadId": {
                        "type": "string",
                        "description": "Optional filter by thread id."
                    },
                    "replyToMessageId": {
                        "type": "integer",
                        "description": "Optional filter by parent message id."
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
                        "description": "Maximum number of messages to return. Defaults to 50."
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
            "name": "reconcile_tracker",
            "description": "Regenerate the dynamic sections of the project tracker ({NS}-0) from current DB state. Call this at session start before reading the tracker. Preserves human-authored sections (About, Current Focus, Blockers, Key Decisions) and regenerates Epics, Top-Level Items, Standalone, Active Worktrees, and Pending Inbox from live data.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "get_messages",
            "description": "Check this agent's inbox. Returns unread messages by default and marks them as read. Use this for routine inbox checks.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "unreadOnly": {
                        "type": "boolean",
                        "description": "When true (default), return only unread messages. Set to false to include already-read messages."
                    },
                    "markAsRead": {
                        "type": "boolean",
                        "description": "When true (default), mark returned messages as read."
                    },
                    "fromAgent": {
                        "type": "string",
                        "description": "Optional filter by sender agent name."
                    },
                    "threadId": {
                        "type": "string",
                        "description": "Optional filter by thread id."
                    },
                    "replyToMessageId": {
                        "type": "integer",
                        "description": "Optional filter by parent message id."
                    },
                    "messageType": {
                        "type": "string",
                        "description": "Optional filter by message type (e.g. 'directive', 'complete', 'question')."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of messages to return. Defaults to 20."
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
            "name": "wait_for_messages",
            "description": "Block until this agent has unread broker messages or the timeout elapses. Use this to wait on dispatcher or worker replies without mailbox polling.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "timeoutMs": {
                        "type": "integer",
                        "description": "Maximum time to wait in milliseconds. Defaults to 30000."
                    },
                    "markAsRead": {
                        "type": "boolean",
                        "description": "When true (default), mark returned messages as read."
                    },
                    "fromAgent": {
                        "type": "string",
                        "description": "Optional filter by sender agent name."
                    },
                    "threadId": {
                        "type": "string",
                        "description": "Optional filter by thread id."
                    },
                    "replyToMessageId": {
                        "type": "integer",
                        "description": "Optional filter by parent message id."
                    },
                    "messageType": {
                        "type": "string",
                        "description": "Optional filter by message type."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of messages to return. Defaults to 20."
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
            "name": "reconcile_inbox",
            "description": "Mark all unread messages from agents whose sessions have ended as read. Call at dispatcher startup before checking your inbox to suppress noise from previous-session agents. Returns the count of messages marked stale.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "call_http_integration",
            "description": "Execute a supervisor-brokered HTTPS request through a configured integration template. Secrets stay in the Project Commander vault and are injected into template-defined headers only inside the supervisor.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "integrationId": {
                        "type": "integer",
                        "description": "Configured integration id from Settings -> Vault."
                    },
                    "method": {
                        "type": "string",
                        "description": "HTTP method allowed by the integration template."
                    },
                    "path": {
                        "type": "string",
                        "description": "Relative API path under the template's base URL, such as '/repos/owner/repo/issues'."
                    },
                    "query": {
                        "type": "object",
                        "description": "Optional query-string parameters as string pairs.",
                        "additionalProperties": {
                            "type": "string"
                        }
                    },
                    "headers": {
                        "type": "object",
                        "description": "Optional non-secret request headers as string pairs. Reserved auth and transport headers are rejected.",
                        "additionalProperties": {
                            "type": "string"
                        }
                    },
                    "body": {
                        "type": "string",
                        "description": "Optional raw request body."
                    },
                    "jsonBody": {
                        "type": ["object", "array"],
                        "description": "Optional JSON request body. Mutually exclusive with body."
                    }
                },
                "required": ["integrationId", "method", "path"],
                "additionalProperties": false
            },
            "_meta": {
                "anthropic/maxResultSizeChars": 200000
            }
        }),
        json!({
            "name": "call_cli_integration",
            "description": "Execute a supervisor-owned CLI integration template. Secrets stay in the Project Commander vault and are injected into template-defined environment variables only inside the supervisor-owned child process.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "integrationId": {
                        "type": "integer",
                        "description": "Configured integration id from Settings -> Vault."
                    },
                    "args": {
                        "type": "array",
                        "description": "Optional extra CLI arguments appended after the template's default args.",
                        "items": {
                            "type": "string"
                        }
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Optional working directory for the spawned CLI process."
                    },
                    "stdin": {
                        "type": "string",
                        "description": "Optional stdin payload written to the spawned CLI process."
                    }
                },
                "required": ["integrationId"],
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
        let bytes_read = reader.read_line(&mut first_line).map_err(|error| {
            AppError::io(format!(
                "failed to read Project Commander MCP input: {error}"
            ))
        })?;

        if bytes_read == 0 {
            return Ok(None);
        }

        if !first_line.trim().is_empty() {
            break;
        }
    }

    let trimmed = first_line.trim_end_matches(['\r', '\n']);

    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return serde_json::from_str(trimmed).map(Some).map_err(|error| {
            AppError::internal(format!(
                "failed to decode Project Commander MCP JSON line: {error}"
            ))
        });
    }

    let mut content_length = parse_content_length_header(trimmed)?;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).map_err(|error| {
            AppError::io(format!(
                "failed to read Project Commander MCP header: {error}"
            ))
        })?;

        if bytes_read == 0 {
            return Ok(None);
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);

        if trimmed.is_empty() {
            break;
        }

        if let Some(value) = parse_content_length_header(trimmed)? {
            content_length = Some(value);
        }
    }

    let content_length =
        content_length.ok_or_else(|| AppError::io("missing Content-Length header"))?;
    let mut payload = vec![0_u8; content_length];
    reader.read_exact(&mut payload).map_err(|error| {
        AppError::io(format!(
            "failed to read Project Commander MCP payload: {error}"
        ))
    })?;

    serde_json::from_slice(&payload).map(Some).map_err(|error| {
        AppError::internal(format!(
            "failed to decode Project Commander MCP payload: {error}"
        ))
    })
}

fn write_message(writer: &mut impl Write, message: &Value) -> AppResult<()> {
    let raw = serde_json::to_string(message).map_err(|error| {
        AppError::internal(format!(
            "failed to encode Project Commander MCP response: {error}"
        ))
    })?;
    let header = format!("Content-Length: {}\r\n\r\n", raw.len());

    writer
        .write_all(header.as_bytes())
        .and_then(|_| writer.write_all(raw.as_bytes()))
        .and_then(|_| writer.flush())
        .map_err(|error| {
            AppError::io(format!(
                "failed to write Project Commander MCP response: {error}"
            ))
        })
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
    coerce_i64(arguments.get(key))
        .ok_or_else(|| AppError::invalid_input(format!("missing required integer field: {key}")))
}

fn read_optional_i64(arguments: &Value, key: &str) -> Option<i64> {
    coerce_i64(arguments.get(key))
}

/// Coerce a JSON value to i64, accepting both JSON numbers and numeric strings.
/// MCP clients sometimes send integer parameters as strings (e.g. `"101"` instead of `101`).
fn coerce_i64(value: Option<&Value>) -> Option<i64> {
    let v = value?;
    if let Some(n) = v.as_i64() {
        return Some(n);
    }
    v.as_str().and_then(|s| s.parse::<i64>().ok())
}

/// Strip noisy fields from get_messages inbox responses.
/// Removes sessionId (agents don't need it) and contextJson when empty.
fn strip_inbox_response_fields(value: &mut Value) {
    if let Some(messages) = value.get_mut("messages").and_then(Value::as_array_mut) {
        for msg in messages.iter_mut() {
            if let Some(obj) = msg.as_object_mut() {
                obj.remove("sessionId");
                if let Some(ctx) = obj.get("contextJson") {
                    let empty = ctx.as_str().map_or(true, |s| s.is_empty() || s == "{}");
                    if empty {
                        obj.remove("contextJson");
                    }
                }
            }
        }
    }
}

fn parse_content_length_header(header_line: &str) -> AppResult<Option<usize>> {
    let Some((name, value)) = header_line.split_once(':') else {
        return Ok(None);
    };

    if !name.trim().eq_ignore_ascii_case("Content-Length") {
        return Ok(None);
    }

    value
        .trim()
        .parse::<usize>()
        .map(Some)
        .map_err(|error| AppError::io(format!("invalid Content-Length header: {error}")))
}

fn strip_wait_response_fields(value: &mut Value) {
    if let Some(messages) = value.get_mut("messages").and_then(Value::as_array_mut) {
        for msg in messages.iter_mut() {
            if let Some(obj) = msg.as_object_mut() {
                obj.remove("sessionId");
                if let Some(ctx) = obj.get("contextJson") {
                    let empty = ctx.as_str().map_or(true, |s| s.is_empty() || s == "{}");
                    if empty {
                        obj.remove("contextJson");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn write_message_emits_content_length_framing() {
        let mut buffer = Vec::new();
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "ok": true }
        });

        write_message(&mut buffer, &payload).expect("message should encode");

        let raw = String::from_utf8(buffer.clone()).expect("frame should be utf-8");
        assert!(raw.starts_with("Content-Length: "));
        assert!(raw.contains("\r\n\r\n"));

        let mut cursor = Cursor::new(buffer);
        let decoded = read_message(&mut cursor)
            .expect("message should decode")
            .expect("frame should contain one message");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn read_message_accepts_case_insensitive_content_length_headers() {
        let payload = br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let raw = format!(
            "content-length: {}\r\n\r\n{}",
            payload.len(),
            String::from_utf8_lossy(payload)
        );
        let mut cursor = Cursor::new(raw.into_bytes());

        let decoded = read_message(&mut cursor)
            .expect("message should decode")
            .expect("frame should contain one message");

        assert_eq!(decoded["method"], "ping");
        assert_eq!(decoded["id"], 1);
    }
}
