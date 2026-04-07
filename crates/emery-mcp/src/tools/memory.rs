use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::rpc_client::RpcClient;

// ── Tool Descriptors ─────────────────────────────────────────────────────────

pub fn tool_memory_add() -> Value {
    json!({
        "name": "emery_memory_add",
        "description": "Add an atomic fact to the temporal memory layer. The content is embedded and reconciled against existing memories: semantically equivalent content is de-duplicated (NOOP or UPDATE), contradicting content supersedes the older memory. Requires VOYAGE_API_KEY in vault. ANTHROPIC_API_KEY is used for reconciliation when similarity is high; falls back to ADD if absent.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "content":    { "type": "string", "description": "The fact or insight to store (short, atomic, self-contained)" },
                "source_ref": { "type": "string", "description": "Optional reference to the source, e.g. \"session:abc123\" or \"wi:EMERY-217.002\"" },
                "namespace":  { "type": "string", "description": "Namespace for the memory (defaults to \"global\")" }
            },
            "required": ["content"]
        }
    })
}

pub fn tool_memory_search() -> Value {
    json!({
        "name": "emery_memory_search",
        "description": "Semantic search over memories using Voyage AI embeddings. Returns memories ranked by cosine similarity × recency decay. Supports time-travel via at_time. Requires VOYAGE_API_KEY in vault.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query_text": { "type": "string", "description": "Natural-language search query" },
                "limit":      { "type": "integer", "description": "Max results to return (default 10, max 100)" },
                "threshold":  { "type": "number",  "description": "Minimum final_score threshold (default 0.0)" },
                "namespace":  { "type": "string",  "description": "Restrict search to this namespace (e.g. global)" },
                "at_time":    { "type": "integer", "description": "Unix timestamp; if set, returns memories that were valid at that point in time (time-travel query)" }
            },
            "required": ["query_text"]
        }
    })
}

pub fn tool_memory_list() -> Value {
    json!({
        "name": "emery_memory_list",
        "description": "List memories without embedding scoring. By default returns only currently-valid memories (valid_to IS NULL). Use include_superseded to see full history.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "namespace":          { "type": "string",  "description": "Filter by namespace" },
                "limit":              { "type": "integer", "description": "Max memories to return (default 50)" },
                "include_superseded": { "type": "boolean", "description": "If true, include superseded memories (valid_to IS NOT NULL). Default false." }
            }
        }
    })
}

pub fn tool_memory_get() -> Value {
    json!({
        "name": "emery_memory_get",
        "description": "Fetch a single memory by ID.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "memory_id": { "type": "string", "description": "Memory ID" }
            },
            "required": ["memory_id"]
        }
    })
}

// ── Handlers ─────────────────────────────────────────────────────────────────

pub fn handle_memory_add(input: Value) -> Result<String> {
    let content = required_str(&input, "content")?;
    let mut params = json!({ "content": content });
    if let Some(v) = input["source_ref"].as_str() { params["source_ref"] = json!(v); }
    if let Some(v) = input["namespace"].as_str()  { params["namespace"]  = json!(v); }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("memory.add", params)?;

    let action = result["action"].as_str().unwrap_or("?");
    let id = result["memory"]["id"].as_str().unwrap_or("?");
    Ok(format!(
        "Memory {} ({}): {}",
        action, id, content
    ))
}

pub fn handle_memory_search(input: Value) -> Result<String> {
    let query_text = required_str(&input, "query_text")?;
    let mut params = json!({ "query_text": query_text });
    if let Some(v) = input["limit"].as_u64()      { params["limit"]     = json!(v); }
    if let Some(v) = input["threshold"].as_f64()  { params["threshold"] = json!(v); }
    if let Some(v) = input["namespace"].as_str()  { params["namespace"] = json!(v); }
    if let Some(v) = input["at_time"].as_i64()    { params["at_time"]   = json!(v); }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("memory.search", params)?;

    let items = result.as_array().map(|a| a.len()).unwrap_or(0);
    Ok(format!("{} memory result(s):\n{}", items, serde_json::to_string_pretty(&result)?))
}

pub fn handle_memory_list(input: Value) -> Result<String> {
    let mut params = json!({});
    if let Some(v) = input["namespace"].as_str()           { params["namespace"]          = json!(v); }
    if let Some(v) = input["limit"].as_u64()               { params["limit"]              = json!(v); }
    if let Some(v) = input["include_superseded"].as_bool() { params["include_superseded"] = json!(v); }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("memory.list", params)?;

    let items = result.as_array().map(|a| a.len()).unwrap_or(0);
    Ok(format!("{} memory/ies:\n{}", items, serde_json::to_string_pretty(&result)?))
}

pub fn handle_memory_get(input: Value) -> Result<String> {
    let memory_id = required_str(&input, "memory_id")?;
    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("memory.get", json!({ "memory_id": memory_id }))?;
    Ok(serde_json::to_string_pretty(&result)?)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn required_str(input: &Value, key: &str) -> Result<String> {
    input[key]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("missing required field: {}", key))
}
