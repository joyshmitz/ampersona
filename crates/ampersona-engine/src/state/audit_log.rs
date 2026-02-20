use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

/// Append an audit entry to the log file, maintaining hash chain.
///
/// Each entry gets a `prev_hash` field containing the SHA-256 of the previous entry.
/// The first entry uses "genesis" as its prev_hash.
pub fn append_audit(path: &str, entry: &serde_json::Value) -> Result<String> {
    let content = if std::path::Path::new(path).exists() {
        std::fs::read_to_string(path).unwrap_or_default()
    } else {
        String::new()
    };

    // Compute prev_hash from last entry
    let prev_hash = content
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(|line| format!("sha256:{:x}", Sha256::digest(line.as_bytes())))
        .unwrap_or_else(|| "genesis".to_string());

    // Inject prev_hash into entry
    let mut entry = entry.clone();
    if let Some(obj) = entry.as_object_mut() {
        obj.insert("prev_hash".into(), serde_json::Value::String(prev_hash));
        obj.insert(
            "ts".into(),
            serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
        );
    }

    let entry_json = serde_json::to_string(&entry)?;
    let hash = format!("sha256:{:x}", Sha256::digest(entry_json.as_bytes()));

    let mut new_content = content;
    new_content.push_str(&entry_json);
    new_content.push('\n');
    std::fs::write(path, new_content).with_context(|| format!("cannot write audit {path}"))?;

    Ok(hash)
}

/// Verify the hash chain in an audit log file.
///
/// Returns the number of valid entries.
pub fn verify_chain(path: &str) -> Result<u64> {
    verify_chain_from(path, 0)
}

/// Verify the hash chain starting from entry `from_entry` (0-based).
///
/// Entries before `from_entry` are traversed to build the chain state but not
/// verified against their prev_hash — this allows verifying a suffix of the chain.
pub fn verify_chain_from(path: &str, from_entry: u64) -> Result<u64> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("cannot read audit {path}"))?;

    let mut count = 0u64;
    let mut prev_hash = "genesis".to_string();

    for (i, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: serde_json::Value = serde_json::from_str(line)
            .with_context(|| format!("invalid JSON at line {}", i + 1))?;

        if count >= from_entry {
            let entry_prev = entry
                .get("prev_hash")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("genesis");

            if entry_prev != prev_hash {
                bail!(
                    "hash chain broken at entry {}: expected prev_hash '{}', got '{}'",
                    count,
                    prev_hash,
                    entry_prev
                );
            }
        }

        let entry_json = serde_json::to_string(&entry)?;
        prev_hash = format!("sha256:{:x}", Sha256::digest(entry_json.as_bytes()));
        count += 1;
    }

    Ok(count)
}

/// Create an integrity checkpoint for audit/drift chains.
///
/// Writes a JSON file recording the chain head hash and entry count,
/// which can later be used to verify chain integrity from a known anchor.
pub fn create_checkpoint(audit_path: &str, checkpoint_path: &str) -> Result<serde_json::Value> {
    let count = verify_chain(audit_path)?;
    let content = std::fs::read_to_string(audit_path)
        .with_context(|| format!("cannot read audit {audit_path}"))?;

    // Get hash of last entry
    let chain_head = content
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(|line| format!("sha256:{:x}", Sha256::digest(line.as_bytes())))
        .unwrap_or_else(|| "genesis".to_string());

    let checkpoint = serde_json::json!({
        "audit_file": audit_path,
        "entries": count,
        "chain_head": chain_head,
        "created_at": chrono::Utc::now().to_rfc3339(),
    });

    let json = serde_json::to_string_pretty(&checkpoint)?;
    std::fs::write(checkpoint_path, json)
        .with_context(|| format!("cannot write checkpoint {checkpoint_path}"))?;

    Ok(checkpoint)
}

