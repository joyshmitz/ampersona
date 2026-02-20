use anyhow::{bail, Context, Result};
use jsonschema::Validator;
use serde_json::Value;

use crate::errors::{CheckIssue, CheckReport};

const SCHEMA_V02: &str = include_str!("../schema/ampersona-v0.2.schema.json");
const SCHEMA_V10: &str = include_str!("../schema/ampersona-v1.0.schema.json");

/// Detect the version of a persona JSON value.
pub fn detect_version(data: &Value) -> &'static str {
    match data.get("version").and_then(Value::as_str) {
        Some("1.0") => "1.0",
        _ => "0.2",
    }
}

/// Create a validator for the given version.
pub fn validator_for(version: &str) -> Result<Validator> {
    let schema_str = match version {
        "1.0" => SCHEMA_V10,
        _ => SCHEMA_V02,
    };
    let schema: Value =
        serde_json::from_str(schema_str).context("embedded schema is invalid JSON")?;
    Validator::new(&schema).map_err(|e| anyhow::anyhow!("schema compilation failed: {e}"))
}

/// Create a validator (auto-detect version from data).
pub fn validator(data: &Value) -> Result<Validator> {
    validator_for(detect_version(data))
}

/// Validate a single persona value (auto-detect version).
pub fn validate(data: &Value) -> Result<()> {
    let v = validator(data)?;
    if v.is_valid(data) {
        return Ok(());
    }
    let mut msgs: Vec<String> = Vec::new();
    for error in v.iter_errors(data) {
        let path = error.instance_path.to_string();
        let loc = if path.is_empty() {
            "(root)".into()
        } else {
            path
        };
        msgs.push(format!("  {loc}: {error}"));
    }
    bail!("validation failed:\n{}", msgs.join("\n"));
}

/// Validate multiple files, printing results. Returns (passed, failed) counts.
pub fn validate_files(paths: &[String]) -> Result<(usize, usize)> {
    let mut passed = 0usize;
    let mut failed = 0usize;
    for path in paths {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("cannot read {path}"))?;
        let data: Value =
            serde_json::from_str(&content).with_context(|| format!("{path}: invalid JSON"))?;
        let v = validator(&data)?;
        if v.is_valid(&data) {
            eprintln!("  ok  {path}");
            passed += 1;
        } else {
            failed += 1;
            eprintln!("  FAIL {path}");
            for error in v.iter_errors(&data) {
                let p = error.instance_path.to_string();
                let loc = if p.is_empty() { "(root)".into() } else { p };
                eprintln!("       {loc}: {error}");
            }
        }
    }
    Ok((passed, failed))
}

/// Full check producing structured report (for `amp check --json`).
pub fn check(data: &Value, file: &str, strict: bool) -> CheckReport {
    let version = detect_version(data).to_string();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Schema validation
    if let Ok(v) = validator(data) {
        for error in v.iter_errors(data) {
            let path = error.instance_path.to_string();
            errors.push(CheckIssue {
                code: "E001".to_string(),
                check: "schema".to_string(),
                message: error.to_string(),
                path: Some(if path.is_empty() {
                    "$(root)".to_string()
                } else {
                    format!("${path}")
                }),
            });
        }
    }

    // Action vocabulary check (v1.0 only)
    if version == "1.0" {
        check_action_vocabulary(data, &mut errors, &mut warnings);
    }

    // Signature check (v1.0 only): warn if present but not verifiable
    if version == "1.0" {
        check_signature(data, &mut warnings);
    }

    // Strict signature verification (v1.0 only): verify ed25519 if signature + public_key present
    if version == "1.0" && strict {
        check_signature_strict(data, &mut errors, &mut warnings);
    }

    // Consistency checks: gate acyclicity and metrics_schema (E020-E029, v1.0 only)
    if version == "1.0" {
        check_gate_consistency(data, &mut warnings);
    }

    // Contract version check (opt-in)
    check_contract(data, &mut warnings);

    // Lint checks
    lint_checks(data, &version, strict, &mut warnings);

    let pass = errors.is_empty() && (!strict || warnings.is_empty());
    CheckReport {
        file: file.to_string(),
        version,
        pass,
        errors,
        warnings,
    }
}

