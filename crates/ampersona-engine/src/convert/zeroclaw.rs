use anyhow::Result;
use serde_json::Value;

/// Import a zeroclaw config into an ampersona persona.
///
/// Expected zeroclaw config fields:
/// - `name` → persona name
/// - `identity.role` → role
/// - `identity.capabilities` → capabilities skills
/// - `security_policy.sandbox` → ext.zeroclaw.sandbox
/// - `security_policy.allowed_commands` → authority.actions.scoped.shell.commands
/// - `security_policy.allowed_paths` → authority.scope.allowed_paths
/// - `security_policy.forbidden_paths` → authority.scope.forbidden_paths
/// - `security_policy.autonomy` → authority.autonomy
pub fn import_zeroclaw(data: &Value) -> Result<Value> {
    let name = data
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Imported")
        .to_string();

    let role = data
        .pointer("/identity/role")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Build capabilities from identity.capabilities
    let capabilities = data
        .pointer("/identity/capabilities")
        .and_then(Value::as_array)
        .map(|caps| {
            let skills: Vec<Value> = caps
                .iter()
                .filter_map(|c| {
                    c.as_str().map(|s| {
                        serde_json::json!({
                            "name": s,
                            "description": "",
                            "priority": 5
                        })
                    })
                })
                .collect();
            serde_json::json!({ "skills": skills })
        });

    // Build authority from security_policy
    let mut authority = serde_json::json!({});
    if let Some(policy) = data.get("security_policy") {
        let autonomy = policy
            .get("autonomy")
            .and_then(Value::as_str)
            .unwrap_or("supervised");
        authority["autonomy"] = Value::String(autonomy.to_string());

        let mut scope = serde_json::json!({});
        if let Some(allowed) = policy.get("allowed_paths") {
            scope["allowed_paths"] = allowed.clone();
        }
        if let Some(forbidden) = policy.get("forbidden_paths") {
            scope["forbidden_paths"] = forbidden.clone();
        }
        authority["scope"] = scope;

        // Shell commands → scoped.shell
        if let Some(cmds) = policy.get("allowed_commands").and_then(Value::as_array) {
            authority["actions"] = serde_json::json!({
                "allow": [],
                "deny": [],
                "scoped": {
                    "shell": {
                        "$type": "shell",
                        "commands": cmds,
                        "block_high_risk": true,
                        "block_subshells": true,
                        "block_redirects": true,
                        "block_background": true
                    }
                }
            });
        }

        // Ext fields
        let mut ext = serde_json::json!({});
        let mut zc = serde_json::json!({});
        if let Some(sandbox) = policy.get("sandbox") {
            zc["sandbox"] = sandbox.clone();
        }
        if let Some(pairing) = policy.get("pairing_required") {
            zc["pairing_required"] = pairing.clone();
        }
        if !zc.as_object().unwrap().is_empty() {
            ext["zeroclaw"] = zc;
            authority["ext"] = ext;
        }
    }

    let mut persona = serde_json::json!({
        "version": "1.0",
        "name": name,
        "role": role,
        "authority": authority,
    });

    if let Some(caps) = capabilities {
        persona["capabilities"] = caps;
    }

    Ok(persona)
}

/// Export an ampersona persona to zeroclaw config format.
pub fn export_zeroclaw(data: &Value) -> Result<Value> {
    let name = data
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("exported")
        .to_string();

    let role = data
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Build identity
    let capabilities: Vec<String> = data
        .pointer("/capabilities/skills")
        .and_then(Value::as_array)
        .map(|skills| {
            skills
                .iter()
                .filter_map(|s| s.get("name").and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Build security_policy from authority
    let mut security_policy = serde_json::json!({});
    if let Some(auth) = data.get("authority") {
        let autonomy = auth
            .get("autonomy")
            .and_then(Value::as_str)
            .unwrap_or("supervised");
        security_policy["autonomy"] = Value::String(autonomy.to_string());

        if let Some(scope) = auth.get("scope") {
            if let Some(allowed) = scope.get("allowed_paths") {
                security_policy["allowed_paths"] = allowed.clone();
            }
            if let Some(forbidden) = scope.get("forbidden_paths") {
                security_policy["forbidden_paths"] = forbidden.clone();
            }
        }

        // Extract shell commands
        if let Some(cmds) = auth.pointer("/actions/scoped/shell/commands") {
            security_policy["allowed_commands"] = cmds.clone();
        }

        // Ext fields
        if let Some(zc) = auth.pointer("/ext/zeroclaw") {
            if let Some(sandbox) = zc.get("sandbox") {
                security_policy["sandbox"] = sandbox.clone();
            }
            if let Some(pairing) = zc.get("pairing_required") {
                security_policy["pairing_required"] = pairing.clone();
            }
        }
    }

    let config = serde_json::json!({
        "name": name,
        "identity": {
            "role": role,
            "capabilities": capabilities,
        },
        "security_policy": security_policy,
    });

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_basic_zeroclaw() {
        let zc = serde_json::json!({
            "name": "TestAgent",
            "identity": {
                "role": "worker",
                "capabilities": ["code_review", "testing"]
            },
            "security_policy": {
                "autonomy": "supervised",
                "sandbox": "landlock",
                "allowed_paths": ["src/**"],
                "allowed_commands": ["cargo", "git"]
            }
        });

        let persona = import_zeroclaw(&zc).unwrap();
        assert_eq!(persona["name"], "TestAgent");
        assert_eq!(persona["authority"]["autonomy"], "supervised");
        assert_eq!(
            persona["authority"]["ext"]["zeroclaw"]["sandbox"],
            "landlock"
        );
    }

    #[test]
    fn import_empty_object_no_panic() {
        let result = import_zeroclaw(&serde_json::json!({})).unwrap();
        assert_eq!(result["version"], "1.0");
    }

    #[test]
    fn export_empty_no_panic() {
        let result = export_zeroclaw(&serde_json::json!({})).unwrap();
        assert!(result.is_object());
    }

    #[test]
    fn export_roundtrip_preserves_name() {
        let persona = serde_json::json!({
            "version": "1.0",
            "name": "RoundTrip",
            "role": "architect",
            "authority": {
                "autonomy": "full",
                "ext": {
                    "zeroclaw": {
                        "sandbox": "landlock"
                    }
                }
            }
        });

        let exported = export_zeroclaw(&persona).unwrap();
        assert_eq!(exported["name"], "RoundTrip");
        assert_eq!(exported["security_policy"]["sandbox"], "landlock");

        // Re-import
        let reimported = import_zeroclaw(&exported).unwrap();
        assert_eq!(reimported["name"], "RoundTrip");
    }
}
