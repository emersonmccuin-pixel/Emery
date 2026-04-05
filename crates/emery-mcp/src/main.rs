mod rpc_client;
mod tools;

use std::io::{self, BufRead, BufReader, BufWriter, Write};

use anyhow::Result;
use serde_json::{Value, json};

fn main() -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }

        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }

        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("emery-mcp: failed to parse message: {e}");
                continue;
            }
        };

        // Notifications have no id — ignore them
        let Some(id) = msg.get("id") else {
            continue;
        };
        let id = id.clone();

        let method = msg["method"].as_str().unwrap_or("").to_string();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        let response = handle_request(&id, &method, params);
        let json_str = serde_json::to_string(&response)?;
        writeln!(writer, "{}", json_str)?;
        writer.flush()?;
    }

    Ok(())
}

fn handle_request(id: &Value, method: &str, params: Value) -> Value {
    match method {
        "initialize" => json_ok(id, json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "emery-mcp",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),

        "tools/list" => json_ok(id, json!({ "tools": tools::all_tools() })),

        "tools/call" => {
            let tool_name = params["name"].as_str().unwrap_or("").to_string();
            let input = params
                .get("arguments")
                .cloned()
                .unwrap_or(Value::Object(Default::default()));
            match tools::call_tool(&tool_name, input) {
                Ok(text) => json_ok(id, json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": false
                })),
                Err(e) => json_ok(id, json!({
                    "content": [{ "type": "text", "text": e.to_string() }],
                    "isError": true
                })),
            }
        }

        "ping" => json_ok(id, json!({})),

        _ => json_err(id, -32601, format!("Method not found: {method}")),
    }
}

fn json_ok(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn json_err(id: &Value, code: i32, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}
