use std::collections::HashMap;

use ampersona_core::traits::CriteriaResult;
use ampersona_core::types::{GateDirection, GateEnforcement};
use serde::{Deserialize, Serialize};

/// Record of a gate evaluation decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecisionRecord {
    pub gate_id: String,
    pub direction: GateDirection,
    pub enforcement: GateEnforcement,
    pub decision: String,
    pub from_phase: Option<String>,
    pub to_phase: String,
    pub metrics_snapshot: HashMap<String, serde_json::Value>,
    pub criteria_results: Vec<CriteriaResult>,
    pub is_override: bool,
    pub state_rev: u64,
    pub metrics_hash: String,
}
