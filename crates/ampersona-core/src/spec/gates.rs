use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::spec::authority::AuthorityOverlay;
use crate::types::{CriterionOp, GateApproval, GateDirection, GateEnforcement};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gate {
    pub id: String,
    pub direction: GateDirection,

    #[serde(default)]
    pub enforcement: GateEnforcement,

    #[serde(default)]
    pub priority: i32,

    #[serde(default)]
    pub cooldown_seconds: u64,

    pub from_phase: Option<String>,
    pub to_phase: String,

    pub criteria: Vec<Criterion>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics_schema: Option<HashMap<String, MetricSchema>>,

    #[serde(default = "default_auto")]
    pub approval: GateApproval,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_pass: Option<GateEffect>,
}

fn default_auto() -> GateApproval {
    GateApproval::Auto
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Criterion {
    pub metric: String,
    pub op: CriterionOp,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSchema {
    #[serde(rename = "type")]
    pub metric_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateEffect {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority_overlay: Option<AuthorityOverlay>,
}
