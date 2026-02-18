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
