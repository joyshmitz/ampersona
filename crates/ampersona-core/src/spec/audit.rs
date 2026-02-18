use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    #[serde(default)]
    pub log_decisions: bool,

    #[serde(default = "default_true")]
    pub log_gate_transitions: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub compliance_markers: Option<Vec<String>>,
}

fn default_true() -> bool {
    true
}