fn check_action_vocabulary(
    data: &Value,
    errors: &mut Vec<CheckIssue>,
    _warnings: &mut Vec<CheckIssue>,
) {
    if let Some(actions) = data.pointer("/authority/actions") {
        // Check allow list
        if let Some(allow) = actions.get("allow").and_then(Value::as_array) {
            for (i, action) in allow.iter().enumerate() {
                if let Some(name) = action.as_str() {
                    if name.parse::<crate::actions::ActionId>().is_err() {
                        let suggestion = crate::actions::BuiltinAction::suggest(name);
                        let msg = if let Some(s) = suggestion {
                            format!("unknown action '{name}' \u{2014} did you mean '{s}'?")
                        } else {
                            format!("unknown action '{name}'")
                        };
                        errors.push(CheckIssue {
                            code: "E010".to_string(),
                            check: "action_vocab".to_string(),
                            message: msg,
                            path: Some(format!("$.authority.actions.allow[{i}]")),
                        });
                    }
                }
            }
        }
        // Check deny list
        if let Some(deny) = actions.get("deny").and_then(Value::as_array) {
            for (i, entry) in deny.iter().enumerate() {
                let name = entry
                    .as_str()
                    .or_else(|| entry.get("action").and_then(Value::as_str));
                if let Some(name) = name {
                    if name.parse::<crate::actions::ActionId>().is_err() {
                        let suggestion = crate::actions::BuiltinAction::suggest(name);
                        let msg = if let Some(s) = suggestion {
                            format!("unknown action '{name}' \u{2014} did you mean '{s}'?")
                        } else {
                            format!("unknown action '{name}'")
                        };
                        errors.push(CheckIssue {
                            code: "E010".to_string(),
                            check: "action_vocab".to_string(),
                            message: msg,
                            path: Some(format!("$.authority.actions.deny[{i}]")),
                        });
                    }
                }
            }
        }
    }
}

fn check_signature(data: &Value, warnings: &mut Vec<CheckIssue>) {
    if let Some(sig) = data.get("signature") {
        // Check that signed_fields covers expected fields
        if let Some(fields) = sig.get("signed_fields").and_then(Value::as_array) {
            let field_names: Vec<&str> = fields.iter().filter_map(Value::as_str).collect();
            // Should cover all non-signature top-level fields
            if let Some(obj) = data.as_object() {
                for key in obj.keys() {
                    if key != "signature"
                        && key != "$schema"
                        && !field_names.contains(&key.as_str())
                    {
                        warnings.push(CheckIssue {
                            code: "W010".to_string(),
                            check: "signature".to_string(),
                            message: format!("field '{key}' not listed in signature.signed_fields"),
                            path: Some("$.signature.signed_fields".to_string()),
                        });
                    }
                }
            }
        }
        // Check key_id is present
        if sig.get("key_id").and_then(Value::as_str).is_none() {
            warnings.push(CheckIssue {
                code: "W011".to_string(),
                check: "signature".to_string(),
                message: "signature missing key_id (needed for key rotation)".to_string(),
                path: Some("$.signature.key_id".to_string()),
            });
        }
    }
}

