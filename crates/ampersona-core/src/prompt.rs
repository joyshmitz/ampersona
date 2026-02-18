use anyhow::{Context, Result};
use serde_json::Value;

/// Convert an ampersona JSON to a Markdown system prompt.
pub fn to_system_prompt(data: &Value, sections: &[String]) -> String {
    let mut out = String::with_capacity(2048);
    let all = sections.is_empty();

    if all || sections.iter().any(|s| s == "identity") {
        emit_identity(&mut out, data);
    }
    if let Some(psych) = data.get("psychology") {
        if all || sections.iter().any(|s| s == "psychology") {
            emit_psychology(&mut out, psych);
        }
    }
    if let Some(voice) = data.get("voice") {
        if all || sections.iter().any(|s| s == "voice") {
            emit_voice(&mut out, voice);
        }
    }
    if let Some(cap) = data.get("capabilities") {
        if all || sections.iter().any(|s| s == "capabilities") {
            emit_capabilities(&mut out, cap);
        }
    }
    if let Some(dir) = data.get("directives") {
        if all || sections.iter().any(|s| s == "directives") {
            emit_directives(&mut out, dir);
        }
    }
    if let Some(auth) = data.get("authority") {
        if all || sections.iter().any(|s| s == "authority") {
            emit_authority(&mut out, auth);
        }
    }
    if let Some(gates) = data.get("gates") {
        if all || sections.iter().any(|s| s == "gates") {
            emit_gates(&mut out, gates);
        }
    }
    out
}

/// Convert to TOON format.
pub fn to_toon(data: &Value) -> Result<String> {
    let json_str = serde_json::to_string(data).context("serialize for TOON")?;
    let parsed: toon::JsonValue =
        toon::JsonValue::from(serde_json::from_str::<Value>(&json_str).unwrap());
    Ok(toon::encode(parsed, None))
}

/// Load persona JSON from a file path.
pub fn load_persona(path: &str) -> Result<Value> {
    let content = std::fs::read_to_string(path).with_context(|| format!("cannot read {path}"))?;
    serde_json::from_str(&content).with_context(|| format!("{path}: invalid JSON"))
}

// ── Helpers ─────────────────────────────────────────────────────

