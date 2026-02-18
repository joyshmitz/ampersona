use anyhow::Result;
use serde_json::{json, Value};

use crate::prompt;

/// Build the `register_agent` arguments from a persona JSON.
pub fn build_args(
    data: &Value,
    project: &str,
    program: &str,
    model: &str,
    include_prompt: bool,
    toon: bool,
) -> Result<Value> {
    let name = data
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Unknown");

    let task_description = if include_prompt {
        if toon {
            prompt::to_toon(data)?
        } else {
            prompt::to_system_prompt(data, &[])
        }
    } else {
        data.get("role")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };

    Ok(json!({
        "project_key": project,
        "program": program,
        "model": model,
        "name": name,
        "task_description": task_description
    }))
}

/// Wrap arguments in a JSON-RPC 2.0 envelope for `register_agent`.
pub fn wrap_rpc(args: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "1",
        "method": "tools/call",
        "params": {
            "name": "register_agent",
            "arguments": args
        }
    })
}
