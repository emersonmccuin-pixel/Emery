use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::rpc_client::RpcClient;

// ── Work Item Tool Descriptors ───────────────────────────────────────────────

pub fn tool_work_item_list() -> Value {
    json!({
        "name": "emery_work_item_list",
        "description": "List work items, scoped by namespace (preferred) or project_id. Provide at least one.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "namespace":        { "type": "string", "description": "Namespace to list work items from (e.g. EURI, FORGE). Preferred over project_id." },
                "project_id":       { "type": "string", "description": "Project ID (fallback if namespace not provided)" },
                "parent_id":        { "type": "string", "description": "Filter by parent work item ID" },
                "root_work_item_id": { "type": "string", "description": "Filter by root work item ID" },
                "status":           { "type": "string", "description": "Filter by status (e.g. backlog, in_progress, done)" },
                "work_item_type":   { "type": "string", "description": "Filter by type (e.g. task, bug, feature)" },
                "limit":            { "type": "integer", "description": "Max items to return" },
                "compact":          { "type": "boolean", "description": "If true, return only callsign, title, status, priority, and type (much smaller response). Default false." }
            }
        }
    })
}

pub fn tool_work_item_get() -> Value {
    json!({
        "name": "emery_work_item_get",
        "description": "Get detailed information about a single work item by ID.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "work_item_id": { "type": "string", "description": "Work item ID" }
            },
            "required": ["work_item_id"]
        }
    })
}

pub fn tool_work_item_create() -> Value {
    json!({
        "name": "emery_work_item_create",
        "description": "Create a new work item in the knowledge store. Provide namespace (preferred) or project_id.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "namespace":           { "type": "string", "description": "Namespace for the work item (e.g. EURI). Preferred over project_id." },
                "project_id":          { "type": "string", "description": "Project ID (fallback if namespace not provided)" },
                "parent_id":           { "type": "string", "description": "Parent work item ID (for sub-tasks)" },
                "title":               { "type": "string", "description": "Work item title" },
                "description":         { "type": "string", "description": "Detailed description (markdown)" },
                "acceptance_criteria":  { "type": "string", "description": "Acceptance criteria (markdown)" },
                "work_item_type":      { "type": "string", "description": "Type: task, bug, feature, epic, story" },
                "status":              { "type": "string", "description": "Initial status (default: backlog)" },
                "priority":            { "type": "string", "description": "Priority: critical, high, medium, low" },
                "created_by":          { "type": "string", "description": "Creator identifier (e.g. session ID)" }
            },
            "required": ["title", "description", "work_item_type"]
        }
    })
}

pub fn tool_work_item_update() -> Value {
    json!({
        "name": "emery_work_item_update",
        "description": "Update an existing work item. Only provided fields are changed.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "work_item_id":        { "type": "string", "description": "Work item ID to update" },
                "title":               { "type": "string", "description": "New title" },
                "description":         { "type": "string", "description": "New description" },
                "acceptance_criteria":  { "type": "string", "description": "New acceptance criteria" },
                "work_item_type":      { "type": "string", "description": "New type" },
                "status":              { "type": "string", "description": "New status (backlog, in_progress, done, cancelled)" },
                "priority":            { "type": "string", "description": "New priority" },
                "created_by":          { "type": "string", "description": "Updated creator" }
            },
            "required": ["work_item_id"]
        }
    })
}

// ── Document Tool Descriptors ────────────────────────────────────────────────

pub fn tool_document_list() -> Value {
    json!({
        "name": "emery_document_list",
        "description": "List documents, scoped by namespace (preferred) or project_id. Provide at least one.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "namespace":     { "type": "string", "description": "Namespace to list documents from (e.g. EURI). Preferred over project_id." },
                "project_id":    { "type": "string", "description": "Project ID (fallback if namespace not provided)" },
                "work_item_id":  { "type": "string", "description": "Filter by associated work item" },
                "session_id":    { "type": "string", "description": "Filter by originating session" },
                "doc_type":      { "type": "string", "description": "Filter by document type (e.g. prd, architecture, notes)" },
                "status":        { "type": "string", "description": "Filter by status (e.g. draft, published, archived)" },
                "limit":         { "type": "integer", "description": "Max items to return" }
            }
        }
    })
}

pub fn tool_document_get() -> Value {
    json!({
        "name": "emery_document_get",
        "description": "Get a single document with full content by ID.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "document_id": { "type": "string", "description": "Document ID" }
            },
            "required": ["document_id"]
        }
    })
}