/// Strict signature verification: verify ed25519 if signature + public_key present.
fn check_signature_strict(
    data: &Value,
    errors: &mut Vec<CheckIssue>,
    warnings: &mut Vec<CheckIssue>,
) {
    let Some(sig) = data.get("signature") else {
        return;
    };
    let Some(sig_value_hex) = sig.get("value").and_then(Value::as_str) else {
        return;
    };

    let Some(public_key_hex) = sig.get("public_key").and_then(Value::as_str) else {
        warnings.push(CheckIssue {
            code: "W012".to_string(),
            check: "signature_verify".to_string(),
            message: "signature present but no public_key for verification".to_string(),
            path: Some("$.signature.public_key".to_string()),
        });
        return;
    };

    // Parse public key
    let pubkey_bytes: Result<Vec<u8>, _> = (0..public_key_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(public_key_hex.get(i..i + 2).unwrap_or("00"), 16))
        .collect();
    let Ok(pubkey_bytes) = pubkey_bytes else {
        errors.push(CheckIssue {
            code: "E030".to_string(),
            check: "signature_verify".to_string(),
            message: "invalid hex in signature.public_key".to_string(),
            path: Some("$.signature.public_key".to_string()),
        });
        return;
    };
    if pubkey_bytes.len() != 32 {
        errors.push(CheckIssue {
            code: "E030".to_string(),
            check: "signature_verify".to_string(),
            message: format!("public_key must be 32 bytes, got {}", pubkey_bytes.len()),
            path: Some("$.signature.public_key".to_string()),
        });
        return;
    }
    let Ok(verifying_key) =
        ed25519_dalek::VerifyingKey::from_bytes(pubkey_bytes.as_slice().try_into().unwrap())
    else {
        errors.push(CheckIssue {
            code: "E030".to_string(),
            check: "signature_verify".to_string(),
            message: "invalid ed25519 public key".to_string(),
            path: Some("$.signature.public_key".to_string()),
        });
        return;
    };

    // Parse signature
    let sig_bytes: Result<Vec<u8>, _> = (0..sig_value_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(sig_value_hex.get(i..i + 2).unwrap_or("00"), 16))
        .collect();
    let Ok(sig_bytes) = sig_bytes else {
        errors.push(CheckIssue {
            code: "E030".to_string(),
            check: "signature_verify".to_string(),
            message: "invalid hex in signature.value".to_string(),
            path: Some("$.signature.value".to_string()),
        });
        return;
    };
    let Ok(signature) = ed25519_dalek::Signature::from_slice(&sig_bytes) else {
        errors.push(CheckIssue {
            code: "E030".to_string(),
            check: "signature_verify".to_string(),
            message: "invalid ed25519 signature format".to_string(),
            path: Some("$.signature.value".to_string()),
        });
        return;
    };

    // Reconstruct the signed content: canonicalize signed fields
    let signed_fields = sig
        .get("signed_fields")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut signable = serde_json::Map::new();
    if let Some(obj) = data.as_object() {
        for field in &signed_fields {
            if let Some(val) = obj.get(*field) {
                signable.insert(field.to_string(), val.clone());
            }
        }
    }
    let canonical = serde_json::to_string(&Value::Object(signable)).unwrap_or_default();

    // Verify
    use ed25519_dalek::Verifier;
    if verifying_key
        .verify(canonical.as_bytes(), &signature)
        .is_err()
    {
        errors.push(CheckIssue {
            code: "E030".to_string(),
            check: "signature_verify".to_string(),
            message: "signature verification failed".to_string(),
            path: Some("$.signature.value".to_string()),
        });
    }
}

/// Known contract versions.
const KNOWN_CONTRACT_VERSIONS: &[&str] = &["1.0"];

