use ampersona_core::state::DriftEntry;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};

/// Append a drift entry to the ledger file, maintaining hash chain.
///
/// Automatically reads the last entry's hash as prev_hash.
pub fn append_drift(path: &str, metrics: serde_json::Value) -> Result<String> {
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

    let entry = DriftEntry {
        prev_hash,
        metrics,
        ts: Utc::now(),
    };

    let entry_json = serde_json::to_string(&entry)?;
    let hash = format!("sha256:{:x}", Sha256::digest(entry_json.as_bytes()));

    let mut new_content = content;
    new_content.push_str(&entry_json);
    new_content.push('\n');
    std::fs::write(path, new_content).with_context(|| format!("cannot write drift {path}"))?;

    Ok(hash)
}

/// Read all drift entries from a ledger file as raw JSON values.
pub fn read_drift_entries(path: &str) -> Result<Vec<serde_json::Value>> {
    if !std::path::Path::new(path).exists() {
        return Ok(Vec::new());
    }
    let content =
        std::fs::read_to_string(path).with_context(|| format!("cannot read drift {path}"))?;
    let mut entries = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: serde_json::Value = serde_json::from_str(line)
            .with_context(|| format!("invalid JSON in drift ledger {path}"))?;
        entries.push(entry);
    }
    Ok(entries)
}

/// Verify the hash chain in a drift ledger file.
pub fn verify_drift_chain(path: &str) -> Result<u64> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("cannot read drift {path}"))?;

    let mut count = 0u64;
    let mut prev_hash = "genesis".to_string();

    for (i, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: serde_json::Value = serde_json::from_str(line)
            .with_context(|| format!("invalid JSON at line {}", i + 1))?;

        let entry_prev = entry
            .get("prev_hash")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("genesis");

        if entry_prev != prev_hash {
            bail!(
                "drift chain broken at entry {}: expected '{}', got '{}'",
                i + 1,
                prev_hash,
                entry_prev
            );
        }

        let entry_json = serde_json::to_string(&entry)?;
        prev_hash = format!("sha256:{:x}", Sha256::digest(entry_json.as_bytes()));
        count += 1;
    }

    Ok(count)
}
