//! Golden tests: persona -> expected prompt output.

mod test_helpers;
use test_helpers::{amp_bin, workspace_root};

use std::path::PathBuf;

#[test]
fn golden_quiet_stone_prompt() {
    let out = amp_bin()
        .args(["prompt", "examples/quiet_stone.json"])
        .output()
        .expect("failed to run amp");
    assert!(out.status.success(), "prompt failed");

    let actual = String::from_utf8_lossy(&out.stdout);
    let golden_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/quiet_stone.expected.md");
    let expected = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|e| panic!("missing golden file {}: {e}", golden_path.display()));

    assert_eq!(
        actual.trim(),
        expected.trim(),
        "golden prompt mismatch for quiet_stone"
    );
}

#[test]
fn golden_prompt_contains_all_sections() {
    let out = amp_bin()
        .args(["prompt", "examples/quiet_stone.json"])
        .output()
        .expect("failed to run amp");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for section in [
        "## Identity",
        "## Psychology",
        "## Voice",
        "## Capabilities",
        "## Directives",
    ] {
        assert!(stdout.contains(section), "missing section: {section}");
    }
}

#[test]
fn golden_prompt_has_name_and_role() {
    let out = amp_bin()
        .args(["prompt", "examples/quiet_stone.json"])
        .output()
        .expect("failed to run amp");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("QuietStone"), "missing name");
    assert!(stdout.contains("Senior DevOps Engineer"), "missing role");
}

#[test]
fn v10_prompt_includes_authority_and_gates() {
    let out = amp_bin()
        .args(["prompt", "examples/quiet_stone_v1.json"])
        .output()
        .expect("failed to run amp");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "prompt failed for v1.0");
    assert!(stdout.contains("## Authority"), "missing Authority section");
    assert!(stdout.contains("## Gates"), "missing Gates section");
    assert!(
        stdout.contains("**Autonomy:** supervised"),
        "missing autonomy"
    );
    assert!(stdout.contains("trust_decay"), "missing trust_decay gate");
}
