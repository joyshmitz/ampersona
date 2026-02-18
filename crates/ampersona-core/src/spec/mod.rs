pub mod audit;
pub mod authority;
pub mod gates;
pub mod identity;
pub mod signature;

use serde::{Deserialize, Serialize};

use self::audit::AuditConfig;
use self::authority::Authority;
use self::gates::Gate;
use self::identity::{Capabilities, Directives, Psychology, Voice};
use self::signature::Signature;

/// Top-level ampersona persona document (v1.0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema_uri: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    pub name: String,
    pub role: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub backstory: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<Signature>,

    pub psychology: Psychology,
    pub voice: Voice,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Capabilities>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub directives: Option<Directives>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority: Option<Authority>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gates: Option<Vec<Gate>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit: Option<AuditConfig>,
}

impl Persona {
    /// Detect version: "1.0" if version field present, else "0.2".
    pub fn detected_version(&self) -> &str {
        match &self.version {
            Some(v) => v.as_str(),
            None => "0.2",
        }
    }

    /// True if this is a v0.2 persona (no version field, no authority/gates).
    pub fn is_v02(&self) -> bool {
        self.version.is_none()
    }
}