pub fn tool_document_create() -> Value {
    json!({
        "name": "emery_document_create",
        "description": "Create a new document in the knowledge store. Provide namespace (preferred) or project_id.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "namespace":        { "type": "string", "description": "Namespace for the document (e.g. EURI). Preferred over project_id." },
                "project_id":       { "type": "string", "description": "Project ID (fallback if namespace not provided)" },
                "work_item_id":     { "type": "string", "description": "Associated work item ID (optional)" },
                "session_id":       { "type": "string", "description": "Originating session ID (optional)" },
                "doc_type":         { "type": "string", "description": "Document type: prd, architecture, notes, decision, runbook" },
                "title":            { "type": "string", "description": "Document title" },
                "slug":             { "type": "string", "description": "URL-safe slug (auto-generated from title if omitted)" },
                "status":           { "type": "string", "description": "Initial status (default: draft)" },
                "content_markdown": { "type": "string", "description": "Document content in markdown" }
            },
            "required": ["doc_type", "title", "content_markdown"]
        }
    })
}

pub fn tool_document_update() -> Value {
    json!({
        "name": "emery_document_update",
        "description": "Update an existing document. Only provided fields are changed.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "document_id":      { "type": "string", "description": "Document ID to update" },
                "work_item_id":     { "type": "string", "description": "New associated work item ID" },
                "session_id":       { "type": "string", "description": "New originating session ID" },
                "doc_type":         { "type": "string", "description": "New document type" },
                "title":            { "type": "string", "description": "New title" },
                "slug":             { "type": "string", "description": "New slug" },
                "status":           { "type": "string", "description": "New status" },
                "content_markdown": { "type": "string", "description": "New content" }
            },
            "required": ["document_id"]
        }
    })
}

// ── Work Item Handlers ───────────────────────────────────────────────────────

pub fn handle_work_item_list(input: Value) -> Result<String> {
    let mut params = json!({});
    if let Some(v) = input["namespace"].as_str() { params["namespace"] = json!(v); }
    if let Some(v) = input["project_id"].as_str() { params["project_id"] = json!(v); }
    if let Some(v) = input["parent_id"].as_str() { params["parent_id"] = json!(v); }
    if let Some(v) = input["root_work_item_id"].as_str() { params["root_work_item_id"] = json!(v); }
    if let Some(v) = input["status"].as_str() { params["status"] = json!(v); }
    if let Some(v) = input["work_item_type"].as_str() { params["work_item_type"] = json!(v); }
    if let Some(v) = input["limit"].as_u64() { params["limit"] = json!(v); }

    let compact = input["compact"].as_bool().unwrap_or(false);

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("work_item.list", params)?;

    let items = result.as_array().map(|a| a.len()).unwrap_or(0);

    if compact {
        // Return only essential fields to keep response small
        let summary: Vec<Value> = result
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|item| {
                json!({
                    "id": item["id"],
                    "callsign": item["callsign"],
                    "title": item["title"],
                    "status": item["status"],
                    "priority": item["priority"],
                    "work_item_type": item["work_item_type"],
                })
            })
            .collect();
        Ok(format!("{} work item(s):\n{}", items, serde_json::to_string_pretty(&summary)?))
    } else {
        Ok(format!("{} work item(s) returned:\n{}", items, serde_json::to_string_pretty(&result)?))
    }
}

pub fn handle_work_item_get(input: Value) -> Result<String> {
    let work_item_id = required_str(&input, "work_item_id")?;
    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("work_item.get", json!({ "work_item_id": work_item_id }))?;
    Ok(serde_json::to_string_pretty(&result)?)
}

pub fn handle_work_item_create(input: Value) -> Result<String> {
    let title = required_str(&input, "title")?;
    let description = required_str(&input, "description")?;
    let work_item_type = required_str(&input, "work_item_type")?;

    let mut params = json!({
        "title": title,
        "description": description,
        "work_item_type": work_item_type,
    });
    if let Some(v) = input["namespace"].as_str() { params["namespace"] = json!(v); }
    if let Some(v) = input["project_id"].as_str() { params["project_id"] = json!(v); }
    if let Some(v) = input["parent_id"].as_str() { params["parent_id"] = json!(v); }
    if let Some(v) = input["acceptance_criteria"].as_str() { params["acceptance_criteria"] = json!(v); }
    if let Some(v) = input["status"].as_str() { params["status"] = json!(v); }
    if let Some(v) = input["priority"].as_str() { params["priority"] = json!(v); }
    if let Some(v) = input["created_by"].as_str() { params["created_by"] = json!(v); }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("work_item.create", params)?;

    let id = result["id"].as_str().unwrap_or("?");
    let callsign = result["callsign"].as_str().unwrap_or("?");
    Ok(format!("Created work item `{}` ({}): {}", callsign, id, title))
}

