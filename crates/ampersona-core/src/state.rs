use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::spec::authority::AuthorityOverlay;

/// Persistent phase state for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseState {
    pub name: String,
    pub current_phase: Option<String>,
    pub state_rev: u64,
    #[serde(default)]
    pub active_elevations: Vec<ActiveElevation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_transition: Option<TransitionRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_transition: Option<PendingTransition>,
    /// Active authority overlay from last gate on_pass effect.
    /// Applied as a post-resolution patch in authority checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_overlay: Option<AuthorityOverlay>,
    pub updated_at: DateTime<Utc>,
}

impl PhaseState {
    pub fn new(name: String) -> Self {
        Self {
            name,
            current_phase: None,
            state_rev: 0,
            active_elevations: Vec::new(),
            last_transition: None,
            pending_transition: None,
            active_overlay: None,
            updated_at: Utc::now(),
        }
    }
}

/// An active temporary elevation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveElevation {
    pub elevation_id: String,
    pub granted_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub reason: String,
    pub granted_by: String,
}

impl ActiveElevation {
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

/// Record of a gate transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionRecord {
    pub gate_id: String,
    pub from_phase: Option<String>,
    pub to_phase: String,
    pub at: DateTime<Utc>,
    pub decision_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics_hash: Option<String>,
    /// The state_rev at which this transition was recorded (for idempotency).
    #[serde(default)]
    pub state_rev: u64,
}

/// A pending gate transition awaiting human approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTransition {
    pub gate_id: String,
    pub from_phase: Option<String>,
    pub to_phase: String,
    pub decision: String,
    pub metrics_hash: String,
    pub state_rev: u64,
    pub created_at: DateTime<Utc>,
}

/// A single drift ledger entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftEntry {
    pub prev_hash: String,
    pub metrics: serde_json::Value,
    pub ts: DateTime<Utc>,
}
