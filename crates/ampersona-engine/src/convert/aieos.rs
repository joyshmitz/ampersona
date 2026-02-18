//! Convert AIEOS identity format to ampersona.
//!
//! AIEOS (AI Entity Operating System) is zeroclaw's identity format.
//! This module provides a best-effort conversion preserving:
//! - Identity fields → psychology/voice/capabilities
//! - SecurityPolicy → authority (scope, actions, limits)
//! - Trust levels → gates

use anyhow::Result;
use serde_json::Value;

/// Convert an AIEOS identity JSON to ampersona v1.0 format.
pub fn import_aieos(aieos: &Value) -> Result<Value> {
    let mut persona = serde_json::json!({
        "$schema": "https://ampersona.dev/schema/v1.0/ampersona.schema.json",
        "version": "1.0"
    });

    let obj = persona.as_object_mut().unwrap();

    // Map name
    if let Some(name) = aieos.get("name").or_else(|| aieos.get("entity_id")) {
        obj.insert("name".into(), name.clone());
    }

    // Map role
    if let Some(role) = aieos.get("role").or_else(|| aieos.get("description")) {
        obj.insert("role".into(), role.clone());
    }

    // Map identity → psychology/voice/capabilities
    if let Some(identity) = aieos.get("identity") {
        // Capabilities from identity.capabilities
        if let Some(caps) = identity.get("capabilities") {
            if let Some(arr) = caps.as_array() {
                let skills: Vec<Value> = arr
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let name = c
                            .as_str()
                            .map(String::from)
                            .or_else(|| c.get("name").and_then(|n| n.as_str()).map(String::from))
                            .unwrap_or_else(|| format!("capability_{i}"));
                        let desc = c
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                            .to_string();
                        serde_json::json!({
                            "name": name,
                            "description": desc,
                            "priority": i + 1
                        })
                    })
                    .collect();
                obj.insert("capabilities".into(), serde_json::json!({"skills": skills}));
            }
        }

        // Directives from identity.directives or identity.goals
        let mut directives = serde_json::Map::new();
        if let Some(core_drive) = identity
            .get("core_directive")
            .or_else(|| identity.get("mission"))
        {
            directives.insert("core_drive".into(), core_drive.clone());
        }
        if let Some(goals) = identity.get("goals") {
            directives.insert("goals".into(), goals.clone());
        }
        if let Some(constraints) = identity.get("constraints") {
            directives.insert("constraints".into(), constraints.clone());
        }
        if !directives.is_empty() {
            obj.insert("directives".into(), Value::Object(directives));
        }
    }

    // Map security_policy → authority
    if let Some(policy) = aieos.get("security_policy").or_else(|| aieos.get("policy")) {
        let mut authority = serde_json::Map::new();

        // Autonomy from trust_level or autonomy
        let autonomy = policy
            .get("autonomy")
            .and_then(|v| v.as_str())
            .or_else(|| {
                policy
                    .get("trust_level")
                    .and_then(|v| v.as_str())
                    .map(|tl| match tl {
                        "full" | "trusted" => "full",
                        "restricted" | "untrusted" => "readonly",
                        _ => "supervised",
                    })
            })
            .unwrap_or("supervised");
        authority.insert("autonomy".into(), Value::String(autonomy.into()));

        // Scope from allowed_paths / forbidden_paths
        if policy.get("allowed_paths").is_some() || policy.get("forbidden_paths").is_some() {
            let mut scope = serde_json::Map::new();
            scope.insert("workspace_only".into(), Value::Bool(true));
            if let Some(ap) = policy.get("allowed_paths") {
                scope.insert("allowed_paths".into(), ap.clone());
            }
            if let Some(fp) = policy.get("forbidden_paths") {
                scope.insert("forbidden_paths".into(), fp.clone());
            }
            authority.insert("scope".into(), Value::Object(scope));
        }

        // Actions from allowed_actions / denied_actions
        if policy.get("allowed_actions").is_some() || policy.get("denied_actions").is_some() {
            let mut actions = serde_json::Map::new();
            if let Some(allow) = policy.get("allowed_actions") {
                actions.insert("allow".into(), allow.clone());
            }
            if let Some(deny) = policy.get("denied_actions") {
                // Convert simple strings to deny entries
                if let Some(arr) = deny.as_array() {
                    let deny_entries: Vec<Value> = arr
                        .iter()
                        .map(|d| {
                            if d.is_string() {
                                serde_json::json!({"action": d, "reason": "imported from AIEOS"})
                            } else {
                                d.clone()
                            }
                        })
                        .collect();
                    actions.insert("deny".into(), Value::Array(deny_entries));
                }
            }
            authority.insert("actions".into(), Value::Object(actions));
        }

        // Limits
        if let Some(rate_limit) = policy.get("rate_limit").and_then(|v| v.as_u64()) {
            authority.insert(
                "limits".into(),
                serde_json::json!({"max_actions_per_hour": rate_limit}),
            );
        }

        obj.insert("authority".into(), Value::Object(authority));
    }

    Ok(persona)
}

