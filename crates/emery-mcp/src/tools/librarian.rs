use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::rpc_client::RpcClient;

// ── Tool Descriptors ─────────────────────────────────────────────────────────

pub fn tool_librarian_digest() -> Value {
    json!({
        "name": "emery_librarian_digest",
        "description": "Read-only digest of recent session librarian capture activity. Groups kept memories by grain type (decision/insight/open_question/contradiction) and reports how many candidates were dropped by the critic. Optionally returns the dropped-candidate bodies so you can tune extraction prompts.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "namespace":       { "type": "string",  "description": "Restrict to a single namespace (e.g. EMERY). Omit to see all namespaces." },
                "since_days":      { "type": "integer", "description": "Look-back window in days (default 7, min 1)." },
                "include_dropped": { "type": "boolean", "description": "If true, include the bodies of critic-dropped candidates. Default false." }
            }
        }
    })
}

pub fn tool_gardener_run() -> Value {
    json!({
        "name": "emery_gardener_run",
        "description": "Run one gardener pass against a namespace. Propose-only — no memory is retired by this call. Hard 20% retirement cap and 24h per-namespace rate limit are enforced in code. Within the rate-limit window the previous run's proposals are returned unchanged.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "namespace":  { "type": "string",  "description": "Namespace to garden (required)." },
                "batch_size": { "type": "integer", "description": "Max memories to consider in one pass (default 50, max 200)." },
                "context":    { "type": "string",  "description": "Optional free-form context passed to the prompt — e.g. recent commit log or top-level dirs — for 'no longer exists' judgments." }
            },
            "required": ["namespace"]
        }
    })
}

pub fn tool_gardener_review() -> Value {
    json!({
        "name": "emery_gardener_review",
        "description": "List pending gardener proposals (proposals that have not yet been approved or rejected). Optionally filter by namespace.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "namespace": { "type": "string", "description": "Restrict to a single namespace. Omit to see all pending proposals." }
            }
        }
    })
}

pub fn tool_gardener_decide() -> Value {
    json!({
        "name": "emery_gardener_decide",
        "description": "Apply a user decision to a single gardener proposal. 'approve' retires the underlying memory (sets valid_to=now); 'reject' leaves it alone. Either way the proposal row is marked. Idempotent guard: a proposal that already has a decision returns an error.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "proposal_id": { "type": "string", "description": "Gardener proposal ID (gprp_…)." },
                "decision":    { "type": "string", "description": "Either 'approve' or 'reject'." }
            },
            "required": ["proposal_id", "decision"]
        }
    })
}

// ── Handlers ─────────────────────────────────────────────────────────────────

pub fn handle_librarian_digest(input: Value) -> Result<String> {
    let mut params = json!({});
    if let Some(v) = input["namespace"].as_str()       { params["namespace"]       = json!(v); }
    if let Some(v) = input["since_days"].as_i64()      { params["since_days"]      = json!(v); }
    if let Some(v) = input["include_dropped"].as_bool() { params["include_dropped"] = json!(v); }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("librarian.digest", params)?;
    Ok(serde_json::to_string_pretty(&result)?)
}

pub fn handle_gardener_run(input: Value) -> Result<String> {
    let namespace = required_str(&input, "namespace")?;
    let mut params = json!({ "namespace": namespace });
    if let Some(v) = input["batch_size"].as_u64() { params["batch_size"] = json!(v); }
    if let Some(v) = input["context"].as_str()    { params["context"]    = json!(v); }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("gardener.run", params)?;
    Ok(serde_json::to_string_pretty(&result)?)
}

pub fn handle_gardener_review(input: Value) -> Result<String> {
    let mut params = json!({});
    if let Some(v) = input["namespace"].as_str() { params["namespace"] = json!(v); }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("gardener.review", params)?;
    let count = result.as_array().map(|a| a.len()).unwrap_or(0);
    Ok(format!(
        "{} pending proposal(s):\n{}",
        count,
        serde_json::to_string_pretty(&result)?
    ))
}

pub fn handle_gardener_decide(input: Value) -> Result<String> {
    let proposal_id = required_str(&input, "proposal_id")?;
    let decision = required_str(&input, "decision")?;
    let mut rpc = RpcClient::connect()?;
    let result = rpc.call(
        "gardener.decide",
        json!({ "proposal_id": proposal_id, "decision": decision }),
    )?;
    Ok(serde_json::to_string_pretty(&result)?)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn required_str(input: &Value, key: &str) -> Result<String> {
    input[key]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("missing required field: {}", key))
}
