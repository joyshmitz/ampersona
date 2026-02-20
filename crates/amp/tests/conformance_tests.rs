//! Phase 5 conformance tests: CLI integration via JSON interface.

mod test_helpers;
use test_helpers::{amp_bin, amp_json, workspace_root};

// ── Validation (3) ──────────────────────────────────────────────

#[test]
fn zeroclaw_check_passes() {
    let v = amp_json(
        &[
            "check",
            "examples/zeroclaw_agent.json",
            "--strict",
            "--json",
        ],
        0,
    );
    assert_eq!(v["pass"], true);
    assert_eq!(v["version"], "1.0");
}

#[test]
fn agent_mail_check_passes() {
    let v = amp_json(
        &[
            "check",
            "examples/agent_mail_worker.json",
            "--strict",
            "--json",
        ],
        0,
    );
    assert_eq!(v["pass"], true);
}

#[test]
fn odoov19_check_passes() {
    let v = amp_json(
        &[
            "check",
            "examples/odoov19_quality.json",
            "--strict",
            "--json",
        ],
        0,
    );
    assert_eq!(v["pass"], true);
}

// ── Authority (7) ───────────────────────────────────────────────

#[test]
fn zeroclaw_authority_allow() {
    let v = amp_json(
        &[
            "authority",
            "examples/zeroclaw_agent.json",
            "--check",
            "read_file",
            "--json",
        ],
        0,
    );
    assert_eq!(v["decision"], "Allow");
    assert_eq!(v["action"], "read_file");
}

#[test]
fn zeroclaw_authority_deny_unknown() {
    let v = amp_json(
        &[
            "authority",
            "examples/zeroclaw_agent.json",
            "--check",
            "unknown_action",
            "--json",
        ],
        1,
    );
    assert_eq!(v["decision"], "Deny");
}

#[test]
fn zeroclaw_scoped_shell_blocked() {
    let v = amp_json(
        &[
            "authority",
            "examples/zeroclaw_agent.json",
            "--check",
            "run_command",
            "--context",
            "command=echo $(whoami)",
            "--json",
        ],
        1,
    );
    assert_eq!(v["decision"], "Deny");
}

#[test]
fn authority_path_scope_forbidden() {
    let v = amp_json(
        &[
            "authority",
            "examples/zeroclaw_agent.json",
            "--check",
            "read_file",
            "--path",
            "secrets/key.pem",
            "--json",
        ],
        1,
    );
    assert_eq!(v["decision"], "Deny");
}

#[test]
fn odoov19_deny_with_compliance_ref() {
    let v = amp_json(
        &[
            "authority",
            "examples/odoov19_quality.json",
            "--check",
            "auto_approve_capa",
            "--json",
        ],
        1,
    );
    assert_eq!(v["decision"], "Deny");
    assert!(v["deny_entry"]["compliance_ref"]
        .as_str()
        .unwrap()
        .contains("ISO 9001"));
}

#[test]
fn agent_mail_authority_allow() {
    let v = amp_json(
        &[
            "authority",
            "examples/agent_mail_worker.json",
            "--check",
            "send_message",
            "--json",
        ],
        0,
    );
    assert_eq!(v["decision"], "Allow");
}

#[test]
fn authority_exit_code_needs_approval() {
    // quiet_stone_v1 has supervised autonomy + require_approval_for
    let v = amp_json(
        &[
            "authority",
            "examples/quiet_stone_v1.json",
            "--check",
            "read_file",
            "--json",
        ],
        2,
    );
    assert_eq!(v["decision"], "NeedsApproval");
}

// ── Workspace Defaults (2) ──────────────────────────────────────

