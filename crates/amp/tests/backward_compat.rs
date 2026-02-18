//! Backward compatibility tests: v0.2 examples MUST validate unchanged.

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap().parent().unwrap().to_path_buf()
}

fn amp_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_amp"));
    cmd.current_dir(workspace_root());
    cmd
}

#[test]
fn v02_quiet_stone_validates() {
    let out = amp_bin()
        .args(["validate", "examples/quiet_stone.json"])
        .output()
        .expect("failed to run amp");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "validate failed: {stderr}");
}

#[test]
fn v02_sharp_flint_validates() {
    let out = amp_bin()
        .args(["validate", "examples/sharp_flint.json"])
        .output()
        .expect("failed to run amp");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "validate failed: {stderr}");
}

#[test]
fn v02_warm_birch_validates() {
    let out = amp_bin()
        .args(["validate", "examples/warm_birch.json"])
        .output()
        .expect("failed to run amp");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "validate failed: {stderr}");
}

#[test]
fn v10_quiet_stone_validates() {
    let out = amp_bin()
        .args(["validate", "examples/quiet_stone_v1.json"])
        .output()
        .expect("failed to run amp");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "validate failed: {stderr}");
}

#[test]
fn v02_check_json_produces_structured_output() {
    let out = amp_bin()
        .args(["check", "examples/quiet_stone.json", "--json"])
        .output()
        .expect("failed to run amp");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"file\""),
        "missing 'file' key in: {stdout}"
    );
    assert!(
        stdout.contains("\"pass\""),
        "missing 'pass' key in: {stdout}"
    );
}

#[test]
fn v10_check_json_passes() {
    let out = amp_bin()
        .args(["check", "examples/quiet_stone_v1.json", "--json"])
        .output()
        .expect("failed to run amp");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"pass\":true") || stdout.contains("\"pass\": true"),
        "v1.0 check should pass: {stdout}"
    );
}
