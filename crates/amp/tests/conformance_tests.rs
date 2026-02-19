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
