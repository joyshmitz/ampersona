use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub algorithm: String,
    pub key_id: String,
    pub signer: String,
    pub canonicalization: String,
    pub signed_fields: Vec<String>,
    pub created_at: String,
    pub digest: String,
    pub value: String,
}
