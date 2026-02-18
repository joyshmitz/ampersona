use std::collections::HashMap;

use ampersona_core::types::{GateDirection, GateEnforcement};

use super::decision::GateDecisionRecord;

pub struct OverrideRequest {
    pub gate_id: String,
    pub direction: GateDirection,
    pub from_phase: Option<String>,
    pub to_phase: String,
    pub reason: String,
    pub approver: String,
    pub state_rev: u64,
    pub metrics_snapshot: HashMap<String, serde_json::Value>,
}

/// Process a gate override (emergency bypass of failed gate).
pub fn process_override(req: &OverrideRequest) -> GateDecisionRecord {
    GateDecisionRecord {
        gate_id: req.gate_id.clone(),
        direction: req.direction,
        enforcement: GateEnforcement::Enforce,
        decision: format!("override by {}: {}", req.approver, req.reason),
        from_phase: req.from_phase.clone(),
        to_phase: req.to_phase.clone(),
        metrics_snapshot: req.metrics_snapshot.clone(),
        criteria_results: vec![],
        is_override: true,
        state_rev: req.state_rev,
        metrics_hash: String::new(),
    }
}
