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

pub fn tool_memory_flag() -> Value {
    json!({
        "name": "emery_memory_flag",
        "description": "Record user feedback against a librarian-written memory. signal=noise retires the memory immediately (sets valid_to=now) AND records a feedback row — the row is never deleted, the audit trail is the point. Other signals (valuable, wrong_type, wrong_content) just record the row.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "memory_id": { "type": "string", "description": "Memory ID (mem_…)." },
                "signal":    { "type": "string", "description": "One of: noise, valuable, wrong_type, wrong_content." },
                "note":      { "type": "string", "description": "Optional free-form note explaining the feedback." }
            },
            "required": ["memory_id", "signal"]
        }
    })
}

pub fn tool_librarian_metrics() -> Value {
    json!({
        "name": "emery_librarian_metrics",
        "description": "Compute librarian health metrics over a window: capture rate, critic drop rate, gardener approval rate, noise-flag rate. Includes a per-prompt-version breakdown so you can see which prompt revisions produced the most user-flagged noise. The noise-flag rate is the headline number — if it climbs above ~10% in a week, the prompts are wrong.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "namespace":  { "type": "string",  "description": "Restrict to a single namespace. Omit for all." },
                "since_days": { "type": "integer", "description": "Look-back window in days (default 7, min 1)." }
            }
        }
    })
}

pub fn tool_librarian_config_get() -> Value {
    json!({
        "name": "emery_librarian_config_get",
        "description": "Read the per-namespace librarian tuning knobs (triage_min_score, max_grains_per_run, gardener_cap_percent, gardener_cooldown_h). Returns code defaults if no row has been written for the namespace.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "namespace": { "type": "string", "description": "Namespace (required)." }
            },
            "required": ["namespace"]
        }
    })
}

pub fn tool_librarian_config_set() -> Value {
    json!({
        "name": "emery_librarian_config_set",
        "description": "Update one or more per-namespace librarian tuning knobs. Only the supplied fields are changed; the rest are preserved (read-modify-write happens on the supervisor side).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "namespace":            { "type": "string",  "description": "Namespace (required)." },
                "triage_min_score":     { "type": "integer", "description": "Skip pipeline below this score (0..=3)." },
                "max_grains_per_run":   { "type": "integer", "description": "Hard cap on extractor output per run (1..=50)." },
                "gardener_cap_percent": { "type": "integer", "description": "Max % of memories the gardener may propose to retire per pass (1..=100)." },
                "gardener_cooldown_h":  { "type": "integer", "description": "Hours between gardener runs per namespace (1..=720)." }
            },
            "required": ["namespace"]
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

pub fn handle_memory_flag(input: Value) -> Result<String> {
    let memory_id = required_str(&input, "memory_id")?;
    let signal = required_str(&input, "signal")?;
    let mut params = json!({ "memory_id": memory_id, "signal": signal });
    if let Some(v) = input["note"].as_str() { params["note"] = json!(v); }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("memory.flag", params)?;
    Ok(serde_json::to_string_pretty(&result)?)
}

pub fn handle_librarian_metrics(input: Value) -> Result<String> {
    let mut params = json!({});
    if let Some(v) = input["namespace"].as_str()  { params["namespace"]  = json!(v); }
    if let Some(v) = input["since_days"].as_i64() { params["since_days"] = json!(v); }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("librarian.metrics", params)?;
    Ok(serde_json::to_string_pretty(&result)?)
}

pub fn handle_librarian_config_get(input: Value) -> Result<String> {
    let namespace = required_str(&input, "namespace")?;
    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("librarian.config_get", json!({ "namespace": namespace }))?;
    Ok(serde_json::to_string_pretty(&result)?)
}

pub fn handle_librarian_config_set(input: Value) -> Result<String> {
    let namespace = required_str(&input, "namespace")?;
    let mut params = json!({ "namespace": namespace });
    if let Some(v) = input["triage_min_score"].as_i64()     { params["triage_min_score"]     = json!(v); }
    if let Some(v) = input["max_grains_per_run"].as_i64()   { params["max_grains_per_run"]   = json!(v); }
    if let Some(v) = input["gardener_cap_percent"].as_i64() { params["gardener_cap_percent"] = json!(v); }
    if let Some(v) = input["gardener_cooldown_h"].as_i64()  { params["gardener_cooldown_h"]  = json!(v); }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("librarian.config_set", params)?;
    Ok(serde_json::to_string_pretty(&result)?)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn required_str(input: &Value, key: &str) -> Result<String> {
    input[key]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("missing required field: {}", key))
}
