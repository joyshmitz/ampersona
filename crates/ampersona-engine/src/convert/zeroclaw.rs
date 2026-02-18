//! Convert between ZeroClaw config format and ampersona v1.0.
//!
//! ZeroClaw config is a runtime agent configuration with identity,
//! security, and optional behavioral sections. This module normalizes
//! ZeroClaw configs into ampersona's persona schema.
//!
//! Normalization handles:
//! - Fallback paths (e.g. `security_policy.autonomy` OR `autonomy.level`)
//! - Autonomy level mapping (ZeroClaw levels → ampersona autonomy)
//! - Command/path security policy → authority scope
//! - ZeroClaw-specific fields preserved in `authority.ext.zeroclaw`
//! - Optional behavioral sections (psychology, voice, directives) with validation

use anyhow::Result;
use serde_json::{Map, Value};

// ── Utilities ────────────────────────────────────────────────────────

fn value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.as_object()?.get(*segment)?;
    }
    Some(current)
}

fn scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        }
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn numeric_from_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

fn unit_clamp(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Extract a list of strings from a JSON array (handles both `["a"]` and `[{"name":"a"}]`).
fn string_list(value: &Value) -> Vec<String> {
    let arr = match value.as_array() {
        Some(a) => a,
        None => return vec![],
    };
    arr.iter()
        .filter_map(|item| {
            item.as_str()
                .map(String::from)
                .or_else(|| item.get("name").and_then(Value::as_str).map(String::from))
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn string_array_at(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .map(|v| string_list(v))
        .unwrap_or_default()
}

fn normalize_alignment(raw: &str) -> Option<String> {
    let normalized = raw.trim().to_lowercase().replace(' ', "-");
    let valid = [
        "lawful-good",
        "neutral-good",
        "chaotic-good",
        "lawful-neutral",
        "true-neutral",
        "chaotic-neutral",
        "lawful-evil",
        "neutral-evil",
        "chaotic-evil",
    ];
    if valid.contains(&normalized.as_str()) {
        Some(normalized)
    } else {
        None
    }
}

/// Reverse alignment: "lawful-good" → "Lawful Good".
fn denormalize_alignment(alignment: &str) -> String {
    alignment
        .split('-')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(first) => first.to_uppercase().to_string() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ── ZeroClaw → ampersona normalization ───────────────────────────────

/// Extract name, role, backstory from ZeroClaw config.
fn normalize_identity(zc: &Value, obj: &mut Map<String, Value>) {
    let name = zc
        .get("name")
        .and_then(scalar_to_string)
        .or_else(|| value_at_path(zc, &["identity", "name"]).and_then(scalar_to_string));
    if let Some(name) = name {
        obj.insert("name".into(), Value::String(name));
    }

    let role = value_at_path(zc, &["identity", "role"])
        .and_then(scalar_to_string)
        .or_else(|| zc.get("role").and_then(scalar_to_string))
        .or_else(|| value_at_path(zc, &["identity", "description"]).and_then(scalar_to_string));
    if let Some(role) = role {
        obj.insert("role".into(), Value::String(role));
    }

    if let Some(backstory) =
        value_at_path(zc, &["identity", "backstory"]).and_then(scalar_to_string)
    {
        obj.insert("backstory".into(), Value::String(backstory));
    }
}

/// Normalize capabilities from `identity.capabilities` or top-level `capabilities`.
fn normalize_capabilities(zc: &Value) -> Option<Value> {
    let source = value_at_path(zc, &["identity", "capabilities"])
        .or_else(|| zc.get("capabilities"))?;

    let arr = source.get("skills").and_then(Value::as_array)
        .or_else(|| source.as_array())?;

    let skills: Vec<Value> = arr
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let name = item
                .as_str()
                .map(String::from)
                .or_else(|| item.get("name").and_then(scalar_to_string))?;
            let desc = item
                .get("description")
                .and_then(scalar_to_string)
                .unwrap_or_default();
            let priority = item
                .get("priority")
                .and_then(|v| v.as_u64())
                .unwrap_or((i + 1) as u64)
                .clamp(1, 10);
            Some(serde_json::json!({
                "name": name,
                "description": desc,
                "priority": priority,
            }))
        })
        .collect();

    if skills.is_empty() {
        return None;
    }
    Some(serde_json::json!({ "skills": skills }))
}

/// Build authority from `security_policy`, `autonomy`, `security`, and `gateway` sections.
fn normalize_authority(zc: &Value) -> Option<Value> {
    let policy = zc.get("security_policy");
    let autonomy_cfg = zc.get("autonomy");
    let security_cfg = zc.get("security");
    let gateway_cfg = zc.get("gateway");

    if policy.is_none() && autonomy_cfg.is_none() && security_cfg.is_none() {
        return None;
    }

    let mut authority = Map::new();

    // Autonomy level
    let autonomy_raw = policy
        .and_then(|p| p.get("autonomy"))
        .and_then(scalar_to_string)
        .or_else(|| autonomy_cfg.and_then(|a| a.get("level")).and_then(scalar_to_string))
        .or_else(|| autonomy_cfg.and_then(scalar_to_string))
        .unwrap_or_else(|| "supervised".into());

    let mapped = match autonomy_raw.to_lowercase().as_str() {
        "full" | "autonomous" => "full",
        "readonly" | "restricted" | "none" => "readonly",
        _ => "supervised",
    };
    authority.insert("autonomy".into(), Value::String(mapped.into()));

    // Scope
    let mut scope = Map::new();
    let mut has_scope = false;

    let workspace_only = policy
        .and_then(|p| p.get("workspace_only"))
        .and_then(Value::as_bool)
        .or_else(|| {
            autonomy_cfg
                .and_then(|a| a.get("workspace_only"))
                .and_then(Value::as_bool)
        });
    if let Some(wo) = workspace_only {
        scope.insert("workspace_only".into(), Value::Bool(wo));
        has_scope = true;
    }

    let allowed_paths = policy
        .and_then(|p| p.get("allowed_paths"))
        .or_else(|| autonomy_cfg.and_then(|a| a.get("allowed_paths")));
    if let Some(ap) = allowed_paths {
        scope.insert("allowed_paths".into(), ap.clone());
        has_scope = true;
    }

    let forbidden_paths = policy
        .and_then(|p| p.get("forbidden_paths"))
        .or_else(|| autonomy_cfg.and_then(|a| a.get("forbidden_paths")));
    if let Some(fp) = forbidden_paths {
        scope.insert("forbidden_paths".into(), fp.clone());
        has_scope = true;
    }

    if has_scope {
        authority.insert("scope".into(), Value::Object(scope));
    }

    // Actions: allowed_commands → scoped.shell
    let allowed_commands = policy
        .and_then(|p| p.get("allowed_commands"))
        .or_else(|| autonomy_cfg.and_then(|a| a.get("allowed_commands")));
    let auto_approve = autonomy_cfg.and_then(|a| a.get("auto_approve"));

    if allowed_commands.is_some() || auto_approve.is_some() {
        let mut actions = Map::new();

        if let Some(aa) = auto_approve.and_then(Value::as_array) {
            actions.insert("allow".into(), Value::Array(aa.clone()));
        }

        if let Some(cmds) = allowed_commands.and_then(Value::as_array) {
            let block_high_risk = policy
                .and_then(|p| p.get("block_high_risk_commands"))
                .and_then(Value::as_bool)
                .or_else(|| {
                    autonomy_cfg
                        .and_then(|a| a.get("block_high_risk_commands"))
                        .and_then(Value::as_bool)
                })
                .unwrap_or(true);

            actions.insert(
                "scoped".into(),
                serde_json::json!({
                    "shell": {
                        "$type": "shell",
                        "commands": cmds,
                        "block_high_risk": block_high_risk,
                        "block_subshells": true,
                        "block_redirects": true,
                        "block_background": true
                    }
                }),
            );
        }

        if !actions.is_empty() {
            authority.insert("actions".into(), Value::Object(actions));
        }
    }

    // Limits
    let max_actions = policy
        .and_then(|p| p.get("max_actions_per_hour"))
        .and_then(|v| v.as_u64())
        .or_else(|| {
            autonomy_cfg
                .and_then(|a| a.get("max_actions_per_hour"))
                .and_then(|v| v.as_u64())
        });
    let max_cost = policy
        .and_then(|p| p.get("max_cost_per_day_cents"))
        .and_then(|v| v.as_u64())
        .or_else(|| {
            autonomy_cfg
                .and_then(|a| a.get("max_cost_per_day_cents"))
                .and_then(|v| v.as_u64())
        });

    if max_actions.is_some() || max_cost.is_some() {
        let mut limits = Map::new();
        if let Some(ma) = max_actions {
            limits.insert("max_actions_per_hour".into(), serde_json::json!(ma));
        }
        if let Some(mc) = max_cost {
            limits.insert("max_cost_per_day_cents".into(), serde_json::json!(mc));
        }
        authority.insert("limits".into(), Value::Object(limits));
    }

    // Ext: ZeroClaw-specific fields
    let mut zc_ext = Map::new();

    let sandbox = policy
        .and_then(|p| p.get("sandbox"))
        .and_then(scalar_to_string)
        .or_else(|| value_at_path(zc, &["security", "sandbox", "backend"]).and_then(scalar_to_string))
        .or_else(|| value_at_path(zc, &["security", "sandbox"]).and_then(scalar_to_string));
    if let Some(sb) = sandbox {
        zc_ext.insert("sandbox".into(), Value::String(sb));
    }

    let pairing = policy
        .and_then(|p| p.get("pairing_required"))
        .and_then(Value::as_bool)
        .or_else(|| {
            gateway_cfg
                .and_then(|g| g.get("require_pairing"))
                .and_then(Value::as_bool)
        });
    if let Some(pr) = pairing {
        zc_ext.insert("pairing_required".into(), Value::Bool(pr));
    }

    if let Some(aa) = autonomy_cfg.and_then(|a| a.get("always_ask")) {
        if aa.as_array().map_or(false, |a| !a.is_empty()) {
            zc_ext.insert("always_ask".into(), aa.clone());
        }
    }

    if let Some(approval) = autonomy_cfg
        .and_then(|a| a.get("require_approval_for_medium_risk"))
        .and_then(Value::as_bool)
    {
        zc_ext.insert(
            "require_approval_for_medium_risk".into(),
            Value::Bool(approval),
        );
    }

    if let Some(resources) = value_at_path(zc, &["security", "resources"]) {
        if resources.is_object() {
            zc_ext.insert("resources".into(), resources.clone());
        }
    }

    if !zc_ext.is_empty() {
        authority.insert(
            "ext".into(),
            serde_json::json!({ "zeroclaw": Value::Object(zc_ext) }),
        );
    }

    Some(Value::Object(authority))
}

/// Normalize optional `psychology` section with value clamping.
fn normalize_psychology(zc: &Value) -> Option<Value> {
    let section = zc.get("psychology")?;
    let mut psych = Map::new();

    // neural_matrix — clamp to [0,1]
    if let Some(nm) = section.get("neural_matrix").and_then(Value::as_object) {
        let dims = [
            "creativity",
            "empathy",
            "logic",
            "adaptability",
            "charisma",
            "reliability",
        ];
        let mut result = Map::new();
        for dim in &dims {
            if let Some(v) = nm.get(*dim).and_then(numeric_from_value) {
                result.insert((*dim).into(), serde_json::json!(unit_clamp(v)));
            }
        }
        if !result.is_empty() {
            psych.insert("neural_matrix".into(), Value::Object(result));
        }
    }

    // traits: mbti + ocean + temperament
    if let Some(traits) = section.get("traits") {
        let mut t = Map::new();
        if let Some(mbti) = traits.get("mbti").and_then(scalar_to_string) {
            t.insert("mbti".into(), Value::String(mbti));
        }
        if let Some(ocean) = traits.get("ocean").and_then(Value::as_object) {
            let dims = [
                "openness",
                "conscientiousness",
                "extraversion",
                "agreeableness",
                "neuroticism",
            ];
            let mut o = Map::new();
            for dim in &dims {
                if let Some(v) = ocean.get(*dim).and_then(numeric_from_value) {
                    o.insert((*dim).into(), serde_json::json!(unit_clamp(v)));
                }
            }
            if !o.is_empty() {
                t.insert("ocean".into(), Value::Object(o));
            }
        }
        if let Some(temp) = traits.get("temperament").and_then(scalar_to_string) {
            t.insert("temperament".into(), Value::String(temp));
        }
        if !t.is_empty() {
            psych.insert("traits".into(), Value::Object(t));
        }
    }

    // moral_compass
    if let Some(mc) = section.get("moral_compass") {
        let mut result = Map::new();
        if let Some(alignment_raw) = mc.get("alignment").and_then(scalar_to_string) {
            let normalized = normalize_alignment(&alignment_raw)
                .unwrap_or_else(|| alignment_raw.to_lowercase());
            result.insert("alignment".into(), Value::String(normalized));
        }
        if let Some(values) = mc.get("core_values").and_then(Value::as_array) {
            let filtered: Vec<Value> = values
                .iter()
                .filter_map(|v| scalar_to_string(v).map(Value::String))
                .collect();
            if !filtered.is_empty() {
                result.insert("core_values".into(), Value::Array(filtered));
            }
        }
        if !result.is_empty() {
            psych.insert("moral_compass".into(), Value::Object(result));
        }
    }

    // emotional_profile
    if let Some(ep) = section.get("emotional_profile") {
        let mut result = Map::new();
        if let Some(mood) = ep.get("base_mood").and_then(scalar_to_string) {
            result.insert("base_mood".into(), Value::String(mood));
        }
        if let Some(vol) = ep.get("volatility").and_then(numeric_from_value) {
            result.insert("volatility".into(), serde_json::json!(unit_clamp(vol)));
        }
        if !result.is_empty() {
            psych.insert("emotional_profile".into(), Value::Object(result));
        }
    }

    if psych.is_empty() {
        None
    } else {
        Some(Value::Object(psych))
    }
}

/// Normalize optional `voice` section with value clamping.
fn normalize_voice(zc: &Value) -> Option<Value> {
    let section = zc.get("voice")?;
    let mut voice = Map::new();

    if let Some(style) = section.get("style") {
        let mut s = Map::new();
        if let Some(descs) = style.get("descriptors").and_then(Value::as_array) {
            let filtered: Vec<Value> = descs
                .iter()
                .filter_map(|v| scalar_to_string(v).map(Value::String))
                .collect();
            if !filtered.is_empty() {
                s.insert("descriptors".into(), Value::Array(filtered));
            }
        }
        if let Some(f) = style.get("formality").and_then(numeric_from_value) {
            s.insert("formality".into(), serde_json::json!(unit_clamp(f)));
        }
        if let Some(v) = style.get("verbosity").and_then(numeric_from_value) {
            s.insert("verbosity".into(), serde_json::json!(unit_clamp(v)));
        }
        if !s.is_empty() {
            voice.insert("style".into(), Value::Object(s));
        }
    }

    if let Some(syn) = section.get("syntax") {
        let mut result = Map::new();
        if let Some(structure) = syn.get("structure").and_then(scalar_to_string) {
            result.insert("structure".into(), Value::String(structure));
        }
        if let Some(contractions) = syn.get("contractions").and_then(Value::as_bool) {
            result.insert("contractions".into(), Value::Bool(contractions));
        }
        if !result.is_empty() {
            voice.insert("syntax".into(), Value::Object(result));
        }
    }

    if let Some(idiolect) = section.get("idiolect") {
        let mut id = Map::new();
        let catchphrases = string_array_at(idiolect, "catchphrases");
        if !catchphrases.is_empty() {
            id.insert(
                "catchphrases".into(),
                Value::Array(catchphrases.into_iter().map(Value::String).collect()),
            );
        }
        let forbidden = string_array_at(idiolect, "forbidden_words");
        if !forbidden.is_empty() {
            id.insert(
                "forbidden_words".into(),
                Value::Array(forbidden.into_iter().map(Value::String).collect()),
            );
        }
        if !id.is_empty() {
            voice.insert("idiolect".into(), Value::Object(id));
        }
    }

    if voice.is_empty() {
        None
    } else {
        Some(Value::Object(voice))
    }
}

/// Normalize optional `directives` section.
fn normalize_directives(zc: &Value) -> Option<Value> {
    let section = zc.get("directives")?;
    let mut result = Map::new();

    if let Some(cd) = section.get("core_drive").and_then(scalar_to_string) {
        result.insert("core_drive".into(), Value::String(cd));
    }
    let goals = string_array_at(section, "goals");
    if !goals.is_empty() {
        result.insert(
            "goals".into(),
            Value::Array(goals.into_iter().map(Value::String).collect()),
        );
    }
    let constraints = string_array_at(section, "constraints");
    if !constraints.is_empty() {
        result.insert(
            "constraints".into(),
            Value::Array(constraints.into_iter().map(Value::String).collect()),
        );
    }

    if result.is_empty() {
        None
    } else {
        Some(Value::Object(result))
    }
}

// ── Public API ──────────────────────────────────────────────────────

/// Import a ZeroClaw config into an ampersona persona.
///
/// Handles both the simplified interchange format and Config-like structures:
/// - `name` / `identity.name` → persona name
/// - `identity.role` / `role` → role
/// - `identity.backstory` → backstory
/// - `identity.capabilities` / `capabilities` → capabilities
/// - `security_policy.*` / `autonomy.*` / `security.*` → authority
/// - `psychology`, `voice`, `directives` → behavioral sections (pass-through)
pub fn import_zeroclaw(data: &Value) -> Result<Value> {
    if !data.is_object() {
        anyhow::bail!("ZeroClaw config must be a JSON object");
    }

    let mut obj = Map::new();
    obj.insert("version".into(), Value::String("1.0".into()));

    normalize_identity(data, &mut obj);

    if let Some(psych) = normalize_psychology(data) {
        obj.insert("psychology".into(), psych);
    }
    if let Some(voice) = normalize_voice(data) {
        obj.insert("voice".into(), voice);
    }
    if let Some(caps) = normalize_capabilities(data) {
        obj.insert("capabilities".into(), caps);
    }
    if let Some(dir) = normalize_directives(data) {
        obj.insert("directives".into(), dir);
    }
    if let Some(auth) = normalize_authority(data) {
        obj.insert("authority".into(), auth);
    }

    Ok(Value::Object(obj))
}

/// Export an ampersona persona to ZeroClaw config format.
pub fn export_zeroclaw(data: &Value) -> Result<Value> {
    let mut config = Map::new();

    // Name
    if let Some(name) = data.get("name").and_then(scalar_to_string) {
        config.insert("name".into(), Value::String(name));
    }

    // Identity section
    let mut identity = Map::new();
    if let Some(role) = data.get("role").and_then(scalar_to_string) {
        identity.insert("role".into(), Value::String(role));
    }
    if let Some(backstory) = data.get("backstory").and_then(scalar_to_string) {
        identity.insert("backstory".into(), Value::String(backstory));
    }
    if let Some(skills) = data.pointer("/capabilities/skills").and_then(Value::as_array) {
        let caps: Vec<Value> = skills
            .iter()
            .filter_map(|s| s.get("name").and_then(scalar_to_string).map(Value::String))
            .collect();
        if !caps.is_empty() {
            identity.insert("capabilities".into(), Value::Array(caps));
        }
    }
    if !identity.is_empty() {
        config.insert("identity".into(), Value::Object(identity));
    }

    // Authority → security_policy
    if let Some(auth) = data.get("authority") {
        let mut policy = Map::new();

        if let Some(autonomy) = auth.get("autonomy").and_then(scalar_to_string) {
            policy.insert("autonomy".into(), Value::String(autonomy));
        }

        if let Some(scope) = auth.get("scope") {
            if let Some(wo) = scope.get("workspace_only") {
                policy.insert("workspace_only".into(), wo.clone());
            }
            if let Some(ap) = scope.get("allowed_paths") {
                policy.insert("allowed_paths".into(), ap.clone());
            }
            if let Some(fp) = scope.get("forbidden_paths") {
                policy.insert("forbidden_paths".into(), fp.clone());
            }
        }

        if let Some(cmds) = auth.pointer("/actions/scoped/shell/commands") {
            policy.insert("allowed_commands".into(), cmds.clone());
        }

        if let Some(limits) = auth.get("limits") {
            if let Some(ma) = limits.get("max_actions_per_hour") {
                policy.insert("max_actions_per_hour".into(), ma.clone());
            }
            if let Some(mc) = limits.get("max_cost_per_day_cents") {
                policy.insert("max_cost_per_day_cents".into(), mc.clone());
            }
        }

        // Ext fields → security_policy flat keys
        if let Some(zc) = auth.pointer("/ext/zeroclaw") {
            if let Some(sandbox) = zc.get("sandbox") {
                policy.insert("sandbox".into(), sandbox.clone());
            }
            if let Some(pairing) = zc.get("pairing_required") {
                policy.insert("pairing_required".into(), pairing.clone());
            }
            if let Some(always_ask) = zc.get("always_ask") {
                policy.insert("always_ask".into(), always_ask.clone());
            }
            if let Some(approval) = zc.get("require_approval_for_medium_risk") {
                policy.insert("require_approval_for_medium_risk".into(), approval.clone());
            }
            if let Some(resources) = zc.get("resources") {
                policy.insert("resources".into(), resources.clone());
            }
        }

        if !policy.is_empty() {
            config.insert("security_policy".into(), Value::Object(policy));
        }
    }

    // Behavioral sections → passthrough
    if let Some(psych) = data.get("psychology") {
        // Reverse alignment normalization for ZeroClaw format
        let mut p = psych.clone();
        if let Some(alignment) = psych
            .pointer("/moral_compass/alignment")
            .and_then(Value::as_str)
        {
            p["moral_compass"]["alignment"] =
                Value::String(denormalize_alignment(alignment));
        }
        config.insert("psychology".into(), p);
    }
    if let Some(voice) = data.get("voice") {
        config.insert("voice".into(), voice.clone());
    }
    if let Some(dir) = data.get("directives") {
        config.insert("directives".into(), dir.clone());
    }

    Ok(Value::Object(config))
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
        assert_eq!(persona["role"], "worker");
        assert_eq!(persona["version"], "1.0");
        assert_eq!(persona["authority"]["autonomy"], "supervised");
        assert_eq!(
            persona["authority"]["ext"]["zeroclaw"]["sandbox"],
            "landlock"
        );
        assert_eq!(persona["authority"]["scope"]["allowed_paths"][0], "src/**");
        assert_eq!(
            persona["authority"]["actions"]["scoped"]["shell"]["commands"][0],
            "cargo"
        );
        assert_eq!(persona["capabilities"]["skills"][0]["name"], "code_review");
        assert_eq!(persona["capabilities"]["skills"][1]["name"], "testing");
    }

    #[test]
    fn import_empty_object_no_panic() {
        let result = import_zeroclaw(&serde_json::json!({})).unwrap();
        assert_eq!(result["version"], "1.0");
    }

    #[test]
    fn import_rejects_non_object() {
        assert!(import_zeroclaw(&serde_json::json!("string")).is_err());
        assert!(import_zeroclaw(&serde_json::json!(42)).is_err());
        assert!(import_zeroclaw(&serde_json::json!(null)).is_err());
    }

    #[test]
    fn import_autonomy_config_shape() {
        let zc = serde_json::json!({
            "name": "ConfigAgent",
            "identity": { "role": "architect" },
            "autonomy": {
                "level": "full",
                "workspace_only": true,
                "allowed_commands": ["cargo", "git", "rustfmt"],
                "forbidden_paths": ["/etc", "/root"],
                "max_actions_per_hour": 100,
                "max_cost_per_day_cents": 500,
                "block_high_risk_commands": false,
                "auto_approve": ["read_file", "write_file"],
                "always_ask": ["delete_file"],
                "require_approval_for_medium_risk": true
            },
            "security": {
                "sandbox": { "backend": "landlock" },
                "resources": { "max_memory_mb": 512, "max_cpu_time_seconds": 60 }
            },
            "gateway": {
                "require_pairing": true
            }
        });

        let persona = import_zeroclaw(&zc).unwrap();
        assert_eq!(persona["authority"]["autonomy"], "full");
        assert_eq!(persona["authority"]["scope"]["workspace_only"], true);
        assert_eq!(
            persona["authority"]["scope"]["forbidden_paths"][0],
            "/etc"
        );
        assert_eq!(persona["authority"]["limits"]["max_actions_per_hour"], 100);
        assert_eq!(
            persona["authority"]["limits"]["max_cost_per_day_cents"],
            500
        );
        assert_eq!(
            persona["authority"]["actions"]["scoped"]["shell"]["block_high_risk"],
            false
        );
        assert_eq!(
            persona["authority"]["actions"]["allow"][0],
            "read_file"
        );
        assert_eq!(
            persona["authority"]["ext"]["zeroclaw"]["sandbox"],
            "landlock"
        );
        assert_eq!(
            persona["authority"]["ext"]["zeroclaw"]["pairing_required"],
            true
        );
        assert_eq!(
            persona["authority"]["ext"]["zeroclaw"]["always_ask"][0],
            "delete_file"
        );
        assert_eq!(
            persona["authority"]["ext"]["zeroclaw"]["require_approval_for_medium_risk"],
            true
        );
        assert_eq!(
            persona["authority"]["ext"]["zeroclaw"]["resources"]["max_memory_mb"],
            512
        );
    }

    #[test]
    fn import_full_with_behavioral_sections() {
        let zc = serde_json::json!({
            "name": "FullAgent",
            "identity": {
                "role": "engineer",
                "backstory": "Built for code quality",
                "capabilities": ["rust", "testing"]
            },
            "security_policy": { "autonomy": "supervised" },
            "psychology": {
                "neural_matrix": {
                    "creativity": 0.7, "empathy": 0.5, "logic": 0.9,
                    "adaptability": 0.6, "charisma": 0.4, "reliability": 0.95
                },
                "traits": {
                    "mbti": "INTJ",
                    "ocean": {
                        "openness": 0.7, "conscientiousness": 0.9,
                        "extraversion": 0.3, "agreeableness": 0.5, "neuroticism": 0.2
                    },
                    "temperament": "melancholic"
                },
                "moral_compass": {
                    "alignment": "lawful-good",
                    "core_values": ["Correctness", "Reliability"]
                },
                "emotional_profile": { "base_mood": "focused", "volatility": 0.15 }
            },
            "voice": {
                "style": { "descriptors": ["precise", "concise"], "formality": 0.8, "verbosity": 0.3 },
                "syntax": { "structure": "compound-complex", "contractions": false },
                "idiolect": {
                    "catchphrases": ["Let me verify that"],
                    "forbidden_words": ["maybe"]
                }
            },
            "directives": {
                "core_drive": "Ensure code correctness",
                "goals": ["Reduce bug count", "Improve test coverage"],
                "constraints": ["Never skip tests", "Always review before merge"]
            }
        });

        let persona = import_zeroclaw(&zc).unwrap();

        // Identity
        assert_eq!(persona["name"], "FullAgent");
        assert_eq!(persona["role"], "engineer");
        assert_eq!(persona["backstory"], "Built for code quality");

        // Psychology
        assert_eq!(persona["psychology"]["neural_matrix"]["creativity"], 0.7);
        assert_eq!(persona["psychology"]["neural_matrix"]["reliability"], 0.95);
        assert_eq!(persona["psychology"]["traits"]["mbti"], "INTJ");
        assert_eq!(persona["psychology"]["traits"]["ocean"]["openness"], 0.7);
        assert_eq!(persona["psychology"]["traits"]["temperament"], "melancholic");
        assert_eq!(
            persona["psychology"]["moral_compass"]["alignment"],
            "lawful-good"
        );
        assert_eq!(
            persona["psychology"]["moral_compass"]["core_values"][0],
            "Correctness"
        );
        assert_eq!(
            persona["psychology"]["emotional_profile"]["base_mood"],
            "focused"
        );
        assert_eq!(
            persona["psychology"]["emotional_profile"]["volatility"],
            0.15
        );

        // Voice
        assert_eq!(persona["voice"]["style"]["descriptors"][0], "precise");
        assert_eq!(persona["voice"]["style"]["formality"], 0.8);
        assert_eq!(persona["voice"]["style"]["verbosity"], 0.3);
        assert_eq!(persona["voice"]["syntax"]["structure"], "compound-complex");
        assert_eq!(persona["voice"]["syntax"]["contractions"], false);
        assert_eq!(
            persona["voice"]["idiolect"]["catchphrases"][0],
            "Let me verify that"
        );
        assert_eq!(
            persona["voice"]["idiolect"]["forbidden_words"][0],
            "maybe"
        );

        // Directives
        assert_eq!(
            persona["directives"]["core_drive"],
            "Ensure code correctness"
        );
        assert_eq!(persona["directives"]["goals"][0], "Reduce bug count");
        assert_eq!(
            persona["directives"]["constraints"][1],
            "Always review before merge"
        );

        // Capabilities
        assert_eq!(persona["capabilities"]["skills"][0]["name"], "rust");
    }

    #[test]
    fn import_capabilities_as_objects() {
        let zc = serde_json::json!({
            "name": "Bot",
            "identity": {
                "role": "test",
                "capabilities": [
                    { "name": "Code Review", "description": "Reviews pull requests", "priority": 1 },
                    { "name": "Testing", "description": "Writes unit tests" }
                ]
            }
        });

        let persona = import_zeroclaw(&zc).unwrap();
        assert_eq!(
            persona["capabilities"]["skills"][0]["name"],
            "Code Review"
        );
        assert_eq!(persona["capabilities"]["skills"][0]["priority"], 1);
        assert_eq!(persona["capabilities"]["skills"][1]["name"], "Testing");
        assert_eq!(persona["capabilities"]["skills"][1]["priority"], 2);
    }

    #[test]
    fn autonomy_level_mapping() {
        let cases = [
            ("full", "full"),
            ("autonomous", "full"),
            ("supervised", "supervised"),
            ("readonly", "readonly"),
            ("restricted", "readonly"),
            ("none", "readonly"),
            ("unknown_value", "supervised"),
        ];

        for (input, expected) in &cases {
            let zc = serde_json::json!({
                "name": "Bot",
                "security_policy": { "autonomy": input }
            });
            let persona = import_zeroclaw(&zc).unwrap();
            assert_eq!(
                persona["authority"]["autonomy"].as_str().unwrap(),
                *expected,
                "input={input}"
            );
        }
    }

    #[test]
    fn psychology_clamps_out_of_range_values() {
        let zc = serde_json::json!({
            "name": "Bot",
            "psychology": {
                "neural_matrix": { "creativity": 1.5, "empathy": -0.2, "logic": 0.7,
                    "adaptability": 0.5, "charisma": 0.5, "reliability": 0.5 },
                "traits": {
                    "ocean": { "openness": 2.0, "conscientiousness": -1.0,
                        "extraversion": 0.5, "agreeableness": 0.5, "neuroticism": 0.5 }
                },
                "emotional_profile": { "volatility": 3.0 }
            }
        });

        let persona = import_zeroclaw(&zc).unwrap();
        assert_eq!(persona["psychology"]["neural_matrix"]["creativity"], 1.0);
        assert_eq!(persona["psychology"]["neural_matrix"]["empathy"], 0.0);
        assert_eq!(persona["psychology"]["traits"]["ocean"]["openness"], 1.0);
        assert_eq!(
            persona["psychology"]["traits"]["ocean"]["conscientiousness"],
            0.0
        );
        assert_eq!(
            persona["psychology"]["emotional_profile"]["volatility"],
            1.0
        );
    }

    #[test]
    fn export_empty_no_panic() {
        let result = export_zeroclaw(&serde_json::json!({})).unwrap();
        assert!(result.is_object());
    }

    #[test]
    fn export_roundtrip_preserves_identity() {
        let persona = serde_json::json!({
            "version": "1.0",
            "name": "RoundTrip",
            "role": "architect",
            "backstory": "Created for testing",
            "authority": {
                "autonomy": "full",
                "scope": { "workspace_only": true, "allowed_paths": ["src/**"] },
                "limits": { "max_actions_per_hour": 50 },
                "ext": {
                    "zeroclaw": {
                        "sandbox": "landlock",
                        "pairing_required": true
                    }
                }
            }
        });

        let exported = export_zeroclaw(&persona).unwrap();
        assert_eq!(exported["name"], "RoundTrip");
        assert_eq!(exported["identity"]["role"], "architect");
        assert_eq!(exported["identity"]["backstory"], "Created for testing");
        assert_eq!(exported["security_policy"]["autonomy"], "full");
        assert_eq!(exported["security_policy"]["sandbox"], "landlock");
        assert_eq!(exported["security_policy"]["pairing_required"], true);
        assert_eq!(
            exported["security_policy"]["allowed_paths"][0],
            "src/**"
        );
        assert_eq!(exported["security_policy"]["max_actions_per_hour"], 50);

        // Re-import
        let reimported = import_zeroclaw(&exported).unwrap();
        assert_eq!(reimported["name"], "RoundTrip");
        assert_eq!(reimported["role"], "architect");
        assert_eq!(reimported["authority"]["autonomy"], "full");
    }

    #[test]
    fn export_roundtrip_behavioral_sections() {
        let persona = serde_json::json!({
            "version": "1.0",
            "name": "BehavBot",
            "role": "test",
            "psychology": {
                "neural_matrix": { "creativity": 0.8 },
                "traits": { "mbti": "ENTP" },
                "moral_compass": { "alignment": "chaotic-good", "core_values": ["Freedom"] }
            },
            "voice": {
                "style": { "descriptors": ["witty"], "formality": 0.3 },
                "idiolect": { "catchphrases": ["Actually..."] }
            },
            "directives": {
                "core_drive": "Explore ideas",
                "goals": ["Learn"],
                "constraints": ["Stay focused"]
            }
        });

        let exported = export_zeroclaw(&persona).unwrap();

        // Psychology exported with alignment reversed
        assert_eq!(
            exported["psychology"]["moral_compass"]["alignment"],
            "Chaotic Good"
        );
        assert_eq!(exported["psychology"]["traits"]["mbti"], "ENTP");

        // Voice passthrough
        assert_eq!(exported["voice"]["style"]["descriptors"][0], "witty");
        assert_eq!(
            exported["voice"]["idiolect"]["catchphrases"][0],
            "Actually..."
        );

        // Directives passthrough
        assert_eq!(exported["directives"]["core_drive"], "Explore ideas");

        // Re-import and verify roundtrip
        let reimported = import_zeroclaw(&exported).unwrap();
        assert_eq!(reimported["psychology"]["traits"]["mbti"], "ENTP");
        assert_eq!(
            reimported["psychology"]["moral_compass"]["alignment"],
            "chaotic-good"
        );
        assert_eq!(reimported["voice"]["style"]["descriptors"][0], "witty");
        assert_eq!(reimported["directives"]["goals"][0], "Learn");
    }

    #[test]
    fn export_roundtrip_authority_limits_and_ext() {
        let persona = serde_json::json!({
            "version": "1.0",
            "name": "LimitBot",
            "role": "worker",
            "authority": {
                "autonomy": "supervised",
                "scope": {
                    "workspace_only": true,
                    "forbidden_paths": ["/etc"]
                },
                "actions": {
                    "scoped": {
                        "shell": {
                            "$type": "shell",
                            "commands": ["cargo", "git"],
                            "block_high_risk": true,
                            "block_subshells": true,
                            "block_redirects": true,
                            "block_background": true
                        }
                    }
                },
                "limits": {
                    "max_actions_per_hour": 200,
                    "max_cost_per_day_cents": 1000
                },
                "ext": {
                    "zeroclaw": {
                        "sandbox": "landlock",
                        "always_ask": ["rm", "reboot"],
                        "require_approval_for_medium_risk": true,
                        "resources": { "max_memory_mb": 1024 }
                    }
                }
            }
        });

        let exported = export_zeroclaw(&persona).unwrap();
        assert_eq!(exported["security_policy"]["autonomy"], "supervised");
        assert_eq!(
            exported["security_policy"]["allowed_commands"][0],
            "cargo"
        );
        assert_eq!(
            exported["security_policy"]["max_actions_per_hour"],
            200
        );
        assert_eq!(
            exported["security_policy"]["max_cost_per_day_cents"],
            1000
        );
        assert_eq!(exported["security_policy"]["sandbox"], "landlock");
        assert_eq!(
            exported["security_policy"]["always_ask"][0],
            "rm"
        );
        assert_eq!(
            exported["security_policy"]["resources"]["max_memory_mb"],
            1024
        );

        // Re-import roundtrip
        let reimported = import_zeroclaw(&exported).unwrap();
        assert_eq!(reimported["authority"]["autonomy"], "supervised");
        assert_eq!(
            reimported["authority"]["ext"]["zeroclaw"]["sandbox"],
            "landlock"
        );
        assert_eq!(
            reimported["authority"]["limits"]["max_actions_per_hour"],
            200
        );
    }
}