/// Export ampersona to AIEOS format (best-effort).
pub fn export_aieos(persona: &Value) -> Result<Value> {
    let mut aieos = serde_json::Map::new();

    if let Some(name) = persona.get("name") {
        aieos.insert("entity_id".into(), name.clone());
        aieos.insert("name".into(), name.clone());
    }

    if let Some(role) = persona.get("role") {
        aieos.insert("description".into(), role.clone());
    }

    // Build identity section
    let mut identity = serde_json::Map::new();
    if let Some(caps) = persona.pointer("/capabilities/skills") {
        if let Some(arr) = caps.as_array() {
            let capabilities: Vec<Value> = arr
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "name": s.get("name").cloned().unwrap_or(Value::Null),
                        "description": s.get("description").cloned().unwrap_or(Value::Null)
                    })
                })
                .collect();
            identity.insert("capabilities".into(), Value::Array(capabilities));
        }
    }
    if let Some(directives) = persona.get("directives") {
        if let Some(cd) = directives.get("core_drive") {
            identity.insert("mission".into(), cd.clone());
        }
        if let Some(goals) = directives.get("goals") {
            identity.insert("goals".into(), goals.clone());
        }
        if let Some(constraints) = directives.get("constraints") {
            identity.insert("constraints".into(), constraints.clone());
        }
    }
    if !identity.is_empty() {
        aieos.insert("identity".into(), Value::Object(identity));
    }

    // Build security_policy from authority
    if let Some(authority) = persona.get("authority") {
        let mut policy = serde_json::Map::new();

        if let Some(autonomy) = authority.get("autonomy") {
            policy.insert("autonomy".into(), autonomy.clone());
        }
        if let Some(scope) = authority.get("scope") {
            if let Some(ap) = scope.get("allowed_paths") {
                policy.insert("allowed_paths".into(), ap.clone());
            }
            if let Some(fp) = scope.get("forbidden_paths") {
                policy.insert("forbidden_paths".into(), fp.clone());
            }
        }
        if let Some(actions) = authority.get("actions") {
            if let Some(allow) = actions.get("allow") {
                policy.insert("allowed_actions".into(), allow.clone());
            }
            if let Some(deny) = actions.get("deny") {
                if let Some(arr) = deny.as_array() {
                    let denied: Vec<Value> = arr
                        .iter()
                        .map(|d| d.get("action").cloned().unwrap_or_else(|| d.clone()))
                        .collect();
                    policy.insert("denied_actions".into(), Value::Array(denied));
                }
            }
        }

        aieos.insert("security_policy".into(), Value::Object(policy));
    }

    Ok(Value::Object(aieos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_basic_aieos() {
        let aieos = serde_json::json!({
            "name": "TestBot",
            "role": "test agent",
            "security_policy": {
                "autonomy": "supervised",
                "allowed_actions": ["read_file", "write_file"],
                "rate_limit": 50
            }
        });

        let result = import_aieos(&aieos).unwrap();
        assert_eq!(result["name"], "TestBot");
        assert_eq!(result["version"], "1.0");
        assert_eq!(result["authority"]["autonomy"], "supervised");
        assert_eq!(result["authority"]["limits"]["max_actions_per_hour"], 50);
    }

    #[test]
    fn import_empty_object_no_panic() {
        let result = import_aieos(&serde_json::json!({})).unwrap();
        assert_eq!(result["version"], "1.0");
    }

    #[test]
    fn import_null_no_panic() {
        let result = import_aieos(&serde_json::Value::Null).unwrap();
        assert_eq!(result["version"], "1.0");
    }

    #[test]
    fn export_empty_no_panic() {
        let result = export_aieos(&serde_json::json!({})).unwrap();
        assert!(result.is_object());
    }

    #[test]
    fn export_roundtrip_preserves_name() {
        let persona = serde_json::json!({
            "name": "TestBot",
            "role": "test",
            "authority": {
                "autonomy": "supervised",
                "actions": {
                    "allow": ["read_file"]
                }
            }
        });

        let exported = export_aieos(&persona).unwrap();
        assert_eq!(exported["name"], "TestBot");
        assert_eq!(exported["security_policy"]["autonomy"], "supervised");
    }
}