/// Count all audit events that correspond to a state_rev increment.
///
/// Events: GateTransition, ElevationChange, Override.
pub fn count_state_mutations(path: &str) -> Result<u64> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("cannot read audit {path}"))?;

    let mut count = 0u64;
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(et) = entry.get("event_type").and_then(|v| v.as_str()) {
                if matches!(et, "GateTransition" | "ElevationChange" | "Override") {
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

/// Verify a checkpoint against the current audit chain.
pub fn verify_checkpoint(audit_path: &str, checkpoint_path: &str) -> Result<bool> {
    let checkpoint_content = std::fs::read_to_string(checkpoint_path)
        .with_context(|| format!("cannot read checkpoint {checkpoint_path}"))?;
    let checkpoint: serde_json::Value = serde_json::from_str(&checkpoint_content)?;

    let expected_count = checkpoint
        .get("entries")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let expected_head = checkpoint
        .get("chain_head")
        .and_then(|v| v.as_str())
        .unwrap_or("genesis");

    // Verify the chain up to the checkpoint's entry count
    let content = std::fs::read_to_string(audit_path)
        .with_context(|| format!("cannot read audit {audit_path}"))?;

    let mut count = 0u64;
    let mut prev_hash = "genesis".to_string();
    let mut last_hash = "genesis".to_string();

    for (i, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if count >= expected_count {
            break;
        }

        let entry: serde_json::Value = serde_json::from_str(line)
            .with_context(|| format!("invalid JSON at line {}", i + 1))?;

        let entry_prev = entry
            .get("prev_hash")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("genesis");

        if entry_prev != prev_hash {
            return Ok(false);
        }

        let entry_json = serde_json::to_string(&entry)?;
        prev_hash = format!("sha256:{:x}", Sha256::digest(entry_json.as_bytes()));
        last_hash = format!("sha256:{:x}", Sha256::digest(line.as_bytes()));
        count += 1;
    }

    Ok(count == expected_count && last_hash == expected_head)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn append_and_verify_chain() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();
        // Ensure file is empty
        file.write_all(b"").unwrap();

        // Append 3 entries
        let e1 = serde_json::json!({"event": "first"});
        let e2 = serde_json::json!({"event": "second"});
        let e3 = serde_json::json!({"event": "third"});

        append_audit(&path, &e1).unwrap();
        append_audit(&path, &e2).unwrap();
        append_audit(&path, &e3).unwrap();

        // Verify chain
        let count = verify_chain(&path).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn checkpoint_create_and_verify() {
        let dir = tempfile::tempdir().unwrap();
        let audit_path = dir.path().join("test.audit.jsonl");
        let checkpoint_path = dir.path().join("test.checkpoint.json");
        let audit_str = audit_path.to_str().unwrap();
        let checkpoint_str = checkpoint_path.to_str().unwrap();

        // Append entries
        append_audit(audit_str, &serde_json::json!({"event": "a"})).unwrap();
        append_audit(audit_str, &serde_json::json!({"event": "b"})).unwrap();

        // Create checkpoint
        let checkpoint = create_checkpoint(audit_str, checkpoint_str).unwrap();
        assert_eq!(checkpoint["entries"], 2);

        // Verify checkpoint
        assert!(verify_checkpoint(audit_str, checkpoint_str).unwrap());

        // Append more entries — checkpoint should still verify for first 2
        append_audit(audit_str, &serde_json::json!({"event": "c"})).unwrap();
        assert!(verify_checkpoint(audit_str, checkpoint_str).unwrap());
    }

    #[test]
    fn verify_detects_tampering() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();

        append_audit(&path, &serde_json::json!({"event": "first"})).unwrap();
        append_audit(&path, &serde_json::json!({"event": "second"})).unwrap();

        // Tamper with the file — modify first line
        let content = std::fs::read_to_string(&path).unwrap();
        let tampered = content.replacen("first", "TAMPERED", 1);
        std::fs::write(&path, tampered).unwrap();

        // Verify should fail
        let result = verify_chain(&path);
        assert!(result.is_err());
    }

    #[test]
    fn tampered_checkpoint_detected() {
        let dir = tempfile::tempdir().unwrap();
        let audit_path = dir.path().join("test.audit.jsonl");
        let checkpoint_path = dir.path().join("test.checkpoint.json");
        let audit_str = audit_path.to_str().unwrap();
        let checkpoint_str = checkpoint_path.to_str().unwrap();

        append_audit(audit_str, &serde_json::json!({"event": "a"})).unwrap();
        append_audit(audit_str, &serde_json::json!({"event": "b"})).unwrap();

        create_checkpoint(audit_str, checkpoint_str).unwrap();

        // Tamper with checkpoint: change entry count
        let content = std::fs::read_to_string(checkpoint_str).unwrap();
        let tampered = content.replace("\"entries\": 2", "\"entries\": 99");
        std::fs::write(checkpoint_str, tampered).unwrap();

        // verify_checkpoint must return false (not Ok(true))
        assert!(!verify_checkpoint(audit_str, checkpoint_str).unwrap());
    }

    #[test]
    fn tampered_checkpoint_chain_head_detected() {
        let dir = tempfile::tempdir().unwrap();
        let audit_path = dir.path().join("test.audit.jsonl");
        let checkpoint_path = dir.path().join("test.checkpoint.json");
        let audit_str = audit_path.to_str().unwrap();
        let checkpoint_str = checkpoint_path.to_str().unwrap();

        append_audit(audit_str, &serde_json::json!({"event": "a"})).unwrap();

        create_checkpoint(audit_str, checkpoint_str).unwrap();

        // Tamper with checkpoint: change chain_head
        let content = std::fs::read_to_string(checkpoint_str).unwrap();
        let mut cp: serde_json::Value = serde_json::from_str(&content).unwrap();
        cp["chain_head"] = serde_json::json!(
            "sha256:0000000000000000000000000000000000000000000000000000000000000000"
        );
        std::fs::write(checkpoint_str, serde_json::to_string_pretty(&cp).unwrap()).unwrap();

        assert!(!verify_checkpoint(audit_str, checkpoint_str).unwrap());
    }

    #[test]
    fn verify_chain_empty_file_is_zero_entries() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();
        std::fs::write(&path, "").unwrap();

        let count = verify_chain(&path).unwrap();
        assert_eq!(count, 0);
    }
}
