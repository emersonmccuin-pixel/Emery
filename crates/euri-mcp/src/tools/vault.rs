use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::rpc_client::RpcClient;

// ── Tool descriptors ─────────────────────────────────────────────────────────

pub fn tool_vault_status() -> Value {
    json!({
        "name": "euri_vault_status",
        "description": "Check the current vault lock state — whether it is unlocked, when it was unlocked, and when it expires.",
        "inputSchema": {
            "type": "object",
            "properties": {}
        }
    })
}

pub fn tool_vault_unlock() -> Value {
    json!({
        "name": "euri_vault_unlock",
        "description": "Unlock the vault to allow reading and writing secret values. Optionally specify a duration in minutes (default 60).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "duration_minutes": {
                    "type": "integer",
                    "description": "How long to keep the vault unlocked (minutes). Defaults to 60."
                }
            }
        }
    })
}

pub fn tool_vault_lock() -> Value {
    json!({
        "name": "euri_vault_lock",
        "description": "Lock the vault immediately, preventing access to secret values until unlocked again.",
        "inputSchema": {
            "type": "object",
            "properties": {}
        }
    })
}

pub fn tool_vault_list() -> Value {
    json!({
        "name": "euri_vault_list",
        "description": "List vault entries (metadata only — no decrypted values). Optionally filter by scope.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "description": "Filter entries by scope (e.g. 'global' or a project_id). Omit to list all scopes."
                }
            }
        }
    })
}

pub fn tool_vault_set() -> Value {
    json!({
        "name": "euri_vault_set",
        "description": "Create or update a vault entry. If an entry with the same scope+key already exists, it is updated.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "description": "Scope for this entry (e.g. 'global' or a project_id)"
                },
                "key": {
                    "type": "string",
                    "description": "Key name for this entry (e.g. 'OPENAI_API_KEY')"
                },
                "value": {
                    "type": "string",
                    "description": "The secret value to store (encrypted at rest)"
                },
                "description": {
                    "type": "string",
                    "description": "Optional human-readable description of this entry"
                }
            },
            "required": ["scope", "key", "value"]
        }
    })
}

pub fn tool_vault_delete() -> Value {
    json!({
        "name": "euri_vault_delete",
        "description": "Delete a vault entry by its ID.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Vault entry ID to delete"
                }
            },
            "required": ["id"]
        }
    })
}

// ── Tool handlers ─────────────────────────────────────────────────────────────

pub fn handle_vault_status(_input: Value) -> Result<String> {
    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("vault.status", json!({}))?;

    let unlocked = result["unlocked"].as_bool().unwrap_or(false);
    let unlocked_at = result["unlocked_at"].as_i64();
    let expires_at = result["unlock_expires_at"].as_i64();

    if unlocked {
        let mut msg = "Vault is unlocked.".to_string();
        if let Some(exp) = expires_at {
            msg.push_str(&format!(" Expires at unix timestamp {}.", exp));
        }
        if let Some(ua) = unlocked_at {
            msg.push_str(&format!(" Unlocked at unix timestamp {}.", ua));
        }
        Ok(msg)
    } else {
        Ok("Vault is locked.".to_string())
    }
}

pub fn handle_vault_unlock(input: Value) -> Result<String> {
    let duration_minutes = input["duration_minutes"].as_i64();

    let mut params = json!({});
    if let Some(d) = duration_minutes {
        params["duration_minutes"] = json!(d);
    }

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("vault.unlock", params)?;

    let expires_at = result["unlock_expires_at"].as_i64();
    let mut msg = "Vault unlocked.".to_string();
    if let Some(exp) = expires_at {
        msg.push_str(&format!(" Auto-locks at unix timestamp {}.", exp));
    }
    Ok(msg)
}

pub fn handle_vault_lock(_input: Value) -> Result<String> {
    let mut rpc = RpcClient::connect()?;
    rpc.call("vault.lock", json!({}))?;
    Ok("Vault locked.".to_string())
}

pub fn handle_vault_list(input: Value) -> Result<String> {
    let scope = input["scope"].as_str().map(str::to_string);

    let mut params = json!({});
    if let Some(s) = &scope {
        params["scope"] = json!(s);
    }

    let mut rpc = RpcClient::connect()?;
    let entries = rpc.call("vault.list", params)?;

    let items = match entries.as_array() {
        Some(a) if !a.is_empty() => a,
        _ => return Ok("No vault entries found.".to_string()),
    };

    let mut table = String::from(
        "| ID | Scope | Key | Description | Created | Updated |\n\
         |----|-------|-----|-------------|---------|--------|\n",
    );

    for entry in items {
        let id = entry["id"].as_str().unwrap_or("-");
        let scope = entry["scope"].as_str().unwrap_or("-");
        let key = entry["key"].as_str().unwrap_or("-");
        let description = entry["description"].as_str().unwrap_or("-");
        let created_at = entry["created_at"].as_i64().map(|n| n.to_string()).unwrap_or_else(|| "-".to_string());
        let updated_at = entry["updated_at"].as_i64().map(|n| n.to_string()).unwrap_or_else(|| "-".to_string());

        table.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            id, scope, key, description, created_at, updated_at
        ));
    }

    Ok(table)
}

pub fn handle_vault_set(input: Value) -> Result<String> {
    let scope = required_str(&input, "scope")?;
    let key = required_str(&input, "key")?;
    let value = required_str(&input, "value")?;
    let description = input["description"].as_str().map(str::to_string);

    let mut params = json!({ "scope": scope, "key": key, "value": value });
    if let Some(d) = description {
        params["description"] = json!(d);
    }

    let mut rpc = RpcClient::connect()?;
    let entry = rpc.call("vault.set", params)?;

    let id = entry["id"].as_str().unwrap_or("unknown");
    Ok(format!(
        "Vault entry set: scope={}, key={}, id={}.",
        scope, key, id
    ))
}

pub fn handle_vault_delete(input: Value) -> Result<String> {
    let id = required_str(&input, "id")?;

    let mut rpc = RpcClient::connect()?;
    rpc.call("vault.delete", json!({ "id": id }))?;

    Ok(format!("Vault entry {} deleted.", id))
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn required_str(input: &Value, key: &str) -> Result<String> {
    input[key]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("missing required field: {}", key))
}
