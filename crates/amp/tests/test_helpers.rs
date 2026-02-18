use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

pub fn amp_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_amp"));
    cmd.current_dir(workspace_root());
    cmd
}

/// Run amp, assert exit code, return parsed JSON stdout.
pub fn amp_json(args: &[&str], expected_exit: i32) -> Value {
    let out = amp_bin().args(args).output().expect("failed to run amp");
    let code = out.status.code().unwrap_or(-1);
    assert_eq!(
        code,
        expected_exit,
        "exit mismatch for: amp {}\nstdout: {}\nstderr: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid JSON from: amp {}\n{e}\nstdout: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stdout)
        )
    })
}

/// Run amp, return stdout as string (exit 0 expected).
#[allow(dead_code)]
pub fn amp_stdout(args: &[&str]) -> String {
    let out = amp_bin().args(args).output().expect("failed to run amp");
    assert!(
        out.status.success(),
        "amp {} failed with exit {}\nstderr: {}",
        args.join(" "),
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}