fn check_gate_consistency(data: &Value, warnings: &mut Vec<CheckIssue>) {
    let gates = match data.get("gates").and_then(Value::as_array) {
        Some(g) if !g.is_empty() => g,
        _ => return,
    };

    // E020: Gate same-direction cycle detection
    // A promote A→B paired with a demote B→A is the expected trust progression pattern.
    // Only flag when two gates of the SAME direction form a cycle.
    let mut edges: Vec<(String, String, String)> = Vec::new(); // (from, to, direction)
    for gate in gates {
        let from = gate
            .get("from_phase")
            .and_then(Value::as_str)
            .unwrap_or("null");
        let to = gate.get("to_phase").and_then(Value::as_str).unwrap_or("");
        let dir = gate
            .get("direction")
            .and_then(Value::as_str)
            .unwrap_or("promote");
        if !to.is_empty() {
            edges.push((from.to_string(), to.to_string(), dir.to_string()));
        }
    }
    for (i, (from_a, to_a, dir_a)) in edges.iter().enumerate() {
        for (from_b, to_b, dir_b) in edges.iter().skip(i + 1) {
            if from_a == to_b && to_a == from_b && dir_a == dir_b {
                warnings.push(CheckIssue {
                    code: "E020".to_string(),
                    check: "consistency".to_string(),
                    message: format!(
                        "same-direction gate cycle ({dir_a}): {from_a} \u{2192} {to_a} and {from_b} \u{2192} {to_b}"
                    ),
                    path: Some("$.gates".to_string()),
                });
            }
        }
    }

    // E021: metrics_schema references metric not used in criteria
    for (i, gate) in gates.iter().enumerate() {
        if let Some(schema) = gate.get("metrics_schema").and_then(Value::as_object) {
            let criteria = gate
                .get("criteria")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let criteria_metrics: Vec<String> = criteria
                .iter()
                .filter_map(|c| c.get("metric").and_then(Value::as_str))
                .map(|s| s.to_string())
                .collect();
            for key in schema.keys() {
                if !criteria_metrics.contains(key) {
                    warnings.push(CheckIssue {
                        code: "E021".to_string(),
                        check: "consistency".to_string(),
                        message: format!(
                            "metrics_schema declares '{key}' but no criterion references it"
                        ),
                        path: Some(format!("$.gates[{i}].metrics_schema.{key}")),
                    });
                }
            }
        }
    }
}

fn check_contract(data: &Value, warnings: &mut Vec<CheckIssue>) {
    if let Some(contract) = data.get("ampersona_contract").and_then(Value::as_str) {
        if !KNOWN_CONTRACT_VERSIONS.contains(&contract) {
            warnings.push(CheckIssue {
                code: "W020".to_string(),
                check: "contract".to_string(),
                message: format!("ampersona_contract references unknown version '{contract}'"),
                path: Some("$.ampersona_contract".to_string()),
            });
        }
    }
    // No warning for missing field — opt-in
}

