use anyhow::Result;
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::canonical::canonicalize_fields;

/// Sign a persona JSON, adding a signature block.
pub fn sign_persona(
    data: &mut Value,
    signing_key: &SigningKey,
    key_id: &str,
    signer: &str,
) -> Result<()> {
    let obj = data
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("persona must be a JSON object"))?;

    // Determine signed_fields: all top-level keys except "signature" and "$schema"
    let signed_fields: Vec<String> = obj
        .keys()
        .filter(|k| *k != "signature" && *k != "$schema")
        .cloned()
        .collect();

    // Canonicalize
    let canonical = canonicalize_fields(data, &signed_fields);

    // Hash
    let digest = Sha256::digest(&canonical);
    let digest_hex = format!("sha256:{:x}", digest);

    // Sign
    let signature = signing_key.sign(&canonical);
    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

    // Build signature block
    let sig_block = serde_json::json!({
        "algorithm": "ed25519",
        "key_id": key_id,
        "signer": signer,
        "canonicalization": "JCS-RFC8785",
        "signed_fields": signed_fields,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "digest": digest_hex,
        "value": sig_b64
    });

    data.as_object_mut()
        .unwrap()
        .insert("signature".to_string(), sig_block);

    Ok(())
}