pub fn handle_work_item_update(input: Value) -> Result<String> {
    let work_item_id = required_str(&input, "work_item_id")?;

    let mut params = json!({ "work_item_id": work_item_id });
    if let Some(v) = input["title"].as_str() { params["title"] = json!(v); }
    if let Some(v) = input["description"].as_str() { params["description"] = json!(v); }
    if let Some(v) = input["acceptance_criteria"].as_str() { params["acceptance_criteria"] = json!(v); }
    if let Some(v) = input["work_item_type"].as_str() { params["work_item_type"] = json!(v); }
    if let Some(v) = input["status"].as_str() { params["status"] = json!(v); }
    if let Some(v) = input["priority"].as_str() { params["priority"] = json!(v); }
    if let Some(v) = input["created_by"].as_str() { params["created_by"] = json!(v); }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("work_item.update", params)?;

    let callsign = result["callsign"].as_str().unwrap_or("?");
    Ok(format!("Updated work item `{}`", callsign))
}

// ── Document Handlers ────────────────────────────────────────────────────────

pub fn handle_document_list(input: Value) -> Result<String> {
    let mut params = json!({});
    if let Some(v) = input["namespace"].as_str() { params["namespace"] = json!(v); }
    if let Some(v) = input["project_id"].as_str() { params["project_id"] = json!(v); }
    if let Some(v) = input["work_item_id"].as_str() { params["work_item_id"] = json!(v); }
    if let Some(v) = input["session_id"].as_str() { params["session_id"] = json!(v); }
    if let Some(v) = input["doc_type"].as_str() { params["doc_type"] = json!(v); }
    if let Some(v) = input["status"].as_str() { params["status"] = json!(v); }
    if let Some(v) = input["limit"].as_u64() { params["limit"] = json!(v); }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("document.list", params)?;

    let items = result.as_array().map(|a| a.len()).unwrap_or(0);
    Ok(format!("{} document(s) returned:\n{}", items, serde_json::to_string_pretty(&result)?))
}

pub fn handle_document_get(input: Value) -> Result<String> {
    let document_id = required_str(&input, "document_id")?;
    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("document.get", json!({ "document_id": document_id }))?;
    Ok(serde_json::to_string_pretty(&result)?)
}

pub fn handle_document_create(input: Value) -> Result<String> {
    let doc_type = required_str(&input, "doc_type")?;
    let title = required_str(&input, "title")?;
    let content_markdown = required_str(&input, "content_markdown")?;

    let mut params = json!({
        "doc_type": doc_type,
        "title": title,
        "content_markdown": content_markdown,
    });
    if let Some(v) = input["namespace"].as_str() { params["namespace"] = json!(v); }
    if let Some(v) = input["project_id"].as_str() { params["project_id"] = json!(v); }
    if let Some(v) = input["work_item_id"].as_str() { params["work_item_id"] = json!(v); }
    if let Some(v) = input["session_id"].as_str() { params["session_id"] = json!(v); }
    if let Some(v) = input["slug"].as_str() { params["slug"] = json!(v); }
    if let Some(v) = input["status"].as_str() { params["status"] = json!(v); }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("document.create", params)?;

    let id = result["id"].as_str().unwrap_or("?");
    let slug = result["slug"].as_str().unwrap_or("?");
    Ok(format!("Created document `{}` ({}): {}", slug, id, title))
}

pub fn handle_document_update(input: Value) -> Result<String> {
    let document_id = required_str(&input, "document_id")?;

    let mut params = json!({ "document_id": document_id });
    if let Some(v) = input["work_item_id"].as_str() { params["work_item_id"] = json!(v); }
    if let Some(v) = input["session_id"].as_str() { params["session_id"] = json!(v); }
    if let Some(v) = input["doc_type"].as_str() { params["doc_type"] = json!(v); }
    if let Some(v) = input["title"].as_str() { params["title"] = json!(v); }
    if let Some(v) = input["slug"].as_str() { params["slug"] = json!(v); }
    if let Some(v) = input["status"].as_str() { params["status"] = json!(v); }
    if let Some(v) = input["content_markdown"].as_str() { params["content_markdown"] = json!(v); }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("document.update", params)?;

    let slug = result["slug"].as_str().unwrap_or("?");
    Ok(format!("Updated document `{}`", slug))
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn required_str(input: &Value, key: &str) -> Result<String> {
    input[key]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("missing required field: {}", key))
}
