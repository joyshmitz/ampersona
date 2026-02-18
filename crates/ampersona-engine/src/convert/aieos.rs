//! Convert between AIEOS v1.1 identity format and ampersona v1.0.
//!
//! AIEOS (AI Entity Object Specification) is zeroclaw's identity format.
//! This module normalizes canonical AIEOS v1.1 payloads (including the
//! official generator shape with nested `traits`, `goals`, `fears`, etc.)
//! into ampersona's flat schema.
//!
//! Normalization handles:
//! - Fallback paths (e.g. `psychology.mbti` OR `psychology.traits.mbti`)
//! - Object→text summarization (e.g. nested `bio` or `residence` objects)
//! - Array-of-objects extraction (e.g. `[{"name":"X"}]` → `["X"]`)
//! - Alignment format conversion (`"Lawful Good"` → `"lawful-good"`)
//! - Fear merging (`fears.rational` + `fears.irrational` → constraints)

use anyhow::Result;
use serde_json::{Map, Value};

// ── Normalization utilities (ported from zeroclaw identity.rs) ──────

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

fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(_) | Value::Number(_) | Value::Bool(_) => scalar_to_string(value),
        Value::Array(_) => {
            let values = list_from_value(value);
            if values.is_empty() {
                None
            } else {
                Some(values.join(", "))
            }
        }
        Value::Object(map) => summarize_object(map),
    }
}

fn summarize_object(map: &Map<String, Value>) -> Option<String> {
    let mut parts = Vec::new();
    summarize_object_into_parts("", map, &mut parts);
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

fn summarize_object_into_parts(prefix: &str, map: &Map<String, Value>, parts: &mut Vec<String>) {
    for (key, value) in map {
        if key.starts_with('@') || key.starts_with('$') {
            continue;
        }
        let label = key.replace('_', " ");
        let full_label = if prefix.is_empty() {
            label
        } else {
            format!("{prefix} {label}")
        };
        match value {
            Value::Object(inner) => summarize_object_into_parts(&full_label, inner, parts),
            Value::Array(_) => {
                let values = list_from_value(value);
                if !values.is_empty() {
                    parts.push(format!("{full_label}: {}", values.join(", ")));
                }
            }
            _ => {
                if let Some(text) = scalar_to_string(value) {
                    parts.push(format!("{full_label}: {text}"));
                }
            }
        }
    }
}

fn list_from_value(value: &Value) -> Vec<String> {
    let mut values = Vec::new();
    match value {
        Value::Array(entries) => {
            for entry in entries {
                values.extend(list_from_value(entry));
            }
        }
        Value::Object(map) => {
            if let Some(name) = map.get("name").and_then(scalar_to_string) {
                values.push(name);
            } else if let Some(title) = map.get("title").and_then(scalar_to_string) {
                values.push(title);
            } else if let Some(summary) = summarize_object(map) {
                values.push(summary);
            }
        }
        _ => {
            if let Some(text) = scalar_to_string(value) {
                values.push(text);
            }
        }
    }
    dedupe_non_empty(values)
}

fn dedupe_non_empty(values: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !deduped
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(trimmed))
        {
            deduped.push(trimmed.to_owned());
        }
    }
    deduped
}

fn numeric_from_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