fn lint_checks(data: &Value, version: &str, _strict: bool, warnings: &mut Vec<CheckIssue>) {
    if version != "1.0" {
        return;
    }

    // W001: supervised autonomy without gates
    let autonomy = data.pointer("/authority/autonomy").and_then(Value::as_str);
    let has_gates = data
        .get("gates")
        .and_then(Value::as_array)
        .is_some_and(|g| !g.is_empty());

    if autonomy == Some("supervised") && !has_gates {
        warnings.push(CheckIssue {
            code: "W001".to_string(),
            check: "lint".to_string(),
            message: "supervised autonomy without gates".to_string(),
            path: Some("$.authority.autonomy".to_string()),
        });
    }

    // W002: deny entry without compliance_ref
    if let Some(deny) = data
        .pointer("/authority/actions/deny")
        .and_then(Value::as_array)
    {
        for (i, entry) in deny.iter().enumerate() {
            if entry.is_object()
                && entry.get("action").is_some()
                && entry.get("compliance_ref").is_none()
            {
                warnings.push(CheckIssue {
                    code: "W002".to_string(),
                    check: "lint".to_string(),
                    message: "deny entry without compliance_ref".to_string(),
                    path: Some(format!("$.authority.actions.deny[{i}]")),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_v10() -> Value {
        serde_json::json!({
            "version": "1.0",
            "name": "Test",
            "role": "test",
            "psychology": {
                "neural_matrix": {
                    "creativity": 0.5, "empathy": 0.5, "logic": 0.5,
                    "adaptability": 0.5, "charisma": 0.5, "reliability": 0.5
                },
                "traits": {
                    "mbti": "INTJ", "temperament": "phlegmatic",
                    "ocean": {
                        "openness": 0.5, "conscientiousness": 0.5, "extraversion": 0.5,
                        "agreeableness": 0.5, "neuroticism": 0.5
                    }
                },
                "moral_compass": { "alignment": "neutral", "core_values": ["test"] },
                "emotional_profile": { "base_mood": "calm", "volatility": 0.1 }
            },
            "voice": {
                "style": { "descriptors": ["terse"], "formality": 0.5, "verbosity": 0.3 },
                "syntax": { "structure": "declarative", "contractions": true },
                "idiolect": { "catchphrases": ["test"], "forbidden_words": [] }
            }
        })
    }

    #[test]
    fn contract_known_version_no_warning() {
        let mut data = minimal_v10();
        data.as_object_mut()
            .unwrap()
            .insert("ampersona_contract".into(), Value::String("1.0".into()));
        let report = check(&data, "test.json", true);
        assert!(
            report.warnings.iter().all(|w| w.code != "W020"),
            "should not warn on known contract version"
        );
    }

    #[test]
    fn check_strict_valid_signature_passes() {
        use ed25519_dalek::Signer;
        let mut data = minimal_v10();

        // Sign the persona
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[0xAA; 32]);
        let verifying_key = signing_key.verifying_key();
        let pubkey_hex: String = verifying_key
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();

        let signed_fields = vec!["name", "role", "psychology", "voice"];
        let mut signable = serde_json::Map::new();
        if let Some(obj) = data.as_object() {
            for field in &signed_fields {
                if let Some(val) = obj.get(*field) {
                    signable.insert(field.to_string(), val.clone());
                }
            }
        }
        let canonical = serde_json::to_string(&Value::Object(signable)).unwrap();
        let sig = signing_key.sign(canonical.as_bytes());
        let sig_hex: String = sig.to_bytes().iter().map(|b| format!("{b:02x}")).collect();

        data.as_object_mut().unwrap().insert(
            "signature".into(),
            serde_json::json!({
                "algorithm": "ed25519",
                "key_id": "test",
                "signed_fields": signed_fields,
                "value": sig_hex,
                "public_key": pubkey_hex,
            }),
        );

        let report = check(&data, "test.json", true);
        assert!(
            report.errors.iter().all(|e| e.code != "E030"),
            "valid signature should not produce E030, errors: {:?}",
            report.errors
        );
    }

    #[test]
    fn check_strict_invalid_signature_fails() {
        let mut data = minimal_v10();

        // Use a valid-format but wrong signature
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[0xAA; 32]);
        let verifying_key = signing_key.verifying_key();
        let pubkey_hex: String = verifying_key
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();

        // Sign wrong content
        use ed25519_dalek::Signer;
        let sig = signing_key.sign(b"wrong content");
        let sig_hex: String = sig.to_bytes().iter().map(|b| format!("{b:02x}")).collect();

        data.as_object_mut().unwrap().insert(
            "signature".into(),
            serde_json::json!({
                "algorithm": "ed25519",
                "key_id": "test",
                "signed_fields": ["name", "role"],
                "value": sig_hex,
                "public_key": pubkey_hex,
            }),
        );

        let report = check(&data, "test.json", true);
        assert!(
            report.errors.iter().any(|e| e.code == "E030"),
            "tampered persona should produce E030"
        );
    }

    #[test]
    fn check_normal_ignores_signature() {
        let mut data = minimal_v10();

        // Add invalid signature — without --strict, no E030
        data.as_object_mut().unwrap().insert(
            "signature".into(),
            serde_json::json!({
                "algorithm": "ed25519",
                "key_id": "test",
                "signed_fields": ["name"],
                "value": "deadbeef",
                "public_key": "cafebabe",
            }),
        );

        let report = check(&data, "test.json", false);
        assert!(
            report.errors.iter().all(|e| e.code != "E030"),
            "without --strict, signature should not be verified"
        );
    }

    #[test]
    fn contract_unknown_version_warns() {
        let mut data = minimal_v10();
        data.as_object_mut()
            .unwrap()
            .insert("ampersona_contract".into(), Value::String("99.0".into()));
        let report = check(&data, "test.json", false);
        assert!(
            report.warnings.iter().any(|w| w.code == "W020"),
            "should warn on unknown contract version"
        );
    }
}