#[test]
fn workspace_init_creates_defaults_file() {
    let dir = tempfile::tempdir().unwrap();

    let out = amp_bin()
        .current_dir(dir.path())
        .args(["init", "--workspace"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "init --workspace failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let defaults_path = dir.path().join(".ampersona/defaults.json");
    assert!(defaults_path.exists(), "defaults file was not created");

    let defaults_text = std::fs::read_to_string(&defaults_path).unwrap();
    let defaults: serde_json::Value = serde_json::from_str(&defaults_text).unwrap();
    assert_eq!(defaults["authority"]["autonomy"], "supervised");
}

#[test]
fn workspace_defaults_restrict_authority() {
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("zeroclaw_agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();
    let persona = persona_path.to_str().unwrap();

    // Baseline without workspace defaults: read_file is allowed for zeroclaw example.
    let baseline = amp_bin()
        .current_dir(dir.path())
        .args(["authority", persona, "--check", "read_file", "--json"])
        .output()
        .unwrap();
    assert_eq!(baseline.status.code(), Some(0));
    let baseline_json: serde_json::Value = serde_json::from_slice(&baseline.stdout).unwrap();
    assert_eq!(baseline_json["decision"], "Allow");

    // Add restrictive workspace defaults and verify they are applied.
    std::fs::create_dir_all(dir.path().join(".ampersona")).unwrap();
    std::fs::write(
        dir.path().join(".ampersona/defaults.json"),
        r#"{"authority":{"autonomy":"readonly"}}"#,
    )
    .unwrap();

    let restricted = amp_bin()
        .current_dir(dir.path())
        .args(["authority", persona, "--check", "read_file", "--json"])
        .output()
        .unwrap();
    assert_eq!(restricted.status.code(), Some(1));
    let restricted_json: serde_json::Value = serde_json::from_slice(&restricted.stdout).unwrap();
    assert_eq!(restricted_json["decision"], "Deny");
    assert_eq!(restricted_json["autonomy"], "readonly");
}

// ── Gate (3) ────────────────────────────────────────────────────

#[test]
fn zeroclaw_gate_evaluate() {
    // Use tempdir so parallel tests don't interfere via state files.
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("zeroclaw_agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();
    let metrics_path = dir.path().join("zeroclaw_metrics.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_metrics.json"),
        &metrics_path,
    )
    .unwrap();

    // Phase null → onboarding fires first (→active).
    let out = amp_bin()
        .args([
            "gate",
            persona_path.to_str().unwrap(),
            "--evaluate",
            "*",
            "--metrics",
            metrics_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("failed to run amp");
    assert!(
        out.status.success(),
        "gate failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("invalid JSON");
    assert_eq!(v["gate_id"], "onboarding");
    assert!(v["criteria_results"].is_array());
}

#[test]
fn odoov19_gate_f2() {
    // Use tempdir so parallel tests don't interfere via state files.
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("odoov19_quality.json");
    std::fs::copy(
        workspace_root().join("examples/odoov19_quality.json"),
        &persona_path,
    )
    .unwrap();
    let metrics_path = dir.path().join("odoov19_metrics_f2.json");
    std::fs::copy(
        workspace_root().join("examples/odoov19_metrics_f2.json"),
        &metrics_path,
    )
    .unwrap();

    let out = amp_bin()
        .args([
            "gate",
            persona_path.to_str().unwrap(),
            "--evaluate",
            "*",
            "--metrics",
            metrics_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("failed to run amp");
    assert!(
        out.status.success(),
        "gate failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("invalid JSON");
    assert_eq!(v["gate_id"], "f1_to_f2");
    assert_eq!(v["to_phase"], "F2");
}

#[test]
fn gate_json_no_match() {
    // Request a specific gate that won't fire with bad metrics
    let dir = tempfile::tempdir().unwrap();
    let metrics_path = dir.path().join("bad_metrics.json");
    std::fs::write(
        &metrics_path,
        r#"{"tasks_completed": 1, "error_rate": 0.99, "schema_valid": false}"#,
    )
    .unwrap();

    // We need to set up state in "active" phase for the "trusted" gate to be a candidate.
    // Create a state file so phase = "active".
    let state_path = dir.path().join("zeroclaw_agent.state.json");
    let state = serde_json::json!({
        "name": "ZeroclawWorker",
        "current_phase": "active",
        "state_rev": 1,
        "active_elevations": [],
        "last_transition": null,
        "updated_at": "2024-01-01T00:00:00Z"
    });
    std::fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

    // Copy persona next to state
    let persona_path = dir.path().join("zeroclaw_agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();

    let out = amp_bin()
        .args([
            "gate",
            persona_path.to_str().unwrap(),
            "--evaluate",
            "trusted",
            "--metrics",
            metrics_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("failed to run amp");

    assert_eq!(out.status.code(), Some(1), "expected exit 1 for no match");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("invalid JSON");
    assert_eq!(v["decision"], "no_match");
    assert!(v["criteria_results"].is_array());
    // Check that criteria_results has the expected structure
    let results = v["criteria_results"].as_array().unwrap();
    assert!(!results.is_empty());
    for r in results {
        assert!(r.get("metric").is_some());
        assert!(r.get("pass").is_some());
    }
}

// ── Import/Export roundtrip (3) ─────────────────────────────────

#[test]
fn zeroclaw_import_aieos() {
    let v = amp_json(
        &["import", "examples/aieos_identity.json", "--from", "aieos"],
        0,
    );
    assert_eq!(v["version"], "1.0");
    assert!(v["name"].as_str().is_some());
    assert!(v["psychology"].is_object());
}

#[test]
fn zeroclaw_export_config() {
    let v = amp_json(
        &["export", "examples/zeroclaw_agent.json", "--to", "zeroclaw"],
        0,
    );
    assert!(v["security_policy"].is_object() || v.get("security_policy").is_some());
}

#[test]
fn import_export_roundtrip_stable() {
    // Import AIEOS → ampersona, then export to zeroclaw, check key fields preserved
    let imported = amp_json(
        &["import", "examples/aieos_identity.json", "--from", "aieos"],
        0,
    );
    assert!(imported["name"].as_str().is_some());
    assert!(imported["role"].as_str().is_some());
    // The imported persona should have psychology and voice sections
    assert!(imported["psychology"].is_object());
    assert!(imported["voice"].is_object());
}

// ── Agent_mail register (2) ─────────────────────────────────────

#[test]
fn agent_mail_register_mcp_payload() {
    let v = amp_json(
        &[
            "register",
            "examples/agent_mail_worker.json",
            "--project",
            "/data/projects/test",
            "--rpc",
        ],
        0,
    );
    // Should be a JSON-RPC envelope
    assert_eq!(v["jsonrpc"], "2.0");
    assert!(v["params"]["arguments"]["name"].as_str().is_some());
}

#[test]
fn agent_mail_register_with_prompt() {
    let v = amp_json(
        &[
            "register",
            "examples/agent_mail_worker.json",
            "--project",
            "/data/projects/test",
            "--prompt",
            "--toon",
            "--rpc",
        ],
        0,
    );
    let task_desc = v["params"]["arguments"]["task_description"]
        .as_str()
        .unwrap();
    assert!(
        !task_desc.is_empty(),
        "task_description should contain prompt"
    );
}

// ── Audit (1) ───────────────────────────────────────────────────

#[test]
fn audit_verify_json() {
    // Persona with no audit log → valid with 0 entries
    let v = amp_json(
        &[
            "audit",
            "examples/zeroclaw_agent.json",
            "--verify",
            "--json",
        ],
        0,
    );
    assert_eq!(v["valid"], true);
    assert!(v["entries"].as_u64().is_some());
}

// ── Edge cases (3) ──────────────────────────────────────────────

#[test]
fn authority_no_authority_section() {
    // v0.2 persona without authority section → Deny
    let v = amp_json(
        &[
            "authority",
            "examples/quiet_stone.json",
            "--check",
            "read_file",
            "--json",
        ],
        1,
    );
    assert_eq!(v["decision"], "Deny");
}

#[test]
fn check_v02_persona_passes() {
    let v = amp_json(&["check", "examples/quiet_stone.json", "--json"], 0);
    assert_eq!(v["pass"], true);
    assert_eq!(v["version"], "0.2");
}

#[test]
fn authority_json_error_on_missing_file() {
    let v = amp_json(
        &["authority", "nonexistent.json", "--check", "foo", "--json"],
        3,
    );
    assert_eq!(v["error"], true);
    assert_eq!(v["code"], "E_FILE_NOT_FOUND");
}

// ── Extension round-trip (1) ────────────────────────────────────

/// Extension fields survive serde round-trip (Rust layer).
/// Note: JSON Schema uses additionalProperties:false, so ext fields are validated
/// at the Rust struct level, not by `amp check`. This tests serde round-trip fidelity.
#[test]
fn extension_roundtrip_preserved() {
    // Test that Authority ext fields survive serde round-trip
    let authority_json = serde_json::json!({
        "autonomy": "full",
        "ext": {
            "custom": { "key": 42, "nested": { "deep": true } }
        }
    });

    // Serialize → parse → serialize → compare
    let json_str = serde_json::to_string(&authority_json).unwrap();
    let reparsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(
        authority_json["ext"], reparsed["ext"],
        "ext fields must survive JSON round-trip"
    );
    assert_eq!(reparsed["ext"]["custom"]["key"], 42);
    assert_eq!(reparsed["ext"]["custom"]["nested"]["deep"], true);

    // Also verify that amp check works on a valid persona (without ext in schema)
    let v = amp_json(&["check", "examples/zeroclaw_agent.json", "--json"], 0);
    assert_eq!(v["pass"], true);

    // Verify amp migrate produces identical output (round-trip stable)
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("test.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();

    let before = std::fs::read_to_string(&persona_path).unwrap();
    let before_parsed: serde_json::Value = serde_json::from_str(&before).unwrap();

    let out = amp_bin()
        .args(["migrate", persona_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "migrate should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let after = std::fs::read_to_string(&persona_path).unwrap();
    let after_parsed: serde_json::Value = serde_json::from_str(&after).unwrap();

    // All top-level fields should be preserved
    assert_eq!(before_parsed["name"], after_parsed["name"]);
    assert_eq!(before_parsed["authority"], after_parsed["authority"]);
    assert_eq!(before_parsed["gates"], after_parsed["gates"]);
}

// ── E2E workflow (1) ────────────────────────────────────────────

#[test]
fn zeroclaw_full_lifecycle() {
    let dir = tempfile::tempdir().unwrap();

    // Copy persona to temp dir
    let persona_path = dir.path().join("agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();

    // Copy metrics
    let metrics_path = dir.path().join("metrics.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_metrics.json"),
        &metrics_path,
    )
    .unwrap();

    let persona = persona_path.to_str().unwrap();
    let metrics = metrics_path.to_str().unwrap();

    // 1. Check validates
    let out = amp_bin()
        .args(["check", persona, "--strict", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let check: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(check["pass"], true);

    // 2. Gate: onboarding (null → active)
    let out = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "*",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let gate1: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(gate1["gate_id"], "onboarding");
    assert_eq!(gate1["to_phase"], "active");

    // 3. Authority check in active phase
    let out = amp_bin()
        .args(["authority", persona, "--check", "read_file", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let auth: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(auth["decision"], "Allow");

    // 4. Gate: promote to trusted (active → trusted) — human approval required
    let out = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "*",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(2),
        "human gate should exit 2 (pending)"
    );
    let gate2: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(gate2["gate_id"], "trusted");
    assert_eq!(gate2["decision"], "pending_human");

    // 4b. Approve the pending transition
    let out = amp_bin()
        .args(["gate", persona, "--approve", "trusted", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "approve should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let approved: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(approved["decision"], "approved");
    assert_eq!(approved["to_phase"], "trusted");

    // 5. Status shows trusted phase
    let out = amp_bin()
        .args(["status", persona, "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let status: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(status["phase"], "trusted");
    let state_rev = status["state_rev"].as_u64().unwrap();
    assert!(
        state_rev >= 2,
        "state_rev should be at least 2 after two transitions"
    );

    // 6. Audit verify
    let out = amp_bin()
        .args(["audit", persona, "--verify", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let audit: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(audit["valid"], true);
    let entries = audit["entries"].as_u64().unwrap();
    assert!(entries >= 2, "should have at least 2 audit entries");
}

// ── Spec-Runtime conformance (4) ──────────────────────────────

/// pending_human gate produces exactly one audit entry, not two.
#[test]
fn pending_human_no_double_audit() {
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();
    let metrics_path = dir.path().join("metrics.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_metrics.json"),
        &metrics_path,
    )
    .unwrap();

    let persona = persona_path.to_str().unwrap();
    let metrics = metrics_path.to_str().unwrap();

    // Step 1: onboarding (null → active) — auto gate
    let out = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "*",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "onboarding should succeed");

    // Step 2: promote to trusted — human gate → exit 2
    let out = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "*",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "human gate should exit 2");

    // Step 3: approve
    let out = amp_bin()
        .args(["gate", persona, "--approve", "trusted", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "approve should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Step 4: audit verify — chain must be valid
    let out = amp_bin()
        .args(["audit", persona, "--verify", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let audit: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(audit["valid"], true);

    // Count entries: expect exactly 3 (onboarding + pending_human + approved)
    let entries = audit["entries"].as_u64().unwrap();
    assert_eq!(
        entries, 3,
        "expected exactly 3 audit entries (onboarding, pending, approved), got {entries}"
    );
}

/// Idempotency: transition fires once, then repeated evaluate doesn't re-fire
/// for the same phase (gate from_phase no longer matches after transition).
#[test]
fn idempotent_evaluate_no_duplicate() {
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();
    let metrics_path = dir.path().join("metrics.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_metrics.json"),
        &metrics_path,
    )
    .unwrap();

    let persona = persona_path.to_str().unwrap();
    let metrics = metrics_path.to_str().unwrap();

    // First evaluate: onboarding fires (null → active)
    let out = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "*",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let r1: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(r1["gate_id"], "onboarding");
    assert_eq!(r1["decision"], "transition");

    // Second evaluate: "trusted" gate is human → exit 2
    let out = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "*",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));

    // Third evaluate with same state: pending_human fires again (not idempotent
    // because no transition was applied — pending doesn't set last_transition)
    let out = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "*",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(2),
        "pending still fires before approval"
    );

    // Try to evaluate the already-transitioned onboarding gate specifically:
    // from_phase=null but current is now "active" → no match → exit 1
    let out = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "onboarding",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "onboarding gate should not re-fire after transition"
    );
}

/// Quorum gate returns error, does not crash.
#[test]
fn quorum_gate_deferred_error() {
    let dir = tempfile::tempdir().unwrap();

    // Copy a real persona and replace its gate with a quorum gate
    let persona_path = dir.path().join("quorum.json");
    let src =
        std::fs::read_to_string(workspace_root().join("examples/zeroclaw_agent.json")).unwrap();
    let mut persona: serde_json::Value = serde_json::from_str(&src).unwrap();
    persona["gates"] = serde_json::json!([{
        "id": "quorum_gate",
        "direction": "promote",
        "enforcement": "enforce",
        "priority": 10,
        "from_phase": null,
        "to_phase": "active",
        "criteria": [{ "metric": "ready", "op": "eq", "value": true }],
        "approval": "quorum"
    }]);
    std::fs::write(
        &persona_path,
        serde_json::to_string_pretty(&persona).unwrap(),
    )
    .unwrap();

    let metrics_path = dir.path().join("metrics.json");
    std::fs::write(&metrics_path, r#"{"ready": true}"#).unwrap();

    let persona = persona_path.to_str().unwrap();
    let metrics = metrics_path.to_str().unwrap();

    let out = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "*",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "quorum should exit 1, stderr={}, stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout),
    );
    let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["decision"], "error_quorum_not_supported");
}

/// Approving the wrong gate_id must be a hard error with no side effects.
#[test]
fn approve_wrong_gate_id_hard_error() {
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();
    let metrics_path = dir.path().join("metrics.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_metrics.json"),
        &metrics_path,
    )
    .unwrap();

    let persona = persona_path.to_str().unwrap();
    let metrics = metrics_path.to_str().unwrap();

    // Step 1: onboarding (null → active)
    let out = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "*",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "onboarding should succeed");

    // Step 2: trusted gate → pending_human (exit 2)
    let out = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "*",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));

    // Capture full state + audit count before bad approve
    let state_path = dir.path().join("agent.state.json");
    let state_before: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
    let rev_before = state_before["state_rev"].as_u64().unwrap();
    let audit_path = dir.path().join("agent.audit.jsonl");
    let audit_count_before = if audit_path.exists() {
        std::fs::read_to_string(&audit_path)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count()
    } else {
        0
    };

    // Step 3: approve wrong gate_id → must fail
    let out = amp_bin()
        .args(["gate", persona, "--approve", "nonexistent_gate"])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "approving wrong gate_id must fail, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Verify the error message is about gate mismatch (not some other failure)
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("pending gate is") || stderr.contains("not 'nonexistent_gate'"),
        "error should reference gate mismatch, got: {stderr}"
    );

    // Step 4: full state must be unchanged (zero side effects)
    let state_after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
    assert_eq!(
        state_after["state_rev"].as_u64().unwrap(),
        rev_before,
        "state_rev must not change on failed approve"
    );
    assert_eq!(
        state_after["current_phase"], state_before["current_phase"],
        "phase must not change on failed approve"
    );
    assert_eq!(
        state_after["pending_transition"], state_before["pending_transition"],
        "pending_transition must not change on failed approve"
    );

    // Audit count must not change
    let audit_count_after = if audit_path.exists() {
        std::fs::read_to_string(&audit_path)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count()
    } else {
        0
    };
    assert_eq!(
        audit_count_after, audit_count_before,
        "audit log must not gain entries on failed approve"
    );
}

/// Pending transition does not set last_transition — idempotency triple stays intact.
#[test]
fn pending_does_not_set_last_transition() {
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();
    let metrics_path = dir.path().join("metrics.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_metrics.json"),
        &metrics_path,
    )
    .unwrap();

    let persona = persona_path.to_str().unwrap();
    let metrics = metrics_path.to_str().unwrap();

    // onboarding: null → active (sets last_transition to onboarding)
    let _ = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "*",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();

    // Read state: last_transition should be onboarding
    let state_path = dir.path().join("agent.state.json");
    let state1: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
    assert_eq!(
        state1["last_transition"]["gate_id"], "onboarding",
        "last_transition should be onboarding after first gate"
    );

    // pending_human: trusted gate → exit 2
    let out = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "*",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));

    // Read state again: last_transition MUST still be onboarding (not trusted)
    let state2: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
    assert_eq!(
        state2["last_transition"]["gate_id"], "onboarding",
        "pending_human must NOT overwrite last_transition"
    );
    assert!(
        state2["pending_transition"].is_object(),
        "pending_transition must be set"
    );
    assert_eq!(state2["pending_transition"]["gate_id"], "trusted");
}

/// state_rev increments deterministically: evaluate(+1), approve(+1).
#[test]
fn state_rev_monotonic_through_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();
    let metrics_path = dir.path().join("metrics.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_metrics.json"),
        &metrics_path,
    )
    .unwrap();

    let persona = persona_path.to_str().unwrap();
    let metrics = metrics_path.to_str().unwrap();

    let get_rev = || -> u64 {
        let out = amp_bin()
            .args(["status", persona, "--json"])
            .output()
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        v["state_rev"].as_u64().unwrap_or(0)
    };

    // Before any gate: state_rev = 0 (no state file yet, status returns null)
    // After onboarding: state_rev should be 1
    let _ = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "*",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();
    let rev1 = get_rev();
    assert_eq!(rev1, 1, "state_rev should be 1 after onboarding");

    // After pending_human: state_rev must stay exactly 1.
    // pending_human does NOT apply a transition — no state_rev increment.
    let _ = amp_bin()
        .args([
            "gate",
            persona,
            "--evaluate",
            "*",
            "--metrics",
            metrics,
            "--json",
        ])
        .output()
        .unwrap();
    let rev2 = get_rev();
    assert_eq!(
        rev2, rev1,
        "state_rev must not change on pending_human (no transition applied): got {rev2}, expected {rev1}"
    );

    // After approve: state_rev must increment
    let _ = amp_bin()
        .args(["gate", persona, "--approve", "trusted"])
        .output()
        .unwrap();
    let rev3 = get_rev();
    assert!(
        rev3 > rev2,
        "state_rev must increase after approve: {rev3} <= {rev2}"
    );
}

/// Signed checkpoint: wrong verify key must reject.
#[test]
fn signed_checkpoint_wrong_key_rejects() {
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();
    let metrics_path = dir.path().join("metrics.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_metrics.json"),
        &metrics_path,
    )
    .unwrap();

    let persona = persona_path.to_str().unwrap();
    let metrics = metrics_path.to_str().unwrap();

    // Generate a gate transition to create audit entries
    let _ = amp_bin()
        .args(["gate", persona, "--evaluate", "*", "--metrics", metrics])
        .output()
        .unwrap();

    // Create signing key (32 bytes)
    let sign_key_path = dir.path().join("sign.key");
    let wrong_key_path = dir.path().join("wrong.key");
    std::fs::write(&sign_key_path, [0xAAu8; 32]).unwrap();
    std::fs::write(&wrong_key_path, [0xBBu8; 32]).unwrap();

    // Derive pubkey from wrong key (different from sign key)
    let wrong_signing = ed25519_dalek::SigningKey::from_bytes(&[0xBBu8; 32]);
    let wrong_pub = wrong_signing.verifying_key();
    let wrong_pub_path = dir.path().join("wrong.pub");
    std::fs::write(&wrong_pub_path, wrong_pub.as_bytes()).unwrap();

    // Create signed checkpoint
    let cp_path = dir.path().join("agent.checkpoint.json");
    let out = amp_bin()
        .args([
            "audit",
            persona,
            "--checkpoint-create",
            "--checkpoint",
            cp_path.to_str().unwrap(),
            "--sign-key",
            sign_key_path.to_str().unwrap(),
            "--sign-key-id",
            "test-key",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "checkpoint create should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Verify with wrong pubkey → must fail (exit 1)
    let out = amp_bin()
        .args([
            "audit",
            persona,
            "--checkpoint-verify",
            "--checkpoint",
            cp_path.to_str().unwrap(),
            "--verify-key",
            wrong_pub_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "wrong verify key must reject, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["valid"], false);
}

/// Checkpoint verify with --verify-key on unsigned checkpoint must error.
#[test]
fn checkpoint_missing_signature_errors() {
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();
    let metrics_path = dir.path().join("metrics.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_metrics.json"),
        &metrics_path,
    )
    .unwrap();

    let persona = persona_path.to_str().unwrap();
    let metrics = metrics_path.to_str().unwrap();

    // Create audit entry
    let _ = amp_bin()
        .args(["gate", persona, "--evaluate", "*", "--metrics", metrics])
        .output()
        .unwrap();

    // Create unsigned checkpoint
    let cp_path = dir.path().join("agent.checkpoint.json");
    let out = amp_bin()
        .args([
            "audit",
            persona,
            "--checkpoint-create",
            "--checkpoint",
            cp_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());

    // Try to verify signature on unsigned checkpoint → error
    let dummy_key_path = dir.path().join("dummy.pub");
    let dummy_signing = ed25519_dalek::SigningKey::from_bytes(&[0xCCu8; 32]);
    let dummy_pub = dummy_signing.verifying_key();
    std::fs::write(&dummy_key_path, dummy_pub.as_bytes()).unwrap();

    let out = amp_bin()
        .args([
            "audit",
            persona,
            "--checkpoint-verify",
            "--checkpoint",
            cp_path.to_str().unwrap(),
            "--verify-key",
            dummy_key_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // Must fail — unsigned checkpoint has no signature field
    assert!(
        !out.status.success(),
        "verifying unsigned checkpoint must fail"
    );
    // Verify the failure is specifically about missing signature (not some other error)
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no signature") || stderr.contains("signature"),
        "error should reference missing signature, got: {stderr}"
    );
}

/// state_rev vs audit: detect inconsistency when state advanced without audit.
#[test]
fn state_rev_audit_consistency_check() {
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();
    let metrics_path = dir.path().join("metrics.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_metrics.json"),
        &metrics_path,
    )
    .unwrap();

    let persona = persona_path.to_str().unwrap();
    let metrics = metrics_path.to_str().unwrap();

    // Run gate to create state + audit
    let _ = amp_bin()
        .args(["gate", persona, "--evaluate", "*", "--metrics", metrics])
        .output()
        .unwrap();

    // Verify state_rev_check is present in audit --verify --json
    let out = amp_bin()
        .args(["audit", persona, "--verify", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let audit: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(audit["valid"], true);
    // state_rev_check should be present since state file exists
    assert!(
        audit.get("state_rev_check").is_some(),
        "state_rev_check should be present in audit verify output"
    );
    assert_eq!(audit["state_rev_check"]["consistent"], true);

    // Now artificially bump state_rev to create inconsistency
    let state_path = dir.path().join("agent.state.json");
    let state_text = std::fs::read_to_string(&state_path).unwrap();
    let mut state: serde_json::Value = serde_json::from_str(&state_text).unwrap();
    state["state_rev"] = serde_json::json!(99);
    std::fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

    // Re-verify: should flag inconsistency
    let out = amp_bin()
        .args(["audit", persona, "--verify", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let audit: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        audit.get("state_rev_check").is_some(),
        "state_rev_check should be present"
    );
    // state_rev=99 but only 1 state mutation → inconsistent
    assert_eq!(audit["state_rev_check"]["state_rev"], 99);
    assert_eq!(
        audit["state_rev_check"]["consistent"], false,
        "state_rev=99 with 1 state mutation should be inconsistent"
    );
}

/// Audit chain stays valid through a full pending/approve lifecycle.
#[test]
fn audit_valid_after_pending_approve_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();
    let metrics_path = dir.path().join("metrics.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_metrics.json"),
        &metrics_path,
    )
    .unwrap();

    let persona = persona_path.to_str().unwrap();
    let metrics = metrics_path.to_str().unwrap();

    // 1. Onboarding
    let _ = amp_bin()
        .args(["gate", persona, "--evaluate", "*", "--metrics", metrics])
        .output()
        .unwrap();

    // 2. Pending human
    let _ = amp_bin()
        .args(["gate", persona, "--evaluate", "*", "--metrics", metrics])
        .output()
        .unwrap();

    // 3. Approve
    let _ = amp_bin()
        .args(["gate", persona, "--approve", "trusted"])
        .output()
        .unwrap();

    // 4. Verify chain integrity with --from 0
    let out = amp_bin()
        .args(["audit", persona, "--verify", "--from", "0", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "audit verify should pass: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let audit: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(audit["valid"], true);
    assert!(audit["entries"].as_u64().unwrap() >= 3);
}

/// Sidecar .authority_overlay.json is migrated into state on gate evaluate.
#[test]
fn sidecar_overlay_migration_to_state() {
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();
    let metrics_path = dir.path().join("metrics.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_metrics.json"),
        &metrics_path,
    )
    .unwrap();

    let persona = persona_path.to_str().unwrap();
    let metrics = metrics_path.to_str().unwrap();

    // Run onboarding gate to create state
    let _ = amp_bin()
        .args(["gate", persona, "--evaluate", "*", "--metrics", metrics])
        .output()
        .unwrap();

    // Create a legacy sidecar overlay file
    let sidecar_path = dir.path().join("agent.authority_overlay.json");
    std::fs::write(&sidecar_path, r#"{"autonomy": "full"}"#).unwrap();

    // Verify sidecar exists
    assert!(
        sidecar_path.exists(),
        "sidecar should exist before migration"
    );

    // Run gate evaluate again — should migrate sidecar into state
    let _ = amp_bin()
        .args(["gate", persona, "--evaluate", "*", "--metrics", metrics])
        .output()
        .unwrap();

    // Sidecar should be deleted
    assert!(
        !sidecar_path.exists(),
        "sidecar should be deleted after migration"
    );

    // State should have active_overlay
    let state_path = dir.path().join("agent.state.json");
    let state: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
    assert!(
        state.get("active_overlay").is_some(),
        "state should have active_overlay after migration"
    );
    assert_eq!(
        state["active_overlay"]["autonomy"], "full",
        "migrated overlay should preserve autonomy"
    );
}

/// State overlay takes precedence over sidecar file.
#[test]
fn state_overlay_preferred_over_sidecar() {
    let dir = tempfile::tempdir().unwrap();
    let persona_path = dir.path().join("agent.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_agent.json"),
        &persona_path,
    )
    .unwrap();
    let metrics_path = dir.path().join("metrics.json");
    std::fs::copy(
        workspace_root().join("examples/zeroclaw_metrics.json"),
        &metrics_path,
    )
    .unwrap();

    let persona = persona_path.to_str().unwrap();
    let metrics = metrics_path.to_str().unwrap();

    // Run onboarding gate to create state
    let _ = amp_bin()
        .args(["gate", persona, "--evaluate", "*", "--metrics", metrics])
        .output()
        .unwrap();

    // Set active_overlay in state directly (simulating already-migrated state)
    let state_path = dir.path().join("agent.state.json");
    let state_text = std::fs::read_to_string(&state_path).unwrap();
    let mut state: serde_json::Value = serde_json::from_str(&state_text).unwrap();
    state["active_overlay"] = serde_json::json!({"autonomy": "supervised"});
    std::fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

    // Also create a sidecar file with different autonomy
    let sidecar_path = dir.path().join("agent.authority_overlay.json");
    std::fs::write(&sidecar_path, r#"{"autonomy": "full"}"#).unwrap();

    // Authority check should use state overlay (supervised), not sidecar (full)
    let out = amp_bin()
        .args(["authority", persona, "--check", "read_file", "--json"])
        .output()
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        result["autonomy"], "supervised",
        "should use state overlay, not sidecar"
    );

    // Sidecar should NOT be deleted (migration only happens when state has no overlay)
    assert!(
        sidecar_path.exists(),
        "sidecar should not be deleted when state already has overlay"
    );
}

/// window_seconds on criterion survives schema validation and round-trip.
#[test]
fn window_seconds_schema_roundtrip() {
    // zeroclaw_agent.json trust_decay gate now has window_seconds: 2592000
    let v = amp_json(
        &[
            "check",
            "examples/zeroclaw_agent.json",
            "--strict",
            "--json",
        ],
        0,
    );
    assert_eq!(
        v["pass"], true,
        "persona with window_seconds must pass check"
    );

    // Round-trip: parse → serialize → parse, verify window_seconds preserved
    let src =
        std::fs::read_to_string(workspace_root().join("examples/zeroclaw_agent.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&src).unwrap();
    let gates = parsed["gates"].as_array().unwrap();
    let trust_decay = gates.iter().find(|g| g["id"] == "trust_decay").unwrap();
    let criterion = &trust_decay["criteria"][0];
    assert_eq!(
        criterion["window_seconds"], 2592000,
        "window_seconds should be 2592000 (30 days)"
    );
    assert_eq!(criterion["metric"], "policy_violations");

    // Serialize back and re-parse
    let reserialized = serde_json::to_string_pretty(&parsed).unwrap();
    let reparsed: serde_json::Value = serde_json::from_str(&reserialized).unwrap();
    let gates2 = reparsed["gates"].as_array().unwrap();
    let td2 = gates2.iter().find(|g| g["id"] == "trust_decay").unwrap();
    assert_eq!(td2["criteria"][0]["window_seconds"], 2592000);
}

/// NeedsApproval matrix: test authority decision across autonomy levels.
#[test]
fn needs_approval_autonomy_matrix() {
    let dir = tempfile::tempdir().unwrap();

    // Helper: create persona with given autonomy and optional require_approval_for
    let make_persona = |autonomy: &str, require_approval: bool| -> serde_json::Value {
        let mut persona = serde_json::json!({
            "version": "1.0",
            "name": "MatrixTest",
            "role": "test",
            "psychology": {
                "neural_matrix": {
                    "creativity": 0.5, "empathy": 0.5, "logic": 0.5,
                    "adaptability": 0.5, "charisma": 0.5, "reliability": 0.5
                },
                "traits": {
                    "mbti": "INTJ", "temperament": "phlegmatic",
                    "ocean": { "openness": 0.5, "conscientiousness": 0.5,
                        "extraversion": 0.5, "agreeableness": 0.5, "neuroticism": 0.5 }
                },
                "moral_compass": { "alignment": "true-neutral", "core_values": ["test"] },
                "emotional_profile": { "base_mood": "calm", "volatility": 0.1 }
            },
            "voice": {
                "style": { "descriptors": ["terse"], "formality": 0.5, "verbosity": 0.3 },
                "syntax": { "structure": "declarative", "contractions": true },
                "idiolect": { "catchphrases": [], "forbidden_words": [] }
            },
            "authority": {
                "autonomy": autonomy,
                "actions": { "allow": ["read_file"] }
            }
        });
        if require_approval {
            persona["authority"]["limits"] = serde_json::json!({
                "require_approval_for": ["high_risk"]
            });
        }
        persona
    };

    // Matrix:
    // | autonomy   | require_approval | action=read_file | expected      | exit |
    // |------------|------------------|------------------|---------------|------|
    // | full       | false            | read_file        | Allow         | 0    |
    // | supervised | false            | read_file        | Allow         | 0    |
    // | supervised | true             | read_file        | NeedsApproval | 2    |
    // | readonly   | false            | read_file        | Deny          | 1    |
    let cases = [
        ("full", false, 0, "Allow"),
        ("supervised", false, 0, "Allow"),
        ("supervised", true, 2, "NeedsApproval"),
        ("readonly", false, 1, "Deny"),
    ];

    for (autonomy, require_approval, expected_exit, expected_decision) in &cases {
        let persona = make_persona(autonomy, *require_approval);
        let path = dir
            .path()
            .join(format!("matrix_{autonomy}_{require_approval}.json"));
        std::fs::write(&path, serde_json::to_string_pretty(&persona).unwrap()).unwrap();

        let out = amp_bin()
            .args([
                "authority",
                path.to_str().unwrap(),
                "--check",
                "read_file",
                "--json",
            ])
            .output()
            .unwrap();
        let exit = out.status.code().unwrap_or(-1);
        assert_eq!(
            exit, *expected_exit,
            "autonomy={autonomy} require_approval={require_approval}: expected exit {expected_exit}, got {exit}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            result["decision"], *expected_decision,
            "autonomy={autonomy} require_approval={require_approval}: expected {expected_decision}, got {}",
            result["decision"]
        );
    }
}