fn non_empty_list_at(value: &Value, path: &[&str]) -> Option<Vec<String>> {
    let values = value_at_path(value, path).map(list_from_value)?;
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

/// Clamp a numeric value to [0.0, 1.0] for UnitFloat fields.
fn unit_clamp(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Convert AIEOS alignment format ("Lawful Good") to ampersona format ("lawful-good").
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

// ── AIEOS → ampersona normalization ─────────────────────────────────

/// Normalize AIEOS `identity` section → ampersona `name`, `role`, `backstory`.
fn normalize_identity(aieos: &Value, obj: &mut Map<String, Value>) {
    // Name: top-level or identity.names
    let name = aieos
        .get("name")
        .and_then(scalar_to_string)
        .or_else(|| aieos.get("entity_id").and_then(scalar_to_string))
        .or_else(|| {
            let names = value_at_path(aieos, &["identity", "names"])?;
            let first = value_at_path(names, &["first"]).and_then(scalar_to_string);
            let last = value_at_path(names, &["last"]).and_then(scalar_to_string);
            let nickname = value_at_path(names, &["nickname"]).and_then(scalar_to_string);
            let full = value_at_path(names, &["full"]).and_then(scalar_to_string);
            full.or_else(|| match (first, last) {
                (Some(f), Some(l)) => Some(format!("{f}{l}")),
                (Some(f), None) => Some(f),
                _ => nickname,
            })
        });
    if let Some(name) = name {
        obj.insert("name".into(), Value::String(name));
    }

    // Role: top-level or identity.role or description
    let role = aieos
        .get("role")
        .and_then(scalar_to_string)
        .or_else(|| value_at_path(aieos, &["identity", "role"]).and_then(scalar_to_string))
        .or_else(|| aieos.get("description").and_then(scalar_to_string));
    if let Some(role) = role {
        obj.insert("role".into(), Value::String(role));
    }

    // Backstory: assembled from identity sub-fields
    let identity = match aieos.get("identity") {
        Some(v) if v.is_object() => v,
        _ => return,
    };

    let mut parts = Vec::new();
    if let Some(bio) = identity.get("bio").and_then(value_to_text) {
        parts.push(bio);
    }
    if let Some(origin) = identity.get("origin").and_then(value_to_text) {
        parts.push(format!("Origin: {origin}"));
    }
    if let Some(residence) = identity.get("residence").and_then(value_to_text) {
        parts.push(format!("Residence: {residence}"));
    }
    // History section as backstory supplement
    if let Some(story) = value_at_path(aieos, &["history", "origin_story"]).and_then(value_to_text)
    {
        parts.push(story);
    }
    if !parts.is_empty() {
        obj.insert("backstory".into(), Value::String(parts.join(". ")));
    }
}

/// Normalize AIEOS `psychology` → ampersona `psychology`.
fn normalize_psychology(aieos: &Value) -> Option<Value> {
    let section = aieos.get("psychology")?;

    // neural_matrix — keep only the 6 ampersona dimensions
    let neural_matrix = value_at_path(section, &["neural_matrix"]).and_then(|nm| {
        let nm_obj = nm.as_object()?;
        let dims = [
            "creativity",
            "empathy",
            "logic",
            "adaptability",
            "charisma",
            "reliability",
        ];
        let mut result = serde_json::Map::new();
        for dim in &dims {
            if let Some(v) = nm_obj.get(*dim).and_then(numeric_from_value) {
                result.insert((*dim).into(), serde_json::json!(unit_clamp(v)));
            }
        }
        if result.is_empty() {
            None
        } else {
            Some(Value::Object(result))
        }
    });

    // traits: mbti + ocean + temperament
    let mbti = value_at_path(section, &["mbti"])
        .and_then(scalar_to_string)
        .or_else(|| value_at_path(section, &["traits", "mbti"]).and_then(scalar_to_string));

    let ocean_source =
        value_at_path(section, &["ocean"]).or_else(|| value_at_path(section, &["traits", "ocean"]));

    let ocean = ocean_source.and_then(|o| {
        let o_obj = o.as_object()?;
        let dims = [
            "openness",
            "conscientiousness",
            "extraversion",
            "agreeableness",
            "neuroticism",
        ];
        let mut result = serde_json::Map::new();
        for dim in &dims {
            if let Some(v) = o_obj.get(*dim).and_then(numeric_from_value) {
                result.insert((*dim).into(), serde_json::json!(unit_clamp(v)));
            }
        }
        if result.is_empty() {
            None
        } else {
            Some(Value::Object(result))
        }
    });

    let temperament = value_at_path(section, &["temperament"])
        .and_then(scalar_to_string)
        .or_else(|| value_at_path(section, &["traits", "temperament"]).and_then(scalar_to_string));

    let traits = if mbti.is_some() || ocean.is_some() {
        let mut t = serde_json::Map::new();
        if let Some(o) = ocean {
            t.insert("ocean".into(), o);
        }
        if let Some(m) = mbti {
            t.insert("mbti".into(), Value::String(m));
        }
        if let Some(temp) = temperament {
            t.insert("temperament".into(), Value::String(temp));
        }
        Some(Value::Object(t))
    } else {
        None
    };

    // moral_compass
    let moral_compass = value_at_path(section, &["moral_compass"]).and_then(|mc| {
        let mut result = serde_json::Map::new();
        if let Some(alignment_raw) = mc.get("alignment").and_then(scalar_to_string) {
            if let Some(normalized) = normalize_alignment(&alignment_raw) {
                result.insert("alignment".into(), Value::String(normalized));
            }
        }
        let core_values = mc
            .get("core_values")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(scalar_to_string)
                    .map(Value::String)
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty());
        if let Some(values) = core_values {
            result.insert("core_values".into(), Value::Array(values));
        }
        if result.is_empty() {
            None
        } else {
            Some(Value::Object(result))
        }
    });

    // emotional_profile
    let emotional_profile = value_at_path(section, &["emotional_profile"]).and_then(|ep| {
        let mut result = serde_json::Map::new();
        if let Some(mood) = ep.get("base_mood").and_then(scalar_to_string) {
            result.insert("base_mood".into(), Value::String(mood));
        }
        if let Some(vol) = ep.get("volatility").and_then(numeric_from_value) {
            result.insert("volatility".into(), serde_json::json!(unit_clamp(vol)));
        }
        if result.is_empty() {
            None
        } else {
            Some(Value::Object(result))
        }
    });

    if neural_matrix.is_none() && traits.is_none() && moral_compass.is_none() {
        return None;
    }

    let mut psych = serde_json::Map::new();
    if let Some(nm) = neural_matrix {
        psych.insert("neural_matrix".into(), nm);
    }
    if let Some(t) = traits {
        psych.insert("traits".into(), t);
    }
    if let Some(mc) = moral_compass {
        psych.insert("moral_compass".into(), mc);
    }
    if let Some(ep) = emotional_profile {
        psych.insert("emotional_profile".into(), ep);
    }
    Some(Value::Object(psych))
}

/// Normalize AIEOS `linguistics` → ampersona `voice`.
fn normalize_voice(aieos: &Value) -> Option<Value> {
    let section = aieos.get("linguistics")?;

    // style.descriptors: from linguistics.style or linguistics.text_style.style_descriptors
    let descriptors =
        non_empty_list_at(section, &["text_style", "style_descriptors"]).or_else(|| {
            value_at_path(section, &["style"])
                .and_then(scalar_to_string)
                .map(|s| vec![s])
        });

    // formality: from linguistics.formality or linguistics.text_style.formality_level
    let formality = value_at_path(section, &["formality"])
        .and_then(numeric_from_value)
        .or_else(|| {
            value_at_path(section, &["text_style", "formality_level"]).and_then(numeric_from_value)
        });

    // verbosity: from linguistics.verbosity or linguistics.text_style.verbosity_level
    let verbosity = value_at_path(section, &["verbosity"])
        .and_then(numeric_from_value)
        .or_else(|| {
            value_at_path(section, &["text_style", "verbosity_level"]).and_then(numeric_from_value)
        });

    let style = if descriptors.is_some() || formality.is_some() {
        let mut s = serde_json::Map::new();
        if let Some(descs) = descriptors {
            s.insert(
                "descriptors".into(),
                Value::Array(descs.into_iter().map(Value::String).collect()),
            );
        }
        if let Some(f) = formality {
            s.insert("formality".into(), serde_json::json!(unit_clamp(f)));
        }
        if let Some(v) = verbosity {
            s.insert("verbosity".into(), serde_json::json!(unit_clamp(v)));
        }
        Some(Value::Object(s))
    } else {
        None
    };

    // syntax
    let syntax = value_at_path(section, &["syntax"]).and_then(|syn| {
        let mut result = serde_json::Map::new();
        if let Some(structure) = syn.get("sentence_structure").and_then(scalar_to_string) {
            result.insert("structure".into(), Value::String(structure));
        } else if let Some(structure) = syn.get("structure").and_then(scalar_to_string) {
            result.insert("structure".into(), Value::String(structure));
        }
        if let Some(contractions) = syn.get("contractions").and_then(Value::as_bool) {
            result.insert("contractions".into(), Value::Bool(contractions));
        }
        if result.is_empty() {
            None
        } else {
            Some(Value::Object(result))
        }
    });

    // idiolect: catchphrases and forbidden_words
    let catchphrases = non_empty_list_at(section, &["catchphrases"])
        .or_else(|| non_empty_list_at(section, &["idiolect", "catchphrases"]));
    let forbidden_words = non_empty_list_at(section, &["forbidden_words"])
        .or_else(|| non_empty_list_at(section, &["idiolect", "forbidden_words"]));

    let idiolect = if catchphrases.is_some() || forbidden_words.is_some() {
        let mut id = serde_json::Map::new();
        if let Some(cp) = catchphrases {
            id.insert(
                "catchphrases".into(),
                Value::Array(cp.into_iter().map(Value::String).collect()),
            );
        }
        if let Some(fw) = forbidden_words {
            id.insert(
                "forbidden_words".into(),
                Value::Array(fw.into_iter().map(Value::String).collect()),
            );
        }
        Some(Value::Object(id))
    } else {
        None
    };

    if style.is_none() && syntax.is_none() && idiolect.is_none() {
        return None;
    }

    let mut voice = serde_json::Map::new();
    if let Some(s) = style {
        voice.insert("style".into(), s);
    }
    if let Some(syn) = syntax {
        voice.insert("syntax".into(), syn);
    }
    if let Some(id) = idiolect {
        voice.insert("idiolect".into(), id);
    }
    Some(Value::Object(voice))
}

/// Normalize AIEOS `capabilities` → ampersona `capabilities`.
fn normalize_capabilities(aieos: &Value) -> Option<Value> {
    let section = aieos
        .get("capabilities")
        .or_else(|| value_at_path(aieos, &["identity", "capabilities"]))?;

    let skills_source = section
        .get("skills")
        .and_then(Value::as_array)
        .or_else(|| section.as_array());

    let skills_source = skills_source?;

    let skills: Vec<Value> = skills_source
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

/// Normalize AIEOS `motivations` → ampersona `directives`.
fn normalize_directives(aieos: &Value) -> Option<Value> {
    let motivations = aieos.get("motivations");

    let core_drive = motivations
        .and_then(|m| m.get("core_drive"))
        .and_then(scalar_to_string)
        .or_else(|| {
            value_at_path(aieos, &["identity", "core_directive"]).and_then(scalar_to_string)
        })
        .or_else(|| value_at_path(aieos, &["identity", "mission"]).and_then(scalar_to_string));

    // Goals: merge short_term + long_term
    let mut goals = Vec::new();
    if let Some(m) = motivations {
        if let Some(g) = non_empty_list_at(m, &["short_term_goals"])
            .or_else(|| non_empty_list_at(m, &["goals", "short_term"]))
        {
            goals.extend(g);
        }
        if let Some(g) = non_empty_list_at(m, &["long_term_goals"])
            .or_else(|| non_empty_list_at(m, &["goals", "long_term"]))
        {
            goals.extend(g);
        }
    }
    if let Some(g) = value_at_path(aieos, &["identity", "goals"]).and_then(|v| {
        let items = list_from_value(v);
        if items.is_empty() {
            None
        } else {
            Some(items)
        }
    }) {
        goals.extend(g);
    }
    goals = dedupe_non_empty(goals);

    // Constraints: from motivations.fears + identity.constraints
    let mut constraints = Vec::new();
    if let Some(m) = motivations {
        if let Some(fears) = m.get("fears") {
            let fear_items = if fears.is_object() {
                let mut combined = non_empty_list_at(m, &["fears", "rational"]).unwrap_or_default();
                if let Some(mut irrational) = non_empty_list_at(m, &["fears", "irrational"]) {
                    combined.append(&mut irrational);
                }
                if combined.is_empty() {
                    list_from_value(fears)
                } else {
                    combined
                }
            } else {
                list_from_value(fears)
            };
            for fear in fear_items {
                constraints.push(format!("Avoid: {fear}"));
            }
        }
    }
    if let Some(c) = value_at_path(aieos, &["identity", "constraints"]).and_then(|v| {
        let items = list_from_value(v);
        if items.is_empty() {
            None
        } else {
            Some(items)
        }
    }) {
        constraints.extend(c);
    }
    constraints = dedupe_non_empty(constraints);

    if core_drive.is_none() && goals.is_empty() && constraints.is_empty() {
        return None;
    }

    let mut directives = serde_json::Map::new();
    if let Some(cd) = core_drive {
        directives.insert("core_drive".into(), Value::String(cd));
    }
    if !goals.is_empty() {
        directives.insert(
            "goals".into(),
            Value::Array(goals.into_iter().map(Value::String).collect()),
        );
    }
    if !constraints.is_empty() {
        directives.insert(
            "constraints".into(),
            Value::Array(constraints.into_iter().map(Value::String).collect()),
        );
    }
    Some(Value::Object(directives))
}

/// Normalize AIEOS `security_policy` → ampersona `authority`.
fn normalize_authority(aieos: &Value) -> Option<Value> {
    let policy = aieos
        .get("security_policy")
        .or_else(|| aieos.get("policy"))?;

    let mut authority = serde_json::Map::new();

    // Autonomy
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

    // Scope
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

    // Actions
    if policy.get("allowed_actions").is_some() || policy.get("denied_actions").is_some() {
        let mut actions = serde_json::Map::new();
        if let Some(allow) = policy.get("allowed_actions") {
            actions.insert("allow".into(), allow.clone());
        }
        if let Some(deny) = policy.get("denied_actions") {
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

    Some(Value::Object(authority))
}

// ── Public API ──────────────────────────────────────────────────────

/// Convert an AIEOS identity JSON to ampersona v1.0 format.
///
/// Handles both the simplified shape (flat fields) and the canonical
/// AIEOS v1.1 generator shape (nested `traits`, `goals`, `fears`, etc.).
pub fn import_aieos(aieos: &Value) -> Result<Value> {
    if !aieos.is_object() {
        anyhow::bail!("AIEOS payload must be a JSON object");
    }

    let mut obj = serde_json::Map::new();
    obj.insert(
        "$schema".into(),
        Value::String("https://ampersona.dev/schema/v1.0/ampersona.schema.json".into()),
    );
    obj.insert("version".into(), Value::String("1.0".into()));

    // Identity → name, role, backstory
    normalize_identity(aieos, &mut obj);

    // Psychology
    if let Some(psych) = normalize_psychology(aieos) {
        obj.insert("psychology".into(), psych);
    }

    // Voice (from linguistics)
    if let Some(voice) = normalize_voice(aieos) {
        obj.insert("voice".into(), voice);
    }

    // Capabilities
    if let Some(caps) = normalize_capabilities(aieos) {
        obj.insert("capabilities".into(), caps);
    }

    // Directives (from motivations)
    if let Some(dir) = normalize_directives(aieos) {
        obj.insert("directives".into(), dir);
    }

    // Authority (from security_policy)
    if let Some(auth) = normalize_authority(aieos) {
        obj.insert("authority".into(), auth);
    }

    Ok(Value::Object(obj))
}

/// Export ampersona to AIEOS format (best-effort reverse mapping).
pub fn export_aieos(persona: &Value) -> Result<Value> {
    let mut aieos = serde_json::Map::new();

    // Name / entity_id
    if let Some(name) = persona.get("name") {
        aieos.insert("entity_id".into(), name.clone());
        aieos.insert("name".into(), name.clone());
    }
    if let Some(role) = persona.get("role") {
        aieos.insert("description".into(), role.clone());
    }

    // Psychology → psychology
    if let Some(psych) = persona.get("psychology") {
        let mut p = serde_json::Map::new();
        if let Some(nm) = psych.get("neural_matrix") {
            p.insert("neural_matrix".into(), nm.clone());
        }
        if let Some(traits) = psych.get("traits") {
            p.insert("traits".into(), traits.clone());
        }
        if let Some(mc) = psych.get("moral_compass") {
            // Reverse alignment: "lawful-good" → "Lawful Good"
            let mut mc_out = mc.clone();
            if let Some(alignment) = mc.get("alignment").and_then(Value::as_str) {
                let words: Vec<String> = alignment
                    .split('-')
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            Some(first) => first.to_uppercase().to_string() + c.as_str(),
                            None => String::new(),
                        }
                    })
                    .collect();
                mc_out["alignment"] = Value::String(words.join(" "));
            }
            p.insert("moral_compass".into(), mc_out);
        }
        if let Some(ep) = psych.get("emotional_profile") {
            p.insert("emotional_profile".into(), ep.clone());
        }
        if !p.is_empty() {
            aieos.insert("psychology".into(), Value::Object(p));
        }
    }

    // Voice → linguistics
    if let Some(voice) = persona.get("voice") {
        let mut ling = serde_json::Map::new();
        if let Some(style) = voice.get("style") {
            let mut ts = serde_json::Map::new();
            if let Some(descs) = style.get("descriptors") {
                ts.insert("style_descriptors".into(), descs.clone());
            }
            if let Some(f) = style.get("formality") {
                ts.insert("formality_level".into(), f.clone());
            }
            if let Some(v) = style.get("verbosity") {
                ts.insert("verbosity_level".into(), v.clone());
            }
            if !ts.is_empty() {
                ling.insert("text_style".into(), Value::Object(ts));
            }
        }
        if let Some(syn) = voice.get("syntax") {
            ling.insert("syntax".into(), syn.clone());
        }
        let mut idiolect = serde_json::Map::new();
        if let Some(cp) = voice.pointer("/idiolect/catchphrases") {
            idiolect.insert("catchphrases".into(), cp.clone());
        }
        if let Some(fw) = voice.pointer("/idiolect/forbidden_words") {
            idiolect.insert("forbidden_words".into(), fw.clone());
        }
        if !idiolect.is_empty() {
            ling.insert("idiolect".into(), Value::Object(idiolect));
        }
        if !ling.is_empty() {
            aieos.insert("linguistics".into(), Value::Object(ling));
        }
    }

    // Directives → motivations
    if let Some(dir) = persona.get("directives") {
        let mut mot = serde_json::Map::new();
        if let Some(cd) = dir.get("core_drive") {
            mot.insert("core_drive".into(), cd.clone());
        }
        if let Some(goals) = dir.get("goals") {
            mot.insert("goals".into(), serde_json::json!({"short_term": goals}));
        }
        // Constraints with "Avoid: " prefix → fears
        if let Some(constraints) = dir.get("constraints").and_then(Value::as_array) {
            let fears: Vec<Value> = constraints
                .iter()
                .filter_map(|c| {
                    c.as_str()
                        .map(|s| Value::String(s.strip_prefix("Avoid: ").unwrap_or(s).to_string()))
                })
                .collect();
            if !fears.is_empty() {
                mot.insert("fears".into(), Value::Array(fears));
            }
        }
        if !mot.is_empty() {
            aieos.insert("motivations".into(), Value::Object(mot));
        }
    }

    // Capabilities
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
            let mut identity = aieos
                .get("identity")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            identity.insert("capabilities".into(), Value::Array(capabilities));
            aieos.insert("identity".into(), Value::Object(identity));
        }
    }

    // Authority → security_policy
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
    fn import_canonical_aieos_v11_generator_shape() {
        let aieos = serde_json::json!({
            "identity": {
                "names": { "first": "Marta", "last": "Jankowska" },
                "bio": { "gender": "Female", "age_biological": 27 },
                "origin": { "nationality": "Polish", "birthplace": { "city": "Stargard", "country": "Poland" } },
                "residence": { "current_city": "Choszczno", "current_country": "Poland" }
            },
            "psychology": {
                "neural_matrix": {
                    "creativity": 0.55, "empathy": 0.70, "logic": 0.62,
                    "adaptability": 0.50, "charisma": 0.45, "reliability": 0.88
                },
                "traits": {
                    "ocean": {
                        "openness": 0.4, "conscientiousness": 0.82,
                        "extraversion": 0.30, "agreeableness": 0.65, "neuroticism": 0.25
                    },
                    "mbti": "ISFJ"
                },
                "moral_compass": {
                    "alignment": "Lawful Good",
                    "core_values": ["Loyalty", "Helpfulness"],
                    "conflict_resolution_style": "Seeks compromise"
                },
                "emotional_profile": {
                    "base_mood": "calm",
                    "volatility": 0.2
                }
            },
            "linguistics": {
                "text_style": {
                    "formality_level": 0.6,
                    "verbosity_level": 0.4,
                    "style_descriptors": ["Sincere", "Grounded"]
                },
                "syntax": {
                    "sentence_structure": "simple compound",
                    "contractions": true
                },
                "idiolect": {
                    "catchphrases": ["Stay calm, we can do this"],
                    "forbidden_words": ["severe profanity"]
                }
            },
            "motivations": {
                "core_drive": "Maintain a stable and peaceful life",
                "goals": {
                    "short_term": ["Expand greenhouse"],
                    "long_term": ["Support local community"]
                },
                "fears": {
                    "rational": ["Economic downturn"],
                    "irrational": ["Losing keys in a lake"]
                }
            },
            "capabilities": {
                "skills": [
                    { "name": "Gardening", "description": "Expert horticulturist" },
                    { "name": "Community support", "description": "Local organizing" }
                ]
            }
        });

        let result = import_aieos(&aieos).unwrap();

        // Identity
        assert_eq!(result["name"], "MartaJankowska");
        assert!(result["backstory"].as_str().unwrap().contains("Female"));
        assert!(result["backstory"].as_str().unwrap().contains("Polish"));

        // Psychology
        assert_eq!(result["psychology"]["neural_matrix"]["creativity"], 0.55);
        assert_eq!(result["psychology"]["neural_matrix"]["reliability"], 0.88);
        assert_eq!(result["psychology"]["traits"]["mbti"], "ISFJ");
        assert_eq!(result["psychology"]["traits"]["ocean"]["openness"], 0.4);
        assert_eq!(
            result["psychology"]["moral_compass"]["alignment"],
            "lawful-good"
        );
        assert_eq!(
            result["psychology"]["moral_compass"]["core_values"][0],
            "Loyalty"
        );
        assert_eq!(
            result["psychology"]["emotional_profile"]["base_mood"],
            "calm"
        );
        assert_eq!(result["psychology"]["emotional_profile"]["volatility"], 0.2);

        // Voice
        assert_eq!(result["voice"]["style"]["descriptors"][0], "Sincere");
        assert_eq!(result["voice"]["style"]["formality"], 0.6);
        assert_eq!(result["voice"]["style"]["verbosity"], 0.4);
        assert_eq!(result["voice"]["syntax"]["structure"], "simple compound");
        assert_eq!(result["voice"]["syntax"]["contractions"], true);
        assert_eq!(
            result["voice"]["idiolect"]["catchphrases"][0],
            "Stay calm, we can do this"
        );
        assert_eq!(
            result["voice"]["idiolect"]["forbidden_words"][0],
            "severe profanity"
        );

        // Capabilities
        assert_eq!(result["capabilities"]["skills"][0]["name"], "Gardening");
        assert_eq!(
            result["capabilities"]["skills"][1]["name"],
            "Community support"
        );

        // Directives
        assert_eq!(
            result["directives"]["core_drive"],
            "Maintain a stable and peaceful life"
        );
        assert_eq!(result["directives"]["goals"][0], "Expand greenhouse");
        assert_eq!(result["directives"]["goals"][1], "Support local community");
        assert!(result["directives"]["constraints"][0]
            .as_str()
            .unwrap()
            .contains("Economic downturn"));
    }

    #[test]
    fn import_flat_aieos_psychology_paths() {
        let aieos = serde_json::json!({
            "name": "FlatBot",
            "role": "test",
            "psychology": {
                "neural_matrix": { "creativity": 0.9, "logic": 0.8, "empathy": 0.5, "adaptability": 0.6, "charisma": 0.7, "reliability": 0.85 },
                "mbti": "ENTP",
                "ocean": { "openness": 0.9, "conscientiousness": 0.5, "extraversion": 0.8, "agreeableness": 0.6, "neuroticism": 0.3 },
                "moral_compass": { "alignment": "Chaotic Good", "core_values": ["Freedom"] }
            },
            "linguistics": {
                "style": "witty",
                "formality": 0.3,
                "catchphrases": ["Well actually..."],
                "forbidden_words": ["boring"]
            },
            "motivations": {
                "core_drive": "Explore ideas",
                "short_term_goals": ["Learn Rust"],
                "long_term_goals": ["Build something great"]
            }
        });

        let result = import_aieos(&aieos).unwrap();

        assert_eq!(result["psychology"]["traits"]["mbti"], "ENTP");
        assert_eq!(result["psychology"]["traits"]["ocean"]["openness"], 0.9);
        assert_eq!(
            result["psychology"]["moral_compass"]["alignment"],
            "chaotic-good"
        );
        assert_eq!(result["voice"]["style"]["descriptors"][0], "witty");
        assert_eq!(result["voice"]["style"]["formality"], 0.3);
        assert_eq!(
            result["voice"]["idiolect"]["catchphrases"][0],
            "Well actually..."
        );
        assert_eq!(result["directives"]["goals"][0], "Learn Rust");
        assert_eq!(result["directives"]["goals"][1], "Build something great");
    }

    #[test]
    fn import_empty_object_succeeds() {
        let result = import_aieos(&serde_json::json!({})).unwrap();
        assert_eq!(result["version"], "1.0");
    }

    #[test]
    fn import_rejects_non_object() {
        assert!(import_aieos(&serde_json::json!("string")).is_err());
        assert!(import_aieos(&serde_json::json!(42)).is_err());
        assert!(import_aieos(&serde_json::json!(null)).is_err());
    }

    #[test]
    fn alignment_normalization() {
        assert_eq!(
            normalize_alignment("Lawful Good"),
            Some("lawful-good".into())
        );
        assert_eq!(
            normalize_alignment("CHAOTIC NEUTRAL"),
            Some("chaotic-neutral".into())
        );
        assert_eq!(
            normalize_alignment("true-neutral"),
            Some("true-neutral".into())
        );
        assert_eq!(normalize_alignment("invalid"), None);
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

    #[test]
    fn export_maps_psychology_and_voice() {
        let persona = serde_json::json!({
            "name": "ExportBot",
            "role": "test",
            "psychology": {
                "neural_matrix": { "creativity": 0.9 },
                "traits": { "mbti": "ENTP", "ocean": { "openness": 0.8 } },
                "moral_compass": { "alignment": "chaotic-good", "core_values": ["freedom"] }
            },
            "voice": {
                "style": { "descriptors": ["witty"], "formality": 0.3, "verbosity": 0.5 },
                "idiolect": { "catchphrases": ["indeed"], "forbidden_words": ["boring"] }
            },
            "directives": {
                "core_drive": "explore",
                "goals": ["learn"],
                "constraints": ["Avoid: boredom"]
            }
        });

        let exported = export_aieos(&persona).unwrap();

        // Psychology preserved
        assert_eq!(exported["psychology"]["neural_matrix"]["creativity"], 0.9);
        assert_eq!(exported["psychology"]["traits"]["mbti"], "ENTP");
        // Alignment reversed: "chaotic-good" → "Chaotic Good"
        assert_eq!(
            exported["psychology"]["moral_compass"]["alignment"],
            "Chaotic Good"
        );

        // Voice → linguistics
        assert_eq!(
            exported["linguistics"]["text_style"]["style_descriptors"][0],
            "witty"
        );
        assert_eq!(
            exported["linguistics"]["text_style"]["formality_level"],
            0.3
        );
        assert_eq!(
            exported["linguistics"]["idiolect"]["catchphrases"][0],
            "indeed"
        );

        // Directives → motivations
        assert_eq!(exported["motivations"]["core_drive"], "explore");
        assert_eq!(exported["motivations"]["goals"]["short_term"][0], "learn");
        // Constraints with "Avoid:" prefix → fears
        assert_eq!(exported["motivations"]["fears"][0], "boredom");
    }

    #[test]
    fn import_export_roundtrip_behavioral_fields() {
        let aieos = serde_json::json!({
            "name": "RoundTrip",
            "description": "roundtrip test",
            "psychology": {
                "neural_matrix": { "creativity": 0.7, "empathy": 0.6, "logic": 0.8, "adaptability": 0.5, "charisma": 0.4, "reliability": 0.9 },
                "traits": { "mbti": "INTJ", "ocean": { "openness": 0.7, "conscientiousness": 0.9, "extraversion": 0.3, "agreeableness": 0.4, "neuroticism": 0.2 } },
                "moral_compass": { "alignment": "Neutral Good", "core_values": ["Truth"] }
            },
            "linguistics": {
                "text_style": { "style_descriptors": ["precise"], "formality_level": 0.7 },
                "idiolect": { "catchphrases": ["Let me think..."] }
            },
            "motivations": {
                "core_drive": "Understand deeply",
                "goals": { "short_term": ["Analyze"], "long_term": ["Synthesize"] }
            }
        });

        let imported = import_aieos(&aieos).unwrap();
        let exported = export_aieos(&imported).unwrap();

        // Core identity preserved
        assert_eq!(exported["name"], "RoundTrip");

        // Psychology roundtrip
        assert_eq!(exported["psychology"]["neural_matrix"]["creativity"], 0.7);
        assert_eq!(exported["psychology"]["traits"]["mbti"], "INTJ");
        assert_eq!(
            exported["psychology"]["moral_compass"]["alignment"],
            "Neutral Good"
        );

        // Linguistics roundtrip
        assert_eq!(
            exported["linguistics"]["text_style"]["style_descriptors"][0],
            "precise"
        );
        assert_eq!(
            exported["linguistics"]["idiolect"]["catchphrases"][0],
            "Let me think..."
        );

        // Motivations roundtrip
        assert_eq!(exported["motivations"]["core_drive"], "Understand deeply");
    }

    #[test]
    fn capabilities_from_string_array() {
        let aieos = serde_json::json!({
            "name": "Bot",
            "role": "test",
            "capabilities": { "skills": ["coding", "writing", "analysis"] }
        });
        let result = import_aieos(&aieos).unwrap();
        assert_eq!(result["capabilities"]["skills"][0]["name"], "coding");
        assert_eq!(result["capabilities"]["skills"][1]["name"], "writing");
    }

    #[test]
    fn capabilities_from_identity_section() {
        let aieos = serde_json::json!({
            "name": "Bot",
            "role": "test",
            "identity": {
                "capabilities": [
                    { "name": "Gardening" },
                    "Cooking"
                ]
            }
        });
        let result = import_aieos(&aieos).unwrap();
        assert_eq!(result["capabilities"]["skills"][0]["name"], "Gardening");
        assert_eq!(result["capabilities"]["skills"][1]["name"], "Cooking");
    }

    #[test]
    fn name_from_identity_names() {
        let aieos = serde_json::json!({
            "identity": {
                "names": { "first": "Ada", "last": "Lovelace" }
            }
        });
        let result = import_aieos(&aieos).unwrap();
        assert_eq!(result["name"], "AdaLovelace");
    }

    #[test]
    fn trust_level_mapping() {
        let aieos = serde_json::json!({
            "name": "Bot",
            "role": "test",
            "security_policy": { "trust_level": "trusted" }
        });
        let result = import_aieos(&aieos).unwrap();
        assert_eq!(result["authority"]["autonomy"], "full");

        let aieos2 = serde_json::json!({
            "name": "Bot",
            "role": "test",
            "security_policy": { "trust_level": "restricted" }
        });
        let result2 = import_aieos(&aieos2).unwrap();
        assert_eq!(result2["authority"]["autonomy"], "readonly");
    }

    #[test]
    fn unit_clamp_bounds() {
        assert_eq!(unit_clamp(0.5), 0.5);
        assert_eq!(unit_clamp(-0.1), 0.0);
        assert_eq!(unit_clamp(1.5), 1.0);
    }
}