fn s(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn n(v: &Value, key: &str) -> String {
    v.get(key)
        .map(|val| match val {
            Value::Number(n) => n.to_string(),
            Value::String(s) => s.clone(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

fn arr_strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

// ── Section emitters ────────────────────────────────────────────

fn emit_identity(out: &mut String, data: &Value) {
    let name = s(data, "name");
    let role = s(data, "role");
    if name.is_empty() && role.is_empty() {
        return;
    }
    out.push_str("## Identity\n\n");
    if !name.is_empty() {
        out.push_str(&format!("**Name:** {name}\n"));
    }
    if !role.is_empty() {
        out.push_str(&format!("**Role:** {role}\n"));
    }
    let backstory = s(data, "backstory");
    if !backstory.is_empty() {
        out.push_str(&format!("\n{backstory}\n"));
    }
    out.push('\n');
}

fn emit_psychology(out: &mut String, v: &Value) {
    out.push_str("## Psychology\n\n");
    if let Some(nm) = v.get("neural_matrix") {
        out.push_str("**Neural Matrix:** ");
        let dims = [
            "creativity",
            "empathy",
            "logic",
            "adaptability",
            "charisma",
            "reliability",
        ];
        let parts: Vec<String> = dims
            .iter()
            .filter_map(|d| {
                nm.get(*d)
                    .and_then(Value::as_f64)
                    .map(|val| format!("{d}={val:.2}"))
            })
            .collect();
        out.push_str(&parts.join(", "));
        out.push('\n');
    }
    if let Some(traits) = v.get("traits") {
        let mbti = s(traits, "mbti");
        let temp = s(traits, "temperament");
        if !mbti.is_empty() {
            out.push_str(&format!("**Type:** {mbti}"));
            if !temp.is_empty() {
                out.push_str(&format!(", {temp}"));
            }
            out.push('\n');
        }
        if let Some(ocean) = traits.get("ocean") {
            out.push_str("**OCEAN:** ");
            let dims = [
                "openness",
                "conscientiousness",
                "extraversion",
                "agreeableness",
                "neuroticism",
            ];
            let parts: Vec<String> = dims
                .iter()
                .filter_map(|d| {
                    ocean
                        .get(*d)
                        .and_then(Value::as_f64)
                        .map(|val| format!("{}={val:.2}", &d[..1].to_uppercase()))
                })
                .collect();
            out.push_str(&parts.join(" "));
            out.push('\n');
        }
    }
    if let Some(mc) = v.get("moral_compass") {
        let align = s(mc, "alignment");
        let values = arr_strings(mc, "core_values");
        if !align.is_empty() {
            out.push_str(&format!("**Alignment:** {align}"));
            if !values.is_empty() {
                out.push_str(&format!(" \u{2014} {}", values.join(", ")));
            }
            out.push('\n');
        }
    }
    if let Some(ep) = v.get("emotional_profile") {
        let mood = s(ep, "base_mood");
        let vol = n(ep, "volatility");
        if !mood.is_empty() {
            out.push_str(&format!("**Mood:** {mood} (volatility={vol})\n"));
        }
    }
    out.push('\n');
}

fn emit_voice(out: &mut String, v: &Value) {
    out.push_str("## Voice\n\n");
    if let Some(st) = v.get("style") {
        let descs = arr_strings(st, "descriptors");
        if !descs.is_empty() {
            out.push_str(&format!("**Style:** {}\n", descs.join(", ")));
        }
        let formality = n(st, "formality");
        let verbosity = n(st, "verbosity");
        out.push_str(&format!(
            "**Formality:** {formality}, **Verbosity:** {verbosity}\n"
        ));
    }
    if let Some(syn) = v.get("syntax") {
        let structure = s(syn, "structure");
        if !structure.is_empty() {
            out.push_str(&format!("**Syntax:** {structure}\n"));
        }
    }
    if let Some(idio) = v.get("idiolect") {
        let phrases = arr_strings(idio, "catchphrases");
        if !phrases.is_empty() {
            out.push_str(&format!(
                "**Catchphrases:** {}\n",
                phrases
                    .iter()
                    .map(|p| format!("\"{p}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let forbidden = arr_strings(idio, "forbidden_words");
        if !forbidden.is_empty() {
            out.push_str(&format!("**Never says:** {}\n", forbidden.join(", ")));
        }
    }
    if let Some(tts) = v.get("tts") {
        let provider = s(tts, "provider");
        let voice_id = s(tts, "voice_id");
        if !provider.is_empty() {
            out.push_str(&format!("**TTS:** {provider}/{voice_id}"));
            let stability = n(tts, "stability");
            if !stability.is_empty() {
                out.push_str(&format!(" (stability={stability})"));
            }
            out.push('\n');
        }
    }
    out.push('\n');
}

fn emit_capabilities(out: &mut String, v: &Value) {
    if let Some(skills) = v.get("skills").and_then(Value::as_array) {
        if skills.is_empty() {
            return;
        }
        out.push_str("## Capabilities\n\n");
        for skill in skills {
            let name = s(skill, "name");
            let desc = s(skill, "description");
            let prio = n(skill, "priority");
            if !name.is_empty() {
                if !prio.is_empty() {
                    out.push_str(&format!("- **{name}** (p{prio}): {desc}\n"));
                } else {
                    out.push_str(&format!("- **{name}**: {desc}\n"));
                }
            }
        }
        out.push('\n');
    }
}

fn emit_authority(out: &mut String, v: &Value) {
    out.push_str("## Authority\n\n");
    let autonomy = s(v, "autonomy");
    if !autonomy.is_empty() {
        out.push_str(&format!("**Autonomy:** {autonomy}\n"));
    }
    if let Some(scope) = v.get("scope") {
        let workspace_only = scope
            .get("workspace_only")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if workspace_only {
            out.push_str("**Scope:** workspace only\n");
        }
        let allowed = arr_strings(scope, "allowed_paths");
        if !allowed.is_empty() {
            out.push_str(&format!("**Allowed paths:** {}\n", allowed.join(", ")));
        }
        let forbidden = arr_strings(scope, "forbidden_paths");
        if !forbidden.is_empty() {
            out.push_str(&format!("**Forbidden paths:** {}\n", forbidden.join(", ")));
        }
    }
    if let Some(actions) = v.get("actions") {
        let allow = arr_strings(actions, "allow");
        if !allow.is_empty() {
            out.push_str(&format!("**Allowed actions:** {}\n", allow.join(", ")));
        }
        if let Some(deny) = actions.get("deny").and_then(Value::as_array) {
            let deny_names: Vec<String> = deny
                .iter()
                .filter_map(|d| {
                    d.as_str()
                        .map(String::from)
                        .or_else(|| d.get("action").and_then(Value::as_str).map(String::from))
                })
                .collect();
            if !deny_names.is_empty() {
                out.push_str(&format!("**Denied actions:** {}\n", deny_names.join(", ")));
            }
        }
    }
    if let Some(limits) = v.get("limits") {
        let max_actions = limits.get("max_actions_per_hour").and_then(Value::as_u64);
        let max_cost = limits.get("max_cost_per_day_cents").and_then(Value::as_u64);
        if let Some(ma) = max_actions {
            out.push_str(&format!("**Rate limit:** {ma} actions/hour\n"));
        }
        if let Some(mc) = max_cost {
            out.push_str(&format!("**Cost limit:** {mc} cents/day\n"));
        }
    }
    out.push('\n');
}

fn emit_gates(out: &mut String, v: &Value) {
    let gates = match v.as_array() {
        Some(g) if !g.is_empty() => g,
        _ => return,
    };
    out.push_str("## Gates\n\n");
    for gate in gates {
        let id = s(gate, "id");
        let direction = s(gate, "direction");
        let from = s(gate, "from_phase");
        let to = s(gate, "to_phase");
        let enforcement = gate
            .get("enforcement")
            .and_then(Value::as_str)
            .unwrap_or("enforce");
        let mode = if enforcement == "observe" {
            " [observe]"
        } else {
            ""
        };
        let from_str = if from.is_empty() { "(none)" } else { &from };
        out.push_str(&format!(
            "- **{id}** ({direction}): {from_str} \u{2192} {to}{mode}\n"
        ));
        if let Some(criteria) = gate.get("criteria").and_then(Value::as_array) {
            for c in criteria {
                let metric = s(c, "metric");
                let op = s(c, "op");
                let val = c.get("value").map(|v| v.to_string()).unwrap_or_default();
                out.push_str(&format!("  - {metric} {op} {val}\n"));
            }
        }
    }
    out.push('\n');
}

fn emit_directives(out: &mut String, v: &Value) {
    let drive = s(v, "core_drive");
    let goals = arr_strings(v, "goals");
    let constraints = arr_strings(v, "constraints");
    if drive.is_empty() && goals.is_empty() && constraints.is_empty() {
        return;
    }
    out.push_str("## Directives\n\n");
    if !drive.is_empty() {
        out.push_str(&format!("**Core drive:** {drive}\n"));
    }
    if !goals.is_empty() {
        out.push_str(&format!("**Goals:** {}\n", goals.join("; ")));
    }
    if !constraints.is_empty() {
        out.push_str(&format!("**Constraints:** {}\n", constraints.join("; ")));
    }
    out.push('\n');
}
