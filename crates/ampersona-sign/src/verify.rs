use anyhow::{bail, Result};
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_json::Value;

use crate::canonical::canonicalize_fields;

/// Verify the signature on a persona JSON.
pub fn verify_persona(data: &Value, verifying_key: &VerifyingKey) -> Result<bool> {
    let sig_block = data
        .get("signature")
        .ok_or_else(|| anyhow::anyhow!("no signature block found"))?;

    // Verify canonicalization method
    let canon = sig_block
        .get("canonicalization")
        .and_then(Value::as_str)
        .unwrap_or("");
    if canon != "JCS-RFC8785" {
        bail!("unsupported canonicalization: {canon}");
    }

    // Get signed_fields
    let signed_fields: Vec<String> = sig_block
        .get("signed_fields")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    if signed_fields.is_empty() {
        bail!("signed_fields is empty");
    }

    // Canonicalize
    let canonical = canonicalize_fields(data, &signed_fields);

    // Decode signature
    let sig_b64 = sig_block
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing signature value"))?;

    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(sig_b64)
        .map_err(|e| anyhow::anyhow!("invalid base64 signature: {e}"))?;

    let signature = Signature::from_slice(&sig_bytes)
        .map_err(|e| anyhow::anyhow!("invalid ed25519 signature: {e}"))?;

    // Verify
    match verifying_key.verify(&canonical, &signature) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}
